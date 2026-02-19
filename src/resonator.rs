#[repr(C)]
pub struct SwarmUnit {
    pub id: u32,
    pub status: u32,
    pub load: f32,
}

extern "C" {
    pub fn init_hive();
    pub fn get_hive_ptr() -> *mut SwarmUnit;
    pub fn update_unit(id: u32, status: u32);
}

pub struct ResonatorCore;

impl ResonatorCore {
    pub fn ignite() {
        unsafe {
            // This calls the Zig code directly
            init_hive();
        }
    }
}
