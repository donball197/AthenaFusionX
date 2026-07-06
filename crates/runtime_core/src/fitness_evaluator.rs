use anyhow::Result;
use std::time::Instant;
use sysinfo::{System, CpuRefreshKind, MemoryRefreshKind, RefreshKind};
use crate::wasm_runtime::WasmRuntime;

#[derive(Debug, Clone)]
pub struct FitnessReport {
    pub execution_time_ms: u128,
    pub memory_delta_kb: f32,
    pub thermal_impact: f32,
    pub composite_score: f32,
}

pub struct FitnessEvaluator;

impl FitnessEvaluator {
    pub fn evaluate_wasm_cell(wasm_bytes: &[u8], func_name: &str, payload: &[u8]) -> Result<FitnessReport> {
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        sys.refresh_all();
        
        // 1. Capture ARM64 Baseline Metrics
        let mem_before = sys.used_memory() as f32 / 1024.0;
        let temp_before = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
            .unwrap_or_else(|_| "45000".to_string())
            .trim()
            .parse::<f32>()
            .unwrap_or(45000.0) / 1000.0;

        // 2. Execute the Generated WASM Payload
        let start = Instant::now();
        let _result = WasmRuntime::run_with_buffer(wasm_bytes, func_name, payload)?;
        let duration = start.elapsed().as_millis();

        // 3. Capture Post-Execution Metrics
        sys.refresh_all();
        let mem_after = sys.used_memory() as f32 / 1024.0;
        let temp_after = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
            .unwrap_or_else(|_| "45000".to_string())
            .trim()
            .parse::<f32>()
            .unwrap_or(45000.0) / 1000.0;

        // Ensure we don't divide by zero
        let memory_delta = (mem_after - mem_before).max(0.1);
        let thermal_delta = (temp_after - temp_before).max(0.1);

        // 4. Calculate Mathematical Fitness Score (Higher is Better)
        // Inverse relations: Lower latency/memory/heat yields a higher score
        // Weights: 50% Speed, 30% Memory Efficiency, 20% Thermal Stability
        let speed_score = 1000.0 / (duration as f32 + 1.0);
        let mem_score = 1000.0 / memory_delta;
        let heat_score = 1000.0 / thermal_delta;

        let composite_score = (speed_score * 0.5) + (mem_score * 0.3) + (heat_score * 0.2);

        Ok(FitnessReport {
            execution_time_ms: duration,
            memory_delta_kb: memory_delta,
            thermal_impact: thermal_delta,
            composite_score,
        })
    }
}
