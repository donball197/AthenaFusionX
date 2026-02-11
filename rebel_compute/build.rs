fn main() {
    // 1. Tell Cargo where the Zig forge output is
    println!("cargo:rustc-link-search=native=./zig-out/lib");
    
    // 2. Link the static library (lib + .a are added by the compiler)
    println!("cargo:rustc-link-lib=static=rebel_kernels");
    
    // 3. Re-run if our Zig artifact changes
    println!("cargo:rerun-if-changed=zig-out/lib/librebel_kernels.a");
}
