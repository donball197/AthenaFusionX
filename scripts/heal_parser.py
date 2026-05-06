import os

filepath = "src/json_parser.rs"
with open(filepath, "r") as f:
    lines = f.readlines()

with open(filepath, "w") as f:
    for line in lines:
        if 'let clean = candidate.replace("```json", "").replace("```", "").trim();' in line:
            indent = line.split('let')[0]
            f.write(indent + 'let cleaned_string = candidate.replace("```json", "").replace("```", "");\n')
            f.write(indent + 'let clean = cleaned_string.trim();\n')
        else:
            f.write(line)

print(">> [SUTURE] json_parser.rs borrow checker fault resolved.")
