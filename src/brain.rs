use crate::memory::NexusMemory;

pub struct NeuralNet {
    pub memory: NexusMemory,
}

impl NeuralNet {
    pub fn new() -> Self {
        Self {
            memory: NexusMemory::load(),
        }
    }

    pub fn commit(&mut self, entry: String) {
        println!(">> [CORTEX] Committing to permanent storage: {}", entry);
        self.memory.remember(entry);
    }
    
    pub fn recall(&self) -> String {
        self.memory.get_context_window()
    }
}
