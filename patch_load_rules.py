import sys

path = "apps/nexus_ast_tool/src/polyglot.rs"

old = '''    pub fn load_rules<P: AsRef<Path>>(rules_dir: P) -> Vec<SecurityRule> {
        let mut rules = Vec::new();
        let entries = match std::fs::read_dir(rules_dir) {
            Ok(entries) => entries,
            Err(_) => return rules,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ext == "yaml" || ext == "yml" {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        match serde_yaml::from_str::<SecurityRule>(&content) {
                            Ok(rule) => rules.push(rule),
                            Err(e) => eprintln!("==> [Warning] Failed to parse YAML rule file {}: {}", path.display(), e),
                        }
                    }
                }
            }
        }
        rules
    }'''

new = '''    pub fn load_rules<P: AsRef<Path>>(rules_dir: P) -> Vec<SecurityRule> {
        // WalkDir recurses subdirectories (php/, python/, etc). read_dir did not,
        // which silently dropped every rule not sitting at the top level.
        let mut rules = Vec::new();
        for entry in WalkDir::new(rules_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ext == "yaml" || ext == "yml" {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        match serde_yaml::from_str::<SecurityRule>(&content) {
                            Ok(rule) => rules.push(rule),
                            Err(e) => eprintln!("==> [Warning] Failed to parse YAML rule file {}: {}", path.display(), e),
                        }
                    }
                }
            }
        }
        rules
    }'''

with open(path, "r") as f:
    content = f.read()

count = content.count(old)
if count != 1:
    print(f"ANCHOR MISMATCH: found {count} occurrences, expected 1", file=sys.stderr)
    sys.exit(1)

with open(path, "w") as f:
    f.write(content.replace(old, new))

print("OK: load_rules patched to recurse via WalkDir")
