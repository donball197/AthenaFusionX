use nexus_runtime_core::agents::v2_worker::WorkerAgent;
use nexus_runtime_core::orchestrator_v2::OrchestratorV2;
use nexus_runtime_core::orchestrator_v2::task_graph::TaskGraph;

#[tokio::main]
async fn main() {
    println!("\x1b[36m>> [NEXUS] IGNITING V2 DAG ORCHESTRATOR...\x1b[0m");
    
    let mut orchestrator = OrchestratorV2::new();
    orchestrator.register_agent("worker", Box::new(WorkerAgent));

    let mut graph = TaskGraph::new();
    graph.add_task("compile_wasm", vec![], "worker");
    graph.add_task("run_sandbox", vec!["compile_wasm".into()], "worker");
    graph.add_task("extract_metrics", vec!["run_sandbox".into()], "worker");

    match orchestrator.execute_graph(graph).await {
        Ok(results) => {
            for r in results {
                println!("\x1b[32m[{}]\x1b[0m {}", r.id, r.output);
            }
        }
        Err(e) => {
            eprintln!("\x1b[31m[CRITICAL FAULT] {}\x1b[0m", e);
        }
    }
}
