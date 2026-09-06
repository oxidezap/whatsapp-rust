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
use std::sync::{Arc, Mutex};

use anyhow::{Result, bail};
use wasmtime::error::Context as _;
use wasmtime::{Caller, FuncType, Linker, Module, Store, Val, ValType};

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

/// WASI `RIGHTS_FD_WRITE`: without it the descriptor cannot be written, no
/// matter which creation flags were used.
const RIGHTS_FD_WRITE: u32 = 64;

/// Largest single file the memfs will hold. The 256 MiB payload limit cannot
/// stop a sparse attack on its own: seeking a writable descriptor near
/// `u64::MAX` and writing one byte would otherwise `resize` towards it.
const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
/// Largest the whole memfs may hold across live files and unlinked-but-open ones.
const MAX_FS_BYTES: usize = 256 * 1024 * 1024;
/// Largest either captured standard stream may grow to. The per-call payload
/// limit cannot stop a looping guest from retaining output forever.
const MAX_STREAM_BYTES: usize = 16 * 1024 * 1024;
/// Longest guest pathname the host will copy. The memory-range check alone
/// still allocates the whole `path_len` before decoding it.
const MAX_PATH_BYTES: u32 = 4096;

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
    /// Contents kept alive after the path was unlinked; see `unlink_file`.
    /// Shared, not cloned: every handle unlinked from one path reads the
    /// same bytes until one of them writes, and no unlink can multiply the
    /// file's footprint by its descriptor count.
    pub retained: Option<SharedFile>,
}

/// File contents shared by every descriptor that names them.
///
/// Descriptors opened from one path share one object, including across
/// `unlink`: recreating the path starts a new object while retained handles
/// keep the old one. The inner lock is only ever taken with the WASI state
/// lock already held, so no lock ordering exists to invert.
type SharedFile = Arc<Mutex<Vec<u8>>>;

/// Guest-visible environment: arguments, variables, and an in-memory filesystem.
#[derive(Debug, Clone, Default)]
pub struct WasiState {
    /// The command line the guest sees.
    pub args: Vec<String>,
    /// The environment the guest sees.
    pub env: Vec<(String, String)>,
    /// Files the guest can open, by path.
    pub files: BTreeMap<String, SharedFile>,
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
        self.files
            .insert(normalise(&path.into()), Arc::new(Mutex::new(contents)));
    }

    /// Reads a file back, which is how an output file is collected after a run.
    pub fn file(&self, path: &str) -> Option<Vec<u8>> {
        self.files
            .get(&normalise(path))
            .map(|file| file.lock().expect("WASI file poisoned").clone())
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

    /// The shared object visible through `open`: retained post-unlink
    /// contents, else the live file.
    fn open_file(&self, open: &OpenFile) -> Option<SharedFile> {
        open.retained
            .clone()
            .or_else(|| self.files.get(&open.path).cloned())
    }

    /// Removes `path`, keeping its object alive in every open descriptor that
    /// names it. The bytes move, uncloned: any number of descriptors can be
    /// unlinked from one file without multiplying its footprint. Recreating
    /// the path starts a new object; retained handles keep the old one.
    /// Returns whether the path existed.
    fn unlink_file(&mut self, path: &str) -> bool {
        let Some(shared) = self.files.remove(path) else {
            return false;
        };
        for open in self.open.values_mut() {
            if open.path == path && open.retained.is_none() {
                open.retained = Some(Arc::clone(&shared));
            }
        }
        true
    }
}

/// Bytes currently held: live files plus contents retained by unlinked handles.
///
/// A retained object still referenced by several descriptors is counted once
/// per descriptor: the budget stays conservative rather than tracking shares.
fn fs_bytes(wasi: &WasiState) -> usize {
    let live: usize = wasi
        .files
        .values()
        .map(|file| file.lock().expect("WASI file poisoned").len())
        .sum();
    let retained: usize = wasi
        .open
        .values()
        .filter_map(|open| {
            open.retained
                .as_ref()
                .map(|file| file.lock().expect("WASI file poisoned").len())
        })
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
        FD_STDOUT | FD_STDERR => {
            let stream = if fd == FD_STDOUT {
                &mut wasi.stdout
            } else {
                &mut wasi.stderr
            };
            if stream.len().saturating_add(payload.len()) > MAX_STREAM_BYTES {
                return EINVAL;
            }
            stream.extend_from_slice(&payload);
        }
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
            let Some(target) = wasi.open_file(open) else {
                return EBADF;
            };
            // The length is read before the budget check: the file lock is
            // not reentrant, so it cannot be held across `fs_bytes`.
            let current = target.lock().expect("WASI file poisoned").len();
            if end > MAX_FILE_BYTES
                || fs_bytes(&wasi).saturating_sub(current).saturating_add(end) > MAX_FS_BYTES
            {
                return EINVAL;
            }
            let mut contents = target.lock().expect("WASI file poisoned");
            if contents.len() < end {
                contents.resize(end, 0);
            }
            contents[offset..end].copy_from_slice(&payload);
            drop(contents);
            let Some(open) = wasi.open.get_mut(&fd) else {
                return EBADF;
            };
            open.offset = next_offset;
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
    let Some(target) = wasi.open_file(open) else {
        return EBADF;
    };

    let Ok(mut offset) = usize::try_from(at.unwrap_or(open.offset)) else {
        return EINVAL;
    };
    let contents = target.lock().expect("WASI file poisoned");
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
        .and_then(|open| wasi.open_file(open))
        .map_or(0, |file| {
            file.lock().expect("WASI file poisoned").len() as u64
        });
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
    rights_base: u64,
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
        || args.path_len > MAX_PATH_BYTES
        || caller.data().ensure_memory_range(args.out, 4).is_err()
    {
        return EINVAL;
    }

    let Ok(raw) = caller.data().read(args.path_ptr, args.path_len) else {
        return EINVAL;
    };
    // A lossy decode could alias invalid bytes onto a valid filename holding
    // U+FFFD and resolve a file the guest did not name.
    let Ok(path) = String::from_utf8(raw) else {
        return EINVAL;
    };
    let path = normalise(&path);
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
        wasi.files
            .insert(path.clone(), Arc::new(Mutex::new(Vec::new())));
    } else if args.oflags & O_TRUNC != 0 {
        wasi.files
            .get(&path)
            .expect("existence checked")
            .lock()
            .expect("WASI file poisoned")
            .clear();
    }

    let fd = wasi.allocate_fd();
    // Writability comes from the requested rights, not the creation flags: a
    // guest that opens without `RIGHTS_FD_WRITE` gets a read-only descriptor
    // even if it also passed `O_CREAT`.
    let writable = args.rights_base & u64::from(RIGHTS_FD_WRITE) != 0;
    wasi.open.insert(
        fd,
        OpenFile {
            path,
            offset: 0,
            writable,
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
        Some(open) => wasi.open_file(open).map_or(0, |file| {
            file.lock().expect("WASI file poisoned").len() as u64
        }),
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
///
/// Both destination ranges are preflighted before any byte is written: a
/// later out-of-bounds entry must fail without leaving a partially written
/// prefix behind for a guest retry to observe.
fn write_string_list(state: &HostState, entries: &[String], ptrs: u32, buffer: u32) -> i32 {
    let mut cursor = buffer;
    let mut slots = Vec::with_capacity(entries.len());
    let mut spans = Vec::with_capacity(entries.len());
    for entry in entries {
        let bytes = entry.len().checked_add(1);
        let next = bytes.and_then(|len| cursor.checked_add(len as u32));
        let Some(next) = next else {
            return EINVAL;
        };
        slots.push(cursor);
        spans.push((cursor, next));
        cursor = next;
    }
    let table_end = (ptrs as u64)
        .checked_add(entries.len() as u64 * 4)
        .and_then(|end| u32::try_from(end).ok());
    let Some(table_end) = table_end else {
        return EINVAL;
    };
    if state.ensure_memory_range(ptrs, table_end - ptrs).is_err()
        || state.ensure_memory_range(buffer, cursor - buffer).is_err()
    {
        return EINVAL;
    }
    for (index, at) in slots.into_iter().enumerate() {
        if write_u32(state, ptrs + index as u32 * 4, at).is_err() {
            return EINVAL;
        }
    }
    for (entry, (at, _)) in entries.iter().zip(spans) {
        let mut bytes = entry.clone().into_bytes();
        bytes.push(0);
        if state.write(at, &bytes).is_err() {
            return EINVAL;
        }
    }
    ESUCCESS
}

fn list_sizes(state: &HostState, entries: &[String], count_ptr: u32, size_ptr: u32) -> i32 {
    let bytes: usize = entries.iter().map(|entry| entry.len() + 1).sum();
    if state.ensure_memory_range(count_ptr, 4).is_err()
        || state.ensure_memory_range(size_ptr, 4).is_err()
    {
        return EINVAL;
    }
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

/// Exact preview-1 signature each implemented import must declare.
///
/// A module that declares a recognized name with another type would otherwise
/// link, and the argument helpers would silently substitute zero for missing
/// or mistyped operands — a malformed capture then yields plausible evidence
/// instead of failing at instantiation.
fn expected_signatures(name: &str) -> Option<&'static [(&'static [ValType], &'static [ValType])]> {
    use wasmtime::ValType as T;
    const R: &[T] = &[T::I32];
    const NO_RESULTS: &[T] = &[];
    const A1: &[T] = &[T::I32];
    const A2: &[T] = &[T::I32, T::I32];
    const A3: &[T] = &[T::I32, T::I32, T::I32];
    const A4: &[T] = &[T::I32, T::I32, T::I32, T::I32];
    const SEEK: &[T] = &[T::I32, T::I64, T::I32, T::I32];
    const SEEK_LEGALIZED: &[T] = &[T::I32, T::I32, T::I32, T::I32, T::I32];
    const PREAD: &[T] = &[T::I32, T::I32, T::I32, T::I64, T::I32];
    const CLOCK: &[T] = &[T::I32, T::I64, T::I32];
    const OPEN: &[T] = &[
        T::I32,
        T::I32,
        T::I32,
        T::I32,
        T::I32,
        T::I64,
        T::I64,
        T::I32,
        T::I32,
    ];
    const SEEK_FORMS: &[(&[T], &[T])] = &[(SEEK, R), (SEEK_LEGALIZED, R)];
    Some(match name {
        "fd_write" | "fd_read" => &[(A4, R)],
        "fd_pread" => &[(PREAD, R)],
        // Emscripten without BigInt splits the `fd_seek` offset into two
        // `i32` halves. The VOPRF capture declares exactly this; no other
        // capture legalizes any import, so only this spelling is accepted
        // beside the canonical one.
        "fd_seek" => SEEK_FORMS,
        "fd_close" => &[(A1, R)],
        "fd_fdstat_get" | "fd_filestat_get" | "fd_prestat_get" => &[(A2, R)],
        "fd_prestat_dir_name" | "path_unlink_file" => &[(A3, R)],
        "path_open" => &[(OPEN, R)],
        "args_sizes_get" | "args_get" | "environ_sizes_get" | "environ_get" => &[(A2, R)],
        "clock_time_get" => &[(CLOCK, R)],
        "random_get" => &[(A2, R)],
        "proc_exit" => &[(A1, NO_RESULTS)],
        _ => return None,
    })
}

/// Whether a declared import type is exactly the expected signature.
fn signature_matches(ty: &FuncType, params: &[ValType], results: &[ValType]) -> bool {
    ty.params().len() == params.len()
        && ty.results().len() == results.len()
        && ty
            .params()
            .zip(params)
            .all(|(actual, expected)| ValType::eq(&actual, expected))
        && ty
            .results()
            .zip(results)
            .all(|(actual, expected)| ValType::eq(&actual, expected))
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
        if let Some(forms) = expected_signatures(import.name())
            && !forms
                .iter()
                .any(|(params, results)| signature_matches(&ty, params, results))
        {
            bail!("wasi {} declares an unsupported signature", import.name());
        }

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
        "fd_seek" => {
            // Canonical `(fd, offset:i64, whence, out)` or the legalized
            // `(fd, lo, hi, whence, out)` the VOPRF capture declares. The
            // signature was validated at definition, so the length decides.
            let (delta, whence, out) = if params.len() == 5 {
                let delta = u64::from(arg(params, 1)) | (u64::from(arg(params, 2)) << 32);
                (delta as i64, arg(params, 3), arg(params, 4))
            } else {
                (arg_i64(params, 1), arg(params, 2), arg(params, 3))
            };
            fd_seek(caller, arg(params, 0), delta, whence, out)
        }
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
                rights_base: arg_i64(params, 5) as u64,
                fdflags: arg(params, 7),
                out: arg(params, 8),
            },
        ),
        "path_unlink_file" => {
            if arg(params, 0) != FD_ROOT {
                return EBADF;
            }
            if arg(params, 2) > MAX_PATH_BYTES {
                return EINVAL;
            }
            let Ok(raw) = caller.data().read(arg(params, 1), arg(params, 2)) else {
                return EINVAL;
            };
            // Unlinking is destructive, so an encoding the host cannot
            // interpret exactly must fail rather than delete under a
            // lossy alias.
            let Ok(path) = String::from_utf8(raw) else {
                return EINVAL;
            };
            let path = normalise(&path);
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
            assert_eq!(
                wasi.file("out").as_deref(),
                Some([1, 2, 3, 4].as_slice()),
                "{name}"
            );
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
                assert_eq!(wasi.file("out").as_deref(), Some([9].as_slice()));
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
                    "unlinkat",
                    vec![I32, I32, I32],
                    vec![I32],
                    vec![Wave::Arg(0), Wave::Arg(1), Wave::Arg(2), Wave::Call(0)],
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
                (140, &[0xff, 0xfe]),
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
        // A descriptor that is not the preopened root cannot unlink.
        assert!(matches!(
            runtime
                .call("unlinkat", &[Val::I32(99), Val::I32(128), Val::I32(3)])
                .unwrap()[0],
            Val::I32(EBADF)
        ));
        // Invalid UTF-8 cannot name a file, so nothing is deleted.
        assert!(matches!(
            runtime
                .call("unlinkat", &[Val::I32(3), Val::I32(140), Val::I32(2)])
                .unwrap()[0],
            Val::I32(EINVAL)
        ));
        assert!(matches!(
            runtime.call("readout", &[]).unwrap()[0],
            Val::I32(ESUCCESS)
        ));
        assert_eq!(runtime.read_u32_at(200).unwrap(), 3);
        runtime.wasi().open.get_mut(&4).unwrap().offset = 0;
        // A second handle to the same path shares the retained object after
        // unlink instead of cloning the bytes once per descriptor.
        runtime.wasi().open.insert(
            5,
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
        {
            let wasi = runtime.wasi();
            let first = wasi.open[&4].retained.clone().unwrap();
            let second = wasi.open[&5].retained.clone().unwrap();
            assert!(std::sync::Arc::ptr_eq(&first, &second));
            assert_eq!(first.lock().expect("retained").len(), 3);
        }
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
            &[
                (
                    "path_open",
                    vec![I32, I32, I32, I32, I32, I64, I64, I32, I32],
                    vec![I32],
                ),
                ("fd_write", vec![I32, I32, I32, I32], vec![I32]),
            ],
            &[
                (
                    "opendir",
                    vec![I32, I32, I64],
                    vec![I32],
                    vec![
                        Wave::Arg(0),
                        Wave::Arg(1),
                        Wave::I32(128),
                        Wave::I32(3),
                        Wave::I32(8),
                        Wave::Arg(2),
                        Wave::I64(0),
                        Wave::I32(0),
                        Wave::I32(208),
                        Wave::Call(0),
                    ],
                ),
                (
                    "writeout",
                    vec![I32],
                    vec![I32],
                    vec![
                        Wave::Arg(0),
                        Wave::I32(0),
                        Wave::I32(1),
                        Wave::I32(208),
                        Wave::Call(1),
                    ],
                ),
            ],
            &[
                (0, &[64, 0, 0, 0, 1, 0, 0, 0]),
                (64, &[0xAA]),
                (128, b"out"),
            ],
        );
        let mut runtime = crate::Runtime::instantiate(&bytes).unwrap();
        runtime.add_file("out", vec![1]);
        assert!(matches!(
            runtime
                .call("opendir", &[Val::I32(99), Val::I32(0), Val::I64(0)])
                .unwrap()[0],
            Val::I32(EBADF)
        ));
        assert!(matches!(
            runtime
                .call("opendir", &[Val::I32(3), Val::I32(2), Val::I64(0)])
                .unwrap()[0],
            Val::I32(EINVAL)
        ));
        // SYMLINK_FOLLOW is vacuous without symlinks and real modules pass
        // it; only further lookup flags are rejected.
        assert!(matches!(
            runtime
                .call("opendir", &[Val::I32(3), Val::I32(1), Val::I64(0)])
                .unwrap()[0],
            Val::I32(ESUCCESS)
        ));
        // Without FD_WRITE the descriptor is read-only even with O_TRUNC.
        assert!(matches!(
            runtime.call("writeout", &[Val::I32(4)]).unwrap()[0],
            Val::I32(EBADF)
        ));
        assert!(matches!(
            runtime
                .call(
                    "opendir",
                    &[Val::I32(3), Val::I32(0), Val::I64(RIGHTS_FD_WRITE as i64)]
                )
                .unwrap()[0],
            Val::I32(ESUCCESS)
        ));
        assert!(matches!(
            runtime.call("writeout", &[Val::I32(5)]).unwrap()[0],
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

    #[test]
    fn wasi_symbols_require_preview1_signatures() {
        let bytes = harness(
            &[("fd_write", vec![], vec![I32])],
            &[("write", vec![], vec![I32], vec![Wave::Call(0)])],
            &[],
        );
        assert!(crate::Runtime::instantiate(&bytes).is_err());
        let bytes = harness(
            &[("random_get", vec![I32], vec![I32])],
            &[(
                "rand",
                vec![I32],
                vec![I32],
                vec![Wave::Arg(0), Wave::Call(0)],
            )],
            &[],
        );
        assert!(crate::Runtime::instantiate(&bytes).is_err());
    }

    #[test]
    fn legalized_seek_offsets_decode_correctly() {
        let bytes = harness(
            &[("fd_seek", vec![I32, I32, I32, I32, I32], vec![I32])],
            &[(
                "seek5",
                vec![I32, I32, I32, I32, I32],
                vec![I32],
                vec![
                    Wave::Arg(0),
                    Wave::Arg(1),
                    Wave::Arg(2),
                    Wave::Arg(3),
                    Wave::Arg(4),
                    Wave::Call(0),
                ],
            )],
            &[],
        );
        let mut runtime = crate::Runtime::instantiate(&bytes).unwrap();
        runtime.add_file("out", vec![0; 8]);
        runtime.wasi().open.insert(
            4,
            OpenFile {
                path: "out".into(),
                offset: 0,
                writable: true,
                retained: None,
            },
        );
        // High half 1, low half 5: position 0x1_0000_0005, not 5, and the
        // result lands in the fifth slot rather than the fourth.
        assert!(matches!(
            runtime
                .call(
                    "seek5",
                    &[
                        Val::I32(4),
                        Val::I32(5),
                        Val::I32(1),
                        Val::I32(0),
                        Val::I32(200)
                    ]
                )
                .unwrap()[0],
            Val::I32(ESUCCESS)
        ));
        let position = runtime.state().read(200, 8).unwrap();
        assert_eq!(
            u64::from_le_bytes(position.try_into().unwrap()),
            0x1_0000_0005
        );
    }

    #[test]
    fn path_lengths_are_bounded_before_reads() {
        let bytes = harness(
            &[
                (
                    "path_open",
                    vec![I32, I32, I32, I32, I32, I64, I64, I32, I32],
                    vec![I32],
                ),
                ("path_unlink_file", vec![I32, I32, I32], vec![I32]),
            ],
            &[
                (
                    "openat",
                    vec![I32, I32, I32],
                    vec![I32],
                    vec![
                        Wave::Arg(0),
                        Wave::I32(0),
                        Wave::Arg(1),
                        Wave::Arg(2),
                        Wave::I32(0),
                        Wave::I64(0),
                        Wave::I64(0),
                        Wave::I32(0),
                        Wave::I32(208),
                        Wave::Call(0),
                    ],
                ),
                (
                    "unlinkat",
                    vec![I32, I32, I32],
                    vec![I32],
                    vec![Wave::Arg(0), Wave::Arg(1), Wave::Arg(2), Wave::Call(1)],
                ),
            ],
            &[],
        );
        let mut runtime = crate::Runtime::instantiate(&bytes).unwrap();
        // 8192 bytes are addressable in the single page, but no pathname is.
        assert!(matches!(
            runtime
                .call("openat", &[Val::I32(3), Val::I32(64), Val::I32(8192)])
                .unwrap()[0],
            Val::I32(EINVAL)
        ));
        assert!(matches!(
            runtime
                .call("unlinkat", &[Val::I32(3), Val::I32(64), Val::I32(8192)])
                .unwrap()[0],
            Val::I32(EINVAL)
        ));
    }

    #[test]
    fn argv_writes_leave_no_partial_prefix_behind() {
        let bytes = harness(
            &[
                ("args_sizes_get", vec![I32, I32], vec![I32]),
                ("args_get", vec![I32, I32], vec![I32]),
            ],
            &[
                (
                    "sizes",
                    vec![I32, I32],
                    vec![I32],
                    vec![Wave::Arg(0), Wave::Arg(1), Wave::Call(0)],
                ),
                (
                    "getargs",
                    vec![I32, I32],
                    vec![I32],
                    vec![Wave::Arg(0), Wave::Arg(1), Wave::Call(1)],
                ),
            ],
            &[(64, &[0x55; 8])],
        );
        let mut runtime = crate::Runtime::instantiate(&bytes).unwrap();
        runtime.set_args(&["ab".to_owned()]);
        // One argument needs a 4-byte pointer slot and 3 buffer bytes. A
        // buffer with only 2 bytes left fails without writing the pointer.
        assert!(matches!(
            runtime
                .call("getargs", &[Val::I32(64), Val::I32(65534)])
                .unwrap()[0],
            Val::I32(EINVAL)
        ));
        assert_eq!(runtime.state().read(64, 4).unwrap(), [0x55; 4]);
        // Same for the sizes pair: a bad size destination leaves the count.
        assert!(matches!(
            runtime
                .call("sizes", &[Val::I32(64), Val::I32(-1)])
                .unwrap()[0],
            Val::I32(EINVAL)
        ));
        assert_eq!(runtime.state().read(64, 4).unwrap(), [0x55; 4]);
        // The valid layout still works (`set_args` prefixes argv[0]).
        assert!(matches!(
            runtime
                .call("getargs", &[Val::I32(64), Val::I32(128)])
                .unwrap()[0],
            Val::I32(ESUCCESS)
        ));
        assert_eq!(runtime.read_u32_at(64).unwrap(), 128);
        assert_eq!(runtime.read_u32_at(68).unwrap(), 135);
        assert_eq!(runtime.state().read(135, 3).unwrap(), b"ab\0");
    }

    #[test]
    fn standard_streams_are_bounded() {
        use wasm_encoder::{
            CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection,
            Function as WasmFunction, FunctionSection, ImportSection, MemorySection, MemoryType,
            Module as WasmModule, TypeSection,
        };
        const PAGES: u32 = 2;
        const CHUNK: usize = 128 * 1024;
        let mut types = TypeSection::new();
        types.ty().function([I32, I32, I32, I32], [I32]);
        types.ty().function([I32], [I32]);
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
            minimum: PAGES as u64,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
        let mut exports = ExportSection::new();
        exports.export("memory", ExportKind::Memory, 0);
        exports.export("fill", ExportKind::Func, 1);
        let mut body = WasmFunction::new([]);
        let end = PAGES * 65_536;
        body.instructions()
            .local_get(0)
            .i32_const((end - 12) as i32)
            .i32_const(1)
            .i32_const((end - 4) as i32)
            .call(0)
            .end();
        let mut code = CodeSection::new();
        code.function(&body);
        let mut iovec = 0u32.to_le_bytes().to_vec();
        iovec.extend_from_slice(&(CHUNK as u32).to_le_bytes());
        let mut data = DataSection::new();
        data.active(0, &ConstExpr::i32_const((end - 12) as i32), iovec);
        let mut module = WasmModule::new();
        module
            .section(&types)
            .section(&imports)
            .section(&functions)
            .section(&memories)
            .section(&exports)
            .section(&code)
            .section(&data);
        let bytes = module.finish();
        let mut runtime = crate::Runtime::instantiate(&bytes).unwrap();
        for _ in 0..MAX_STREAM_BYTES / CHUNK {
            assert!(matches!(
                runtime.call("fill", &[Val::I32(1)]).unwrap()[0],
                Val::I32(ESUCCESS)
            ));
        }
        assert!(matches!(
            runtime.call("fill", &[Val::I32(1)]).unwrap()[0],
            Val::I32(EINVAL)
        ));
        assert_eq!(runtime.wasi().stdout.len(), MAX_STREAM_BYTES);
    }
}
