use crate::orchestrator_v2::task_graph::{TaskGraph, TaskStatus};
use crate::orchestrator_v2::verifier::VerificationResult;

pub struct Scheduler;

impl Scheduler {
    const MAX_RETRIES: u8 = 2;

    /// Transitions viable tasks from Pending/Retrying to Ready.
    /// Returns the IDs of tasks that are newly marked as Ready.
    pub fn prepare_ready_tasks(graph: &mut TaskGraph) -> Vec<String> {
        let mut newly_ready = Vec::new();

        // We must clone the keys to avoid mutably borrowing the graph while iterating
        let node_ids: Vec<String> = graph.nodes.keys().cloned().collect();

        for id in node_ids {
            let mut is_ready = false;

            // Check dependency status immutably
            if let Some(node) = graph.nodes.get(&id) {
                if node.status == TaskStatus::Pending || node.status == TaskStatus::Retrying {
                    let all_deps_met = node.deps.iter().all(|dep_id| {
                        graph
                            .nodes
                            .get(dep_id)
                            .map(|n| n.status == TaskStatus::Completed)
                            .unwrap_or(false)
                    });
                    if all_deps_met {
                        is_ready = true;
                    }
                }
            }

            // Apply state transition
            if is_ready {
                if let Some(node) = graph.nodes.get_mut(&id) {
                    node.status = TaskStatus::Ready;
                    newly_ready.push(id.clone());
                }
            }
        }
        newly_ready
    }

    pub fn mark_running(graph: &mut TaskGraph, task_id: &str) {
        if let Some(node) = graph.nodes.get_mut(task_id) {
            if node.status == TaskStatus::Ready {
                node.status = TaskStatus::Running;
            }
        }
    }

    pub fn process_verification(graph: &mut TaskGraph, task_id: &str, result: VerificationResult) {
        if let Some(node) = graph.nodes.get_mut(task_id) {
            match result {
                VerificationResult::Success => {
                    node.status = TaskStatus::Completed;
                }
                VerificationResult::Retry(reason) => {
                    node.execution_state.attempts += 1;
                    node.execution_state.last_error = Some(reason.clone());

                    if node.execution_state.attempts >= Self::MAX_RETRIES {
                        node.status = TaskStatus::Escalated; // Budget exhausted
                    } else {
                        node.status = TaskStatus::Retrying;
                    }
                }
                VerificationResult::Escalate(reason) => {
                    node.execution_state.last_error = Some(reason);
                    node.status = TaskStatus::Escalated;
                }
                VerificationResult::Failure(reason) => {
                    node.execution_state.last_error = Some(reason);
                    node.status = TaskStatus::Failed;
                }
            }
        }
    }
}
