use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let lib_dir = manifest_dir.join("../../lib").canonicalize().unwrap_or_else(|_| manifest_dir.join("../../lib"));
    let so_path = lib_dir.join("libnexus_loader.so");

    // Pass the absolute library file path directly to the linker. 
    // This overrides --as-needed stripping and search path mismatches.
    println!("cargo:rustc-link-arg={}", so_path.display());

    // Bake rpath for runtime resolution
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!("cargo:rerun-if-changed={}", so_path.display());
}
