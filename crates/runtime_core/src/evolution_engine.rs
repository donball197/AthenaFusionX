use crate::fitness_evaluator::FitnessEvaluator;
use crate::wasm_forge::WasmForge;
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use nexus_substrate::memory::MemoryPalace;
use nexus_substrate::vector_bridge;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct EvolutionEngine;

impl EvolutionEngine {
    pub async fn evolve_and_memorize(
        _client: &Client,
        api_key: &str,
        memory: Arc<Mutex<MemoryPalace>>,
        objective: &str,
        candidate_code: &str,
        test_payload: &[u8],
        func_name: &str,
    ) -> Result<f32> {
        tracing::info!(
            "\x1b[35m>> [EVOLUTION] Synthesizing candidate genome for:\x1b[0m {}",
            objective
        );

        // 1. Synthesize the candidate code into a WASM cell
        let wasm_b64 = WasmForge::synthesize_cell(candidate_code).await?;
        let wasm_bytes = general_purpose::STANDARD.decode(&wasm_b64)?;

        tracing::info!("\x1b[36m>> [EVOLUTION] Candidate compiled. Initiating physical fitness benchmark...\x1b[0m");

        // 2. Evaluate physical fitness (Speed, Memory, Thermal)
        let report = FitnessEvaluator::evaluate_wasm_cell(&wasm_bytes, func_name, test_payload)?;

        tracing::info!(
            "\x1b[33m>> [EVOLUTION] Composite Fitness Score: {:.2}\x1b[0m ({}ms latency, {:.2}KB mem, {:.2}C temp delta)",
            report.composite_score, report.execution_time_ms, report.memory_delta_kb, report.thermal_impact
        );

        // 3. Selection: Only memorize if it meets a high fitness threshold
        if report.composite_score > 100.0 {
            tracing::info!("\x1b[1;32m>> [EVOLUTION] High-Fitness Pattern Detected! Extracting semantic embedding...\x1b[0m");

            // Embed the context
            let embedding_text = format!("OBJECTIVE: {}\nCODE:\n{}", objective, candidate_code);
            let embedding = vector_bridge::get_embedding(api_key, &embedding_text).await.ok();

            let insight = format!(
                "[HIGH_FITNESS_PATTERN] Score: {:.2} | Algorithm successfully optimized for ARM64 constraints. \nCODE_SNIPPET:\n{}",
                report.composite_score,
                &candidate_code[..candidate_code.len().min(500)]
            );

            // Store in Memory Palace
            let mem = memory.lock().await;
            mem.store_memory("EvolutionEngine", objective, &insight, embedding).await?;

            tracing::info!("\x1b[1;36m>> [EVOLUTION] Genetic material successfully committed to VectorVault.\x1b[0m");
        } else {
            tracing::warn!("\x1b[90m>> [EVOLUTION] Candidate failed to meet fitness threshold. Discarding genome.\x1b[0m");
        }

        Ok(report.composite_score)
    }
}
