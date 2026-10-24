// crates/doms_vm/build.rs
fn main() {
    let rustflags = std::env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    let asan = rustflags.contains("sanitizer=address") || rustflags.contains("sanitizer\x1faddress");
    if asan {
        println!("cargo:rustc-cfg=dom_asan");
    }
}
