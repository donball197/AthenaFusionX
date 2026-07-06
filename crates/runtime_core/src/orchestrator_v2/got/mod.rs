use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(tag = "operation_type")]
pub enum GoTOperation {
    #[default]
    Execute,
    Generate { n: usize },
    Score,
    KeepBest { n: usize },
}


