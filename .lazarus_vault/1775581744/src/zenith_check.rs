use rand::Rng;

/// Returns a random resonance string from the predefined set.
pub fn get_resonance() -> String {
    let resonances = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"];
    let mut rng = rand::thread_rng();
    let idx = rng.gen_range(0..resonances.len());
    resonances[idx].to_string()
}