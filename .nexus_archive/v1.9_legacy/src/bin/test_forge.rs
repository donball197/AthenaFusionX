use nexus_singularity::{wasm_forge::WasmForge, wasm_runtime::WasmRuntime};
use base64::Engine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let zig_code = r#"
        export fn apply_gain(input: f32) f32 {
            return input * 1.45;
        }
    "#;

    let cell = WasmForge::synthesize_cell(zig_code).await?;
    let cell_bytes = base64::engine::general_purpose::STANDARD.decode(cell.as_bytes())?;
    let result = WasmRuntime::run_bytes(&cell_bytes, "apply_gain", 10.0)?;

    println!("Result: {}", result);
    Ok(())
}
