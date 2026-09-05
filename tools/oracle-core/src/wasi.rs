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
}

/// Paths arrive with and without a leading slash depending on how the guest
/// composed them; one spelling is stored so both find the same file.
fn normalise(path: &str) -> String {
    path.trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned()
}

/// An `iovec`/`ciovec`: a pointer and a length, 8 bytes each.
fn read_iovecs(state: &HostState, ptr: u32, count: u32) -> Vec<(u32, u32)> {
    const MAX: u32 = 1024;

    (0..count.min(MAX))
        .filter_map(|index| {
            let base = ptr + index * 8;
            Some((state.read_u32(base).ok()?, state.read_u32(base + 4).ok()?))
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
    let vectors = read_iovecs(caller.data(), iovs, count);

    let mut payload = Vec::new();
    for (ptr, len) in vectors {
        match caller.data().read(ptr, len) {
            Ok(bytes) => payload.extend_from_slice(&bytes),
            Err(_) => return EINVAL,
        }
    }
    let written = payload.len() as u32;

    let state = caller.data_mut();
    match fd {
        FD_STDOUT => state.wasi.stdout.extend_from_slice(&payload),
        FD_STDERR => state.wasi.stderr.extend_from_slice(&payload),
        _ => {
            let Some(open) = state.wasi.open.get_mut(&fd) else {
                return EBADF;
            };
            if !open.writable {
                return EBADF;
            }
            let (path, offset) = (open.path.clone(), open.offset as usize);
            open.offset += written as u64;

            let file = state.wasi.files.entry(path).or_default();
            if file.len() < offset + payload.len() {
                file.resize(offset + payload.len(), 0);
            }
            file[offset..offset + payload.len()].copy_from_slice(&payload);
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
    if fd == FD_STDIN {
        // Always at end of input; no interactive stdin exists here.
        return match write_u32(caller.data(), out, 0) {
            Ok(()) => ESUCCESS,
            Err(_) => EINVAL,
        };
    }

    let vectors = read_iovecs(caller.data(), iovs, count);
    let state = caller.data();
    let Some(open) = state.wasi.open.get(&fd) else {
        return EBADF;
    };
    let Some(contents) = state.wasi.files.get(&open.path) else {
        return EBADF;
    };

    let mut offset = at.unwrap_or(open.offset) as usize;
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
        total += take as u32;
    }

    // A positional read leaves the cursor where it was: that is the whole
    // difference between `pread` and `read`.
    if at.is_none()
        && let Some(open) = caller.data_mut().wasi.open.get_mut(&fd)
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
    let state = caller.data();
    let Some(open) = state.wasi.open.get(&fd) else {
        return EBADF;
    };
    let size = state
        .wasi
        .files
        .get(&open.path)
        .map(|file| file.len() as i64)
        .unwrap_or(0);

    // whence: 0 = set, 1 = current, 2 = end
    let base = match whence {
        0 => 0,
        1 => open.offset as i64,
        2 => size,
        _ => return EINVAL,
    };
    let position = (base + delta).max(0) as u64;

    if let Some(open) = caller.data_mut().wasi.open.get_mut(&fd) {
        open.offset = position;
    }
    match write_u64(caller.data(), out, position) {
        Ok(()) => ESUCCESS,
        Err(_) => EINVAL,
    }
}

fn path_open(
    caller: &mut Caller<'_, HostState>,
    path_ptr: u32,
    path_len: u32,
    oflags: u32,
    out: u32,
) -> i32 {
    // oflags bit 0 is O_CREAT.
    const O_CREAT: u32 = 1;

    let Ok(raw) = caller.data().read(path_ptr, path_len) else {
        return EINVAL;
    };
    let path = normalise(&String::from_utf8_lossy(&raw));

    // What the guest asked for, whether or not it got it. A refusal here is
    // invisible from the outside — the VoIP engine reports only "Failed to get
    // voip storage dir" and carries on — so without this the host cannot tell a
    // path it should have provided from one the guest never wanted.
    caller.data().log(format!(
        "wasi path_open {path:?} (create={})",
        oflags & O_CREAT != 0
    ));

    let state = caller.data_mut();
    let exists = state.wasi.files.contains_key(&path);
    if !exists {
        if oflags & O_CREAT == 0 {
            return ENOENT;
        }
        state.wasi.files.insert(path.clone(), Vec::new());
    }

    let fd = state.wasi.allocate_fd();
    state.wasi.open.insert(
        fd,
        OpenFile {
            path,
            offset: 0,
            writable: true,
        },
    );

    match write_u32(caller.data(), out, fd) {
        Ok(()) => ESUCCESS,
        Err(_) => EINVAL,
    }
}

/// `filestat` is 64 bytes; only the size field carries information here.
fn fd_filestat_get(caller: &mut Caller<'_, HostState>, fd: u32, out: u32) -> i32 {
    let state = caller.data();
    let size = match state.wasi.open.get(&fd) {
        Some(open) => state
            .wasi
            .files
            .get(&open.path)
            .map(|file| file.len() as u64)
            .unwrap_or(0),
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
    let known = fd <= FD_ROOT || state.wasi.open.contains_key(&fd);
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
        .wasi
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
                .wasi
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
            match caller.data().write(arg(params, 1), ROOT_NAME.as_bytes()) {
                Ok(()) => ESUCCESS,
                Err(_) => EINVAL,
            }
        }
        "path_open" => path_open(
            caller,
            arg(params, 2),
            arg(params, 3),
            arg(params, 4),
            arg(params, 8),
        ),
        "path_unlink_file" => {
            let Ok(raw) = caller.data().read(arg(params, 1), arg(params, 2)) else {
                return EINVAL;
            };
            let path = normalise(&String::from_utf8_lossy(&raw));
            match caller.data_mut().wasi.files.remove(&path) {
                Some(_) => ESUCCESS,
                None => ENOENT,
            }
        }
        "args_sizes_get" => {
            let args = caller.data().wasi.args.clone();
            list_sizes(caller.data(), &args, arg(params, 0), arg(params, 1))
        }
        "args_get" => {
            let args = caller.data().wasi.args.clone();
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
