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


pub struct SwarmUnit {
    pub id: u32,
    pub sector: String,
    pub active: bool,
}

pub fn spawn_swarm(count: u32) -> Vec<SwarmUnit> {
    let mut swarm = Vec::new();
    
    for i in 1..=count {
        let sector = match i {
            1 => "Main/Alloc".to_string(),
            2 => "Server/Network".to_string(),
            3 => "Lazarus/Pulse".to_string(),
            _ => "General Maintenance".to_string(),
        };

        swarm.push(SwarmUnit {
            id: i,
            sector,
            active: true,
        });
    }
    
    swarm
}
