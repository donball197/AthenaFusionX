#[tokio::main]
async fn main() {
    let wasm_bytes = std::fs::read("sum.wasm").expect("FATAL: sum.wasm not found in root!");
    let payload = vec![1u8; 1024];

    let report = nexus_runtime_core::fitness_evaluator::FitnessEvaluator::evaluate_wasm_cell(
        &wasm_bytes,
        "sumBytes",
        &payload,
    )
    .expect("FATAL: Wasm execution failed!");

    println!(">> [WASM KERNEL VITALS]");
    println!("execution_time_ms: {}", report.execution_time_ms);
    println!("memory_delta_kb: {:.2}", report.memory_delta_kb);
    println!("composite_score: {:.2}", report.composite_score);
}
