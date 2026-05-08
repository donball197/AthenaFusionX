/**
 * Copyright 2026 Donald Ball
 * Licensed under the Apache License, Version 2.0
 */

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
            return Err(format!(
                "MEMORY FAULT: Buffer overflow prevented. Tried to write {} bytes, only {} remaining.",
                payload.len(),
                self.max_capacity.saturating_sub(self.data.len())
            ));
        }
        self.data.extend_from_slice(payload);
        Ok(())
    }

    /// Returns current length.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Clears the buffer, resetting it for reuse.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Returns a read-only slice of the buffer contents.
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
}
