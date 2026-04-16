import os

path = "src/llama_cpu.zig"
print("[*] Synchronizing Shadow Parser exact return trace...")

with open(path, "r") as f:
    lines = f.readlines()

out_lines = []
in_func = False
for line in lines:
    if "fn readExpectedUnsigned" in line:
        in_func = True
        
    if in_func and "else => return error.InvalidMetadataType" in line:
        out_lines.append("        .int8 => try castUnsigned(T, try parser.readInt(u8)),\n")
        out_lines.append("        .int16 => try castUnsigned(T, try parser.readInt(u16)),\n")
        out_lines.append("        .int32 => try castUnsigned(T, try parser.readInt(u32)),\n")
        out_lines.append("        .int64 => try castUnsigned(T, try parser.readInt(u64)),\n")
        in_func = False
        print("[+] Signed integer pathways injected into readExpectedUnsigned.")
        
    out_lines.append(line)

with open(path, "w") as f:
    f.writelines(out_lines)

print("[*] DNA Sync Complete.")
