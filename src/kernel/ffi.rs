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


pub const ROOT_VMAR_HANDLE: u32 = 0;
pub const MAX_REGION_SIZE: usize = 1024 * 1024 * 1024;
pub const ZX_VM_PERM_READ: u32 = 1 << 1;
pub const ZX_VM_PERM_WRITE: u32 = 1 << 2;
pub const ZX_VM_MAP_RANGE: u32 = 1 << 4;

pub trait KernelOps: Send + Sync {
    fn vmo_clone(&self, parent: u32, size: usize) -> u32;
    fn vmar_map(&self, vmar: u32, options: u32, vmo: u32, size: usize) -> *mut u8;
    fn handle_close(&self, handle: u32);
}

pub struct HardwareInterface;

impl KernelOps for HardwareInterface {
    fn vmo_clone(&self, parent: u32, size: usize) -> u32 {
        println!(">> [MOCK KERNEL] Cloning VMO parent {} with size {}", parent, size);
        parent + 1
    }
    fn vmar_map(&self, vmar: u32, _options: u32, vmo: u32, size: usize) -> *mut u8 {
        println!(">> [MOCK KERNEL] Mapping VMO {} to VMAR {} (Size: {})", vmo, vmar, size);
        std::ptr::null_mut() // Safe null pointer for testing
    }
    fn handle_close(&self, handle: u32) {
        println!(">> [MOCK KERNEL] Closing handle {}", handle);
    }
}

pub mod raw {
    pub unsafe fn nexus_vmar_map(_vmar: u32, _options: u32, _vmo: u32, _size: usize) -> *mut u8 {
        std::ptr::null_mut()
    }
    pub unsafe fn nexus_vmo_clone(parent: u32, _size: usize) -> u32 {
        parent + 1
    }
    pub unsafe fn nexus_handle_close(_handle: u32) {}
}

#[derive(Clone, Copy)]
pub struct VmarHandle(pub u32);

impl Default for VmarHandle {
    fn default() -> Self {
        Self(ROOT_VMAR_HANDLE)
    }
}
