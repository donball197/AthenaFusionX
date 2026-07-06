fn main() {
    // ⚡ Tell Cargo specifically to link openblas for turbovec matrix math
    println!("cargo:rustc-link-search=native=/usr/lib/aarch64-linux-gnu/openblas-pthread");
    println!("cargo:rustc-link-search=native=/usr/lib/aarch64-linux-gnu");
    println!("cargo:rustc-link-lib=dylib=openblas");
}
