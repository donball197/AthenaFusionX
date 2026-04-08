mod resonator; // Link to our Zig bridge

fn main() {
    println!(">> [NEXUS] INITIATING HIVE PULSE CHECK...");
    
    // Pulse the Zig Hive
    resonator::ignite_swarm();
    
    let hive = resonator::sync_swarm_to_ui();
    let active_count = hive.iter().filter(|u| u.status > 0).count();
    
    println!(">> [STATUS] HIVE COUNT: {}/68 UNITS DETECTED", hive.len());
    println!(">> [STATUS] ACTIVE UNITS: {}", active_count);
    
    if hive.len() == 68 {
        println!(">> [SUCCESS] THE HIVE IS ALIVE.");
    } else {
        println!(">> [WARNING] FRAGMENTATION DETECTED IN HIVE MEMORY.");
    }
}
