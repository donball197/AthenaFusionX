import os

exec_path = "crates/runtime_core/src/executor.rs"
with open(exec_path, "r") as f:
    data = f.read()

# Add the missing ampersands (&) to borrow the Strings as &str
data = data.replace(
    "crate::ast_editor::AstEditor::read_function(path, function_name)?",
    "crate::ast_editor::AstEditor::read_function(&path, &function_name)?"
)

with open(exec_path, "w") as f:
    f.write(data)

print("[✓] Fixed missing string references in ToolExecutor.")
