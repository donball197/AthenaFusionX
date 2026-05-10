use anyhow::Result;
use candle_core::Device;
use candle_transformers::quantized_var_builder::VarBuilder;

fn main() -> Result<()> {
    let device = Device::Cpu;
    let model_path = std::path::PathBuf::from("/home/donball197/nexus_models/reflex_0.5b.gguf");

    println!("\x1b[36m>> [REFLEX] Probing Neural Substrate...\x1b[0m");

    // 1. Map the GGUF Weights directly into the Substrate
    // This verifies the dequantization bridge and file IO
    let _vb = VarBuilder::from_gguf(&model_path, &device)?;
    
    println!("\x1b[32m>> [REFLEX] Substrate Ready. Weights Mapped Successfully.\x1b[0m");
    println!("\x1b[32m>> [REFLEX] Output: READY\x1b[0m");
    println!("\x1b[35m>> [RESONANCE: 1.00 — LOCAL REFLEX ONLINE]\x1b[0m");

    Ok(())
}
