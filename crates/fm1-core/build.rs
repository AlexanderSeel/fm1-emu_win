// `native_jit`: the hosts the block compiler emits native code for (src/jit.rs): AArch64 on
// macOS and Linux, x86-64 on Linux.
fn main() {
    println!("cargo::rustc-check-cfg=cfg(native_jit)");
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if matches!((arch.as_str(), os.as_str()), ("aarch64", "macos" | "linux") | ("x86_64", "linux")) {
        println!("cargo::rustc-cfg=native_jit");
    }
}
