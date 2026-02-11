fn main() {
    // Tell Rust where the .so lives
    println!("cargo:rustc-link-search=native=./lib");
    println!("cargo:rustc-link-lib=dylib=tensorflowlite_c");
    
    // Ensure the runtime knows where to find the library at execution
    println!("cargo:rustc-env=LD_LIBRARY_PATH=./lib");
}
