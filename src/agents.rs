pub struct SwarmUnit {
    pub id: u32,
    pub sector: String,
    pub active: bool,
}

pub fn spawn_swarm(count: u32) -> Vec<SwarmUnit> {
    let mut swarm = Vec::new();
    
    for i in 1..=count {
        // Assigning maintenance sectors based on the Numbered Hierarchy
        let sector = match i {
            1 => "Main/Alloc".to_string(),
            2 => "Server/Network".to_string(),
            3 => "Lazarus/Pulse".to_string(),
            _ => "General Maintenance".to_string(),
        };

        swarm.push(SwarmUnit {
            id: i,
            sector: sector,
            active: true,
        });
    }
    
    swarm
}
