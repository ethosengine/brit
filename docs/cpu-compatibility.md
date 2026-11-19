# CPU compatibility of Linux deliveries

The x86-64 Linux delivery floor is **x86-64-v2**. Release builds pin both Rust
and C compiler flags; `brit-cli` rejects global Rust features or C compiler
configuration above that floor during build/check/Clippy. Function-local,
runtime-guarded acceleration is permitted. This is an early configuration guard,
not a proof about every instruction emitted by dependencies.

The required `brit-cpu-compatibility` CI job and both binary publication workflows
exercise the exact optimized bytes on native, Nehalem (v2) and Haswell (AVX2)
CPUs, including packed and incremental fetches, object-byte comparison and TLS
certificate refusal. An AVX2 negative control checks that emulation actually
restricts instructions. All four followed executables receive startup probes.
CI retains the tested candidate, source revision, checksums and qualification log;
those results cover these workloads and CPU models, not all possible code paths.

Linux `install-brit-release.sh` also tests TLS initialization on the receiving
host before atomically switching the executable. OpenSSL 3 and GNU `timeout`
are prerequisites; a missing prerequisite, signal, timeout or unexpected response
refuses installation and retains the current binary. This probe is offline and
uses an untrusted loopback certificate. EPR followed-tool adoption consumes the
source-qualified build release; it must not treat a successful `--version` as
CPU qualification. Local observations do not confer network standing or reach.

The first hosted run caught a compiler-default mismatch: Ubuntu's unqualified
`cc -E` enabled instructions above v2 even though Rust had no such global flag.
Since `cc` 1.11 (the lock pins 1.11.1 and `brit-cli` requires it), `cc` passes
`-march=x86-64` for an x86_64 Linux target whose rustc target-cpu is unset or
`x86-64`, so C dependencies compile at the Rust target's baseline whatever the
host compiler defaults to, and an ordinary local build needs no `CFLAGS`. The
probe checks the command line `cc` gives dependencies, its defaults included;
every C dependency of `brit` compiles through `cc` or cmake-rs, which inherits
those defaults. Once `-C target-cpu=x86-64-v2` is set, `cc` passes no `-march`,
so delivery builds pin both Rust and C flags, and CI also declares
`CFLAGS_x86_64_unknown_linux_gnu` for ordinary source tests. The negative
controls turn `cc`'s defaults off or name a target-cpu without C flags, so a
compiler that defaults above the floor is still refused there.

Limits of the probe: it assumes every C dependency compiles through `cc`, or
through cmake-rs with `cc`'s defaults. A future dependency with its own build
system, or one that calls `no_default_flags` (aws-lc-sys already does for FIPS
dynamic builds, a configuration brit does not enable), would compile at the host
compiler's default and pass the probe unseen on a host whose compiler defaults
to x86-64-v3; only the exact-byte CPU qualification above would catch it. The
probe runs in `brit-cli`'s build script only (`brit` and `brit-rebase-retained`);
`brit-build-ref` and `brit-verify` ship without it and rely on the workflows'
Rust and C flags.

Linux binaries are built and CPU-qualified on the explicit Ubuntu 24.04 shipping
environment. The ordinary source matrix continues to cover newer Ubuntu runners.
Before an expensive CPU build, a v2 C positive control must also run under the
restricted model: compiler flags cannot lower the floor of prebuilt runtime
objects or libraries. The emulation preflight refuses an incompatible measurement
runtime without attributing its failure to Brit. Failed CPU diagnostics are kept
for one day under a distinct artifact name; only the successful candidate artifact
is eligible for the delivery exercise.
