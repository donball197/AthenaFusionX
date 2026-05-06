use serde::{Serialize, Deserialize};
#[derive(Serialize, Deserialize)]
pub struct MeshMessage { pub payload: Vec<u8> }
