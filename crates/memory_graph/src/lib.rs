pub struct MemoryGraph {
    pub state: std::collections::HashMap<String, Vec<u8>>,
}

impl Default for MemoryGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryGraph {
    pub fn new() -> Self {
        Self { state: std::collections::HashMap::new() }
    }

    pub fn atomic_update(&mut self, key: String, value: Vec<u8>) {
        self.state.insert(key, value);
    }

    pub fn get_state(&self, key: &str) -> Option<&Vec<u8>> {
        self.state.get(key)
    }
}
