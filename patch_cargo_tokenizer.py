path = "crates/nexus_edge_minion/Cargo.toml"
with open(path) as f:
    src = f.read()

old = '''memmap2 = { version = "0.9", optional = true }
half = "2.3"
ureq = { version = "2", features = ["json"] }
dotenvy.workspace = true'''

new = '''memmap2 = { version = "0.9", optional = true }
half = "2.3"
ureq = { version = "2", features = ["json"] }
dotenvy.workspace = true
tokenizers = "0.20"'''

assert old in src, "Cargo.toml anchor not found -- aborting"
src = src.replace(old, new, 1)
with open(path, "w") as f:
    f.write(src)
print("Cargo.toml patched OK.")
