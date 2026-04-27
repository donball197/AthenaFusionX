// src/distributed_scheduler.rs — v2.0 [PRE-EMPTIVE FORGE OFFLOAD]
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

    pub async fn synthesize(&self, code: &str) -> Result<String> {
        if let Some(node) = self.mesh.best_node() {
            if node.cpu < 85.0 && node.temp < 68.0 {
                 println!(
                    "\x1b[35m>> [MESH_ROUTING] Offloading WASM Synthesis to {} (Temp: {:.1}°C Load: {:.1}%)\x1b[0m",
                    node.id, node.temp, node.cpu
                );
                return self.remote_synthesize(&node.host, node.port, code).await;
            }
        }
        
        println!("\x1b[33m>> [LOCAL_EXEC] Substrate utilized for WASM synthesis (Heavy Load)\x1b[0m");
        crate::wasm_forge::WasmForge::synthesize_cell(code).await
    }

    async fn remote_synthesize(&self, host: &str, port: u16, code: &str) -> Result<String> {
        #[derive(Serialize)]
        struct Req<'a> { code: &'a str }
        #[derive(Deserialize)]
        struct Reply { ok: bool, b64: Option<String>, error: Option<String> }

        let url = format!("http://{}:{}/api/cluster/forge", host, port);
        let res = reqwest::Client::new()
            .post(&url)
            .header("Authorization", "SOVEREIGN")
            .json(&Req { code })
            .send()
            .await?;

        let reply: Reply = res.json().await?;
        if reply.ok { 
            Ok(reply.b64.unwrap_or_default()) 
        } else { 
            Err(anyhow!(reply.error.unwrap_or_else(|| "Remote forge error".into()))) 
        }
    }

    pub async fn execute(&self, wasm: Vec<u8>, func_name: &str, input: f32) -> Result<f32> {
        if let Some(node) = self.mesh.best_node() {
            if node.cpu < 85.0 && node.temp < 68.0 {
                 println!(
                    ">> [MESH_ROUTING] Offloading execution of {} to {}",
                    func_name, node.id
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
