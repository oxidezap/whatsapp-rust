# Media Buffer In-Memory Allocation Benchmark Results

Comparison of standard `Vec<u8>` against anonymous memory-mapped (`mmap`/`munmap`) page-backed buffers for in-memory media transfers across allocation sizes from 256 KiB to 512 MiB.

Run with:
```bash
cargo run --example media_buffer_benchmark
```

## Benchmark Measurements (Linux x86_64, glibc 2.41)

| Buffer Size | Backend | Throughput | Peak RSS Delta | RSS After Drop Delta | Minor Page Faults |
|---|---|---|---|---|---|
| 256 KiB | Vec | 3176.7 MiB/s | +328 KiB | +68 KiB | 67 |
| 256 KiB | Mmap | 2923.2 MiB/s | +256 KiB | +0 KiB | 64 |
| 1 MiB | Vec | 3129.5 MiB/s | +1028 KiB | +0 KiB | 257 |
| 1 MiB | Mmap | 3110.9 MiB/s | +1024 KiB | +0 KiB | 256 |
| 8 MiB | Vec | 3109.9 MiB/s | +8196 KiB | +0 KiB | 2049 |
| 8 MiB | Mmap | 3005.7 MiB/s | +8192 KiB | +0 KiB | 2048 |
| 32 MiB | Vec | 2988.6 MiB/s | +32772 KiB | +0 KiB | 8193 |
| 32 MiB | Mmap | 3048.1 MiB/s | +32768 KiB | +0 KiB | 8192 |
| 128 MiB | Vec | 2983.9 MiB/s | +131076 KiB | +0 KiB | 32769 |
| 128 MiB | Mmap | 3088.5 MiB/s | +131072 KiB | +0 KiB | 32768 |
| 256 MiB | Vec | 2676.8 MiB/s | +262148 KiB | +0 KiB | 65537 |
| 256 MiB | Mmap | 2296.7 MiB/s | +262144 KiB | +0 KiB | 65536 |
| 512 MiB | Vec | 2184.1 MiB/s | +524292 KiB | +0 KiB | 131073 |
| 512 MiB | Mmap | 2337.1 MiB/s | +524288 KiB | +0 KiB | 131072 |

## Numerical Impact Comparison

| Size | Operation | Backend | Virtual Memory (VSZ) | Peak Physical (RSS) | RSS After Drop | Realloc Count | API Interop Zero-Copy |
|---|---|---|---|---|---|---|---|
| 10 MB | Upload | Vec | +10 MB | +10 MB | Baseline (+0 MB) | 0 | Yes (`bytes::Bytes::from`) |
| 10 MB | Upload | Mmap | +10 MB | +10 MB | Baseline (+0 MB) | 0 | No (requires copy) |
| 10 MB | Download | Vec (Current) | +10 MB | +10 MB | Baseline (+0 MB) | 0 | Yes (returns owned `Vec<u8>`) |
| 10 MB | Download | Mmap (Exact) | +10 MB | +10 MB | Baseline (+0 MB) | 0 | Diverges (custom wrapper / slice) |
| 50 MB | Upload | Vec | +50 MB | +50 MB | Baseline (+0 MB) | 0 | Yes (`bytes::Bytes::from`) |
| 50 MB | Upload | Mmap | +50 MB | +50 MB | Baseline (+0 MB) | 0 | No (requires copy) |
| 50 MB | Download | Vec (Current) | +50 MB | +50 MB | Baseline (+0 MB) | 0 | Yes (returns owned `Vec<u8>`) |
| 50 MB | Download | Mmap (Exact) | +50 MB | +50 MB | Baseline (+0 MB) | 0 | Diverges (custom wrapper / slice) |
| 100 MB | Upload | Vec | +100 MB | +100 MB | Baseline (+0 MB) | 0 | Yes (`bytes::Bytes::from`) |
| 100 MB | Upload | Mmap | +100 MB | +100 MB | Baseline (+0 MB) | 0 | No (requires copy) |
| 100 MB | Download | Vec (Current) | +128 MB | +100 MB | Baseline (+0 MB) | 1 | Yes (returns owned `Vec<u8>`) |
| 100 MB | Download | Mmap (Exact) | +100 MB | +100 MB | Baseline (+0 MB) | 0 | Diverges (custom wrapper / slice) |
| 500 MB | Upload | Vec | +500 MB | +500 MB | Baseline (+0 MB) | 0 | Yes (`bytes::Bytes::from`) |
| 500 MB | Upload | Mmap | +500 MB | +500 MB | Baseline (+0 MB) | 0 | No (requires copy) |
| 500 MB | Download | Vec (Current) | +512 MB | +500 MB | Baseline (+0 MB) | 3 | Yes (returns owned `Vec<u8>`) |
| 500 MB | Download | Mmap (Exact) | +500 MB | +500 MB | Baseline (+0 MB) | 0 | Diverges (custom wrapper / slice) |
| 1024 MB | Upload | Vec | +1024 MB | +1024 MB | Baseline (+0 MB) | 0 | Yes (`bytes::Bytes::from`) |
| 1024 MB | Upload | Mmap | +1024 MB | +1024 MB | Baseline (+0 MB) | 0 | No (requires copy) |
| 1024 MB | Download | Vec (Current) | +1024 MB | +1024 MB | Baseline (+0 MB) | 4 | Yes (returns owned `Vec<u8>`) |
| 1024 MB | Download | Mmap (Exact) | +1024 MB | +1024 MB | Baseline (+0 MB) | 0 | Diverges (custom wrapper / slice) |

## Key Findings

1. **Retained Memory & Heap Return:** For buffers >= 1 MiB, glibc allocator already allocates memory directly via `mmap` and immediately returns the pages to the OS via `munmap` upon `Vec::drop`. In the 256 MiB decisive test, dropping the `Vec` returned process RSS from 264.6 MiB back to the 2.5 MiB baseline (+0 KiB retained delta).
2. **Upload Zero-Copy:** The `EncryptedMedia` upload pipeline relies on `bytes::Bytes::from(vec)` for zero-copy slicing across network retries. Using custom mmap buffers breaks this invariant and requires extra buffer copies.
3. **Streaming Alternatives:** The codebase already provides `download_to_writer` and `upload_stream` for handling arbitrarily large transfers directly to files or temporary sinks without loading full payloads into memory.
