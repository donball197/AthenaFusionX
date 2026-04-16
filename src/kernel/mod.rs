pub mod vmo;
pub mod vmar;
pub mod error;
pub mod ffi;

use std::collections::HashMap;
use std::sync::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    static ref HANDLE_REGISTRY: Mutex<HashMap<u32, HandleType>> = Mutex::new(HashMap::new());
}

pub enum HandleType {
    Vmo(vmo::Vmo),
    Vmar(vmar::Vmar),
}

pub fn register_handle(id: u32, h: HandleType) {
    let mut reg = HANDLE_REGISTRY.lock().unwrap();
    reg.insert(id, h);
}
