use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use syn::{ImplItem, Item};

#[derive(Parser, Debug)]
#[command(author, version, about = "Nexus Polyglot AST Mutation Engine")]
struct Args {
    /// Target file to parse and mutate
    #[arg(short, long)]
    target: PathBuf,

    /// Exact symbol (function/class) to replace
    #[arg(short, long)]
    symbol: String,

    /// File containing the replacement payload
    #[arg(short, long)]
    patch_file: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let ext = args.target.extension().and_then(|s| s.to_str()).unwrap_or("");

    println!(
        ">> [AST ENGINE] Intercepted mutation request for {} (Symbol: {})",
        args.target.display(),
        args.symbol
    );

    match ext {
        "rs" => {
            println!(">> Routing to native Rust Syn/Quote engine...");

            let src = fs::read_to_string(&args.target)
                .with_context(|| format!("Failed to read target file: {:?}", args.target))?;
            let patch_src = fs::read_to_string(&args.patch_file)
                .with_context(|| format!("Failed to read patch payload: {:?}", args.patch_file))?;

            // 1. Parse the original file into an AST
            let mut ast: syn::File = syn::parse_file(&src)
                .with_context(|| "Failed to parse target file as a valid Rust AST")?;

            // 2. Parse the LLM's patch into a valid Function AST node
            let patch_ast: syn::ItemFn = syn::parse_str(&patch_src).with_context(|| {
                "Patch payload must contain a complete, valid Rust function definition (signature + body)."
            })?;

            let mut replaced = false;

            // 3. Traverse the AST and swap the target symbol
            for item in &mut ast.items {
                match item {
                    Item::Fn(func) if func.sig.ident == args.symbol => {
                        *func = patch_ast.clone();
                        replaced = true;
                        break;
                    }
                    Item::Impl(impl_block) => {
                        for impl_item in &mut impl_block.items {
                            if let ImplItem::Fn(method) = impl_item {
                                if method.sig.ident == args.symbol {
                                    let method_tokens = quote::quote!(#patch_ast);
                                    let new_method: syn::ImplItemFn = syn::parse2(method_tokens)
                                        .with_context(|| {
                                            "Failed to convert patch into an impl block method"
                                        })?;

                                    *method = new_method;
                                    replaced = true;
                                    break;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            if !replaced {
                anyhow::bail!(
                    "Symbol '{}' not found in {:?}. Ensure the symbol name is correct.",
                    args.symbol,
                    args.target
                );
            }

            // 4. Unparse the mutated AST back to perfectly formatted Rust code
            let formatted_code = prettyplease::unparse(&ast);

            // 5. Write back to disk
            fs::write(&args.target, formatted_code).with_context(|| {
                format!("Failed to write mutated code back to {:?}", args.target)
            })?;

            println!(
                "[OK] Semantic AST mutation successful. Symbol '{}' replaced and auto-formatted in {:?}.",
                args.symbol, args.target
            );
        }
        "py" | "js" | "ts" | "zig" => {
            println!(">> Routing to Tree-sitter {} engine...", ext);

            let mut parser = tree_sitter::Parser::new();

            let language = match ext {
                "py" => tree_sitter_python::LANGUAGE.into(),
                "ts" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                "js" => tree_sitter_javascript::LANGUAGE.into(),
                "zig" => tree_sitter_zig::LANGUAGE.into(),
                _ => unreachable!(),
            };

            parser.set_language(&language).context("Error loading Tree-sitter grammar")?;

            let source_code = fs::read_to_string(&args.target)
                .with_context(|| format!("Failed to read target file: {:?}", args.target))?;

            let tree = parser.parse(&source_code, None).context("Failed to parse source code")?;
            let root_node = tree.root_node();

            println!("✅ Successfully parsed {} as {}", args.target.display(), ext);
            println!("🌲 AST Root Node Type: {:?}", root_node.kind());
            println!("📊 Total child nodes at root: {}", root_node.child_count());
            println!(
                ">> [PENDING] Tree-sitter AST surgical swap logic queued for Phase 2 integration."
            );
        }
        _ => anyhow::bail!("Unsupported DNA format for AST engine: {}", ext),
    }

    Ok(())
}
