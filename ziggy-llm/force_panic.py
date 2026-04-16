import os

path = "src/gguf.zig"
with open(path, "r") as f:
    code = f.read()

print("[*] Wiring Panic Traps into the Substrate...")

# Map all silent errors to loud panics
code = code.replace(
    "catch return error.InvalidMetadataType;",
    "catch @panic(\"FRACTURE_1: Unknown ValueType Enum in GGUF Header\");"
)
code = code.replace(
    "if (value_type != .string) return error.InvalidMetadataType;",
    "if (value_type != .string) @panic(\"FRACTURE_2: Key expected a String, got something else\");"
)
code = code.replace(
    "else => error.InvalidMetadataType,",
    "else => @panic(\"FRACTURE_3: Expected an Unsigned Integer\"),"
)
code = code.replace(
    "if (value_type != .bool) return error.InvalidMetadataType;",
    "if (value_type != .bool) @panic(\"FRACTURE_4: Expected a Boolean\");"
)
code = code.replace(
    "if (value_type != .array) return error.InvalidMetadataType;",
    "if (value_type != .array) @panic(\"FRACTURE_5: Expected an Array\");"
)
code = code.replace(
    "if (element_type != .string) return error.InvalidMetadataType;",
    "if (element_type != .string) @panic(\"FRACTURE_6: Array Element is not a String\");"
)
code = code.replace(
    "if (element_type == .array) return error.InvalidMetadataType;",
    "if (element_type == .array) @panic(\"FRACTURE_7: Nested Arrays Unsupported\");"
)

with open(path, "w") as f:
    f.write(code)

print("[+] Panic Traps armed. Recompiling the engine...")
