import os

# 1. FFI Safety Doc
ffi_file = "crates/nexus_substrate/src/kernel/ffi.rs"
if os.path.exists(ffi_file):
    with open(ffi_file, "r") as f:
        data = f.read()
    if "# Safety" not in data:
        data = data.replace(
            "    unsafe fn vmo_get_info", 
            "    /// # Safety\n    /// The caller must ensure `out_info` is a valid, properly aligned pointer to a `VmoInfo` struct.\n    unsafe fn vmo_get_info"
        )
        with open(ffi_file, "w") as f:
            f.write(data)
        print("\x1b[32m    ✔ ffi.rs: Injected Safety doc.\x1b[0m")

# 2 & 3. Token Ledger Fixes
tl_file = "crates/nexus_substrate/src/token_ledger.rs"
if os.path.exists(tl_file):
    with open(tl_file, "r") as f:
        data = f.read()
    
    # Fix unnecessary cast
    if "as u64;" in data:
        data = data.replace("let jitter = (generate_seed() % 500) as u64;", "let jitter = generate_seed() % 500;")
        print("\x1b[32m    ✔ token_ledger.rs: Stripped unnecessary u64 cast.\x1b[0m")
    
    # Fix missing Default trait
    if "impl Default for TokenLedger" not in data:
        data += "\nimpl Default for TokenLedger {\n    fn default() -> Self {\n        Self::new()\n    }\n}\n"
        print("\x1b[32m    ✔ token_ledger.rs: Appended Default implementation.\x1b[0m")

    with open(tl_file, "w") as f:
        f.write(data)
