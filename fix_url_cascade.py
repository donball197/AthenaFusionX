import os

ee_path = "crates/runtime_core/src/epistemic_engine.rs"
with open(ee_path, "r") as f:
    ee_data = f.read()

old_url = """let url = format!("https://generativelanguage.googleapis.com/v1beta/{}?key={}", model_endpoint, api_key);"""
new_url = """let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model_endpoint, api_key);"""

ee_data = ee_data.replace(old_url, new_url)

with open(ee_path, "w") as f:
    f.write(ee_data)

print("[✓] API URL Formatting fixed in Epistemic Engine.")
