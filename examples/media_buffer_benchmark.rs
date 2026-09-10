//! Benchmark comparing Vec with OS mmap page-backed buffers for large media transfers.
//!
//! Measures throughput, peak RSS delta, RSS retained after drop, and minor page faults
//! across allocation sizes from 256 KiB to 512 MiB.
//!
//! Run with:
//!     cargo run --example media_buffer_benchmark

#![allow(clippy::print_stdout)]

use wacore::time::Instant;

unsafe extern "C" {
    fn mmap(
        addr: *mut std::ffi::c_void,
        len: usize,
        prot: i32,
        flags: i32,
        fd: i32,
        offset: i64,
    ) -> *mut std::ffi::c_void;
    fn munmap(addr: *mut std::ffi::c_void, len: usize) -> i32;
    fn sysconf(name: i32) -> i64;
    fn getrusage(who: i32, usage: *mut RUsage) -> i32;
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
struct TimeVal {
    tv_sec: i64,
    tv_usec: i64,
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
struct RUsage {
    ru_utime: TimeVal,
    ru_stime: TimeVal,
    ru_maxrss: i64,
    ru_ixrss: i64,
    ru_idrss: i64,
    ru_isrss: i64,
    ru_minflt: i64,
    ru_majflt: i64,
    ru_nswap: i64,
    ru_inblock: i64,
    ru_oublock: i64,
    ru_msgsnd: i64,
    ru_msgrcv: i64,
    ru_nsignals: i64,
    ru_nvcsw: i64,
    ru_nivcsw: i64,
}

fn get_minflt() -> i64 {
    let mut ru = RUsage::default();
    unsafe {
        getrusage(0, &mut ru);
    }
    ru.ru_minflt
}

const PROT_READ: i32 = 1;
const PROT_WRITE: i32 = 2;
const MAP_PRIVATE: i32 = 2;
const MAP_ANONYMOUS: i32 = 0x20;
const MAP_FAILED: *mut std::ffi::c_void = !0 as *mut std::ffi::c_void;

fn get_rss_kb() -> usize {
    let statm = std::fs::read_to_string("/proc/self/statm").unwrap();
    let parts: Vec<&str> = statm.split_whitespace().collect();
    let resident_pages: usize = parts[1].parse().unwrap();
    let page_size_kb = unsafe { sysconf(30) as usize } / 1024;
    resident_pages * page_size_kb
}

fn run_benchmark(size: usize) {
    let label = if size >= 1024 * 1024 * 1024 {
        format!("{} GiB", size / (1024 * 1024 * 1024))
    } else if size >= 1024 * 1024 {
        format!("{} MiB", size / (1024 * 1024))
    } else {
        format!("{} KiB", size / 1024)
    };
    println!("\n--- Testing size: {} ({} bytes) ---", label, size);

    // Vec benchmark (simulating streaming chunked writes into Cursor<Vec<u8>>)
    {
        let base_rss = get_rss_kb();
        let base_flt = get_minflt();
        let t0 = Instant::now();

        let mut cur = std::io::Cursor::new(Vec::with_capacity(size));
        let chunk = [0x55u8; 8192];
        let mut written = 0;
        use std::io::Write;
        while written < size {
            let to_write = (size - written).min(chunk.len());
            cur.write_all(&chunk[..to_write]).unwrap();
            written += to_write;
        }
        let elapsed = t0.elapsed();
        let peak_rss = get_rss_kb();
        let flt = get_minflt() - base_flt;

        drop(cur);
        std::thread::sleep(std::time::Duration::from_millis(100));
        let after_drop_rss = get_rss_kb();

        let throughput = (size as f64 / 1_048_576.0) / elapsed.as_secs_f64();
        println!(
            "Vec:  time: {:>9.2?}, throughput: {:>7.1} MiB/s, peak delta: {:>8} KiB, after drop delta: {:>6} KiB, minor faults: {:>7}",
            elapsed,
            throughput,
            peak_rss.saturating_sub(base_rss),
            after_drop_rss.saturating_sub(base_rss),
            flt
        );
    }

    // Mmap benchmark (simulating chunked writes into anonymous mmap)
    {
        let base_rss = get_rss_kb();
        let base_flt = get_minflt();
        let t0 = Instant::now();

        let page_size = 4096;
        let mapped_len = (size + page_size - 1) & !(page_size - 1);
        let ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                mapped_len,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        assert_ne!(ptr, MAP_FAILED);

        let chunk = [0x55u8; 8192];
        let mut written = 0;
        while written < size {
            let to_write = (size - written).min(chunk.len());
            unsafe {
                std::ptr::copy_nonoverlapping(
                    chunk.as_ptr(),
                    (ptr as *mut u8).add(written),
                    to_write,
                );
            }
            written += to_write;
        }
        let elapsed = t0.elapsed();
        let peak_rss = get_rss_kb();
        let flt = get_minflt() - base_flt;

        unsafe {
            munmap(ptr, mapped_len);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        let after_drop_rss = get_rss_kb();

        let throughput = (size as f64 / 1_048_576.0) / elapsed.as_secs_f64();
        println!(
            "Mmap: time: {:>9.2?}, throughput: {:>7.1} MiB/s, peak delta: {:>8} KiB, after drop delta: {:>6} KiB, minor faults: {:>7}",
            elapsed,
            throughput,
            peak_rss.saturating_sub(base_rss),
            after_drop_rss.saturating_sub(base_rss),
            flt
        );
    }
}

fn main() {
    println!("Base process RSS: {} KiB", get_rss_kb());
    for &sz in &[
        256 * 1024,
        1024 * 1024,
        8 * 1024 * 1024,
        32 * 1024 * 1024,
        128 * 1024 * 1024,
        256 * 1024 * 1024,
        512 * 1024 * 1024,
    ] {
        run_benchmark(sz);
    }
}
