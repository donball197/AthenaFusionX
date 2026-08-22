import os

print("Applying patches...")

# 1. Fix client.rs Markdown token stripping
client_path = "apps/nexus_ast_tool/src/client.rs"
with open(client_path, "r") as f:
    client_code = f.read()

client_code = client_code.replace(r'.replace("```python\\n", "")', r'.replace("```python\n", "")')
client_code = client_code.replace(r'.replace("```php\\n", "")', r'.replace("```php\n", "")')
client_code = client_code.replace(
    r'.replace("```", "")',
    r'.replace("```python", "").replace("```php", "").replace("```\n", "").replace("```", "")'
)

with open(client_path, "w") as f:
    f.write(client_code)
print("✅ client.rs patched successfully")

# 2. Fix polyglot.rs snippet size mismatch (duplicated context bug)
polyglot_path = "apps/nexus_ast_tool/src/polyglot.rs"
with open(polyglot_path, "r") as f:
    poly_code = f.read()

poly_code = poly_code.replace("start_line: start_line + 1", "start_line: context_start + 1")
poly_code = poly_code.replace("end_line: end_line + 1", "end_line: context_end + 1")
poly_code = poly_code.replace("source_snippet: snippet,", "source_snippet: snippet.clone(),")
poly_code = poly_code.replace("patch_snippet: patch_target,", "patch_snippet: snippet,")

with open(polyglot_path, "w") as f:
    f.write(poly_code)
print("✅ polyglot.rs patched successfully")

print("\n🚀 Ready! Rebuild and run with:")
print("cargo build --bin nexus_ast_tool")
print("cargo run --bin nexus_ast_tool -- audit --repo https://github.com/appsecco/sqlinjection-training-app.git --ai-triage --auto-heal --generate-report")
