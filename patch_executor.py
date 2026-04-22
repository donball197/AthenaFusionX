import os

filepath = "src/executor.rs"
with open(filepath, "r") as f:
    content = f.read()

# Normalize syntax variations
content = content.replace('".into());', '".to_string());')

# Patch the blindspot to feed the LLM the Base64 payload
content = content.replace(
    'results.push(">> [FORGE] Dispatching KERNEL_GENESIS payload to UI.".to_string());',
    'results.push(format!("GENESIS_PAYLOAD: {}", b64));'
)
content = content.replace(
    'results.push(format!(">> [FORGE] Dispatching KERNEL_GENESIS payload to UI."));',
    'results.push(format!("GENESIS_PAYLOAD: {}", b64));'
)

with open(filepath, "w") as f:
    f.write(content)

print(">> [PATCH] executor.rs cognitive blindspot healed.")
