use nexus_executive_cortex::schema::{Observation, ObservationEnvelope};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Translates raw, ad-hoc runtime events into pure, timestamped Observations.
pub struct ObservationAdapter;

impl ObservationAdapter {
    /// Core wrapping function to ensure all observations are uniformly timestamped and ID'd.
    pub fn wrap(observation: Observation) -> ObservationEnvelope {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        ObservationEnvelope {
            id: Uuid::new_v4(),
            timestamp,
            observation,
        }
    }

    /// Translation: User inputs a directive via CLI or Web UI
    pub fn from_human_directive(directive: &str) -> ObservationEnvelope {
        Self::wrap(Observation::HumanDirective {
            directive: directive.to_string(),
        })
    }

    /// Translation: The LLM planner exhausts its retry quota
    pub fn from_planner_timeout() -> ObservationEnvelope {
        Self::wrap(Observation::PlannerTimeout)
    }

    /// Translation: A MicroAgent finishes its assigned task
    pub fn from_task_completion(task_id: &str, output: &str) -> ObservationEnvelope {
        Self::wrap(Observation::TaskCompleted {
            task_id: task_id.to_string(),
            output: output.to_string(),
        })
    }

    /// Translation: An agent hits a fatal error
    pub fn from_goal_failure(goal_id: Uuid, reason: &str) -> ObservationEnvelope {
        Self::wrap(Observation::GoalFailed {
            goal_id,
            reason: reason.to_string(),
        })
    }

    /// Translation: Filtered health spikes from the thermal/system monitor
    pub fn from_health_spike(cpu: f32, ram: f32) -> ObservationEnvelope {
        Self::wrap(Observation::HealthChanged { cpu, ram })
    }
}
