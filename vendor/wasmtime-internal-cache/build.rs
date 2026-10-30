fn main() {
    // This vendored registry crate must not inherit the enclosing repository identity.
    println!("cargo:rustc-env=COMPILER_VERSION={}", env!("CARGO_PKG_VERSION"));
    println!("cargo:rustc-env=USE_MTIME=false");
}
