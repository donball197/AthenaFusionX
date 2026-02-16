#[repr(C)]
pub struct SwarmUnit {
    pub id: u32,
    pub status: u8,
    pub load: u8,
}

extern "C" {
    fn init_hive();
    fn get_hive_ptr() -> *mut SwarmUnit;
    fn update_unit(id: u32, status: u8, load: u8);
}

pub fn ignite_swarm() {
    unsafe {
        init_hive();
        println!(">> [NEXUS] ZIG HIVE INITIALIZED IN CORE MEMORY");
    }
}

pub fn sync_swarm_to_ui() -> Vec<SwarmUnit> {
    let mut units = Vec::with_capacity(68);
    unsafe {
        let ptr = get_hive_ptr();
        for i in 0..68 {
            let unit = &*ptr.add(i);
            units.push(SwarmUnit {
                id: unit.id,
                status: unit.status,
                load: unit.load,
            });
        }
    }
    units
}
