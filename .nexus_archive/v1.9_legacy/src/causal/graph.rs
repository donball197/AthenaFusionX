use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CausalEvent {
    pub id: String,
    pub impact_score: f32,
    pub node_id: String,
}

#[derive(Default)]
pub struct CausalGraph {
    pub events: HashMap<String, CausalEvent>,
}

impl CausalGraph {
    pub fn ingest(&mut self, event: CausalEvent) {
        self.events.insert(event.id.clone(), event);
    }
}
