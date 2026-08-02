use anyhow::Result;
use nexus_executive_cortex::{DecisionSink, schema::{Decision, ExecutiveIntent}};

/// Translates pure Executive Decisions into concrete physical side-effects.
#[derive(Default)]
pub struct RuntimeDispatcher;

impl RuntimeDispatcher {
    pub fn new() -> Self {
        Self
    }
}

impl DecisionSink for RuntimeDispatcher {
    fn dispatch(&self, decision: Decision) -> Result<()> {
        // Log the pure decision intent before applying physical side-effects
        println!(
            "\x1b[35m>> [DISPATCHER] Policy evaluating intent: {:?} | Confidence: {:.2}\x1b[0m",
            decision.intent,
            decision.confidence
        );
        println!("\x1b[90m   Reason: {}\x1b[0m", decision.reason);
        
        // The Policy Routing Table
        match decision.intent {
            ExecutiveIntent::RequestPlanning => {
                println!("\x1b[32m   -> Action: Spawning DAG Planner for new directive.\x1b[0m");
                // Integration Point: Orchestrator DAG invocation will go here.
            }
            ExecutiveIntent::CompleteGoal => {
                println!("\x1b[32m   -> Action: Archiving Goal to Memory Cortex and updating Telemetry.\x1b[0m");
                // Integration Point: MemoryActor & Telemetry sync goes here.
            }
            ExecutiveIntent::Escalate => {
                println!("\x1b[33m   -> Action: System health critical! Escalating to Human Operator.\x1b[0m");
                // Integration Point: UI WebSocket alert goes here.
            }
            ExecutiveIntent::Abort => {
                println!("\x1b[31m   -> Action: Irrecoverable failure. Safely freezing the DAG.\x1b[0m");
                // Integration Point: Graceful soft-halt mechanism goes here.
            }
            ExecutiveIntent::Continue => {
                println!("\x1b[36m   -> Action: No execution required. Proceeding nominally.\x1b[0m");
            }
            _ => {
                println!("\x1b[90m   -> Action: Policy for {:?} is stubbed for future implementation.\x1b[0m", decision.intent);
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation_adapter::ObservationAdapter;
    use nexus_executive_cortex::ExecutiveCortex;

    #[test]
    fn test_pure_cortex_pipeline() {
        println!("\n\x1b[1m--- INITIATING CORTEX PIPELINE SIMULATION ---\x1b[0m");

        // 1. Boot the pure state machine and the side-effect dispatcher
        let mut cortex = ExecutiveCortex::new();
        let dispatcher = RuntimeDispatcher::new();

        // 2. The Real World happens (A user types a command)
        println!("\x1b[90m[Step 1] Real world event: User typed a directive.\x1b[0m");
        let envelope = ObservationAdapter::from_human_directive("Refactor the memory bus architecture.");
        
        // 3. The Cortex evaluates the observation (Zero side-effects)
        println!("\x1b[90m[Step 2] Cortex processing Observation Envelope (ID: {})...\x1b[0m", envelope.id);
        let decision = cortex.receive(envelope);

        // 4. The Dispatcher physically acts on the pure decision
        println!("\x1b[90m[Step 3] Routing Decision to Dispatcher...\x1b[0m");
        dispatcher.dispatch(decision).unwrap();
        
        println!("\x1b[1m--- SIMULATION COMPLETE ---\x1b[0m\n");
    }
}
