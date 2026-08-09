use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use syn::{ImplItem, Item};

#[derive(Parser, Debug)]
#[command(author, version, about = "Semantic AST-aware diff/patch engine for Rust AI Agents")]
struct Args {
    /// Path to the target Rust file to mutate
    #[arg(short, long)]
    target: String,

    /// The exact name of the function or method to replace (e.g., "get_vitals")
    #[arg(short, long)]
    symbol: String,

    /// Path to the file containing the replacement complete function definition
    #[arg(short, long)]
    patch_file: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let src = fs::read_to_string(&args.target)
        .with_context(|| format!("Failed to read target file: {}", args.target))?;
    let patch_src = fs::read_to_string(&args.patch_file)
        .with_context(|| format!("Failed to read patch payload: {}", args.patch_file))?;

    // 1. Parse the original file into an AST
    let mut ast: syn::File =
        syn::parse_file(&src).with_context(|| "Failed to parse target file as a valid Rust AST")?;

    // 2. Parse the LLM's patch into a valid Function AST node
    let patch_ast: syn::ItemFn = syn::parse_str(&patch_src).with_context(|| {
        "Patch payload must contain a complete, valid Rust function definition (signature + body)."
    })?;

    let mut replaced = false;

    // 3. Traverse the AST and swap the target symbol
    for item in &mut ast.items {
        match item {
            // Match standalone global functions
            Item::Fn(func) if func.sig.ident == args.symbol => {
                *func = patch_ast.clone();
                replaced = true;
                break;
            }
            // Match methods inside an impl block
            Item::Impl(impl_block) => {
                for impl_item in &mut impl_block.items {
                    if let ImplItem::Fn(method) = impl_item {
                        if method.sig.ident == args.symbol {
                            // Convert the ItemFn into an ImplItemFn using quote
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
            "Symbol '{}' not found in {}. Ensure the symbol name is correct.",
            args.symbol,
            args.target
        );
    }

    // 4. Unparse the mutated AST back to perfectly formatted Rust code
    let formatted_code = prettyplease::unparse(&ast);

    // 5. Write back to disk
    fs::write(&args.target, formatted_code)
        .with_context(|| format!("Failed to write mutated code back to {}", args.target))?;

    println!(
        "[OK] Semantic AST mutation successful. Symbol '{}' replaced and auto-formatted in {}.",
        args.symbol, args.target
    );

    Ok(())
}
