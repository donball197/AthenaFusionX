use crate::agent_protocol::AgentAction;
use crate::intent_contract::IntentContract;

pub struct MotorCortex;

impl MotorCortex {
    pub fn translate(intent: &IntentContract, action: AgentAction) -> MotorCommand {
        MotorCommand { intent_id: intent.id, action }
    }
}

pub struct MotorCommand {
    pub intent_id: uuid::Uuid,

    pub action: AgentAction,
}
