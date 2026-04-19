use axum::{response::IntoResponse, Json};
use serde_json::json;

pub async fn handle_get_vmos() -> impl IntoResponse {
    let mut handles = [0u32; 1024];
    let count = unsafe { crate::kernel::ffi::zx_vmo_get_all_handles(handles.as_mut_ptr(), 1024) };
    
    let mut vmo_list = Vec::new();
    for i in 0..count {
        let mut info = crate::kernel::ffi::VmoInfo {
            handle: 0, size: 0, committed_pages: 0, is_cow: false, parent_handle: 0
        };
        let res = unsafe { crate::kernel::ffi::zx_vmo_get_info(handles[i], &mut info) };
        
        if res == 0 {
            vmo_list.push(json!({
                "handle": format!("0x{:x}", info.handle),
                "size_bytes": info.size,
                "committed_pages": info.committed_pages,
                "memory_used_kb": (info.committed_pages * 4096) / 1024,
                "is_cow": info.is_cow,
                "parent_handle": if info.parent_handle > 0 { format!("0x{:x}", info.parent_handle) } else { "None".to_string() }
            }));
        }
    }

    Json(json!({ "vmos": vmo_list, "active_count": count }))
}
