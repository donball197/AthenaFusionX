use std::collections::VecDeque;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct NeuralNet {
    pub memory: VecDeque<String>,
    pub depth: usize,
}

impl NeuralNet {
    pub fn new() -> Self {
        Self { memory: VecDeque::new(), depth: 10 }
    }
    pub fn commit(&mut self, entry: String) {
        if self.memory.len() >= self.depth { self.memory.pop_front(); }
        self.memory.push_back(entry);
    }
}
