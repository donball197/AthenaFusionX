import os

print(">> Giving Athena Internet Access...")

path = "crates/runtime_core/src/policy.rs"
if os.path.exists(path):
    with open(path, "r") as f:
        content = f.read()

    new_tool = """    ToolContract {
        name: "nexus_scrape",
        binary_path: "target/release/nexus_scrape",
        flags: &["url", "--extract"],
        usage_pattern: "nexus_scrape <URL> --extract \\"<prompt>\\" (Scrapes a website and uses an LLM to extract specific data as instructed by the prompt. Always outputs to stdout)",
    },
    ToolContract {"""

    if 'name: "nexus_scrape"' not in content:
        content = content.replace('    ToolContract {', new_tool, 1)
        with open(path, "w") as f:
            f.write(content)
        print("[OK] nexus_scrape added to Official Tool Manifest.")
    else:
        print("[!] nexus_scrape is already in the manifest.")
else:
    print(f"[ERROR] File not found: {path}")
