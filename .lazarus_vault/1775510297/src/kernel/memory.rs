pub struct SafeBuffer {
    data: Vec<u8>,
    max_capacity: usize,
}

impl SafeBuffer {
    pub fn new(max_capacity: usize) -> Self {
        Self { data: Vec::with_capacity(max_capacity), max_capacity }
    }

    pub fn write_safe(&mut self, payload: &[u8]) -> Result<(), String> {
        if self.data.len() + payload.len() > self.max_capacity {
            return Err("MEMORY FAULT: Buffer overflow prevented by strict bounds check.".into());
        }
        self.data.extend_from_slice(payload);
        Ok(())
    }
}