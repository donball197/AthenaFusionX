pub fn get_resonance() -> String {
    let resonances = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Omega"];
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as usize)
        .unwrap_or(0);
    resonances[seed % resonances.len()].to_string()
}
