import os

print("[*] Resolving Exhaustive Switch Compliance for BF16...")

# 1. Patch the Universal Substrate (llama_cpu.zig)
path_cpu = "src/llama_cpu.zig"
with open(path_cpu, "r") as f:
    code_cpu = f.read()

code_cpu = code_cpu.replace(
    ".f16 => .f16_reference,",
    ".f16 => .f16_reference,\n                .bf16 => .f32_reference,"
)
code_cpu = code_cpu.replace(
    ".q6_k => try dotQ6KRow(row, row_len, input),",
    ".q6_k => try dotQ6KRow(row, row_len, input),\n        .bf16 => unreachable,"
)
code_cpu = code_cpu.replace(
    ".q6_k => dotQ6KRow(row, row_len, input) catch unreachable,",
    ".q6_k => dotQ6KRow(row, row_len, input) catch unreachable,\n        .bf16 => unreachable,"
)

with open(path_cpu, "w") as f:
    f.write(code_cpu)

# 2. Patch the Fixture Generator (llama_fixture.zig)
path_fix = "src/runtime/llama_fixture.zig"
with open(path_fix, "r") as f:
    code_fix = f.read()

code_fix = code_fix.replace(
    ".q6_k => try writeTensorDataQ6K(list, values, row_len),",
    ".q6_k => try writeTensorDataQ6K(list, values, row_len),\n        .bf16 => unreachable,"
)

with open(path_fix, "w") as f:
    f.write(code_fix)

print("[+] Switch statements fully resolved. Engine is compliant.")
