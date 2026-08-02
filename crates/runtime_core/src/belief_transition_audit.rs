use crate::belief_transition::BeliefTransitionEvent;

pub struct BeliefTransitionAudit;

impl BeliefTransitionAudit {
    pub fn summarize(event: &BeliefTransitionEvent) -> String {
        format!(
            "Belief transition: {:.2} -> {:.2}. Lesson: {}",
            event.previous_confidence, event.new_confidence, event.lesson
        )
    }
}
