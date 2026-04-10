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
use crate::kernel::ffi::{self, ZX_VM_MAP_RANGE, ZX_VM_PERM_READ, ZX_VM_PERM_WRITE};
use crate::kernel::vmo::Vmo;
use std::ptr;

/// VirtualMemoryAddressRegion: Orchestrates address space mapping.
pub struct Vmar;

impl Vmar {
    /// map: Maps a VMO into the specified VMAR handle using centralized permissions.
    pub fn map(vmar_handle: u32, vmo: &Vmo) -> Result<*mut u8, KernelError> {
        let options = ZX_VM_PERM_READ | ZX_VM_PERM_WRITE | ZX_VM_MAP_RANGE;

        let addr = unsafe { ffi::raw::nexus_vmar_map(vmar_handle, options, vmo.handle, vmo.size) };

        if addr == ptr::null_mut() {
            Err(KernelError::VmarMapFailed { vmar_handle, size: vmo.size })
        } else {
            Ok(addr)
        }
    }
}
