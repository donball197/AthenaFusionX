path = "crates/nexus_edge_minion/src/inference_engine.rs"
with open(path) as f:
    src = f.read()

changes = 0

old1 = '''        let get_mmap_bf16 = |name: &str| -> &[bf16] {
            if let Ok(ts) = self.loader.get_tensor(name) {
                if let Ok(tt) = ts.as_bf16() {
                    let ptr = tt.data.as_ptr();
                    let len = tt.data.len();
                    return unsafe { std::slice::from_raw_parts(ptr, len) };
                }
            }
            &[]
        };'''

new1 = '''        let total_bytes_touched = std::cell::Cell::new(0usize);
        let tensor_fetch_count = std::cell::Cell::new(0usize);

        let get_mmap_bf16 = |name: &str| -> &[bf16] {
            if let Ok(ts) = self.loader.get_tensor(name) {
                if let Ok(tt) = ts.as_bf16() {
                    let ptr = tt.data.as_ptr();
                    let len = tt.data.len();
                    total_bytes_touched.set(total_bytes_touched.get() + len * 2); // bf16 = 2 bytes
                    tensor_fetch_count.set(tensor_fetch_count.get() + 1);
                    return unsafe { std::slice::from_raw_parts(ptr, len) };
                }
            }
            eprintln!("[dbg][WARN] tensor fetch returned empty slice for: {}", name);
            &[]
        };'''

assert old1 in src, "Change 1 anchor not found -- aborting, no changes written"
src = src.replace(old1, new1, 1)
changes += 1

old2 = '''        eprintln!("[dbg] hidden_state post-layers[0..4]={:?}", &hidden_state[0..4]);'''

new2 = '''        eprintln!("[dbg] hidden_state post-layers[0..4]={:?}", &hidden_state[0..4]);
        eprintln!(
            "[dbg] weight bytes touched across all layers: {} bytes ({:.2} MB) across {} tensor fetches",
            total_bytes_touched.get(),
            total_bytes_touched.get() as f64 / (1024.0 * 1024.0),
            tensor_fetch_count.get()
        );'''

assert old2 in src, "Change 2 anchor not found -- aborting, no changes written"
src = src.replace(old2, new2, 1)
changes += 1

with open(path, "w") as f:
    f.write(src)

print(f"Applied {changes} changes OK.")
