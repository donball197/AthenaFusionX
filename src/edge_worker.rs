use axum::{extract::State, Json};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::server::AppState;
use crate::wasm_runtime::WasmRuntime;

#[derive(Deserialize)]
pub struct WasmJob {
    pub wasm_b64: String,
    pub func_name: String,
    pub input: f32,
}

#[derive(Serialize)]
pub struct WasmReply {
    pub ok: bool,
    pub result: f32,
    pub error: Option<String>,
}

pub async fn run_remote_job(
    State(_state): State<Arc<AppState>>,
    Json(job): Json<WasmJob>,
) -> Json<WasmReply> {
    let bytes = match general_purpose::STANDARD.decode(&job.wasm_b64) {
        Ok(b) => b,
        Err(e) => return Json(WasmReply { ok: false, result: 0.0, error: Some(e.to_string()) }),
    };

    match WasmRuntime::run_bytes(&bytes, &job.func_name, job.input) {
        Ok(result) => Json(WasmReply { ok: true, result, error: None }),
        Err(e) => Json(WasmReply { ok: false, result: 0.0, error: Some(e.to_string()) }),
    }
}
