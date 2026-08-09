use crate::memory_cortex::MemoryCortex;
use crate::orchestrator_v2::task_graph::TaskGraph;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExecutiveGoalStatus {
    Active,
    Suspended,
    Completed,
    Abandoned,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GoalNode {
    pub id: Uuid,
    pub objective: String,
    pub belief_score: f32,
    pub status: ExecutiveGoalStatus,
    pub children: Vec<Uuid>,
}

#[derive(Debug, Clone)]
pub enum ExecutiveCommand {
    InjectGoal { objective: String, parent_id: Option<Uuid> },
    EvaluateEnvironment,
    EmergencyHalt { reason: String },
}

pub struct ExecutiveCortex {
    goals: HashMap<Uuid, GoalNode>,
    active_graph: Option<TaskGraph>,
    memory: Arc<MemoryCortex>,
    command_rx: mpsc::Receiver<ExecutiveCommand>,
}

impl ExecutiveCortex {
    pub fn new(memory: Arc<MemoryCortex>) -> (Self, mpsc::Sender<ExecutiveCommand>) {
        let (tx, rx) = mpsc::channel(100);
        (Self { goals: HashMap::new(), active_graph: None, memory, command_rx: rx }, tx)
    }

    pub async fn run_actor_loop(mut self) {
        // Run a pulse every 10 seconds for persistent state checking
        let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(10));

        loop {
            tokio::select! {
                Some(cmd) = self.command_rx.recv() => {
                    self.handle_command(cmd).await;
                }
                _ = heartbeat.tick() => {
                    self.cognitive_tick().await;
                }
            }
        }
    }

    async fn cognitive_tick(&mut self) {
        let active_goals: Vec<&GoalNode> =
            self.goals.values().filter(|g| g.status == ExecutiveGoalStatus::Active).collect();

        if active_goals.is_empty() {
            return;
        }

        tracing::info!(
            "[EXECUTIVE CORTEX] Pulse check: Maintaining intent vector for {} active goal(s)...",
            active_goals.len()
        );

        for goal in active_goals {
            let graph_json = serde_json::to_string(&self.active_graph).unwrap_or_default();
            let status_str = format!("{:?}", goal.status);
            let created_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            // Persist the active intention to SQLite, allowing it to survive reboots
            let _ = self
                .memory
                .upsert_plan_run(
                    &goal.id.to_string(),
                    &goal.objective,
                    &graph_json,
                    &status_str,
                    created_at,
                )
                .await;
        }
    }

    pub async fn evaluate_belief_state(
        &self,
        belief_id: uuid::Uuid,
        confidence: f32,
    ) -> anyhow::Result<crate::belief_policy::BeliefDecision> {
        let decision = crate::belief_policy::BeliefDecisionPolicy::evaluate(belief_id, confidence);

        self.memory.store_belief_decision(decision.clone()).await?;

        Ok(decision)
    }

    pub async fn apply_belief_decision(
        &mut self,
        goal_id: uuid::Uuid,
        decision: crate::belief_policy::BeliefDecision,
    ) -> anyhow::Result<crate::goal_adaptation::GoalAdaptationDecision> {
        let adaptation = crate::goal_adaptation::GoalAdaptationEngine::from_belief_action(
            goal_id,
            decision.action,
        );

        match adaptation.action {
            crate::goal_adaptation::GoalAdaptationAction::PauseGoal => {
                if let Some(goal) = self.goals.get_mut(&goal_id) {
                    goal.status = ExecutiveGoalStatus::Suspended;
                }
            }

            crate::goal_adaptation::GoalAdaptationAction::TerminateGoal => {
                if let Some(goal) = self.goals.get_mut(&goal_id) {
                    goal.status = ExecutiveGoalStatus::Abandoned;
                }
            }

            _ => {}
        }

        self.memory.store_goal_adaptation(adaptation.clone()).await?;

        Ok(adaptation)
    }

    async fn handle_command(&mut self, cmd: ExecutiveCommand) {
        match cmd {
            ExecutiveCommand::InjectGoal { objective, parent_id } => {
                let id = Uuid::new_v4();
                tracing::info!(
                    "\x1b[35m[EXECUTIVE CORTEX] Injecting new background goal: {}\x1b[0m",
                    objective
                );

                let goal = GoalNode {
                    id,
                    objective,
                    belief_score: 0.5, // Seed with standard prior
                    status: ExecutiveGoalStatus::Active,
                    children: vec![],
                };
                self.goals.insert(id, goal);

                if let Some(pid) = parent_id {
                    if let Some(parent) = self.goals.get_mut(&pid) {
                        parent.children.push(id);
                    }
                }
            }
            ExecutiveCommand::EmergencyHalt { reason } => {
                tracing::error!(
                    "\x1b[31m[EXECUTIVE CORTEX] GLOBAL HALT TRIGGERED: {}\x1b[0m",
                    reason
                );
                self.goals.values_mut().for_each(|g| g.status = ExecutiveGoalStatus::Suspended);
            }
            ExecutiveCommand::EvaluateEnvironment => {
                tracing::info!("[EXECUTIVE CORTEX] Environment evaluation requested.");
            }
        }
    }
}

pub fn evaluate_causal_memory(memories: Vec<crate::causal_recall::CausalRecall>) -> Vec<String> {
    memories.iter().map(|m| m.recommendation()).collect()
}

// v1.9 hard evidence gate:
// Before marking a GoalNode / strategy as Proven, call:
//   belief.may_prove(required_artifacts, threshold)
// Never treat scalar confidence alone as sufficient.
