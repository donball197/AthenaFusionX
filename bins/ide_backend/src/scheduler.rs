use uuid::Uuid;

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum TaskState {
    Ready,
    Blocked,
    Suspended,
    Completed,
    Failed,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ResourceBudget {
    pub cpu_budget_ms: u32,
    pub token_budget: usize,
    pub max_memory_bytes: usize,
    pub io_weight: u8,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct RuntimeCounters {
    pub cpu_time_used_ms: u32,
    pub tokens_consumed: usize,
    pub memory_used_bytes: usize,
    pub io_operations: u64,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct CognitiveTask {
    pub id: Uuid,
    pub lineage_id: Uuid,
    pub parent_lineage_id: Option<Uuid>,
    pub execution_class: ExecutionClass,
    pub state: TaskState,
    pub static_priority: u8,
    pub dynamic_priority: u32,
    pub budget: ResourceBudget,
    pub counters: RuntimeCounters,
    pub created_at: u64,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum ExecutionClass {
    RealTimeTelemetry,
    SovereignInference,
    BackgroundConsolidation,
}

pub struct DeterministicScheduler {
    pub queue: Vec<CognitiveTask>,
}

impl DeterministicScheduler {
    #[allow(dead_code)]
    pub fn dispatch(&mut self) -> Option<CognitiveTask> {
        if self.queue.is_empty() {
            None
        } else {
            Some(self.queue.remove(0))
        }
    }
}
