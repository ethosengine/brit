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
