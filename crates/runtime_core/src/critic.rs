use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CriticVerdict {
    pub approved: bool,
    pub confidence: f32,
    pub reason: String,
    pub suggested_recovery: Option<String>,
}

pub struct Critic;

impl Critic {
    pub async fn evaluate(
        _objective: &str,
        observations: &str,
        _history: &str,
    ) -> Result<CriticVerdict, Box<dyn std::error::Error>> {
        // If there is no observation history yet, we can't be done
        if observations.trim().is_empty() {
            return Ok(CriticVerdict {
                approved: false,
                confidence: 0.1,
                reason: "No actions have been executed yet. Cannot halt.".into(),
                suggested_recovery: Some("Execute an action first.".into()),
            });
        }

        // We do NOT check for [HALT] here, because the Critic is invoked specifically
        // to validate the Halt intent *before* it gets added to the log.
        let has_write = observations.contains("[WRITE_OK]")
            || observations.contains("[DIFF_PATCH_OK]")
            || observations.contains("[PATCH_FUNCTION_OK]")
            || observations.contains("success");
        let has_read = observations.contains("[READ_OK]");
        let has_cmd = observations.contains("[CMD_OK]");
        let has_search = observations.contains("[SEARCH_OK]");

        let approved = has_write || has_read || has_cmd || has_search;
        let confidence = if approved { 0.95 } else { 0.4 };

        Ok(CriticVerdict {
            approved,
            confidence,
            reason: if approved {
                "Concrete success indicators found in action history.".into()
            } else {
                "Missing concrete success indicators (Read/Write/Cmd).".into()
            },
            suggested_recovery: if !approved {
                Some("Perform a read, write, or command execution to gather evidence.".into())
            } else {
                None
            },
        })
    }
}
