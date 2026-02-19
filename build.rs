fn main() {
    // Tell Cargo where the Zig-compiled static library is
    println!("cargo:rustc-link-search=native=lib");

    // Link the 'resonator' library (libresonator.a)
    println!("cargo:rustc-link-lib=static=resonator");

    // Rebuild if the Zig muscle changes
    println!("cargo:rerun-if-changed=lib/libresonator.a");
}
