use rand::Rng;

/// Returns a random Resonance string from the predefined set.
pub fn get_resonance() -> String {
    let resonances = vec!["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Omega"];
    let mut rng = rand::thread_rng();
    let index = rng.gen_range(0..resonances.len());
    resonances[index].to_string()
}