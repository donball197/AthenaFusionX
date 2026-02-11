use std::time::Instant;

pub struct SwarmUnit {
    pub id: String,
    pub hive_id: usize,
    pub buffer_limit: usize,
    pub buffer: Vec<u8>,
    pub current_target: String,
    pub is_active: bool,     // New: Tracks if RAM is occupied
    pub last_active: Instant, // New: Tracks when it last touched the vault
}

impl SwarmUnit {
    pub fn ignite(&mut self) {
        if !self.is_active {
            // Allocate the 15MB buffer only when needed
            self.buffer = Vec::with_capacity(self.buffer_limit);
            self.is_active = true;
        }
        self.last_active = Instant::now();
    }

    pub fn execute_mission(&mut self, target: &str) {
        self.current_target = target.to_string();
        self.last_active = Instant::now(); // Refresh the 5-minute clock
    }

    pub fn clear_scratchpad(&mut self) {
        // The "Cold Standby" protocol
        self.buffer = Vec::new();
        self.buffer.shrink_to_fit();
        self.is_active = false;
        self.current_target = "STANDBY".to_string();
        self.last_active = Instant::now(); 
    }
}

pub fn spawn_swarm(count: usize) -> Vec<SwarmUnit> {
    (0..count).map(|i| {
        SwarmUnit {
            id: format!("{:02}", i),
            hive_id: i / 4,
            buffer_limit: 15 * 1024 * 1024,
            buffer: Vec::new(), // Zero initial allocation
            current_target: "IDLE".to_string(),
            is_active: false,
            last_active: Instant::now(),
        }
    }).collect()
}
