use nexus_model_hub::neural_agent::MicroAgent;
use std::sync::Arc;

pub struct ArbiterAgent {
    pub agent: Arc<MicroAgent>,
}

impl ArbiterAgent {
    pub fn new(agent: Arc<MicroAgent>) -> Self {
        Self { agent }
    }

    pub fn validate_action(action_type: &str, command: &str) -> Result<(), String> {
        let dangerous_patterns = [
            "rm -rf /",
            "rm -rf ~",
            "mkfs",
            "> /dev/sda",
            "> /dev/nvme",
            "chmod 777 /",
            "/etc/shadow",
            "mv /etc/",
            "mv /usr/",
            ":(){ :|:& };:",
            "dd if=/dev/zero",
            "fork bomb",
            "sudo su",
            "sudo -i",
            "sudo bash",
            "sudo sh",
            "passwd root",
            "chown root",
            "$'\\x",
            "\\u0072\\u006d",
            "base64 -d |",
            "eval $(",
            "exec(",
            "curl | bash",
            "wget | bash",
            "curl | sh",
            "history -c",
            "unset HISTFILE",
            // Hardening pass: podman.rs no longer sandboxes anything, this
            // blocklist is now the only gate for commands run on the host.
            "-exec rm",
            "-exec sh",
            "-exec bash",
            " -delete",
            "git push --force",
            "git push -f",
            "git clean -f",
            "python3 -c",
            "python -c",
            "perl -e",
            "ruby -e",
            "node -e",
        ];

        let normalized = command.to_lowercase();

        for pattern in dangerous_patterns.iter() {
            if normalized.contains(pattern) {
                return Err(format!(
                    "SANDBOX_VIOLATION: Threat pattern '{}' detected in {}.",
                    pattern, action_type
                ));
            }
        }

        if normalized.contains("../../../") || normalized.contains("/etc/passwd") {
            return Err("SANDBOX_VIOLATION: Directory traversal attempt blocked.".to_string());
        }

        if (normalized.contains("| bash")
            || normalized.contains("| sh")
            || normalized.contains("|bash")
            || normalized.contains("|sh"))
            && !normalized.starts_with("echo")
        {
            return Err("SANDBOX_VIOLATION: Pipe-to-shell pattern blocked.".to_string());
        }

        Ok(())
    }

    pub fn evaluate_entropy_delta(base_entropy: f32, new_entropy: f32) -> Result<(), String> {
        let delta = (new_entropy - base_entropy).abs();
        if delta > 0.5 {
            return Err(format!("ENTROPY_ANOMALY: Delta of {:.4} exceeds 0.5 threshold.", delta));
        }
        Ok(())
    }

    pub fn resolve_consensus(alpha_vector: &[f32], beta_vector: &[f32]) -> bool {
        if alpha_vector.len() != beta_vector.len() {
            return false;
        }
        let diff: f32 =
            alpha_vector.iter().zip(beta_vector.iter()).map(|(a, b)| (a - b).abs()).sum();
        diff < 0.5
    }

    pub fn bft_consensus(vectors: &[Vec<f32>], epsilon: f32) -> Option<Vec<f32>> {
        if vectors.is_empty() {
            return None;
        }
        let n = vectors.len();
        let threshold = (2 * n / 3) + 1;
        let dim = vectors[0].len();
        if vectors.iter().any(|v| v.len() != dim) {
            return None;
        }

        let (best_idx, neighbor_count) = (0..n)
            .map(|i| {
                let neighbors = (0..n)
                    .filter(|&j| {
                        vectors[i]
                            .iter()
                            .zip(vectors[j].iter())
                            .map(|(a, b)| (a - b).abs())
                            .sum::<f32>()
                            < epsilon
                    })
                    .count();
                (i, neighbors)
            })
            .max_by_key(|&(_, count)| count)?;

        if neighbor_count < threshold {
            return None;
        }

        let agreeing: Vec<&Vec<f32>> = (0..n)
            .filter(|&j| {
                vectors[best_idx]
                    .iter()
                    .zip(vectors[j].iter())
                    .map(|(a, b)| (a - b).abs())
                    .sum::<f32>()
                    < epsilon
            })
            .map(|j| &vectors[j])
            .collect();

        let centroid: Vec<f32> = (0..dim)
            .map(|k| agreeing.iter().map(|v| v[k]).sum::<f32>() / agreeing.len() as f32)
            .collect();

        Some(centroid)
    }
}
