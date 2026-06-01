mod scheduler;

use scheduler::{DeterministicScheduler, CognitiveTask, ExecutionClass, ResourceBudget, RuntimeCounters, TaskState};
use std::{sync::{Arc, Mutex}, time::{SystemTime, UNIX_EPOCH}};
use uuid::Uuid;
use axum::{routing::get, Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct TaskView { id: String, class: String }

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    
    // 1. Initialize persistent scheduler state
    let scheduler = Arc::new(Mutex::new(DeterministicScheduler::new()));
    
    // 2. Pre-seed with the tasks you verified earlier
    {
        let mut kernel = scheduler.lock().unwrap();
        let current_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        kernel.submit(CognitiveTask {
            id: Uuid::new_v4(),
            lineage_id: Uuid::new_v4(),
            parent_lineage_id: None,
            execution_class: ExecutionClass::SovereignInference,
            state: TaskState::Ready,
            static_priority: 10,
            dynamic_priority: 0,
            budget: ResourceBudget { cpu_budget_ms: 2000, token_budget: 4096, max_memory_bytes: 512, io_weight: 4 },
            counters: RuntimeCounters { cpu_time_used_ms: 0, tokens_consumed: 0, memory_used_bytes: 0, io_operations: 0 },
            created_at: current_timestamp,
        });
    }

    // 3. Setup API Gateway
    let app = Router::new().route("/api/tasks", get({
        let scheduler = Arc::clone(&scheduler);
        move || async move {
            let kernel = scheduler.lock().unwrap();
            let tasks: Vec<TaskView> = kernel.queue.iter().map(|t| TaskView {
                id: t.id.to_string(),
                class: format!("{:?}", t.execution_class),
            }).collect();
            Json(tasks)
        }
    }));

    println!("⚡ COGNITIVE KERNEL SERVICE ONLINE at 0.0.0.0:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
