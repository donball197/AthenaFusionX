path = "crates/nexus_edge_minion/src/lib.rs"
with open(path) as f:
    src = f.read()

old = "pub mod safetensors;\npub mod sandbox;"
new = "pub mod safetensors;\npub mod sandbox;\npub mod tokenizer;"

assert old in src, "lib.rs anchor not found -- aborting"
src = src.replace(old, new, 1)
with open(path, "w") as f:
    f.write(src)
print("lib.rs patched OK.")
