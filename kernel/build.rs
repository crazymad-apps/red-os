use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let ld_script = PathBuf::from(&manifest_dir).join("kernel.ld");

    println!("cargo:rustc-link-arg=-T{}", ld_script.display());
    println!("cargo:rustc-link-arg=-no-pie");
    println!("cargo:rerun-if-changed=kernel.ld");
    println!("cargo:rerun-if-changed=src/arch/aarch64/boot.S");
}
