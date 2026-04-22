use nexus_singularity::resonator::ResonatorCore;
use nexus_singularity::swarm::consensus::AverageConsensus;

fn main() {
    let core = ResonatorCore::new();
    let base_temp = core.read_thermal_temp() as f64;

    println!("\x1b[35m>> [SWARM] Pulling Heterogeneous Thermal Pulses...\x1b[0m");

    // Simulate slight sensor variance across the AARCH64 substrate
    let initial_states =
        vec![("Alpha", base_temp - 2.5), ("Beta", base_temp + 1.2), ("Gamma", base_temp + 0.8)];

    for (node, temp) in &initial_states {
        println!("   [NODE {}] Local Sensor: {:.2}°C", node, temp);
    }

    let mut swarm = AverageConsensus::new(initial_states);

    println!("\x1b[33m>> [SWARM] Initiating Average Consensus Iterations...\x1b[0m");

    // Perform 10 iterations of the consensus algorithm
    for i in 1..=5 {
        swarm.iterate(0.4); // Epsilon = 0.4
        let states: Vec<String> =
            swarm.nodes.iter().map(|n| format!("{:.4}", n.local_state)).collect();
        println!("   [ITER {}] States: {:?}", i, states);
    }

    swarm.converge(0.4, 20);

    println!(
        "\x1b[32m>> [SUCCESS] Swarm Converged on Substrate Mean: {:.2}°C\x1b[0m",
        swarm.nodes[0].local_state
    );

    swarm.persist_to_vmo(0xca0c9);
}
