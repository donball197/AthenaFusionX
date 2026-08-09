import os

print(">> Applying Phase 2 (Item #4) Remediation Fix...")

path = "crates/nexus_substrate/src/kernel/safe_vmo.rs"
if os.path.exists(path):
    with open(path, "r") as f:
        content = f.read()

    old_func = """    pub fn write(&self, offset: usize, data: &[u8]) -> Result<(), KernelError> {
        let end = offset.checked_add(data.len()).ok_or(KernelError::InvalidArgs)?;"""

    new_func = """    pub fn write(&self, offset: usize, data: &[u8]) -> Result<(), KernelError> {
        if self.size == 0 {
            return Err(KernelError::InvalidArgs);
        }
        let end = offset.checked_add(data.len()).ok_or(KernelError::InvalidArgs)?;"""

    if old_func in content:
        content = content.replace(old_func, new_func)
        with open(path, "w") as f:
            f.write(content)
        print(f"[OK] Patched {path} - VMO Handle Validation Gap closed.")
    else:
        print(f"[!] Could not find target signature in {path}. It may already be patched.")
else:
    print(f"[ERROR] File not found: {path}")
