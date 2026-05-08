import os, re

# 1. Force the root workspace to disable default features (MySQL/Postgres)
with open("Cargo.toml", "r") as f:
    data = f.read()
data = re.sub(r'sqlx\s*=.*', 'sqlx = { version = "0.8", default-features = false, features = ["sqlite", "runtime-tokio", "tls-rustls", "macros", "migrate"] }', data)
with open("Cargo.toml", "w") as f:
    f.write(data)

# 2. Scrub all child crates to STRICTLY inherit the workspace definition
for root, dirs, files in os.walk("."):
    if "Cargo.toml" in files and root != ".":
        path = os.path.join(root, "Cargo.toml")
        with open(path, "r") as f:
            cdata = f.read()
        cdata = re.sub(r'^sqlx\s*=.*', 'sqlx = { workspace = true }', cdata, flags=re.MULTILINE)
        with open(path, "w") as f:
            f.write(cdata)

print("\x1b[32m    ✔ Workspace sqlx dependencies strictly aligned.\x1b[0m")
