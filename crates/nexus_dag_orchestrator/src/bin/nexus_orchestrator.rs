use dotenvy::dotenv;
use nexus_dag_orchestrator::executor::DagExecutor;
use nexus_dag_orchestrator::graph::{DagManager, TaskNode, TaskStatus};
use nexus_dag_orchestrator::planner::DagPlanner;
use std::env;
use std::thread;
use std::time::Duration;

fn main() {
    dotenv().ok();
    let args: Vec<String> = env::args().collect();

    // Accept objective from CLI or use a default test objective
    let objective = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        "Audit the local file system structure, then analyze it for security flaws.".to_string()
    };

    println!("==================================================");
    println!(" AthenaFusionX: Executive Cortex & DAG Orchestrator");
    println!(" Target: ARM64 Shared DNA");
    println!(" Objective: {}", objective);
    println!("==================================================");

    println!("[*] Executive Cortex: Generating DAG blueprint...");

    let planned_tasks = match DagPlanner::generate_dag(&objective) {
        Ok(tasks) => tasks,
        Err(e) => {
            eprintln!("[-] FATAL: Planner failed to generate DAG: {}", e);
            std::process::exit(1);
        }
    };

    println!("[+] Blueprint Acquired: {} tasks dynamically mapped.", planned_tasks.len());

    let mut manager = DagManager::new();
    for pt in planned_tasks {
        println!(
            "  -> Node: [{}] Tier: [{}] Deps: {:?}",
            pt.id,
            pt.required_tier.to_uppercase(),
            pt.dependencies
        );
        manager.add_task(TaskNode {
            id: pt.id,
            description: pt.description,
            required_tier: pt.required_tier,
            dependencies: pt.dependencies,
            status: TaskStatus::Pending,
        });
    }

    let mut executor = DagExecutor::new(manager);
    let mut loop_count = 1;

    println!("\n[*] Executing Directed Acyclic Graph...\n");

    loop {
        let all_complete =
            executor.manager.nodes.values().all(|n| n.status == TaskStatus::Completed);
        if all_complete {
            println!("\n[+] DAG Execution Complete. All nodes synchronized.");
            break;
        }

        let failed = executor.manager.nodes.values().any(|n| n.status == TaskStatus::Failed);
        if failed {
            eprintln!("\n[-] DAG Execution Halted. One or more tasks failed.");
            break;
        }

        println!("--- [Tick {}] Evaluating DAG State ---", loop_count);
        match executor.run_step() {
            Ok(executed) => {
                if executed == 0 && loop_count == 1 {
                    println!("  -> No tasks ready. (Graph error or circular dependency)");
                    break;
                }
            }
            Err(e) => {
                eprintln!("[-] Executor Error: {}", e);
                break;
            }
        }

        loop_count += 1;
        thread::sleep(Duration::from_secs(2)); // Pause for API rate limits and stdout clarity
    }
}
