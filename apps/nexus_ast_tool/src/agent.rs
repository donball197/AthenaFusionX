use crate::client::ApiClient;
use crate::mutation::get_strategy;
use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;
use syn::spanned::Spanned;

pub struct RefactorAgent;

impl RefactorAgent {
    pub fn new() -> Self {
        Self
    }

    /// Extract a target symbol's source code lines or block roughly for context
    /// Extract a target symbol's exact source code block for context
    pub fn extract_symbol_snippet(file_path: &Path, symbol: &str) -> Result<String> {
        let source = fs::read_to_string(file_path)
            .map_err(|e| anyhow!("Failed to read file {}: {e}", file_path.display()))?;
        
        let file = syn::parse_file(&source)
            .map_err(|e| anyhow!("Failed to parse file {}: {e}", file_path.display()))?;

        for item in &file.items {
            if let syn::Item::Fn(syn::ItemFn { sig, block, .. }) = item {
                if sig.ident == symbol {
                    let start = crate::mutation::line_column_to_byte_offset(&source, block.span().start());
                    let end = crate::mutation::line_column_to_byte_offset(&source, block.span().end());
                    return Ok(source[start..end].to_string());
                }
            }
            if let syn::Item::Impl(item_impl) = item {
                for impl_item in &item_impl.items {
                    if let syn::ImplItem::Fn(method) = impl_item {
                        if method.sig.ident == symbol {
                            let start = crate::mutation::line_column_to_byte_offset(&source, method.block.span().start());
                            let end = crate::mutation::line_column_to_byte_offset(&source, method.block.span().end());
                            return Ok(source[start..end].to_string());
                        }
                    }
                }
            }
        }
        
        // Fallback to whole file if symbol isn't found (preserves original behavior)
        Ok(source)
    }

    pub fn apply_refactoring(
        &self,
        file_path: &Path,
        symbol: &str,
        strategy_name: &str,
        payload: &str,
    ) -> Result<()> {
        let source = fs::read_to_string(file_path)?;
        let strategy = get_strategy(strategy_name)
            .ok_or_else(|| anyhow!("Unknown mutation strategy: {}", strategy_name))?;

        println!("==> [RefactorAgent] Validating payload for strategy '{}'...", strategy.name());

        strategy
            .validate_payload(payload)
            .map_err(|e| anyhow!("Payload rejected by strategy '{}': {}", strategy.name(), e))?;

        println!("==> [RefactorAgent] Payload accepted by strategy '{}'.", strategy.name());

        println!(
            "==> [RefactorAgent] Applying strategy '{}' to symbol '{}'...",
            strategy.name(),
            symbol
        );

        let mutated = strategy.apply(&source, symbol, payload)?;

        fs::write(file_path, mutated)?;
        println!("==> [RefactorAgent] Successfully updated {}", file_path.display());
        Ok(())
    }

    pub fn execute_instruction(
        &self,
        file_path: &Path,
        symbol: &str,
        instruction: &str,
        strategy_name: &str,
    ) -> Result<()> {
        let strategy = if strategy_name.eq_ignore_ascii_case("auto") {
            Self::classify_instruction(instruction)?
        } else {
            strategy_name.to_string()
        };

        get_strategy(&strategy)
            .ok_or_else(|| anyhow!("Unauthorized mutation strategy: {}", strategy))?;

        println!("==> [Mutation Policy] Authorized strategy: {}", strategy);

        let snippet = Self::extract_symbol_snippet(file_path, symbol)?;

        // The model generates only the payload. It does not choose the
        // mutation primitive.
        let generated_payload = ApiClient::query_model(instruction, symbol, &snippet, &strategy)?;

        println!("==> [ApiClient] Received refactoring payload from model.");

        let strategy_impl = get_strategy(&strategy)
            .ok_or_else(|| anyhow!("Unauthorized mutation strategy: {}", strategy))?;

        // Validate before mutation. A model payload is never allowed to reach
        // the mutation primitive without satisfying its strategy contract.
        match strategy_impl.validate_payload(&generated_payload) {
            Ok(()) => {
                println!("==> [Mutation Boundary] Initial payload accepted for '{}'.", strategy);

                self.apply_refactoring(file_path, symbol, &strategy, &generated_payload)
            }

            Err(validation_error) => {
                println!(
                    "==> [Mutation Boundary] Initial payload rejected for '{}': {}",
                    strategy, validation_error
                );

                // Exactly ONE corrective retry. The strategy remains fixed;
                // the model receives the validator's concrete failure.
                println!(
                    "==> [ApiClient] Requesting bounded corrective payload for '{}'.",
                    strategy
                );

                let retry_payload = ApiClient::query_model_retry(
                    instruction,
                    symbol,
                    &snippet,
                    &strategy,
                    &validation_error.to_string(),
                    &generated_payload,
                )?;

                println!("==> [ApiClient] Received corrective payload for '{}'.", strategy);

                strategy_impl.validate_payload(&retry_payload).map_err(|retry_error| {
                    anyhow!(
                        "Model payload rejected after bounded retry for strategy '{}': {}",
                        strategy,
                        retry_error
                    )
                })?;

                println!("==> [Mutation Boundary] Corrective payload accepted for '{}'.", strategy);

                self.apply_refactoring(file_path, symbol, &strategy, &retry_payload)
            }
        }
    }

    /// Deterministic policy layer for natural-language refactoring requests.
    ///
    /// The model is deliberately NOT allowed to select the mutation strategy.
    fn classify_instruction(instruction: &str) -> Result<String> {
        let text = instruction.to_ascii_lowercase();

        if text.contains("comment")
            || text.contains("documentation")
            || text.contains("document this")
        {
            return Ok("InsertComment".into());
        }

        if text.contains("rename") {
            return Ok("RenameSymbol".into());
        }

        if text.contains("make public")
            || text.contains("make it public")
            || text.contains("export")
        {
            return Ok("MakePublic".into());
        }

        if text.contains("early return") || text.contains("return early") {
            return Ok("InsertEarlyReturn".into());
        }

        if text.contains("replace function body") || text.contains("replace the function body") {
            return Ok("ReplaceFunctionBody".into());
        }

        Err(anyhow!(
            "No safe mutation strategy matches instruction '{}'.              Specify an explicit --strategy or use a supported instruction.",
            instruction
        ))
    }
}

impl Default for RefactorAgent {
    fn default() -> Self {
        Self::new()
    }
}
