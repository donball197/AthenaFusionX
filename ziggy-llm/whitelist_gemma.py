import os

path = "src/llama_cpu.zig"
print("[*] Authorizing Gemma architecture in kernel...")

with open(path, "r") as f:
    code = f.read()

# 1. Authorize the architecture
code = code.replace(
    'if (!std.mem.eql(u8, architecture, "llama") and !std.mem.startsWith(u8, architecture, "qwen")) return error.UnsupportedArchitecture;',
    'if (!std.mem.eql(u8, architecture, "llama") and !std.mem.startsWith(u8, architecture, "qwen") and !std.mem.startsWith(u8, architecture, "gemma")) return error.UnsupportedArchitecture;'
)

# 2. Authorize the tokenizer
code = code.replace(
    'std.mem.startsWith(u8, tokenizer_model, "qwen");',
    'std.mem.startsWith(u8, tokenizer_model, "qwen") or std.mem.startsWith(u8, tokenizer_model, "gemma");'
)

with open(path, "w") as f:
    f.write(code)

print("[+] Gemma 4 officially authorized.")
