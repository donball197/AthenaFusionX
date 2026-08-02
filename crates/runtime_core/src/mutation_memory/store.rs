use super::MutationMemoryRecord;

pub struct MutationMemory {
    records: Vec<MutationMemoryRecord>,
}

impl Default for MutationMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl MutationMemory {
    pub fn new() -> Self {
        Self { records: Vec::new() }
    }

    pub fn store(&mut self, record: MutationMemoryRecord) {
        self.records.push(record);
    }

    pub fn count(&self) -> usize {
        self.records.len()
    }
}
