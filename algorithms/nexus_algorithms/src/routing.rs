#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelTier {
    Heavy,
    Light,
    Planner,
}

pub struct Router;

impl Router {
    /// Returns the tier a named capability should resolve against, or
    /// None if the capability name is unrecognized (caller should choose
    /// a safe default, e.g. Heavy).
    pub fn select(capability: &str) -> Option<ModelTier> {
        match capability {
            "reasoning" | "action_planning" | "critique" | "verify" | "synthesize" => {
                Some(ModelTier::Heavy)
            }
            "evaluate" | "reflect" | "action_translation" => Some(ModelTier::Light),
            "plan" => Some(ModelTier::Planner),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_capabilities_map_to_expected_tier() {
        assert_eq!(Router::select("reasoning"), Some(ModelTier::Heavy));
        assert_eq!(Router::select("action_planning"), Some(ModelTier::Heavy));
        assert_eq!(Router::select("critique"), Some(ModelTier::Heavy));
        assert_eq!(Router::select("verify"), Some(ModelTier::Heavy));
        assert_eq!(Router::select("synthesize"), Some(ModelTier::Heavy));
        assert_eq!(Router::select("evaluate"), Some(ModelTier::Light));
        assert_eq!(Router::select("reflect"), Some(ModelTier::Light));
        assert_eq!(Router::select("action_translation"), Some(ModelTier::Light));
        assert_eq!(Router::select("plan"), Some(ModelTier::Planner));
    }

    #[test]
    fn unknown_capability_returns_none() {
        assert_eq!(Router::select("nonexistent"), None);
    }
}
