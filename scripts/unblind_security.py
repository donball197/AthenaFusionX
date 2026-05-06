import os

fpath = "src/security.rs"
with open(fpath, "r") as f:
    code = f.read()

# Replace the silent error with a tracing log
old_err = "Err(StatusCode::UNAUTHORIZED)"
new_err = """{
        tracing::warn!("[SECURITY] 401 Unauthorized: Path={} | HeaderPresent={}", req.uri().path(), !auth_header.is_empty());
        Err(StatusCode::UNAUTHORIZED)
    }"""

if "tracing::warn!" not in code:
    code = code.replace(old_err, new_err)
    with open(fpath, "w") as f:
        f.write(code)
    print(">> [SUTURE] Backend Security Guard unblinded.")
else:
    print(">> [INFO] Backend already unblinded.")
