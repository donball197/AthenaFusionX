import os
import re

target = "src/llama_cpu.zig"
if not os.path.exists(target):
    print(f"Error: {target} not found")
    exit(1)

with open(target, "r") as f:
    code = f.read()

print("[*] Scrubbing broken syntax and injecting telemetry...")

# 1. Repair broken 'else' logic from previous sed attempt
code = code.replace('@panic("FRACTURE_LLAMA_ELSE"),', 'else => return error.InvalidMetadataType,')
code = code.replace('@panic("FRACTURE_LLAMA_RETURN");', 'return error.InvalidMetadataType;')

# 2. Inject Telemetry into the intToEnum calls
# This looks for the raw_value_type and prints it if conversion fails
pattern = r"std\.meta\.intToEnum\(ValueType, (\w+)\) catch return error\.InvalidMetadataType;"
replacement = r'std.meta.intToEnum(ValueType, \1) catch |err| { std.debug.print("\n\n[!!!] GGUF PARSER FAILURE\n[!] Unknown Metadata Type ID: {d}\n\n", .{\1}); return err; };'
code = re.sub(pattern, replacement, code)

# 3. Add telemetry to generic metadata type checks
code = code.replace(
    "if (value_type != .string) return error.InvalidMetadataType;",
    'if (value_type != .string) { std.debug.print("\\n[!] Expected String, but got Type: {s}\\n", .{@tagName(value_type)}); return error.InvalidMetadataType; }'
)

with open(target, "w") as f:
    f.write(code)

print("[+] src/llama_cpu.zig is now wired for debugging.")
