//! Advanced AST Mutation Strategy system with robust error handling
//! + Tree-sitter based parsing strategy for aggressive fuzzing.

use syn::{parse_file, File, ImplItem, Item, ItemFn, Visibility};
use thiserror::Error;
use tree_sitter::Parser;

/// Structured error type for mutation failures
#[derive(Error, Debug)]
pub enum MutationError {
    #[error("Failed to parse source code with syn: {0}")]
    ParseError(#[from] syn::Error),

    #[error("Symbol '{symbol}' not found in the AST")]
    SymbolNotFound { symbol: String },

    #[error("Payload is not valid Rust for this strategy: {0}")]
    InvalidPayload(String),

    #[error("Mutation produced invalid code: {0}")]
    InvalidResult(String),

    #[error("Tree-sitter failed to parse: {0}")]
    TreeSitterError(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type MutationResult<T> = std::result::Result<T, MutationError>;

/// Core trait
pub trait MutationStrategy: Send + Sync {
    fn name(&self) -> &'static str;
    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String>;
}

// ------------------------------------------------------------------
// Helpers
// ------------------------------------------------------------------

fn parse_source(source: &str) -> MutationResult<File> {
    parse_file(source).map_err(MutationError::ParseError)
}

fn unparse(file: &File) -> String {
    prettyplease::unparse(file)
}

// ------------------------------------------------------------------
// Strategy 1: Replace Function / Method Body
// ------------------------------------------------------------------

pub struct ReplaceFunctionBody;

impl MutationStrategy for ReplaceFunctionBody {
    fn name(&self) -> &'static str {
        "ReplaceFunctionBody"
    }

    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String> {
        let mut file = parse_source(source)?;
        let mut found = false;

        let new_block: syn::Block = syn::parse_str(&format!("{{ {} }}", payload))
            .map_err(|e| MutationError::InvalidPayload(e.to_string()))?;

        for item in &mut file.items {
            match item {
                Item::Fn(ItemFn { sig, block, .. }) if sig.ident == symbol => {
                    *block = Box::new(new_block.clone());
                    found = true;
                    break;
                }
                Item::Impl(item_impl) => {
                    for impl_item in &mut item_impl.items {
                        if let ImplItem::Fn(method) = impl_item {
                            if method.sig.ident == symbol {
                                method.block = new_block.clone();
                                found = true;
                                break;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if !found {
            return Err(MutationError::SymbolNotFound {
                symbol: symbol.to_string(),
            });
        }

        Ok(unparse(&file))
    }
}

// ------------------------------------------------------------------
// Strategy 2: Rename Symbol
// ------------------------------------------------------------------

pub struct RenameSymbol;

impl MutationStrategy for RenameSymbol {
    fn name(&self) -> &'static str {
        "RenameSymbol"
    }

    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String> {
        let mut file = parse_source(source)?;
        let new_ident: syn::Ident = syn::parse_str(payload)
            .map_err(|e| MutationError::InvalidPayload(format!("Invalid identifier: {}", e)))?;

        let mut found = false;

        for item in &mut file.items {
            match item {
                Item::Fn(ItemFn { sig, .. }) if sig.ident == symbol => {
                    sig.ident = new_ident.clone();
                    found = true;
                }
                Item::Impl(item_impl) => {
                    for impl_item in &mut item_impl.items {
                        if let ImplItem::Fn(method) = impl_item {
                            if method.sig.ident == symbol {
                                method.sig.ident = new_ident.clone();
                                found = true;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if !found {
            return Err(MutationError::SymbolNotFound {
                symbol: symbol.to_string(),
            });
        }

        Ok(unparse(&file))
    }
}

// ------------------------------------------------------------------
// Strategy 3: Make Function Public
// ------------------------------------------------------------------

pub struct MakePublic;

impl MutationStrategy for MakePublic {
    fn name(&self) -> &'static str {
        "MakePublic"
    }

    fn apply(&self, source: &str, symbol: &str, _payload: &str) -> MutationResult<String> {
        let mut file = parse_source(source)?;
        let mut found = false;

        for item in &mut file.items {
            if let Item::Fn(ItemFn { vis, sig, .. }) = item {
                if sig.ident == symbol {
                    *vis = Visibility::Public(Default::default());
                    found = true;
                    break;
                }
            }
        }

        if !found {
            return Err(MutationError::SymbolNotFound {
                symbol: symbol.to_string(),
            });
        }

        Ok(unparse(&file))
    }
}

// ------------------------------------------------------------------
// Strategy 4: Insert Early Return
// ------------------------------------------------------------------

pub struct InsertEarlyReturn;

impl MutationStrategy for InsertEarlyReturn {
    fn name(&self) -> &'static str {
        "InsertEarlyReturn"
    }

    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String> {
        let mut file = parse_source(source)?;
        let mut found = false;

        let early_return: syn::Stmt = syn::parse_str(&format!("return {};", payload))
            .map_err(|e| MutationError::InvalidPayload(e.to_string()))?;

        for item in &mut file.items {
            if let Item::Fn(ItemFn { sig, block, .. }) = item {
                if sig.ident == symbol {
                    block.stmts.insert(0, early_return.clone());
                    found = true;
                    break;
                }
            }
        }

        if !found {
            return Err(MutationError::SymbolNotFound {
                symbol: symbol.to_string(),
            });
        }

        Ok(unparse(&file))
    }
}

// ------------------------------------------------------------------
// Strategy 5: Tree-sitter Parse (for aggressive parser fuzzing)
// ------------------------------------------------------------------

/// This strategy does not mutate the code.
/// It forces Tree-sitter to parse the source (and optionally the payload)
/// so the fuzzer can hunt for panics / crashes in the parser.
pub struct TreeSitterParse;

impl MutationStrategy for TreeSitterParse {
    fn name(&self) -> &'static str {
        "TreeSitterParse"
    }

    fn apply(&self, source: &str, _symbol: &str, payload: &str) -> MutationResult<String> {
        let mut parser = Parser::new();

        // Set language to Rust
        let language = tree_sitter_rust::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| MutationError::TreeSitterError(format!("Failed to set language: {e}")))?;

        // Parse the main source
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| MutationError::TreeSitterError("Parser returned None".into()))?;

        // Optionally also parse the payload (increases pressure on the parser)
        if !payload.is_empty() {
            let _ = parser.parse(payload, None);
        }

        // We return the original source – the goal is crash resistance, not mutation
        // You can later inspect tree.root_node() if you want deeper testing
        let _root = tree.root_node();

        Ok(source.to_string())
    }
}

// ------------------------------------------------------------------
// Registry
// ------------------------------------------------------------------

pub fn all_strategies() -> Vec<Box<dyn MutationStrategy>> {
    vec![
        Box::new(ReplaceFunctionBody),
        Box::new(RenameSymbol),
        Box::new(MakePublic),
        Box::new(InsertEarlyReturn),
        Box::new(TreeSitterParse),   // ← new aggressive parser strategy
    ]
}

pub fn get_strategy(name: &str) -> Option<Box<dyn MutationStrategy>> {
    all_strategies()
        .into_iter()
        .find(|s| s.name().eq_ignore_ascii_case(name))
}
