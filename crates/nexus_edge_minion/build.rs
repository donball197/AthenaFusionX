use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let lib_dir = manifest_dir
        .join("../../lib")
        .canonicalize()
        .unwrap_or_else(|_| manifest_dir.join("../../lib"));
    let so_path = lib_dir.join("libnexus_loader.so");

    if so_path.exists() {
        // Library present: link by absolute path and bake rpath for runtime.
        println!("cargo:rustc-link-arg={}", so_path.display());
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
        println!("cargo:rerun-if-changed={}", so_path.display());
    } else {
        // Library missing: warn instead of guaranteeing an ld failure.
        println!(
            "cargo:warning=libnexus_loader.so not found at {} -- skipping loader link args",
            so_path.display()
        );
    }
}
