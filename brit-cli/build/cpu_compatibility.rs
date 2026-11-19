//! The distributable executable must start on x86-64-v2. Function-local runtime
//! dispatch remains available; globally enabled features become unconditional.
pub fn check() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build/cpu_compatibility.rs");
    println!("cargo:rerun-if-changed=build/cpu_baseline.c");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_FEATURE");
    if std::env::var("CARGO_CFG_TARGET_ARCH")? != "x86_64" {
        return Ok(());
    }
    // ISA membership, not a list of today's known newer CPUs: new features fail
    // closed until classified. crt-static changes linkage, not instructions.
    const BASELINE: &[&str] = &[
        "fxsr",
        "sse",
        "sse2",
        "sse3",
        "ssse3",
        "sse4.1",
        "sse4.2",
        "popcnt",
        "cmpxchg16b",
        "lahfsahf",
        "x87",
        "crt-static",
    ];
    let features = std::env::var("CARGO_CFG_TARGET_FEATURE")?;
    let unsupported: Vec<_> = features
        .split(',')
        .filter(|feature| !feature.is_empty() && !BASELINE.contains(feature))
        .collect();
    if !unsupported.is_empty() {
        return Err(format!(
            "Brit CPU baseline is x86-64-v2; globally enabled features exceed it: {}. \
             Remove target-cpu=native / global target-feature overrides. Use guarded library runtime dispatch.",
            unsupported.join(", ")
        )
        .into());
    }
    if std::env::var("CARGO_CFG_TARGET_OS")? == "linux" {
        // Rust's target does not constrain C dependencies (including TLS).
        // Probe the command line cc gives them: compiler, cc's default flags
        // and CFLAGS/target CFLAGS, during check/Clippy as well as full builds.
        // Since cc 1.11 the defaults pin `-march=x86-64` for an x86_64 Linux
        // target unless rustc's target-cpu names another CPU, so a newer
        // compiler default no longer leaks in. Every C dependency of this
        // binary compiles through cc or cmake-rs, which takes cc's defaults.
        cc::Build::new()
            .file("build/cpu_baseline.c")
            .try_expand()
            .map_err(|error| {
                format!(
                    "Brit CPU baseline C compiler check failed: {error}. \
                 CFLAGS, CC or a -C target-cpu without a matching -march raised the C baseline; \
                 use CFLAGS=-march=x86-64-v2 with -C target-cpu=x86-64-v2."
                )
            })?;
    }
    Ok(())
}
