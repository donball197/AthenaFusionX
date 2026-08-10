use axum::{
    body::Body,
    extract::{Json, State},
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use base64::Engine;
use nexus_runtime_core::agent_protocol::AgentAction;
use nexus_runtime_core::wasm_forge::WasmForge;
use nexus_runtime_core::wasm_runtime::WasmRuntime;
use nexus_substrate::kernel::safe_kernel::SafeKernel;
use nexus_substrate::kernel::safe_vmo::SafeVmo;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// --- State Management ---
struct NodeState {
    vmo_registry: Mutex<HashMap<u32, SafeVmo>>,
}

#[derive(Deserialize)]
struct VmoCreateReq {
    size: usize,
}
#[derive(Serialize)]
struct VmoCreateRes {
    handle: u32,
}
#[derive(Deserialize)]
struct VmoWriteReq {
    handle: u32,
    offset: usize,
    data: Vec<u8>,
}
#[derive(Deserialize)]
struct VmoReadReq {
    handle: u32,
    offset: usize,
    size: usize,
}
#[derive(Deserialize)]
struct ForgeReq {
    code: String,
}
#[derive(Serialize)]
struct ForgeRes {
    ok: bool,
    b64: Option<String>,
    error: Option<String>,
}
#[derive(Deserialize)]
struct ExecuteReq {
    wasm_b64: String,
    func_name: String,
    input: f32,
}
#[derive(Serialize)]
struct ExecuteRes {
    ok: bool,
    result: Option<f32>,
    error: Option<String>,
}
#[derive(Deserialize)]
struct VmoEntropyReq {
    handle: u32,
}
#[derive(Serialize)]
struct VmoEntropyRes {
    entropy: f32,
}

// --- Middleware: Sovereign Guard ---
async fn auth_guard(req: Request<Body>, next: axum::middleware::Next) -> impl IntoResponse {
    let auth_header = req.headers().get("Authorization").and_then(|v| v.to_str().ok());
    if auth_header == Some("SOVEREIGN") {
        next.run(req).await
    } else {
        (StatusCode::UNAUTHORIZED, "Access Denied: SOVEREIGN token required").into_response()
    }
}

// --- Handlers ---
async fn create_vmo(
    State(state): State<Arc<NodeState>>,
    Json(req): Json<VmoCreateReq>,
) -> impl IntoResponse {
    match SafeKernel::create_vmo(req.size) {
        Ok(handle) => {
            if let Ok(vmo) = SafeVmo::new(req.size) {
                state.vmo_registry.lock().await.insert(handle, vmo);
                (StatusCode::OK, Json(VmoCreateRes { handle })).into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, "VMO Wrapper failure").into_response()
            }
        }
        Err(e) => (StatusCode::BAD_REQUEST, format!("Kernel Error: {:?}", e)).into_response(),
    }
}

async fn write_vmo(
    State(state): State<Arc<NodeState>>,
    Json(req): Json<VmoWriteReq>,
) -> impl IntoResponse {
    let registry = state.vmo_registry.lock().await;
    if let Some(vmo) = registry.get(&req.handle) {
        match vmo.write(req.offset, &req.data) {
            Ok(_) => StatusCode::OK.into_response(),
            Err(e) => (StatusCode::BAD_REQUEST, format!("Write Error: {:?}", e)).into_response(),
        }
    } else {
        (StatusCode::NOT_FOUND, "VMO handle not found").into_response()
    }
}

async fn read_vmo(
    State(state): State<Arc<NodeState>>,
    Json(req): Json<VmoReadReq>,
) -> impl IntoResponse {
    let registry = state.vmo_registry.lock().await;
    if let Some(vmo) = registry.get(&req.handle) {
        let mut buf = vec![0u8; req.size];
        match vmo.read_into(req.offset, &mut buf) {
            Ok(_) => Json(buf).into_response(),
            Err(e) => (StatusCode::BAD_REQUEST, format!("Read Error: {:?}", e)).into_response(),
        }
    } else {
        (StatusCode::NOT_FOUND, "VMO handle not found").into_response()
    }
}

async fn calculate_entropy(
    State(state): State<Arc<NodeState>>,
    Json(req): Json<VmoEntropyReq>,
) -> impl IntoResponse {
    if req.handle == 0 {
        return (StatusCode::OK, Json(VmoEntropyRes { entropy: 0.0 })).into_response();
    }
    let registry = state.vmo_registry.lock().await;
    if registry.contains_key(&req.handle) {
        let entropy = nexus_substrate::kernel::calculate_entropy(req.handle);
        (StatusCode::OK, Json(VmoEntropyRes { entropy })).into_response()
    } else {
        (StatusCode::NOT_FOUND, "VMO handle not found").into_response()
    }
}

async fn synthesize_wasm(Json(req): Json<ForgeReq>) -> impl IntoResponse {
    match WasmForge::synthesize_cell(&req.code).await {
        Ok(b64) => Json(ForgeRes { ok: true, b64: Some(b64), error: None }).into_response(),
        Err(e) => {
            Json(ForgeRes { ok: false, b64: None, error: Some(e.to_string()) }).into_response()
        }
    }
}

async fn execute_wasm(Json(req): Json<ExecuteReq>) -> impl IntoResponse {
    let bytes = match base64::engine::general_purpose::STANDARD.decode(&req.wasm_b64) {
        Ok(b) => b,
        Err(e) => {
            return Json(ExecuteRes { ok: false, result: None, error: Some(e.to_string()) })
                .into_response()
        }
    };
    match WasmRuntime::run_bytes(&bytes, &req.func_name, req.input) {
        Ok(res) => Json(ExecuteRes { ok: true, result: Some(res), error: None }).into_response(),
        Err(e) => {
            Json(ExecuteRes { ok: false, result: None, error: Some(e.to_string()) }).into_response()
        }
    }
}

// ⚡ PHASE 3: The magic endpoint that receives LLM actions from the Duet and executes them locally on the Node's filesystem.
async fn execute_physical(Json(action): Json<AgentAction>) -> impl IntoResponse {
    // ⚡ FIX: ToolExecutor was deleted during the Executor Unification pass.
    // As `nexus_node` acts as a remote satellite without a true Memory Palace
    // or VmoState right now, we spin up a localized RoboticExecutor instance.
    let (tx, _) = tokio::sync::broadcast::channel(1);
    let mut executor = nexus_runtime_core::executor::RoboticExecutor::new(
        String::new(), // Edge nodes don't need the API key
        tx,
    );

    // Mock up the substrate dependencies for the edge node
    let memory = Arc::new(nexus_runtime_core::memory_cortex::MemoryCortex::new(Arc::new(
        nexus_substrate::memory::MemoryPalace::new(":memory:").await.unwrap(),
    )));
    let vmo_state = Arc::new(Mutex::new(nexus_runtime_core::shared_state::VmoState::default()));
    let dist = Arc::new(nexus_runtime_core::distributed_scheduler::DistributedScheduler::new(
        Arc::new(nexus_runtime_core::cluster::ClusterMesh::new()),
    ));

    let motor_action = match action {
        nexus_runtime_core::agent_protocol::AgentAction::ExecuteCommand { command, .. } => {
            nexus_runtime_core::motor_cortex::MotorAction::ExecuteCommand { command }
        }
        nexus_runtime_core::agent_protocol::AgentAction::WriteFile { path, content, .. } => {
            nexus_runtime_core::motor_cortex::MotorAction::WriteFile { path, content }
        }
        nexus_runtime_core::agent_protocol::AgentAction::ReadFile { path, .. } => {
            nexus_runtime_core::motor_cortex::MotorAction::ReadFile { path }
        }
        nexus_runtime_core::agent_protocol::AgentAction::ReadFileChunk {
            path,
            start_line,
            max_lines,
            ..
        } => nexus_runtime_core::motor_cortex::MotorAction::ReadFileChunk {
            path,
            start_line,
            max_lines,
        },
        nexus_runtime_core::agent_protocol::AgentAction::SearchCode { path, query, .. } => {
            nexus_runtime_core::motor_cortex::MotorAction::SearchCode { path, query }
        }
        nexus_runtime_core::agent_protocol::AgentAction::DiffPatch {
            path,
            search,
            replace,
            ..
        } => nexus_runtime_core::motor_cortex::MotorAction::DiffPatch { path, search, replace },
        nexus_runtime_core::agent_protocol::AgentAction::PatchFunction {
            path,
            function_name,
            new_body,
            ..
        } => nexus_runtime_core::motor_cortex::MotorAction::PatchFunction {
            path,
            function_name,
            new_body,
        },
        nexus_runtime_core::agent_protocol::AgentAction::SearchMemory { query, .. } => {
            nexus_runtime_core::motor_cortex::MotorAction::SearchMemory { query }
        }
        nexus_runtime_core::agent_protocol::AgentAction::Halt { reasoning } => {
            nexus_runtime_core::motor_cortex::MotorAction::Halt { reasoning }
        }
        _ => nexus_runtime_core::motor_cortex::MotorAction::Halt {
            reasoning: "unmapped".to_string(),
        },
    };
    let motor_cmd = nexus_runtime_core::motor_cortex::MotorCommand {
        command_id: uuid::Uuid::new_v4(),
        intent_id: uuid::Uuid::new_v4(),
        goal_id: None,
        action: motor_action,
        capability: nexus_runtime_core::policy::Capability::ExecuteSystemCall, // Human override
        expected_outcome: "CLI_DIRECT_INVOCATION".to_string(),
        created_at: 0,
    };
    match executor.execute_motor_command(motor_cmd, memory, vmo_state, dist).await {
        Ok(obs) => (StatusCode::OK, obs).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("ACTION_FAILED: {}", e)).into_response()
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let state = Arc::new(NodeState { vmo_registry: Mutex::new(HashMap::new()) });

    let app = Router::new()
        .route("/api/vmo/create", post(create_vmo))
        .route("/api/vmo/write", post(write_vmo))
        .route("/api/vmo/read", post(read_vmo))
        .route("/api/vmo/entropy", post(calculate_entropy))
        .route("/api/action/physical", post(execute_physical))
        .route("/api/forge/synthesize", post(synthesize_wasm))
        .route("/api/forge/execute", post(execute_wasm))
        .route("/", get(|| async { "Sovereign Muscle Node Online" }))
        .layer(axum::middleware::from_fn(auth_guard))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    tracing::info!(">> [NODE] Sovereign Muscle active on port 8080");
    axum::serve(listener, app).await.unwrap();
}
