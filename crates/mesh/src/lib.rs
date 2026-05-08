use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
pub struct MeshMessage {
    pub payload: Vec<u8>,
}
