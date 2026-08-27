use crate::agent::RefactorAgent;
use crate::polyglot::PolyglotScanner;
use std::fs;
use std::path::Path;
use std::process::Command;

pub struct Supervisor;

impl Supervisor {
    pub fn self_optimize<P: AsRef<Path>>(target_file: P) -> anyhow::Result<()> {
        let path = target_file.as_ref();
        println!("==> [Supervisor] Initializing Self-Optimization Loop for: {}", path.display());

        // 1. Map all functions in the file
        let symbols = PolyglotScanner::scan_path(path);
        let original_code = fs::read_to_string(path)?;

        let mut success_count = 0;

        for sym in
            symbols.into_iter().filter(|s| s.language == crate::polyglot::SupportedLanguage::Rust)
        {
            println!("\n    [?] Analyzing function '{}'...", sym.name);

            // Extract the function snippet
            let lines: Vec<&str> = original_code.lines().collect();
            let snippet = lines[sym.start_line - 1..=sym.end_line - 1].join("\n");

            // 2. Ask AI to optimize it
            match crate::client::ApiClient::generate_optimization(&sym.name, &snippet) {
                Ok(optimized_body) => {
                    println!("        [🧠] Optimization generated. Splicing into AST...");

                    // Backup the file in memory
                    let current_code = fs::read_to_string(path)?;

                    // 3. Mutate the file
                    let agent = RefactorAgent::new();
                    if agent
                        .apply_refactoring(path, &sym.name, "ReplaceFunctionBody", &optimized_body)
                        .is_ok()
                    {
                        // 4. The Sandbox (Test-Driven Mutation)
                        println!("        [🛠️] Running cargo check to verify integrity...");
                        let status = Command::new("cargo")
                            .args(["check", "--manifest-path", "apps/nexus_ast_tool/Cargo.toml"])
                            .output()?;

                        if status.status.success() {
                            println!("        [✅] SUCCESS: Code is valid. Optimization retained.");
                            success_count += 1;
                        } else {
                            println!("        [❌] REJECTED: Compiler panicked. Reverting to safe state.");
                            fs::write(path, &current_code)?; // Revert!
                        }
                    }
                }
                Err(e) => println!("        [⚠️] AI failed to generate optimization: {}", e),
            }
        }

        println!(
            "\n==> [Supervisor] Optimization complete. {} functions successfully upgraded.",
            success_count
        );
        Ok(())
    }
}
