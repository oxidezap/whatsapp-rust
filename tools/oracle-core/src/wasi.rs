//! A deterministic WASI preview-1 subset.
//!
//! Stubbing WASI is not neutral. `fd_write` returning "wrote nothing" makes libc
//! retry forever — the VoIP module called it 3.2 million times before running
//! out of fuel — and `fd_read` returning success with no data looks like a file
//! that is simultaneously readable and empty. Every call here either does the
//! real thing against an in-memory filesystem or returns a specific errno.
//!
//! The filesystem is in memory on purpose: a module under test must not be able
//! to read or write the host, and an in-memory tree is reproducible.

use std::collections::BTreeMap;

use anyhow::Result;
use wasmtime::error::Context as _;
use wasmtime::{Caller, Linker, Module, Store, Val};

use crate::state::HostState;

// WASI errno values used here.
const ESUCCESS: i32 = 0;
const EBADF: i32 = 8;
const EINVAL: i32 = 28;
const ENOENT: i32 = 44;
const ENOSYS: i32 = 52;

/// Reserved descriptors: 0/1/2 are the standard streams, 3 is the preopened
/// root directory that `path_open` resolves against.
const FD_STDIN: u32 = 0;
const FD_STDOUT: u32 = 1;
const FD_STDERR: u32 = 2;
const FD_ROOT: u32 = 3;
const FIRST_FILE_FD: u32 = 4;

/// The name the root preopen is reported under.
const ROOT_NAME: &str = "/";

/// Largest single file the memfs will hold. The 256 MiB payload limit cannot
/// stop a sparse attack on its own: seeking a writable descriptor near
/// `u64::MAX` and writing one byte would otherwise `resize` towards it.
const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
/// Largest the whole memfs may hold across live files and unlinked-but-open ones.
const MAX_FS_BYTES: usize = 256 * 1024 * 1024;

/// WASI clock ids. `2` and `3` are the process and thread CPU-time clocks,
/// which this host does not have and therefore refuses.
const CLOCKID_REALTIME: u32 = 0;
const CLOCKID_MONOTONIC: u32 = 1;

#[derive(Debug, Clone)]
/// One open descriptor in the in-memory filesystem.
pub struct OpenFile {
    /// Path it was opened under.
    pub path: String,
    /// Current seek position.
    pub offset: u64,
    /// Whether writes are allowed through it.
    pub writable: bool,
    /// Bytes kept alive after the path was unlinked; see `unlink_file`.
    /// Post-unlink clones are per handle: descriptors unlinked from one path
    /// no longer observe each other's writes.
    pub retained: Option<Vec<u8>>,
}

/// Guest-visible environment: arguments, variables, and an in-memory filesystem.
#[derive(Debug, Clone, Default)]
pub struct WasiState {
    /// The command line the guest sees.
    pub args: Vec<String>,
    /// The environment the guest sees.
    pub env: Vec<(String, String)>,
    /// Files the guest can open, by path.
    pub files: BTreeMap<String, Vec<u8>>,
    /// Everything written to stdout.
    pub stdout: Vec<u8>,
    /// Everything written to stderr.
    pub stderr: Vec<u8>,
    open: BTreeMap<u32, OpenFile>,
    next_fd: u32,
}

impl WasiState {
    /// Places a file in the guest filesystem before it runs.
    pub fn add_file(&mut self, path: impl Into<String>, contents: Vec<u8>) {
        self.files.insert(normalise(&path.into()), contents);
    }

    /// Reads a file back, which is how an output file is collected after a run.
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files
            .get(&normalise(path))
            .map(|bytes| bytes.as_slice())
    }

    /// Stdout as text, with invalid UTF-8 replaced.
    #[must_use]
    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    /// Stderr as text, with invalid UTF-8 replaced.
    #[must_use]
    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }

    fn allocate_fd(&mut self) -> u32 {
        if self.next_fd < FIRST_FILE_FD {
            self.next_fd = FIRST_FILE_FD;
        }
        let fd = self.next_fd;
        self.next_fd += 1;
        fd
    }

    /// Bytes visible through `open`: contents retained at unlink, else the live file.
    fn open_bytes<'a>(&'a self, open: &'a OpenFile) -> Option<&'a [u8]> {
        if let Some(retained) = &open.retained {
            return Some(retained);
        }
        self.files.get(&open.path).map(Vec::as_slice)
    }

    /// Removes `path`, keeping its bytes alive in every open descriptor that
    /// names it. Recreating the path starts a new file; retained handles keep
    /// reading and writing the old bytes. Returns whether the path existed.
    fn unlink_file(&mut self, path: &str) -> bool {
        let Some(contents) = self.files.remove(path) else {
            return false;
        };
        for open in self.open.values_mut() {
            if open.path == path && open.retained.is_none() {
                open.retained = Some(contents.clone());
            }
        }
        true
    }
}

/// Bytes currently held: live files plus contents retained by unlinked handles.
fn fs_bytes(wasi: &WasiState) -> usize {
    let live: usize = wasi.files.values().map(Vec::len).sum();
    let retained: usize = wasi
        .open
        .values()
        .filter_map(|open| open.retained.as_ref().map(Vec::len))
        .sum();
    live.saturating_add(retained)
}

/// Paths arrive with and without a leading slash depending on how the guest
/// composed them; one spelling is stored so both find the same file.
fn normalise(path: &str) -> String {
    path.trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned()
}

/// An `iovec`/`ciovec`: a pointer and a length, 8 bytes each.
fn read_iovecs(state: &HostState, ptr: u32, count: u32) -> Result<Vec<(u32, u32)>> {
    const MAX: u32 = 1024;
    anyhow::ensure!(count <= MAX, "iovec count exceeds {MAX}");
    let mut total = 0u32;
    (0..count)
        .map(|index| {
            let base = ptr
                .checked_add(index.checked_mul(8).context("iovec offset overflow")?)
                .context("iovec address overflow")?;
            let length_at = base.checked_add(4).context("iovec address overflow")?;
            let (ptr, len) = (state.read_u32(base)?, state.read_u32(length_at)?);
            total = total
                .checked_add(len)
                .context("iovec payload size overflow")?;
            anyhow::ensure!(total <= 256 * 1024 * 1024, "iovec payload exceeds 256 MiB");
            state.ensure_memory_range(ptr, len)?;
            Ok((ptr, len))
        })
        .collect()
}

fn write_u32(state: &HostState, ptr: u32, value: u32) -> Result<()> {
    state.write(ptr, &value.to_le_bytes())
}

fn write_u64(state: &HostState, ptr: u32, value: u64) -> Result<()> {
    state.write(ptr, &value.to_le_bytes())
}

fn fd_write(caller: &mut Caller<'_, HostState>, fd: u32, iovs: u32, count: u32, out: u32) -> i32 {
    if caller.data().ensure_memory_range(out, 4).is_err() {
        return EINVAL;
    }
    let Ok(vectors) = read_iovecs(caller.data(), iovs, count) else {
        return EINVAL;
    };

    let mut payload = Vec::new();
    for (ptr, len) in vectors {
        match caller.data().read(ptr, len) {
            Ok(bytes) => {
                let Some(total) = payload.len().checked_add(bytes.len()) else {
                    return EINVAL;
                };
                if total > 256 * 1024 * 1024 {
                    return EINVAL;
                }
                payload.extend_from_slice(&bytes);
            }
            Err(_) => return EINVAL,
        }
    }
    let written = payload.len() as u32;

    let state = caller.data();
    let mut wasi = state.wasi();
    match fd {
        FD_STDOUT => wasi.stdout.extend_from_slice(&payload),
        FD_STDERR => wasi.stderr.extend_from_slice(&payload),
        _ => {
            let Some(open) = wasi.open.get(&fd) else {
                return EBADF;
            };
            if !open.writable {
                return EBADF;
            }
            let Ok(offset) = usize::try_from(open.offset) else {
                return EINVAL;
            };
            let Some(end) = offset.checked_add(payload.len()) else {
                return EINVAL;
            };
            let Some(next_offset) = open.offset.checked_add(u64::from(written)) else {
                return EINVAL;
            };
            // Bound growth before the descriptor moves or any byte is
            // allocated: a sparse seek plus a one-byte write must fail rather
            // than resize towards `u64::MAX`.
            let current = open
                .retained
                .as_ref()
                .map_or_else(|| wasi.files.get(&open.path).map_or(0, Vec::len), Vec::len);
            if end > MAX_FILE_BYTES
                || fs_bytes(&wasi).saturating_sub(current).saturating_add(end) > MAX_FS_BYTES
            {
                return EINVAL;
            }
            let Some(open) = wasi.open.get_mut(&fd) else {
                return EBADF;
            };
            open.offset = next_offset;
            let target: &mut Vec<u8> = if let Some(retained) = open.retained.as_mut() {
                retained
            } else {
                let path = open.path.clone();
                wasi.files.entry(path).or_default()
            };
            if target.len() < end {
                target.resize(end, 0);
            }
            target[offset..end].copy_from_slice(&payload);
        }
    }

    // Reporting the byte count is what stops libc from retrying forever.
    match write_u32(caller.data(), out, written) {
        Ok(()) => ESUCCESS,
        Err(_) => EINVAL,
    }
}

fn fd_read(caller: &mut Caller<'_, HostState>, fd: u32, iovs: u32, count: u32, out: u32) -> i32 {
    read_into(caller, fd, iovs, count, None, out)
}

/// `fd_pread(fd, iovs, iovs_len, offset: u64, nread)`.
///
/// A different shape from `fd_read`, not a synonym for it: the offset occupies
/// one *64-bit* parameter, so the output pointer is argument 4 rather than 3.
/// Routing it through `fd_read` read the low half of the offset as the address
/// to report the byte count at, ignored the real one, and moved the descriptor's
/// cursor — three wrong answers from one alias.
fn fd_pread(
    caller: &mut Caller<'_, HostState>,
    fd: u32,
    iovs: u32,
    count: u32,
    offset: u64,
    out: u32,
) -> i32 {
    read_into(caller, fd, iovs, count, Some(offset), out)
}

/// The body both reads share. `at` is `None` for a cursor read, which is the
/// only case that advances the descriptor.
fn read_into(
    caller: &mut Caller<'_, HostState>,
    fd: u32,
    iovs: u32,
    count: u32,
    at: Option<u64>,
    out: u32,
) -> i32 {
    if caller.data().ensure_memory_range(out, 4).is_err() {
        return EINVAL;
    }
    let Ok(vectors) = read_iovecs(caller.data(), iovs, count) else {
        return EINVAL;
    };
    if fd == FD_STDIN {
        // Always at end of input; no interactive stdin exists here.
        return match write_u32(caller.data(), out, 0) {
            Ok(()) => ESUCCESS,
            Err(_) => EINVAL,
        };
    }

    let state = caller.data();
    let mut wasi = state.wasi();
    let Some(open) = wasi.open.get(&fd) else {
        return EBADF;
    };
    let Some(contents) = wasi.open_bytes(open) else {
        return EBADF;
    };

    let Ok(mut offset) = usize::try_from(at.unwrap_or(open.offset)) else {
        return EINVAL;
    };
    let mut total = 0u32;
    for (ptr, len) in vectors {
        if offset >= contents.len() {
            break;
        }
        let take = (len as usize).min(contents.len() - offset);
        if state.write(ptr, &contents[offset..offset + take]).is_err() {
            return EINVAL;
        }
        offset += take;
        let Ok(take) = u32::try_from(take) else {
            return EINVAL;
        };
        let Some(next_total) = total.checked_add(take) else {
            return EINVAL;
        };
        total = next_total;
    }

    // A positional read leaves the cursor where it was: that is the whole
    // difference between `pread` and `read`.
    if at.is_none()
        && let Some(open) = wasi.open.get_mut(&fd)
    {
        open.offset = offset as u64;
    }
    match write_u32(caller.data(), out, total) {
        Ok(()) => ESUCCESS,
        Err(_) => EINVAL,
    }
}

/// `clock_time_get(id, precision, out)`.
///
/// The clock id is the whole of the question, and ignoring it answered every
/// one of them with the monotonic counter — so a guest asking for the wall
/// clock got "nanoseconds since this run started", i.e. a timestamp in 1970,
/// and a guest asking for a CPU clock got a success it should never have had.
///
/// Both clocks come from the same virtual source the emscripten layer uses, so
/// time is consistent whichever way a module asks for it.
fn clock_time_get(caller: &mut Caller<'_, HostState>, id: u32, out: u32) -> i32 {
    // The output range is validated before either clock moves: a failed guest
    // retry must observe the same deterministic timestamp as the first attempt.
    if caller.data().ensure_memory_range(out, 8).is_err() {
        return EINVAL;
    }
    let nanos = match id {
        CLOCKID_REALTIME => {
            let millis = crate::emscripten::EPOCH_MS + caller.data().shared.tick_wall_clock();
            (millis * 1_000_000.0) as u64
        }
        CLOCKID_MONOTONIC => (caller.data().tick_clock() * 1_000_000.0) as u64,
        // The two CPU-time clocks. This host has no notion of either, and
        // reporting success with a number it made up is exactly the "stub that
        // returns zero is a hypothesis" failure.
        _ => return ENOSYS,
    };
    match write_u64(caller.data(), out, nanos) {
        Ok(()) => ESUCCESS,
        Err(_) => EINVAL,
    }
}

fn fd_seek(caller: &mut Caller<'_, HostState>, fd: u32, delta: i64, whence: u32, out: u32) -> i32 {
    if caller.data().ensure_memory_range(out, 8).is_err() {
        return EINVAL;
    }
    let state = caller.data();
    let mut wasi = state.wasi();
    let Some(open) = wasi.open.get(&fd) else {
        return EBADF;
    };
    let size = wasi
        .open
        .get(&fd)
        .and_then(|open| wasi.open_bytes(open))
        .map_or(0, |bytes| bytes.len() as u64);
    let Some(position) = seek_position(open.offset, size, delta, whence) else {
        return EINVAL;
    };

    if let Some(open) = wasi.open.get_mut(&fd) {
        open.offset = position;
    }
    match write_u64(caller.data(), out, position) {
        Ok(()) => ESUCCESS,
        Err(_) => EINVAL,
    }
}

fn seek_position(current: u64, size: u64, delta: i64, whence: u32) -> Option<u64> {
    let base = match whence {
        0 => 0_i128,
        1 => i128::from(current),
        2 => i128::from(size),
        _ => return None,
    };
    let position = base.checked_add(i128::from(delta))?;
    u64::try_from(position).ok()
}

/// The validated `path_open` arguments: directory, flags, path and mode.
struct PathOpenArgs {
    dirfd: u32,
    lookupflags: u32,
    path_ptr: u32,
    path_len: u32,
    oflags: u32,
    fdflags: u32,
    out: u32,
}

fn path_open(caller: &mut Caller<'_, HostState>, args: PathOpenArgs) -> i32 {
    // WASI preview1 oflags: bit 0 is CREAT, bit 3 is TRUNC.
    const O_CREAT: u32 = 1;
    const O_TRUNC: u32 = 8;
    const SUPPORTED: u32 = O_CREAT | O_TRUNC;

    // The memfs has exactly one directory, the preopened root. Anything else
    // names a capability this host never granted.
    if args.dirfd != FD_ROOT {
        return EBADF;
    }
    // Bit 0 is SYMLINK_FOLLOW. The memfs has no symlinks, so following them
    // is vacuous and real modules pass it; any other lookup flag is rejected.
    if args.lookupflags & !1 != 0 {
        return EINVAL;
    }
    if args.oflags & !SUPPORTED != 0
        || args.fdflags != 0
        || caller.data().ensure_memory_range(args.out, 4).is_err()
    {
        return EINVAL;
    }

    let Ok(raw) = caller.data().read(args.path_ptr, args.path_len) else {
        return EINVAL;
    };
    let path = normalise(&String::from_utf8_lossy(&raw));

    // What the guest asked for, whether or not it got it. A refusal here is
    // invisible from the outside — the VoIP engine reports only "Failed to get
    // voip storage dir" and carries on — so without this the host cannot tell a
    // path it should have provided from one the guest never wanted.
    caller.data().log(format!(
        "wasi path_open {path:?} (create={}, truncate={})",
        args.oflags & O_CREAT != 0,
        args.oflags & O_TRUNC != 0
    ));

    let state = caller.data();
    let mut wasi = state.wasi();
    let exists = wasi.files.contains_key(&path);
    if !exists {
        if args.oflags & O_CREAT == 0 {
            return ENOENT;
        }
        wasi.files.insert(path.clone(), Vec::new());
    } else if args.oflags & O_TRUNC != 0 {
        wasi.files
            .get_mut(&path)
            .expect("existence checked")
            .clear();
    }

    let fd = wasi.allocate_fd();
    wasi.open.insert(
        fd,
        OpenFile {
            path,
            offset: 0,
            writable: true,
            retained: None,
        },
    );

    match write_u32(caller.data(), args.out, fd) {
        Ok(()) => ESUCCESS,
        Err(_) => EINVAL,
    }
}

/// `filestat` is 64 bytes; only the size field carries information here.
fn fd_filestat_get(caller: &mut Caller<'_, HostState>, fd: u32, out: u32) -> i32 {
    let state = caller.data();
    let wasi = state.wasi();
    let size = match wasi.open.get(&fd) {
        Some(open) => wasi.open_bytes(open).map_or(0, |bytes| bytes.len() as u64),
        None if fd <= FD_ROOT => 0,
        None => return EBADF,
    };

    let mut buffer = [0u8; 64];
    // filetype at offset 16: 4 = regular file, 3 = directory, 2 = character device.
    buffer[16] = match fd {
        FD_STDIN | FD_STDOUT | FD_STDERR => 2,
        FD_ROOT => 3,
        _ => 4,
    };
    buffer[32..40].copy_from_slice(&size.to_le_bytes());

    match caller.data().write(out, &buffer) {
        Ok(()) => ESUCCESS,
        Err(_) => EINVAL,
    }
}

/// `fdstat` is 24 bytes: filetype, flags, then two rights masks.
fn fd_fdstat_get(caller: &mut Caller<'_, HostState>, fd: u32, out: u32) -> i32 {
    let state = caller.data();
    let wasi = state.wasi();
    let known = fd <= FD_ROOT || wasi.open.contains_key(&fd);
    if !known {
        return EBADF;
    }

    let mut buffer = [0u8; 24];
    buffer[0] = match fd {
        FD_STDIN | FD_STDOUT | FD_STDERR => 2, // character device
        FD_ROOT => 3,                          // directory
        _ => 4,                                // regular file
    };
    // Grant every right; this host enforces access through the memfs itself.
    buffer[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
    buffer[16..24].copy_from_slice(&u64::MAX.to_le_bytes());

    match caller.data().write(out, &buffer) {
        Ok(()) => ESUCCESS,
        Err(_) => EINVAL,
    }
}

/// Writes a NUL-terminated string list in WASI's argv/environ layout: an array
/// of pointers, and a packed buffer they point into.
fn write_string_list(state: &HostState, entries: &[String], ptrs: u32, buffer: u32) -> i32 {
    let mut cursor = buffer;
    for (index, entry) in entries.iter().enumerate() {
        if write_u32(state, ptrs + index as u32 * 4, cursor).is_err() {
            return EINVAL;
        }
        let mut bytes = entry.clone().into_bytes();
        bytes.push(0);
        if state.write(cursor, &bytes).is_err() {
            return EINVAL;
        }
        cursor += bytes.len() as u32;
    }
    ESUCCESS
}

fn list_sizes(state: &HostState, entries: &[String], count_ptr: u32, size_ptr: u32) -> i32 {
    let bytes: usize = entries.iter().map(|entry| entry.len() + 1).sum();
    if write_u32(state, count_ptr, entries.len() as u32).is_err()
        || write_u32(state, size_ptr, bytes as u32).is_err()
    {
        return EINVAL;
    }
    ESUCCESS
}

fn env_strings(state: &HostState) -> Vec<String> {
    state
        .wasi()
        .env
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect()
}

/// Defines the WASI imports the module declares.
pub fn define(
    store: &mut Store<HostState>,
    linker: &mut Linker<HostState>,
    module: &Module,
) -> Result<usize> {
    let mut defined = 0;

    for import in module.imports() {
        if import.module() != "wasi_snapshot_preview1" {
            continue;
        }
        let wasmtime::ExternType::Func(ty) = import.ty() else {
            continue;
        };

        let name = import.name().to_owned();
        let func =
            crate::host::host_func(&mut *store, ty.clone(), move |caller, params, results| {
                let args = params.iter().map(scalar).collect();
                caller.data().record("wasi_snapshot_preview1", &name, args);
                let code = dispatch(&name, caller, params);
                if let Some(slot) = results.first_mut() {
                    *slot = Val::I32(code);
                }
                if name == "proc_exit" {
                    let status = arg(params, 0) as i32;
                    caller.data().set_exit_code(status);
                    return Err(wasmtime::Error::msg(format!(
                        "module called proc_exit({status})"
                    )));
                }
                Ok(())
            });

        linker
            .define(&*store, import.module(), import.name(), func)
            .with_context(|| format!("defining wasi {}", import.name()))?;
        defined += 1;
    }

    Ok(defined)
}

fn scalar(value: &Val) -> i64 {
    match value {
        Val::I32(value) => *value as i64,
        Val::I64(value) => *value,
        _ => 0,
    }
}

fn arg(params: &[Val], index: usize) -> u32 {
    match params.get(index) {
        Some(Val::I32(value)) => *value as u32,
        Some(Val::I64(value)) => *value as u32,
        _ => 0,
    }
}

fn arg_i64(params: &[Val], index: usize) -> i64 {
    match params.get(index) {
        Some(Val::I64(value)) => *value,
        Some(Val::I32(value)) => *value as i64,
        _ => 0,
    }
}

fn dispatch(name: &str, caller: &mut Caller<'_, HostState>, params: &[Val]) -> i32 {
    match name {
        "fd_write" => fd_write(
            caller,
            arg(params, 0),
            arg(params, 1),
            arg(params, 2),
            arg(params, 3),
        ),
        "fd_read" => fd_read(
            caller,
            arg(params, 0),
            arg(params, 1),
            arg(params, 2),
            arg(params, 3),
        ),
        // Note the argument positions: the offset is 64 bits wide, so `nread`
        // is parameter 4. See `fd_pread`.
        "fd_pread" => fd_pread(
            caller,
            arg(params, 0),
            arg(params, 1),
            arg(params, 2),
            arg_i64(params, 3) as u64,
            arg(params, 4),
        ),
        "fd_seek" => fd_seek(
            caller,
            arg(params, 0),
            arg_i64(params, 1),
            arg(params, 2),
            arg(params, 3),
        ),
        "fd_close" => {
            if caller
                .data_mut()
                .wasi()
                .open
                .remove(&arg(params, 0))
                .is_some()
            {
                ESUCCESS
            } else {
                EBADF
            }
        }
        "fd_fdstat_get" => fd_fdstat_get(caller, arg(params, 0), arg(params, 1)),
        "fd_filestat_get" => fd_filestat_get(caller, arg(params, 0), arg(params, 1)),
        "fd_prestat_get" => {
            if arg(params, 0) != FD_ROOT {
                // Ends the guest's scan for preopened directories.
                return EBADF;
            }
            let out = arg(params, 1);
            let mut buffer = [0u8; 8];
            buffer[0] = 0; // preopentype: directory
            buffer[4..8].copy_from_slice(&(ROOT_NAME.len() as u32).to_le_bytes());
            match caller.data().write(out, &buffer) {
                Ok(()) => ESUCCESS,
                Err(_) => EINVAL,
            }
        }
        "fd_prestat_dir_name" => {
            if arg(params, 0) != FD_ROOT {
                return EBADF;
            }
            let (ptr, len) = (arg(params, 1), arg(params, 2));
            if len < ROOT_NAME.len() as u32 || caller.data().ensure_memory_range(ptr, len).is_err()
            {
                return EINVAL;
            }
            match caller.data().write(ptr, ROOT_NAME.as_bytes()) {
                Ok(()) => ESUCCESS,
                Err(_) => EINVAL,
            }
        }
        "path_open" => path_open(
            caller,
            PathOpenArgs {
                dirfd: arg(params, 0),
                lookupflags: arg(params, 1),
                path_ptr: arg(params, 2),
                path_len: arg(params, 3),
                oflags: arg(params, 4),
                fdflags: arg(params, 7),
                out: arg(params, 8),
            },
        ),
        "path_unlink_file" => {
            let Ok(raw) = caller.data().read(arg(params, 1), arg(params, 2)) else {
                return EINVAL;
            };
            let path = normalise(&String::from_utf8_lossy(&raw));
            if caller.data().wasi().unlink_file(&path) {
                ESUCCESS
            } else {
                ENOENT
            }
        }
        "args_sizes_get" => {
            let args = caller.data().wasi().args.clone();
            list_sizes(caller.data(), &args, arg(params, 0), arg(params, 1))
        }
        "args_get" => {
            let args = caller.data().wasi().args.clone();
            write_string_list(caller.data(), &args, arg(params, 0), arg(params, 1))
        }
        "environ_sizes_get" => {
            let env = env_strings(caller.data());
            list_sizes(caller.data(), &env, arg(params, 0), arg(params, 1))
        }
        "environ_get" => {
            let env = env_strings(caller.data());
            write_string_list(caller.data(), &env, arg(params, 0), arg(params, 1))
        }
        "clock_time_get" => clock_time_get(caller, arg(params, 0), arg(params, 2)),
        "random_get" => {
            let (ptr, len) = (arg(params, 0), arg(params, 1));
            if len > 16 * 1024 * 1024 || caller.data().ensure_memory_range(ptr, len).is_err() {
                return EINVAL;
            }
            let mut bytes = Vec::with_capacity(len as usize);
            {
                let state = caller.data();
                for _ in 0..len {
                    bytes.push(state.next_random_byte());
                }
            }
            match caller.data().write(ptr, &bytes) {
                Ok(()) => ESUCCESS,
                Err(_) => EINVAL,
            }
        }
        "proc_exit" => ESUCCESS,
        // Anything else is reported as unimplemented rather than as success,
        // so a module never proceeds on a call this host did not really make.
        _ => ENOSYS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_encoder::ValType;

    #[test]
    fn invalid_result_pointers_do_not_mutate_reads_seeks_or_opens() {
        use ValType::{I32, I64};
        use wasm_encoder::{
            CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection, Function,
            FunctionSection, ImportSection, MemorySection, MemoryType, Module, TypeSection,
            ValType,
        };
        let signatures: &[(&str, &[ValType])] = &[
            ("fd_read", &[I32, I32, I32, I32]),
            ("fd_pread", &[I32, I32, I32, I64, I32]),
            ("fd_seek", &[I32, I64, I32, I32]),
            ("path_open", &[I32, I32, I32, I32, I32, I64, I64, I32, I32]),
        ];
        let mut types = TypeSection::new();
        let mut imports = ImportSection::new();
        let mut functions = FunctionSection::new();
        let mut exports = ExportSection::new();
        let mut code = CodeSection::new();
        for (index, (name, params)) in signatures.iter().enumerate() {
            types.ty().function(params.iter().copied(), [I32]);
            imports.import(
                "wasi_snapshot_preview1",
                name,
                EntityType::Function(index as u32),
            );
            functions.function(index as u32);
            exports.export(name, ExportKind::Func, (signatures.len() + index) as u32);
            let mut body = Function::new([]);
            for argument in 0..params.len() {
                body.instructions().local_get(argument as u32);
            }
            body.instructions().call(index as u32).end();
            code.function(&body);
        }
        let mut memories = MemorySection::new();
        memories.memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
        exports.export("memory", ExportKind::Memory, 0);
        let mut data = DataSection::new();
        data.active(0, &ConstExpr::i32_const(0), [64, 0, 0, 0, 3, 0, 0, 0]);
        data.active(0, &ConstExpr::i32_const(64), [0x55; 3]);
        data.active(0, &ConstExpr::i32_const(128), b"out".iter().copied());
        let mut module = Module::new();
        module
            .section(&types)
            .section(&imports)
            .section(&functions)
            .section(&memories)
            .section(&exports)
            .section(&code)
            .section(&data);
        let bytes = module.finish();
        let cases = [
            (
                "fd_read",
                vec![Val::I32(4), Val::I32(0), Val::I32(1), Val::I32(-1)],
            ),
            (
                "fd_pread",
                vec![
                    Val::I32(4),
                    Val::I32(0),
                    Val::I32(1),
                    Val::I64(0),
                    Val::I32(-1),
                ],
            ),
            (
                "fd_seek",
                vec![Val::I32(4), Val::I64(0), Val::I32(0), Val::I32(-1)],
            ),
            (
                "path_open",
                vec![
                    Val::I32(3),
                    Val::I32(0),
                    Val::I32(128),
                    Val::I32(3),
                    Val::I32(8),
                    Val::I64(0),
                    Val::I64(0),
                    Val::I32(0),
                    Val::I32(-1),
                ],
            ),
        ];
        for (name, params) in cases {
            let mut runtime = crate::Runtime::instantiate(&bytes).unwrap();
            runtime.add_file("out", vec![1, 2, 3, 4]);
            runtime.wasi().open.insert(
                4,
                OpenFile {
                    path: "out".into(),
                    offset: 1,
                    writable: true,
                    retained: None,
                },
            );
            assert!(matches!(
                runtime.call(name, &params).unwrap()[0],
                Val::I32(EINVAL)
            ));
            let wasi = runtime.wasi();
            assert_eq!(wasi.file("out"), Some([1, 2, 3, 4].as_slice()), "{name}");
            assert_eq!(wasi.open[&4].offset, 1, "{name}");
            assert_eq!(runtime.state().read(64, 3).unwrap(), [0x55; 3], "{name}");
        }
    }

    #[test]
    fn invalid_write_result_pointers_preserve_streams_files_and_offsets() {
        use wasm_encoder::{
            CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection, Function,
            FunctionSection, ImportSection, MemorySection, MemoryType, Module, TypeSection,
            ValType,
        };
        let mut types = TypeSection::new();
        types.ty().function([ValType::I32; 4], [ValType::I32]);
        types.ty().function([ValType::I32; 2], [ValType::I32]);
        let mut imports = ImportSection::new();
        imports.import(
            "wasi_snapshot_preview1",
            "fd_write",
            EntityType::Function(0),
        );
        let mut functions = FunctionSection::new();
        functions.function(1);
        let mut memories = MemorySection::new();
        memories.memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
        let mut exports = ExportSection::new();
        exports.export("memory", ExportKind::Memory, 0);
        exports.export("write", ExportKind::Func, 1);
        let mut body = Function::new([]);
        body.instructions()
            .local_get(0)
            .i32_const(0)
            .i32_const(1)
            .local_get(1)
            .call(0)
            .end();
        let mut code = CodeSection::new();
        code.function(&body);
        let mut data = DataSection::new();
        data.active(0, &ConstExpr::i32_const(0), [64, 0, 0, 0, 3, 0, 0, 0]);
        data.active(0, &ConstExpr::i32_const(64), b"abc".iter().copied());
        let mut module = Module::new();
        module
            .section(&types)
            .section(&imports)
            .section(&functions)
            .section(&memories)
            .section(&exports)
            .section(&code)
            .section(&data);
        let bytes = module.finish();
        for fd in [FD_STDOUT, FD_STDERR, FIRST_FILE_FD] {
            let mut runtime = crate::Runtime::instantiate(&bytes).unwrap();
            runtime.add_file("out", vec![9]);
            runtime.wasi().open.insert(
                FIRST_FILE_FD,
                OpenFile {
                    path: "out".into(),
                    offset: 1,
                    writable: true,
                    retained: None,
                },
            );
            let result = runtime
                .call("write", &[Val::I32(fd as i32), Val::I32(-1)])
                .unwrap();
            assert!(matches!(result[0], Val::I32(EINVAL)));
            {
                let wasi = runtime.wasi();
                assert!(wasi.stdout.is_empty() && wasi.stderr.is_empty());
                assert_eq!(wasi.file("out"), Some([9].as_slice()));
                assert_eq!(wasi.open[&FIRST_FILE_FD].offset, 1);
            }
            let result = runtime
                .call("write", &[Val::I32(fd as i32), Val::I32(32)])
                .unwrap();
            assert!(matches!(result[0], Val::I32(ESUCCESS)));
            assert_eq!(runtime.read_u32_at(32).unwrap(), 3);
        }
    }

    #[test]
    fn iovec_tables_and_seek_positions_are_strict() {
        let state = HostState::default();
        assert!(read_iovecs(&state, 0, 1025).is_err());
        assert!(read_iovecs(&state, u32::MAX, 1).is_err());

        assert_eq!(seek_position(10, 20, -5, 1), Some(5));
        assert_eq!(seek_position(10, 20, -11, 1), None);
        assert_eq!(seek_position(10, 20, -21, 2), None);
        assert_eq!(seek_position(10, 20, -1, 0), None);
        assert_eq!(seek_position(10, 20, 0, 3), None);
    }

    /// One-page-memory harness: imports each WASI function, re-exports one
    /// wrapper per entry, and lays out `data` segments. Returns the module
    /// bytes; the caller drives it through `Runtime`.
    type ImportSpec = (&'static str, Vec<ValType>, Vec<ValType>);
    type WrapperSpec = (&'static str, Vec<ValType>, Vec<ValType>, Vec<Wave>);
    fn harness(imports: &[ImportSpec], wrappers: &[WrapperSpec], data: &[(u32, &[u8])]) -> Vec<u8> {
        use wasm_encoder::{
            CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection, Function,
            FunctionSection, ImportSection, MemorySection, MemoryType, Module, TypeSection,
        };
        let mut types = TypeSection::new();
        let mut import_section = ImportSection::new();
        for (index, (name, params, results)) in imports.iter().enumerate() {
            types
                .ty()
                .function(params.iter().copied(), results.iter().copied());
            import_section.import(
                "wasi_snapshot_preview1",
                name,
                EntityType::Function(index as u32),
            );
        }
        let mut functions = FunctionSection::new();
        let mut exports = ExportSection::new();
        let mut code = CodeSection::new();
        for (index, (name, params, results, waves)) in wrappers.iter().enumerate() {
            let ty = (imports.len() + index) as u32;
            types
                .ty()
                .function(params.iter().copied(), results.iter().copied());
            functions.function(ty);
            exports.export(name, ExportKind::Func, (imports.len() + index) as u32);
            let mut body = Function::new([]);
            for wave in waves {
                wave.emit(&mut body);
            }
            body.instructions().end();
            code.function(&body);
        }
        let mut memories = MemorySection::new();
        memories.memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
        exports.export("memory", ExportKind::Memory, 0);
        let mut data_section = DataSection::new();
        for (at, bytes) in data {
            data_section.active(0, &ConstExpr::i32_const(*at as i32), bytes.iter().copied());
        }
        let mut module = Module::new();
        module
            .section(&types)
            .section(&import_section)
            .section(&functions)
            .section(&memories)
            .section(&exports)
            .section(&code)
            .section(&data_section);
        module.finish()
    }

    /// One guest stack push: a constant or a parameter load.
    enum Wave {
        I32(i32),
        I64(i64),
        Arg(u32),
        Call(u32),
    }

    impl Wave {
        fn emit(&self, body: &mut wasm_encoder::Function) {
            match *self {
                Wave::I32(value) => {
                    body.instructions().i32_const(value);
                }
                Wave::I64(value) => {
                    body.instructions().i64_const(value);
                }
                Wave::Arg(index) => {
                    body.instructions().local_get(index);
                }
                Wave::Call(index) => {
                    body.instructions().call(index);
                }
            }
        }
    }

    use ValType::{I32, I64};

    #[test]
    fn unlink_keeps_open_descriptors_usable() {
        let bytes = harness(
            &[
                ("path_unlink_file", vec![I32, I32, I32], vec![I32]),
                ("fd_read", vec![I32, I32, I32, I32], vec![I32]),
            ],
            &[
                (
                    "unlink",
                    vec![],
                    vec![I32],
                    vec![Wave::I32(3), Wave::I32(128), Wave::I32(3), Wave::Call(0)],
                ),
                (
                    "readout",
                    vec![],
                    vec![I32],
                    vec![
                        Wave::I32(4),
                        Wave::I32(0),
                        Wave::I32(1),
                        Wave::I32(200),
                        Wave::Call(1),
                    ],
                ),
            ],
            &[
                (0, &[64, 0, 0, 0, 3, 0, 0, 0]),
                (64, &[0x55; 3]),
                (128, b"out"),
            ],
        );
        let mut runtime = crate::Runtime::instantiate(&bytes).unwrap();
        runtime.add_file("out", vec![7, 8, 9]);
        runtime.wasi().open.insert(
            4,
            OpenFile {
                path: "out".into(),
                offset: 0,
                writable: true,
                retained: None,
            },
        );
        assert!(matches!(
            runtime.call("unlink", &[]).unwrap()[0],
            Val::I32(ESUCCESS)
        ));
        assert!(matches!(
            runtime.call("readout", &[]).unwrap()[0],
            Val::I32(ESUCCESS)
        ));
        assert_eq!(runtime.read_u32_at(200).unwrap(), 3);
        assert_eq!(runtime.state().read(64, 3).unwrap(), [7, 8, 9]);
        // Recreating the path starts a new file; the retained handle keeps the old bytes.
        runtime.add_file("out", vec![1]);
        runtime.wasi().open.get_mut(&4).unwrap().offset = 0;
        assert!(matches!(
            runtime.call("readout", &[]).unwrap()[0],
            Val::I32(ESUCCESS)
        ));
        assert_eq!(runtime.state().read(64, 3).unwrap(), [7, 8, 9]);
    }

    #[test]
    fn path_open_validates_its_directory_descriptor_and_flags() {
        let bytes = harness(
            &[(
                "path_open",
                vec![I32, I32, I32, I32, I32, I64, I64, I32, I32],
                vec![I32],
            )],
            &[(
                "opendir",
                vec![I32, I32],
                vec![I32],
                vec![
                    Wave::Arg(0),
                    Wave::Arg(1),
                    Wave::I32(128),
                    Wave::I32(3),
                    Wave::I32(8),
                    Wave::I64(0),
                    Wave::I64(0),
                    Wave::I32(0),
                    Wave::I32(208),
                    Wave::Call(0),
                ],
            )],
            &[(128, b"out")],
        );
        let mut runtime = crate::Runtime::instantiate(&bytes).unwrap();
        runtime.add_file("out", vec![1]);
        assert!(matches!(
            runtime
                .call("opendir", &[Val::I32(99), Val::I32(0)])
                .unwrap()[0],
            Val::I32(EBADF)
        ));
        assert!(matches!(
            runtime
                .call("opendir", &[Val::I32(3), Val::I32(2)])
                .unwrap()[0],
            Val::I32(EINVAL)
        ));
        // SYMLINK_FOLLOW is vacuous without symlinks and real modules pass
        // it; only further lookup flags are rejected.
        assert!(matches!(
            runtime
                .call("opendir", &[Val::I32(3), Val::I32(1)])
                .unwrap()[0],
            Val::I32(ESUCCESS)
        ));
        assert!(matches!(
            runtime
                .call("opendir", &[Val::I32(3), Val::I32(0)])
                .unwrap()[0],
            Val::I32(ESUCCESS)
        ));
    }

    #[test]
    fn failed_clock_reads_do_not_advance_the_clock() {
        let bytes = harness(
            &[("clock_time_get", vec![I32, I64, I32], vec![I32])],
            &[(
                "clock",
                vec![I32, I32],
                vec![I32],
                vec![Wave::Arg(0), Wave::I64(0), Wave::Arg(1), Wave::Call(0)],
            )],
            &[],
        );
        let mut runtime = crate::Runtime::instantiate(&bytes).unwrap();
        assert!(matches!(
            runtime.call("clock", &[Val::I32(1), Val::I32(-1)]).unwrap()[0],
            Val::I32(EINVAL)
        ));
        assert!(matches!(
            runtime.call("clock", &[Val::I32(1), Val::I32(64)]).unwrap()[0],
            Val::I32(ESUCCESS)
        ));
        let first = runtime.state().read(64, 8).unwrap();
        let mut fresh = crate::Runtime::instantiate(&bytes).unwrap();
        assert!(matches!(
            fresh.call("clock", &[Val::I32(1), Val::I32(64)]).unwrap()[0],
            Val::I32(ESUCCESS)
        ));
        let baseline = fresh.state().read(64, 8).unwrap();
        assert_eq!(
            first, baseline,
            "a failed clock_time_get must not consume a tick"
        );
    }

    #[test]
    fn sparse_writes_are_bounded_before_growth() {
        const FAR: i64 = 64 * 1024 * 1024 + 1;
        let bytes = harness(
            &[
                ("fd_seek", vec![I32, I64, I32, I32], vec![I32]),
                ("fd_write", vec![I32, I32, I32, I32], vec![I32]),
            ],
            &[
                (
                    "seekfar",
                    vec![],
                    vec![I32],
                    vec![
                        Wave::I32(4),
                        Wave::I64(FAR),
                        Wave::I32(0),
                        Wave::I32(200),
                        Wave::Call(0),
                    ],
                ),
                (
                    "writeone",
                    vec![],
                    vec![I32],
                    vec![
                        Wave::I32(4),
                        Wave::I32(0),
                        Wave::I32(1),
                        Wave::I32(208),
                        Wave::Call(1),
                    ],
                ),
            ],
            &[(0, &[64, 0, 0, 0, 1, 0, 0, 0]), (64, &[0xAA])],
        );
        let mut runtime = crate::Runtime::instantiate(&bytes).unwrap();
        runtime.add_file("out", vec![0; 16]);
        runtime.wasi().open.insert(
            4,
            OpenFile {
                path: "out".into(),
                offset: 0,
                writable: true,
                retained: None,
            },
        );
        assert!(matches!(
            runtime.call("seekfar", &[]).unwrap()[0],
            Val::I32(ESUCCESS)
        ));
        assert!(matches!(
            runtime.call("writeone", &[]).unwrap()[0],
            Val::I32(EINVAL)
        ));
        assert_eq!(runtime.wasi().file("out").unwrap().len(), 16);
        assert_eq!(runtime.wasi().open[&4].offset, FAR as u64);
    }

    #[test]
    fn prestat_dir_name_honors_the_caller_buffer_length() {
        let bytes = harness(
            &[("fd_prestat_dir_name", vec![I32, I32, I32], vec![I32])],
            &[(
                "prestat",
                vec![I32, I32],
                vec![I32],
                vec![Wave::I32(3), Wave::Arg(0), Wave::Arg(1), Wave::Call(0)],
            )],
            &[(64, &[0x55; 4])],
        );
        let mut runtime = crate::Runtime::instantiate(&bytes).unwrap();
        assert!(matches!(
            runtime
                .call("prestat", &[Val::I32(64), Val::I32(0)])
                .unwrap()[0],
            Val::I32(EINVAL)
        ));
        assert_eq!(runtime.state().read(64, 1).unwrap(), [0x55]);
        assert!(matches!(
            runtime
                .call("prestat", &[Val::I32(64), Val::I32(1)])
                .unwrap()[0],
            Val::I32(ESUCCESS)
        ));
        assert_eq!(runtime.state().read(64, 1).unwrap(), [b'/']);
    }
}
