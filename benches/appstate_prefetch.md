# App-state prefetch measurements

`appstate_prefetch` uses synthetic encrypted HTTP bodies through production
admission, streaming decryption, authentication and retained blob ownership.
Fixture construction and startup settle outside the measured closure. No live
account, capture or network service is involved.

`four_external_blobs` measures one client without coordinated stream completion.
`eight_clients` runs eight clients on four Tokio workers. Its 32 HTTP bodies
rendezvous at EOF before authentication finishes. Each production writer still
owns its admitted plaintext buffer at that point. This defines a controlled
maximum-overlap workload, rather than letting unrelated task completion choose
how many clients contribute to the peak. No payload copy or returned blob is
retained by the gate. All tasks, verification and cleanup remain measured.
The gate has a bounded wait so a change in admission or a failed stream fails
instead of silently measuring fewer concurrent downloads.

The aligned workload changes the meaning of the existing eight-client series.
Establish a fresh baseline; do not call a difference from the old uncoordinated
series a production regression or improvement. Correctness tests retain both
uncoordinated and aligned concurrent execution and check release of the byte
budget. The gate's scheduling and synchronization overhead is included in CPU
simulation results. Simulation excludes syscall costs and cannot rule out a
real runtime or scheduling change.

Compare repeated CodSpeed runs at the same commit with the same Rust toolchain,
instrument versions, flags, allocator settings and host environment. Inspect
`compare_runs` environment differences before attributing a delta to code.
Report total allocated bytes separately from peak live heap. Also keep RSS
separate: allocator-retained pages and process residency are different metrics.
The gate stabilizes overlap, not allocator or runtime bookkeeping.

CI runs this workload with CPU simulation and memory instrumentation. Walltime
is not uploaded from shared GitHub runners because changes in host hardware and
scheduling make cross-run comparisons unreliable. Native timing investigations
need matched base/head measurements in a controlled environment; do not raise
thresholds to compensate for a changing machine.
