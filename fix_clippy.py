import sys

# 1. Fix prediction_engine.rs
path1 = "crates/runtime_core/src/cognition/prediction_engine.rs"
with open(path1, "r") as f:
    content1 = f.read()

content1 = content1.replace("p.split('.').last()", "p.split('.').next_back()")
content1 = content1.replace(".or_insert(ActionHistory::default())", ".or_default()")

with open(path1, "w") as f:
    f.write(content1)

# 2. Fix action_intercept.rs
path2 = "crates/runtime_core/src/action_intercept.rs"
with open(path2, "r") as f:
    content2 = f.read()

old_assert = """        assert!(matches!(
            intercept_fallback_action("write_file foo.rs bar"),
            None
        ));"""
new_assert = """        assert!(intercept_fallback_action("write_file foo.rs bar").is_none());"""
content2 = content2.replace(old_assert, new_assert)

with open(path2, "w") as f:
    f.write(content2)

print("[OK] Clippy lints resolved.")
