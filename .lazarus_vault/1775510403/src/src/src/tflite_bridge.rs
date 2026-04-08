use tflite::{FlatBufferModel, InterpreterBuilder, ops::builtin::BuiltinOpResolver, Interpreter};
use anyhow::{Result, anyhow};

pub struct LiteRTRuntime {
    // The interpreter will now cleanly own the model and resolver
    interpreter: Interpreter<'static, BuiltinOpResolver>,
}

impl LiteRTRuntime {
    pub fn ignite(model_path: &str) -> Result<Self> {
        eprintln!(">> [NPU_AUTH] Initiating LiteRT Engine (1GB Strict Mode)...");
        
        // 1. Load the model (Mapped to disk, not loaded into RAM)
        let model = FlatBufferModel::build_from_file(model_path)
            .map_err(|e| anyhow!("FRACTURE: Failed to mmap .tflite DNA: {}", e))?;
        
        // 2. Instantiate Operations Resolver
        let resolver = BuiltinOpResolver::default();
        
        // SUTURE: Pass 'model' and 'resolver' by VALUE (Ownership Transfer).
        // This cleanly satisfies the MaybeOwned trait and eliminates the need
        // for Box::leak, resulting in a naturally 'static architecture.
        let builder = InterpreterBuilder::new(model, resolver)
            .map_err(|e| anyhow!("FRACTURE: Failed to build Interpreter context: {}", e))?;
        
        // 3. Build Interpreter
        let mut interpreter = builder.build()
            .map_err(|e| anyhow!("FRACTURE: Could not initialize Interpreter: {}", e))?;
            
        // 1GB RAM GUARD: Strictly 1 thread to prevent memory duplication
        interpreter.set_num_threads(1);

        // 4. Allocate Tensors
        interpreter.allocate_tensors()
            .map_err(|e| anyhow!("MEMORY_STALL: Tensor allocation breached RAM limit: {}", e))?;

        eprintln!(">> [HARDWARE SYMBIOSIS] Tensor Context Allocated. Thread locked to 1. Memory mapped.");

        Ok(Self { interpreter })
    }
}
