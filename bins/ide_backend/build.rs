fn main() {
    // Cargo automatically provides the library link information
    // because of the 'links = "muscle"' key in nexus_substrate.
    // We only need to tell the linker to link it.
    println!("cargo:rustc-link-lib=static=muscle");
}
