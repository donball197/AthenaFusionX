//! Autonomous Code Refactoring Micro-Agent
//! Powered by the MutationStrategy trait and robust AST validation.

use crate::mutation::{all_strategies, get_strategy, MutationError, MutationResult};
use std::fs;
use std::path::Path;

pub struct RefactorAgent {
    source_path: std::path::PathBuf,
}

impl RefactorAgent {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            source_path: path.as_ref().to_path_buf(),
        }
    }

    /// Executes a specific mutation strategy on a given symbol within the target file.
    pub fn apply_refactoring(
        &self,
        strategy_name: &str,
        symbol: &str,
        payload: &str,
    ) -> MutationResult<()> {
        // 1. Load source code from disk
        let source_code = fs::read_to_string(&self.source_path)
            .map_err(|e| MutationError::Other(anyhow::anyhow!("Failed to read source file: {e}")))?;

        // 2. Retrieve the requested strategy
        let strategy = get_strategy(strategy_name).ok_or_else(|| {
            MutationError::InvalidPayload(format!("Strategy '{strategy_name}' not found"))
        })?;

        println!(
            "==> [Micro-Agent] Executing strategy '{}' on symbol '{}' for file: {:?}",
            strategy.name(),
            symbol,
            self.source_path
        );

        // 3. Apply the transformation
        let mutated_code = strategy.apply(&source_code, symbol, payload)?;

        // 4. Validate output code integrity by attempting a dry-run syn parse
        syn::parse_file(&mutated_code).map_err(|e| {
            MutationError::InvalidResult(format!("Refactored code failed syntax validation: {e}"))
        })?;

        // 5. Safely write the validated code back to disk
        fs::write(&self.source_path, mutated_code)
            .map_err(|e| MutationError::Other(anyhow::anyhow!("Failed to write refactored file: {e}")))?;

        println!("==> [Micro-Agent] Refactoring successfully applied and verified.");
        Ok(())
    }

    /// Runs an autonomous optimization sweep using all available strategies.
    pub fn autonomous_sweep(&self, symbol: &str, payload: &str) -> usize {
        let strategies = all_strategies();
        let mut successful_mutations = 0;

        for strategy in strategies {
            let name = strategy.name();
            // Skip pure parse fuzzers during standard refactoring sweeps
            if name == "TreeSitterParse" {
                continue;
            }

            match self.apply_refactoring(name, symbol, payload) {
                Ok(_) => {
                    successful_mutations += 1;
                }
                Err(e) => {
                    // Micro-agents handle expected symbol misses gracefully during blind sweeps
                    eprintln!("   [Micro-Agent Notice] Strategy {} skipped/failed: {}", name, e);
                }
            }
        }

        successful_mutations
    }
}
