/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
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
            return Err("MEMORY FAULT: Buffer overflow prevented by strict bounds check.".into());
        }
        self.data.extend_from_slice(payload);
        Ok(())
    }
}
