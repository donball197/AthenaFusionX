// Project Nexus Singularity v0.2.2
// Zircon Type Definitions for ARM64 Bridge

pub const zx_status_t = i32;
pub const zx_handle_t = u32;
pub const zx_vaddr_t = usize;
pub const zx_paddr_t = usize;

// Status Codes
pub const ZX_OK: zx_status_t = 0;
pub const ZX_ERR_INTERNAL: zx_status_t = -1;
pub const ZX_ERR_NOT_SUPPORTED: zx_status_t = -2;
pub const ZX_ERR_NO_RESOURCES: zx_status_t = -3;
pub const ZX_ERR_NO_MEMORY: zx_status_t = -4;
pub const ZX_ERR_INVALID_ARGS: zx_status_t = -10;
pub const ZX_ERR_BAD_HANDLE: zx_status_t = -11;
pub const ZX_ERR_WRONG_TYPE: zx_status_t = -12;
pub const ZX_ERR_ACCESS_DENIED: zx_status_t = -15;

// VMAR/VMO Mapping Options
pub const ZX_VM_SPECIFIC: u32 = 1 << 0;
pub const ZX_VM_PERM_READ: u32 = 1 << 1;
pub const ZX_VM_PERM_WRITE: u32 = 1 << 2;
pub const ZX_VM_PERM_EXECUTE: u32 = 1 << 3;
pub const ZX_VM_MAP_RANGE: u32 = 1 << 4;
pub const ZX_VM_CAN_MAP_READ: u32 = 1 << 7;
pub const ZX_VM_CAN_MAP_WRITE: u32 = 1 << 8;
pub const ZX_VM_CAN_MAP_EXECUTE: u32 = 1 << 9;

// VMO Clone Options
pub const ZX_VMO_CLONE_COPY_ON_WRITE: u32 = 1;
pub const ZX_VMO_NON_RESIZABLE: u32 = 1 << 1;
