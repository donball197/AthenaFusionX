use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub enum SystemPayload {
    TelemetryMetric { id: u64, value: f64 },
    AgentCommand { task_id: u64, priority: u8 },
    StateUpdate { status_code: u32 },
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct FabricMessage {
    pub sender_id: u64,
    pub timestamp: u64,
    pub payload: SystemPayload,
}
