fn main() {
    // Tells Cargo to link the muscle static library built by nexus_substrate
    println!("cargo:rustc-link-lib=static=muscle");
}
