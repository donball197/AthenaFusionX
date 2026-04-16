import re

path = "src/llama_cpu.zig"
print("[*] Assimilating Gemma 4 Array Structures...")

with open(path, "r") as f:
    data = f.read()

# The recursive array bypass
new_branch = """        .array => {
            const raw_arr_type = try parser.readInt(u32);
            const arr_type = std.meta.intToEnum(ValueType, raw_arr_type) catch return error.InvalidMetadataType;
            const arr_len = try parser.readInt(u64);
            if (arr_len == 0) return 0;
            
            // Extract the first value to use as the baseline
            const first_val = try readExpectedUnsigned(T, parser, arr_type);
            
            // Loop and consume the remaining array elements to maintain byte alignment
            var i: u64 = 1;
            while (i < arr_len) : (i += 1) {
                _ = try readExpectedUnsigned(T, parser, arr_type);
            }
            return first_val;
        },
        else => return error.InvalidMetadataType,"""

# Target the exact Stethoscope panic we left behind
pattern = re.compile(r'else => \{ std\.debug\.print.*?"UNKNOWN_METADATA_TYPE"\); \},', re.DOTALL)

if pattern.search(data):
    data = pattern.sub(new_branch, data)
    with open(path, "w") as f:
        f.write(data)
    print("[+] Array assimilation logic injected.")
else:
    print("[!] Stethoscope panic not found. Check formatting.")
