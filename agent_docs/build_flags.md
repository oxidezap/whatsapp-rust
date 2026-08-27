# Build Flags

Codegen flags this library does **not** set for you, what each is worth on its
hot paths, and the CPU floor each one raises. Nothing here changes a default:
`whatsapp-rust` is published to crates.io and cannot know what it will run on,
and every flag below that pays is one that makes the binary die on a CPU that
lacks the feature.

The decision this doc exists to serve: an application that controls its own
deployment target can buy roughly **a fifth of the per-message instruction
count** on the Signal paths for two words in its `.cargo/config.toml`. A library
cannot buy it on the application's behalf.

## Recommendation

For an application whose deployment hardware is known to report both `bmi2` and
`avx2` as *usable* — which means an OS-filtered feature report, not raw CPUID
(see below):

```toml
# <consumer app>/.cargo/config.toml
[target.x86_64-unknown-linux-gnu]
# APPEND to whatever this key already holds — the assignment replaces the
# array, it does not merge with another `[target.*]` block or with
# `build.rustflags`.
rustflags = ["-Ctarget-feature=+bmi2,+avx2"]
```

Cargo picks exactly one rustflags source, highest first:
`CARGO_ENCODED_RUSTFLAGS`, `RUSTFLAGS`, `target.<triple>.rustflags`,
`build.rustflags`. They do not combine. So an invocation that sets `RUSTFLAGS`
for any reason silently discards the config entry above, and the feature flags
have to be merged into that variable instead.

**Check the features, not the CPU's age or its brand.** Neither "2013 or newer"
nor a product family is a sound test. By microarchitecture, Intel Silvermont
(2013) and Goldmont (2016) and AMD Jaguar (2013) and Puma (2014) all lack both
features — Goldmont and Puma while being outright later than Haswell, Silvermont
and Jaguar while being its contemporaries — and those cores ship under Atom,
Celeron and Pentium names alongside parts built on entirely different cores that
do have both (Pentium Gold 8505 is Alder Lake, and has AVX2), so the marketing
name settles nothing either way. The dates in the floor column below mark when
the mainstream core gained the feature, not when every part did.

**And the CPU bit alone is not enough for `+avx2`.** AVX2 needs the OS to have
enabled YMM state; where it has not — some hypervisor configurations — the CPUID
feature bit still reads set and the instruction still faults. A raw CPUID probe
or a fleet inventory built from one has to check `OSXSAVE` and the relevant
`XCR0` bits too. The Linux check below sidesteps this by construction: the
kernel clears `avx2` from `/proc/cpuinfo` when XSAVE state is not enabled, so
what it reports is already OS-filtered.

```bash
# nonzero exit if ANY logical CPU is missing either feature: a union over
# /proc/cpuinfo would pass on a heterogeneous host and still SIGILL the moment
# the process is scheduled onto the odd core. Token-boundary matched, so a
# flags line that ends in the feature name still counts.
check_isa() {
  awk '/^flags/ { if (!/(^| )avx2( |$)/ || !/(^| )bmi2( |$)/) bad++ }
       END { exit bad > 0 }' /proc/cpuinfo && return 0
  echo "some CPU lacks avx2/bmi2" >&2
  return 1
}
check_isa   # usable as a gate: the status survives the diagnostic
```

**The build host needs the features too, not just the deployment host.** A
`[target.x86_64-unknown-linux-gnu]` rustflags entry reaches build scripts and
proc macros whenever cargo is invoked *without* an explicit `--target`, because
that is the case where cargo unifies host and target compilation. Building on a
baseline machine for a newer fleet then traps during the build itself. Either
require both features on the builder, or pass
`--target x86_64-unknown-linux-gnu` explicitly, which splits host from target
and leaves the host tools unflagged.

For `wasm32-unknown-unknown`, add `-Ctarget-feature=+simd128` — provided the
runtimes you deploy to accept it. A module using `v128` fails *validation*
outright on an engine without the proposal, so this is a per-runtime decision,
not a per-CPU one; see [wasm32](#wasm32) for what was and was not checked here.

Do not use `-Ctarget-cpu=native` — see [Rejected](#measured-and-rejected).

## Flag × gain × CPU floor

Instruction counts on this repository's own benches, `wacore` and
`wacore-libsignal`, measured as described in [Method](#method). Percentages are
Ir deltas against a build with no `target-feature` at all.

| flag | key gen | sig create | sig verify ᵃ | group encrypt | DM encrypt | `bench_dm_send` | `bench_group_send_10` | CPU floor (Intel / AMD) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| *(none — today's default)* | — | — | — | — | — | — | — | x86-64 baseline |
| `+bmi2` | **−11.2%** | **−11.1%** | −6.6% | **−11.6%** | **−12.3%** | **−12.5%** | **−11.9%** | Haswell 2013 / Excavator 2015 |
| `+adx,+bmi2` | −11.2% | −11.1% | −6.6% | −11.6% | −12.3% | −12.5% | −11.9% | Broadwell 2014 / Zen 2017 |
| `+avx2` | −13.5% | −12.4% | −1.7% | −11.9% | −8.0% | −7.5% | −8.2% | Haswell 2013 / Excavator 2015 |
| **`+bmi2,+avx2`** | **−24.1%** | **−22.9%** | **−8.2%** | **−23.0%** | **−19.7%** | **−19.4%** | **−19.5%** | Haswell 2013 / Excavator 2015 |
| `-Ctarget-cpu=native` | — | — | — | — | — | — | — | whatever built it — *see [Rejected](#measured-and-rejected)* |

ᵃ `bench_signature_verification` is a **mixed workload**, not verification in
isolation: each measured iteration creates one signature and then verifies ten
times. Signing moves 11–12% under these flags, so roughly a tenth of that
column's operations are on the faster-moving side and its numbers overstate the
verify-only effect by a little. Left as-is rather than corrected — the bench
already exists under that name on CodSpeed, and reshaping it to hoist the
signing into `with_inputs` would break its series for a footnote.

Two results decide the recommendation:

**`+adx` buys nothing here, and it costs a CPU generation.** `+bmi2` and
`+adx,+bmi2` are not merely close, they are the same number: 47,630,320 vs
47,630,153 Ir on `bench_group_send_10`, a difference of 0.0004% — a handful of
instructions out of 47 million, from independent code layout rather than from
`+adx` doing any work. ADX
(`adcx`/`adox`) would pay in a carry-chained multiprecision multiply, and
`FieldElement51` does not have one — its 5×51-bit limbs leave enough headroom
that the u64 backend accumulates into `u128` and never needs an add-with-carry
chain. So the flag that makes LLVM emit `mulx` is `+bmi2`, and it is the whole
of that column. Since ADX's floor is a generation *above* BMI2's on both
vendors, `+adx` is strictly a cost.

**`+bmi2` and `+avx2` are additive because they touch disjoint functions.**
Self cost over 100 `bench_key_generation` iterations:

| function | none | `+bmi2` | `+avx2` | `+bmi2,+avx2` |
| --- | ---: | ---: | ---: | ---: |
| `FieldElement51::mul` | 104,533,000 | **88,924,000** (−14.9%) | 104,533,000 (0%) | 88,924,000 (−14.9%) |
| `FieldElement51::pow2k` | 35,018,000 | **28,808,000** (−17.7%) | 35,018,000 (0%) | 28,808,000 (−17.7%) |
| `LookupTable<AffineNielsPoint>::select` | 36,288,000 | 36,288,000 (0%) | **17,536,000** (−51.7%) | 17,536,000 (−51.7%) |

`+bmi2` replaces the `mul`/`mulq` pairs in the field arithmetic with `mulx`,
which does not clobber flags and frees the scheduler. `+avx2` vectorizes
something else entirely: `select` is the constant-time table scan at the heart
of fixed-base scalar multiplication — eight `conditional_assign`s per window,
64 windows per multiplication, each one touching every table entry precisely so
the access pattern reveals nothing. That is a wide, branch-free, data-parallel
loop, and AVX2 halves it. Neither flag can take the other's function, which is
why the combined figure is close to the sum.

`sig verify` moves least, and its two columns say why. Its scalar
multiplication is the one that escapes both flags: `curve25519-dalek`
runtime-dispatches `vartime_double_base_mul` through a CPUID check and, on any
AVX2-capable host, runs a hand-written vector backend that no `target-feature`
of ours touches (see [Runtime dispatch](#runtime-dispatch-is-not-the-same-thing)).
That is why `+avx2` is worth only −1.7% here against −13.5% on key generation.
It still keeps −6.6% from `+bmi2`, from two sources. Verification does not end
at the scalar multiplication: `verify_signature_prepared`
(`wacore/libsignal/src/core/curve/curve25519.rs`) compresses the resulting
`EdwardsPoint`, and `compress` is a serial field inversion — `FieldElement51`
work, on the flag's side of the line. The rest is the one signature the bench
creates per iteration (footnote ᵃ), which moves with the sig-create column.

## Why these cannot be defaults

`+avx2` and `+bmi2` are compile-time promises, not requests. The compiler emits
the instruction wherever it likes, and a CPU without the feature raises
`SIGILL` on the first one it reaches. There is no fallback path to take,
because there is no check.

The failure mode is worse than a refusal to boot, and worth being precise
about. A load-time check does exist in principle — glibc 2.33+ reads
`GNU_PROPERTY_X86_ISA_1_NEEDED` from `.note.gnu.property` and refuses a binary
whose ISA level the CPU cannot meet — but `-C target-feature` does not emit that
property, so this recipe does not get you one. What you get instead: the process
starts normally, passes its readiness probe, and traps whenever control first
reaches an emitted instruction, potentially the first Signal operation and
potentially much later under traffic. **A successful startup is not evidence of
compatibility**, and there is no runtime fallback to take.

That is fine for an application whose deployment target is known. It is not
fine for a library on crates.io, which is compiled by people whose hardware it
will never see. Setting it in this workspace's own `.cargo/config.toml` would
not help them either — cargo does not propagate that file across package
boundaries, so it would raise the floor for this repo's contributors and CI
while delivering nothing to a single consumer. The recommendation therefore
lives here and in the README, and the flags belong in the *consumer's* config.

### Runtime dispatch is not the same thing

`curve25519-dalek` ships an AVX2 backend that *is* safe by default, and it is
easy to conflate the two. Its `get_selected_backend` (`src/backend.rs`) runs a
`cpufeatures` CPUID probe and falls back to the serial implementation when AVX2
is absent, so on x86-64 the vector backend is already active at runtime with no
flags from us. It covers the multiscalar and variable-base entry points only.

The functions in the table above are the ones dispatch does *not* reach —
`FieldElement51` arithmetic and the fixed-base `LookupTable` — which is exactly
why a compile-time flag still has something to buy, and equally why it has no
fallback to offer when the CPU turns out to be older.

## LTO

Already done, and worth knowing about before someone re-derives it:
`[profile.release]` in the root `Cargo.toml` has carried `lto = "fat"` (with
`codegen-units = 1`) for some time. There is no LTO change to make on the
release path. Note that the per-package `opt-level` overrides documented in
`binary_size_ci.md` are *not* conditional on it — cargo applies a profile
override when it compiles that dependency, whatever the LTO mode — but the
~530 KiB those overrides are credited with was measured under fat LTO, so
changing the LTO mode would invalidate that figure rather than the mechanism.

`[profile.bench]` deliberately overrides it back to `lto = "thin"`. That is not
an oversight: a fat-LTO bench build of `send_receive_benchmark` +
`libsignal_benchmark` takes 3m30s against 3m03s for thin on a 4-core runner
(+15%, both from cold), and the benches exist to be re-run. Unlike a
`target-feature`, LTO has
no CPU floor, so a consumer who wants the release profile's codegen in a bench
build can set `CARGO_PROFILE_BENCH_LTO=fat` for that run without changing what
the binary can run on.

## wasm32

`wasm32-unknown-unknown` does not enable SIMD by default even though `simd128`
has been stable in the spec and in every current runtime for years. The
workspace does not set it: `.github/workflows/wasm.yml` builds with
`RUSTFLAGS: '--cfg getrandom_backend="wasm_js"'` and nothing else, and
`.cargo/config.toml`'s `rustflags` block is scoped to
`[target.x86_64-unknown-linux-gnu]`, so it never applied to wasm in the first
place.

What is verified here is that the flag is safe to recommend: adding
`-Ctarget-feature=+simd128` to that job's flags builds `whatsapp-rust` for
`wasm32-unknown-unknown` clean, so a consumer who sets it does not hit a
compile error in this tree.

What is **not** verified here, and should not be repeated as if it were: any
wasm speed or module-size figure. This workspace declares no `cdylib`, so a
wasm build produces an rlib of pre-link bitcode and never a module — measuring
one would mean adding a build target that exists for nothing else. For the
record the rlib moves by +2,256 bytes of 27.8 MB (+0.008%) with the flag on,
which is a number about rlib metadata and says nothing about a linked module.
There is also no wasm benchmark harness in this repository, so nothing on the
wasm side has been timed at all.

The same reasoning as x86-64 applies to who should set it, with the floor being
a runtime that supports the SIMD proposal rather than a CPU generation. Every
browser and every current standalone runtime does; a consumer targeting one
that might not is the one who knows.

## Measured and rejected

**`-Ctarget-cpu=native`** — do not use it, and do not suggest it as the
convenient shorthand for the above. Two independent reasons:

1. **It cannot be measured, and it breaks Valgrind-based tooling.** On the
   measurement host (Xeon with AVX-512) a `native` build of
   `libsignal_benchmark` dies under callgrind with `Unrecognised instruction
   … Process terminating with default action of signal 4 (SIGILL)`. It runs
   fine on the bare CPU. Anything that profiles under Valgrind — which includes
   CodSpeed, this repo's benchmark CI — cannot run the binary.
2. **It is slower than the explicit list on at least one real host.** Prior
   wall-clock A/B on the same host family recorded `bench_dm_session_establishment`
   at 493 µs stock, 445 µs with `+adx,+bmi2,+avx2`, and **544 µs** with
   `native`; `bench_key_generation` at 182 / 159 / 196 µs. `native` turns on
   AVX-512, and the frequency cost of touching 512-bit units outweighs what the
   field arithmetic gains. An explicit feature list is both faster and
   predictable about the floor it sets.

**Putting the flags in this workspace's `.cargo/config.toml`.** Rejected above:
no consumer benefit, and it would put a one-time step change through every
CodSpeed series while raising the floor for contributor machines.

**`+adx`.** Rejected on the measurement: zero gain over `+bmi2`, one CPU
generation of extra floor.

## Method

Wall-clock is not the instrument for this on ordinary CI or cloud hardware.
Four reps of the full matrix on a 4-vCPU VM produced a rep-to-rep spread of up
to **116%** inside a single arm, which is larger than every effect being looked
for; no wall-clock delta from that run is reported here or should be believed.

Instruction counts are deterministic, they are what a compile-time
`target-feature` actually changes, and they are the same quantity CodSpeed
reports. Per-iteration Ir is obtained by running each bench under callgrind at
two pinned iteration counts and dividing the difference — which cancels process
startup, divan's own setup, and each bench's one-off fixture work exactly,
rather than estimating them:

```bash
# one target dir per config; RUSTFLAGS must repeat .cargo/config.toml's own
# flags, since setting it replaces that target's rustflags wholesale
BASE='-Zshare-generics=y -Zunstable-options -Clinker-features=+lld -Clink-arg=-Wl,--icf=all'
RUSTFLAGS="$BASE -Ctarget-feature=+bmi2,+avx2" CARGO_TARGET_DIR=/tmp/t-bmi2avx2 \
  cargo build --profile bench --bench libsignal_benchmark -p wacore-libsignal

# `deps/` holds the executable *and* its `libsignal_benchmark-<hash>.d`
# dep-info file, so a bare glob passes the .d path to divan as a positional
# filter and silently benchmarks nothing. Select the executable.
BIN=$(find /tmp/t-bmi2avx2/release/deps -name 'libsignal_benchmark-*' -type f -perm -u+x ! -name '*.d')

for size in 20 40; do
  valgrind --tool=callgrind --callgrind-out-file="cg.bmi2avx2.$size" \
    --cache-sim=no --branch-sim=no \
    "$BIN" --bench bench_key_generation \
    --sample-count 5 --sample-size $size --threads 1
done   # (Ir@40 - Ir@20) / ((40-20) * 5) = Ir per iteration

# the per-function table below reads the profiles this kept; a
# --callgrind-out-file=/dev/null run gives you the totals and nothing else
callgrind_annotate cg.bmi2avx2.20 | grep curve25519
```

`--bench` is required: without it divan lists the benchmarks and exits without
running them, and the run looks like a 0.016 s success.

Per-function attribution in the mechanism table comes from
`callgrind_annotate` on the retained `--sample-size 20` profile of each config.

Tool and toolchain versions, since both bound the result: valgrind/callgrind
3.22.0, and `nightly-2026-06-16` as pinned by `rust-toolchain.toml` — the
`BASE` flags above are nightly-only (`-Zshare-generics`, `-Zunstable-options`),
so a build that bypasses or overrides the pinned toolchain fails on stable
rather than producing a comparable number.

### Caveats

- **Ir is a proxy for time, and an imperfect one in both directions.** `mulx`
  also shortens dependency chains, which Ir does not see, so `+bmi2` is
  probably worth more in wall-clock than the −11% here; a vectorized `select`
  may be worth less than its −52% if it is not on the critical path. The
  ordering of the flags is what this measurement establishes, not the exact
  size of the win.
- **CodSpeed will not show any of this.** CI builds without `target-feature`,
  so its baseline is the first row. That is expected, and a flat CodSpeed
  report is not a refutation of the table.
- **`+bmi2` is not universally free on AMD.** Zen 1 and Zen 2 implement
  `pext`/`pdep` in microcode at roughly 18 and 300 cycles. It does not apply
  here — dalek's field arithmetic uses `mulx`, which is fast on those parts —
  but code that generalizes "`+bmi2` is free" to other crates in the binary is
  wrong on those two generations.
- The floors above are the *architectural* ones. A specific target triple can
  already imply more; `x86-64-v3`, for example, includes both BMI2 and AVX2.
