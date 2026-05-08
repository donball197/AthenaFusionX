import os

path = "crates/runtime_core/src/cli_utils.rs"
with open(path, "r") as f:
    data = f.read()

missing_tools = """- "scrape_url"     { "url": "...", "reasoning": "..." }
- "search_memory"  { "query": "...", "reasoning": "..." }
- "export_report"  { "title": "...", "content": "...", "reasoning": "..." }"""

target_anchor = "- \"halt\"           { \"reasoning\": \"...\" }"

if target_anchor in data and "scrape_url" not in data:
    new_data = data.replace(target_anchor, missing_tools + "\n" + target_anchor)
    with open(path, "w") as f:
        f.write(new_data)
    print("\x1b[32m    ✔ cli_utils.rs patched. Cortex is now aware of ScrapeUrl and SearchMemory.\x1b[0m")
else:
    print("\x1b[31m    [!] Could not patch or already patched.\x1b[0m")
