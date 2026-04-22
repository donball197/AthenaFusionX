use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::cluster::ClusterMesh;
use crate::wasm_runtime::WasmRuntime;

#[derive(Clone)]
pub struct DistributedScheduler {
    pub mesh: Arc<ClusterMesh>,
}

impl DistributedScheduler {
    pub fn new(mesh: Arc<ClusterMesh>) -> Self {
        Self { mesh }
    }

    pub async fn execute(&self, wasm: Vec<u8>, func_name: &str, input: f32) -> Result<f32> {
        if let Some(node) = self.mesh.best_node() {
            // Using correct field names: node.cpu and node.temp
            if node.cpu < 85.0 && node.temp < 68.0 {
                 println!(
                    ">> [MESH_ROUTING] Offloading {} to {} (Temp: {:.1}°C Load: {:.1}%)",
                    func_name, node.id, node.temp, node.cpu
                );
                return self.remote_call(&node.host, node.port, wasm, func_name, input).await;
            }
        }
        
        println!(">> [LOCAL_EXEC] Substrate utilized for {}", func_name);
        WasmRuntime::run_bytes(&wasm, func_name, input)
    }

    async fn remote_call(&self, host: &str, port: u16, wasm: Vec<u8>, func_name: &str, input: f32) -> Result<f32> {
        #[derive(Serialize)]
        struct Req<'a> { wasm_b64: String, func_name: &'a str, input: f32 }
        #[derive(Deserialize)]
        struct Reply { ok: bool, result: f32, error: Option<String> }

        let url = format!("http://{}:{}/api/cluster/run", host, port);
        let res = reqwest::Client::new()
            .post(&url)
            .header("Authorization", "SOVEREIGN")
            .json(&Req { wasm_b64: general_purpose::STANDARD.encode(&wasm), func_name, input })
            .send()
            .await?;

        let reply: Reply = res.json().await?;
        if reply.ok { Ok(reply.result) } 
        else { Err(anyhow!(reply.error.unwrap_or_else(|| "remote error".into()))) }
    }
}
