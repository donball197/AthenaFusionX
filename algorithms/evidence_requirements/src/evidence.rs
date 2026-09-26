#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActionIdentity(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TargetId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Scope(pub String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evidence {
    pub action_identity: ActionIdentity,
    pub target: Option<TargetId>,
    pub scope: Option<Scope>,
    pub success: bool,
    pub observation: Option<String>,
}

impl Evidence {
    pub fn new(
        action_identity: ActionIdentity,
        target: Option<TargetId>,
        scope: Option<Scope>,
        success: bool,
        observation: Option<String>,
    ) -> Self {
        Self { action_identity, target, scope, success, observation }
    }
}
