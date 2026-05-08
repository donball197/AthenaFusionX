use nexus_singularity::wasm_forge::WasmForge;
use nexus_singularity::wasm_registry::WasmRegistry;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("\x1b[35m>> [FORGE] Priming WASM Standard Library...\x1b[0m");
    let registry = WasmRegistry::new(".nexus_wasm_registry.db").await?;

    let cells = vec![
        ("compute_resonance", r#"
            pub extern "C" fn compute_resonance(cpu: f32, temp: f32) -> f32 {
                (cpu * 0.7) + (temp / 100.0 * 0.3)
            }
        "#),
        ("entropy_shield", r#"
            pub extern "C" fn entropy_shield(input: f32) -> f32 {
                if input > 0.8 { 1.0 } else { input * 0.5 }
            }
        "#),
        ("signal_gain", r#"
            pub extern "C" fn signal_gain(val: f32) -> f32 {
                val * 1.05
            }
        "#),
        ("resonance_calibrator", r#"
            pub extern "C" fn resonance_calibrator(resonance: f32) -> f32 {
                if resonance > 0.6 { 1.0 } else { 0.0 }
            }
        "#),
    ];

    for (name, code) in cells {
        println!("   -> Forging: {}", name);
        let b64 = WasmForge::synthesize_cell(code).await?;
        registry.store(name, &b64, code).await?;
    }

    println!("\x1b[32m>> [SUCCESS] Standard Library Enshrined.\x1b[0m");
    Ok(())
}
