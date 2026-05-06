use anyhow::{Result, anyhow};
use syn::{File, Item, Block};

pub fn patch_function(mut file: File, target_name: &str, new_body: &str) -> Result<File> {
    let mut found = false;
    
    // Ensure body is wrapped in braces for syn::Block compatibility
    let formatted_body = if !new_body.trim().starts_with('{') {
        format!("{{ {} }}", new_body)
    } else {
        new_body.to_string()
    };

    for item in file.items.iter_mut() {
        if let Item::Fn(func) = item {
            if func.sig.ident == target_name {
                let new_block: Block = syn::parse_str(&formatted_body)
                    .map_err(|e| anyhow!("AST_PARSE_FAIL in new body: {}", e))?;
                func.block = Box::new(new_block);
                found = true;
            }
        }
    }

    if !found { return Err(anyhow!("FUNCTION NOT FOUND: {}", target_name)); }
    Ok(file)
}
