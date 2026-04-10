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


use crate::kernel::error::KernelError;
use crate::kernel::ffi::KernelOps;
use std::sync::Arc;

/// Vmo: Virtual Memory Object owning a kernel handle.
pub struct Vmo {
    pub handle: u32,
    pub size: usize,
    ops: Arc<dyn KernelOps>,
}

impl Vmo {
    /// clone: Generates a child VMO using the provided hardware interface.
    pub fn clone(ops: Arc<dyn KernelOps>, parent: u32, size: usize) -> Result<Self, KernelError> {
        if parent == 0 {
            return Err(KernelError::InvalidHandle(0));
        }

        let handle = ops.vmo_clone(parent, size);
        if handle == 0 {
            Err(KernelError::VmoCloneFailed { parent, size })
        } else {
            Ok(Self { handle, size, ops })
        }
    }
}

impl Drop for Vmo {
    fn drop(&mut self) {
        self.ops.handle_close(self.handle);
    }
}
