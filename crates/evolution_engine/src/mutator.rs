use crate::genome::SystemGenome;
use rand::Rng;

pub struct Mutator;

impl Mutator {
    pub fn mutate(parent: &SystemGenome) -> SystemGenome {
        let mut rng = rand::thread_rng();
        let mut child = parent.clone();

        child.generation += 1;

        // Drift cognitive weights by +/- 10%
        let mut drift = || rng.gen_range(0.90..1.10);

        child.weights.meta_reasoning_threshold *= drift();
        child.weights.critic_penalty_weight *= drift();
        child.weights.heuristic_reward *= drift();

        // Enforce boundary constraints
        child.weights.meta_reasoning_threshold =
            child.weights.meta_reasoning_threshold.clamp(0.1, 0.9);
        child.weights.heuristic_reward = child.weights.heuristic_reward.clamp(0.1, 0.8);

        // Generate deterministic child hash
        child.hash = format!("gen_{}_{}", child.generation, hex::encode(rng.gen::<[u8; 4]>()));

        child
    }
}
