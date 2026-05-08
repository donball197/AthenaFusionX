#!/usr/bin/env bash
set -euo pipefail
echo ""
echo -e "\x1b[35m╔══════════════════════════════════════════════════════════╗\x1b[0m"
echo -e "\x1b[35m║   NEXUS SINGULARITY v3 — BULLETPROOF UPGRADE SEQUENCE    ║\x1b[0m"
echo -e "\x1b[35m╚══════════════════════════════════════════════════════════╝\x1b[0m"
echo ""

echo "[ 1/13 ] factory.rs — fixing dead model-selection branches..."
cat > crates/runtime_core/src/factory.rs << 'RUST_EOF'
use std::time::{SystemTime, UNIX_EPOCH};

pub struct AgentFactory;

impl AgentFactory {
    pub fn get_model_for_turn(turn: usize, current_error_count: usize) -> &'static str {
        if turn >= 8 || current_error_count > 1 {
            "gemma-4-26b-a4b-it"
        } else {
            "gemma-4-31b-a12b-it"
        }
    }

    pub fn select_model_for_mission(mission_type: &str) -> &'static str {
        match mission_type {
            "heavy_reasoning" | "refactor" | "security" | "dream" => "gemma-4-26b-a4b-it",
            "docs" | "qa" | "shell" | "devops" => "gemma-4-31b-a12b-it",
            _ => "gemma-4-31b-a12b-it",
        }
    }

    pub fn get_persona_for_mission(mission_type: &str) -> String {
        match mission_type {
            "security" => "IDENTITY: NEXUS Security Auditor. ROLE: Identify logic flaws, \
                path traversal, unsafe unwraps, and auth gaps. \
                OUTPUT: ONLY a valid JSON array. No prose.".to_string(),

            "docs" => "IDENTITY: NEXUS Archivist. ROLE: Generate technical markdown, \
                README files, and architecture maps. \
                OUTPUT: ONLY a valid JSON array of write_file actions.".to_string(),

            "qa" => "IDENTITY: NEXUS QA Agent. ROLE: Ensure 100% test coverage and zero \
                compiler warnings. WORKFLOW: cargo check → cargo test → fix failures → halt. \
                OUTPUT: ONLY a valid JSON array.".to_string(),

            "dream" => "IDENTITY: NEXUS Archivist. ROLE: Synthesize memory strands into \
                architectural insights. OUTPUT: Plain text summary — no JSON required.".to_string(),

            _ => "IDENTITY: NEXUS Lead DevOps. ROLE: Write high-performance Rust code and \
                manage system operations. OUTPUT: ONLY a valid JSON array. No prose.".to_string(),
        }
    }

    pub fn certify_product(title: &str, client: &str, content: &str) -> String {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        format!("--- NEXUS DELIVERABLE ---\nCLIENT: {}\nTITLE: {}\nSTAMP: VERIFIED_AGENT_DNA\nTS: {}\n\n{}", client, title, ts, content)
    }
}
RUST_EOF
echo "    ✔ factory.rs"

echo "[ 2/13 ] conductor.rs — fixing dead failover + real model escalation..."
cat > crates/runtime_core/src/conductor.rs << 'RUST_EOF'
use crate::factory::AgentFactory;
use crate::metrics::METRICS;
use crate::neural_agent::ContextPacket;
use crate::orchestrator::Orchestrator;
use crate::router::AgentProfile;
use nexus_substrate::token_ledger::wait_for_quota;
use reqwest::Client;
use serde_json::json;
use std::sync::atomic::Ordering;
use tokio::sync::broadcast;

const ESTIMATED_TOKENS: u32 = 2_000;

pub struct Conductor;

impl Conductor {
    pub async fn run_turn(
        profile: &AgentProfile,
        prompt: String,
        turn: usize,
        error_count: usize,
        orch: &Orchestrator,
        client: &Client,
        api_key: &str,
        tx: Option<broadcast::Sender<String>>,
    ) -> Result<String, String> {
        METRICS.api_calls.fetch_add(1, Ordering::Relaxed);
        let model = AgentFactory::get_model_for_turn(turn, error_count);

        if let Err(e) = wait_for_quota(model, ESTIMATED_TOKENS).await {
            return Err(format!("[CONDUCTOR] Quota exhausted for {}: {}", model, e));
        }

        let ctx = ContextPacket {
            system_prompt: Some(profile.persona.clone()),
            model_override: Some(model.to_string()),
            ..Default::default()
        };

        if let Some(ref sender) = tx {
            let _ = sender.send(json!({
                "type": "NEURAL_THOUGHT",
                "payload": format!("[CONDUCTOR] turn {} | errors {} → {}", turn, error_count, model)
            }).to_string());
        }

        let result = orch.run(prompt.clone(), ctx, client, api_key, tx.clone()).await;

        if let Err(ref e) = result {
            let is_retriable = e.contains("STALE") || e.contains("500") || e.contains("RESOURCE_EXHAUSTED") || e.contains("INTERNAL") || e.contains("CONTEXT_OVERFLOW");
            if is_retriable {
                METRICS.task_failures.fetch_add(1, Ordering::Relaxed);
                let failover = "gemma-4-26b-a4b-it";
                if failover == model { return result; }
                
                if let Err(e) = wait_for_quota(failover, ESTIMATED_TOKENS).await {
                    return Err(format!("[CONDUCTOR] Failover quota exhausted: {}", e));
                }

                let fail_ctx = ContextPacket {
                    system_prompt: Some(profile.persona.clone()),
                    model_override: Some(failover.to_string()),
                    ..Default::default()
                };
                return orch.run(prompt, fail_ctx, client, api_key, tx).await;
            }
        }
        result
    }
}
RUST_EOF
echo "    ✔ conductor.rs"

echo "[ 3/13 ] token_ledger.rs — propagating quota errors + adding 31b tier..."
cat > crates/nexus_substrate/src/token_ledger.rs << 'RUST_EOF'
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

#[derive(Debug, Clone)]
pub struct ModelQuota { pub rpm_limit: u32, pub tpm_limit: u32 }

impl ModelQuota {
    fn for_model(model: &str) -> Self {
        match model {
            m if m.contains("flash") => Self { rpm_limit: 4, tpm_limit: 240_000 },
            m if m.contains("31b")   => Self { rpm_limit: 14, tpm_limit: 100_000 },
            m if m.contains("26b")   => Self { rpm_limit: 14, tpm_limit: 100_000 },
            m if m.contains("27b")   => Self { rpm_limit: 28, tpm_limit: 14_000  },
            _                        => Self { rpm_limit: 10, tpm_limit: 50_000  },
        }
    }
}

#[derive(Debug)]
struct WindowEntry { timestamp: Instant, tokens: u32 }

#[derive(Debug, Default)]
struct ModelWindow { entries: Vec<WindowEntry> }

impl ModelWindow {
    fn purge_stale(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(60);
        self.entries.retain(|e| e.timestamp > cutoff);
    }
    fn token_sum(&self) -> u32 { self.entries.iter().map(|e| e.tokens).sum() }
    fn ms_until_slot_free(&self) -> u64 {
        if self.entries.is_empty() { return 0; }
        let oldest = self.entries.iter().map(|e| e.timestamp).min().unwrap();
        (Duration::from_secs(60).saturating_sub(oldest.elapsed())).as_millis() as u64 + 100
    }
}

pub struct TokenLedger { windows: Mutex<HashMap<String, ModelWindow>> }

impl TokenLedger {
    pub fn new() -> Self { Self { windows: Mutex::new(HashMap::new()) } }
    pub fn check_and_reserve(&self, model: &str, estimated_tokens: u32) -> Result<(), u64> {
        let quota = ModelQuota::for_model(model);
        let mut map = self.windows.lock().unwrap();
        let window = map.entry(model.to_string()).or_default();
        window.purge_stale();
        if window.entries.len() as u32 >= quota.rpm_limit { return Err(window.ms_until_slot_free()); }
        if window.token_sum() + estimated_tokens > quota.tpm_limit { return Err(window.ms_until_slot_free()); }
        window.entries.push(WindowEntry { timestamp: Instant::now(), tokens: estimated_tokens });
        Ok(())
    }
}
impl Default for TokenLedger { fn default() -> Self { Self::new() } }

pub static LEDGER: Lazy<TokenLedger> = Lazy::new(TokenLedger::new);

pub async fn wait_for_quota(model: &str, estimated_tokens: u32) -> Result<(), String> {
    const MAX_ATTEMPTS: usize = 10;
    for attempt in 0..MAX_ATTEMPTS {
        match LEDGER.check_and_reserve(model, estimated_tokens) {
            Ok(()) => return Ok(()),
            Err(wait_ms) => {
                let jitter = (generate_seed() % 500) as u64;
                tokio::time::sleep(Duration::from_millis(wait_ms + jitter)).await;
            }
        }
    }
    Err(format!("[LEDGER] Quota for '{}' unavailable after {} attempts. Aborting API call.", model, MAX_ATTEMPTS))
}

fn http_rpm_limit() -> u32 { std::env::var("NEXUS_HTTP_RPM").ok().and_then(|v| v.parse().ok()).unwrap_or(60) }

static HTTP_WINDOWS: Lazy<Mutex<HashMap<String, Vec<Instant>>>> = Lazy::new(|| Mutex::new(HashMap::new()));
pub fn http_check(ip: &str) -> Result<(), u64> {
    let limit  = http_rpm_limit();
    let cutoff = Instant::now() - Duration::from_secs(60);
    let mut map    = HTTP_WINDOWS.lock().unwrap();
    let window     = map.entry(ip.to_string()).or_default();
    window.retain(|&ts| ts > cutoff);
    if window.len() as u32 >= limit {
        let oldest = *window.first().unwrap();
        let wait   = (Duration::from_secs(60).saturating_sub(oldest.elapsed())).as_millis() as u64;
        return Err(wait + 100);
    }
    window.push(Instant::now());
    Ok(())
}

fn generate_seed() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let t = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(t);
    h.finish()
}
RUST_EOF
echo "    ✔ token_ledger.rs"

echo "[ 4/13 ] orchestrator_v2/task_graph.rs — replacing broken sort with Kahn's..."
cat > crates/runtime_core/src/orchestrator_v2/task_graph.rs << 'RUST_EOF'
use std::collections::{HashMap, HashSet, VecDeque};
use anyhow::{anyhow, Result};

#[derive(Clone)]
pub struct TaskNode {
    pub id:             String,
    pub deps:           Vec<String>,
    pub required_agent: String,
}

pub struct TaskGraph { pub nodes: HashMap<String, TaskNode> }

impl TaskGraph {
    pub fn new() -> Self { Self { nodes: HashMap::new() } }
    pub fn add_task(&mut self, id: &str, deps: Vec<String>, required_agent: &str) {
        self.nodes.insert(id.to_string(), TaskNode { id: id.to_string(), deps, required_agent: required_agent.to_string() });
    }

    pub fn resolve_execution_order(&self) -> Result<Vec<String>> {
        for node in self.nodes.values() {
            for dep in &node.deps {
                if !self.nodes.contains_key(dep.as_str()) { return Err(anyhow!("DAG_ERROR: Task '{}' depends on unknown task '{}'.", node.id, dep)); }
            }
        }

        let mut in_degree: HashMap<&str, usize> = self.nodes.keys().map(|k| (k.as_str(), 0)).collect();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

        for node in self.nodes.values() {
            for dep in &node.deps {
                dependents.entry(dep.as_str()).or_default().push(node.id.as_str());
                *in_degree.entry(node.id.as_str()).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<&str> = in_degree.iter().filter(|(_, &d)| d == 0).map(|(&id, _)| id).collect();
        let mut order = Vec::with_capacity(self.nodes.len());

        while let Some(id) = queue.pop_front() {
            order.push(id.to_string());
            if let Some(kids) = dependents.get(id) {
                for &child in kids {
                    let deg = in_degree.get_mut(child).unwrap();
                    *deg -= 1;
                    if *deg == 0 { queue.push_back(child); }
                }
            }
        }

        if order.len() != self.nodes.len() {
            let resolved: HashSet<_> = order.iter().map(|s| s.as_str()).collect();
            let cycle_nodes: Vec<_> = self.nodes.keys().filter(|k| !resolved.contains(k.as_str())).cloned().collect();
            return Err(anyhow!("DAG_DEADLOCK: Circular dependency detected among: {:?}", cycle_nodes));
        }
        Ok(order)
    }
}
RUST_EOF
echo "    ✔ orchestrator_v2/task_graph.rs"

echo "[ 5/13 ] task_manager.rs — history cap + stub hallucination guard..."
cat > crates/runtime_core/src/task_manager.rs << 'RUST_EOF'
use nexus_substrate::memory::MemoryPalace;
use crate::{
    agent_protocol::AgentAction, cli_utils, json_parser, logic_engine::LogicEngine,
    metrics::METRICS, orchestrator::Orchestrator, executor::RoboticExecutor,
    shared_state::VmoState, distributed_scheduler::DistributedScheduler,
};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, error, warn};

fn max_history() -> usize { std::env::var("NEXUS_MAX_HISTORY").ok().and_then(|v| v.parse().ok()).unwrap_or(4) }
fn max_fractures() -> usize { std::env::var("NEXUS_MAX_FRACTURES").ok().and_then(|v| v.parse().ok()).unwrap_or(3) }
fn max_turns() -> usize { std::env::var("NEXUS_MAX_TURNS").ok().and_then(|v| v.parse().ok()).unwrap_or(12) }

pub async fn send_task(
    orchestrator: Arc<Orchestrator>,
    client: &Client,
    api_key: &str,
    raw_directive: &str,
    memory: Arc<Mutex<MemoryPalace>>,
    vmo_state: Arc<Mutex<VmoState>>,
    tx: broadcast::Sender<String>,
    dist: Arc<DistributedScheduler>,
) -> String {
    METRICS.inc_task();
    let mut session_obs: Vec<String> = vec![];
    let mut history: Vec<(String, String)> = vec![];
    let mut turn = 0usize;
    let mut consecutive_fractures = 0usize;
    let mut error_count = 0usize;

    let execution_law = cli_utils::get_execution_law_v6().await;
    let mut executor  = RoboticExecutor::new(api_key.to_string(), tx.clone());
    let cap = max_history();

    'mission: while turn < max_turns() {
        turn += 1;
        info!("=== Turn {} / {} ===", turn, max_turns());

        if LogicEngine::detect_deadlock(&session_obs) { break 'mission; }
        if consecutive_fractures >= max_fractures() { break 'mission; }

        let window_start = history.len().saturating_sub(cap);
        let mut full_prompt = String::new();
        for (q, a) in &history[window_start..] { full_prompt.push_str(&format!("USER: {}\nMODEL: {}\n", q, a)); }

        let turn_prompt = if turn == 1 { raw_directive.to_string() } else {
            let last_obs = session_obs.last().cloned().unwrap_or_else(|| "SUCCESS".into());
            format!("The last action resulted in: {}. What is the next logical step?", last_obs)
        };
        full_prompt.push_str(&format!("USER: {}", turn_prompt));

        let ctx = crate::neural_agent::ContextPacket {
            system_prompt: Some(execution_law.clone()),
            recent_logs: session_obs.iter().rev().take(3).rev().cloned().collect(),
            ..Default::default()
        };

        let response = match orchestrator.run(full_prompt, ctx, client, api_key, Some(tx.clone())).await {
            Ok(resp) => resp,
            Err(e)   => { error_count += 1; break 'mission; }
        };

        match json_parser::extract_actions(&response) {
            Ok(actions) => {
                if actions.is_empty() { break 'mission; }
                history.push((turn_prompt, response.clone()));
                if history.len() > cap { history.drain(0..history.len() - cap); }
                consecutive_fractures = 0;

                match executor.execute_handle(actions.clone(), memory.clone(), vmo_state.clone(), dist.clone()).await {
                    Ok(observations) => {
                        for obs in observations {
                            let real_obs = if obs.contains("[STUB_OK]") {
                                "[STUB] This action is not yet implemented. No data was returned.".to_string()
                            } else { obs };
                            session_obs.push(real_obs);
                        }
                    }
                    Err(e) => { error_count += 1; session_obs.push(format!("EXECUTOR_FAULT: {}", e)); }
                }
                if actions.iter().any(|a| matches!(a, AgentAction::Halt { .. })) { break 'mission; }
            }
            Err(_) => { consecutive_fractures += 1; error_count += 1; session_obs.push("[FORMAT_VIOLATION] Output ONLY a raw JSON array.".into()); }
        }
    }
    "TASK_COMPLETE".to_string()
}
RUST_EOF
echo "    ✔ task_manager.rs"

echo "[ 6/13 ] muscle.zig — fixing refcount leak + removing global shard lock..."
cat > crates/nexus_substrate/src/kernel/muscle.zig << 'ZIG_EOF'
const std = @import("std");
const posix = std.posix;
const Thread = std.Thread;

const PAGE_SIZE: usize = 4096;
const MAX_SLOTS: usize = 4096;
const SLOT_MASK: u32 = MAX_SLOTS - 1;
const SENTINEL_MAGIC: u32 = 0xDEADC0DE;
const GENERATION_SHIFT: u5 = 12;
const SHARD_COUNT: usize = 16;
const SHARD_MASK: u32 = SHARD_COUNT - 1;
const SLOTS_PER_SHARD: usize = MAX_SLOTS / SHARD_COUNT;

var shard_mutexes: [SHARD_COUNT]Thread.Mutex = [_]Thread.Mutex{.{}} ** SHARD_COUNT;
var active_vmos: u32 = 0;
var next_handle: u32 = 0xc8;

const VmoMetadata = struct { ptr: [*]align(PAGE_SIZE) u8, size: usize, magic: u32, generation: u32, refcount: u32, is_cow_child: bool, parent_handle: u32 };
const VmoInfo = extern struct { handle: u32, size: usize, committed_pages: usize, is_cow: u8, parent_handle: u32 };

var handle_table: [MAX_SLOTS]?VmoMetadata = [_]?VmoMetadata{null} ** MAX_SLOTS;
var current_load: f32 = 0.0;
var stress_mode:  bool = false;

inline fn shard_for(h: u32) usize      { return @intCast(h & SHARD_MASK); }
inline fn decode_slot(h: u32) usize    { return @intCast(h & SLOT_MASK); }
inline fn decode_generation(h: u32) u32 { return h >> GENERATION_SHIFT; }

fn lookup_valid(h: u32) ?*VmoMetadata {
    const slot = decode_slot(h);
    const gen  = decode_generation(h);
    const meta = &(handle_table[slot] orelse return null);
    if (meta.magic     != SENTINEL_MAGIC) return null;
    if (meta.generation != gen)           return null;
    return meta;
}

fn allocate_vmo(size: usize) !struct { ptr: [*]align(PAGE_SIZE) u8, size: usize } {
    const aligned = std.mem.alignForward(usize, size, PAGE_SIZE);
    const ptr = try posix.mmap(null, aligned, posix.PROT.READ | posix.PROT.WRITE, .{ .TYPE = .PRIVATE, .ANONYMOUS = true }, -1, 0);
    return .{ .ptr = @alignCast(ptr.ptr), .size = aligned };
}

export fn zx_vmo_create_native(size: usize) u32 {
    const mapping = allocate_vmo(size) catch return 0;
    const h     = @atomicRmw(u32, &next_handle, .Add, 1, .monotonic);
    const shard = shard_for(h);
    const slot  = decode_slot(h);
    const gen   = decode_generation(h);

    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();

    handle_table[slot] = VmoMetadata{ .ptr = mapping.ptr, .size = mapping.size, .magic = SENTINEL_MAGIC, .generation = gen, .refcount = 1, .is_cow_child = false, .parent_handle = 0 };
    _ = @atomicRmw(u32, &active_vmos, .Add, 1, .monotonic);
    return h;
}

export fn zx_vmo_clone_native(parent_handle: u32, _: usize) u32 {
    const p_shard = shard_for(parent_handle);
    var parent_size: usize = 0;
    var parent_ptr:  [*]align(PAGE_SIZE) u8 = undefined;
    {
        shard_mutexes[p_shard].lock();
        defer shard_mutexes[p_shard].unlock();
        const meta = lookup_valid(parent_handle) orelse return 0;
        parent_size = meta.size;
        parent_ptr  = meta.ptr;
    }

    const mapping = allocate_vmo(parent_size) catch return 0;
    @memcpy(mapping.ptr[0..parent_size], parent_ptr[0..parent_size]);

    const h     = @atomicRmw(u32, &next_handle, .Add, 1, .monotonic);
    const shard = shard_for(h);
    const slot  = decode_slot(h);
    const gen   = decode_generation(h);

    shard_mutexes[p_shard].lock();
    const parent_still_valid = lookup_valid(parent_handle) != null;
    if (parent_still_valid) { _ = @atomicRmw(u32, &(lookup_valid(parent_handle).?.refcount), .Add, 1, .monotonic); }
    shard_mutexes[p_shard].unlock();

    if (!parent_still_valid) {
        posix.munmap(mapping.ptr[0..mapping.size]);
        return 0;
    }

    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();

    handle_table[slot] = VmoMetadata{ .ptr = mapping.ptr, .size = mapping.size, .magic = SENTINEL_MAGIC, .generation = gen, .refcount = 1, .is_cow_child = true, .parent_handle = parent_handle };
    _ = @atomicRmw(u32, &active_vmos, .Add, 1, .monotonic);
    return h;
}

export fn zx_handle_close_native(h: u32) void {
    const shard = shard_for(h);
    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();
    const meta = lookup_valid(h) orelse return;
    const old_ref = @atomicRmw(u32, &meta.refcount, .Sub, 1, .monotonic);
    if (old_ref == 1) {
        posix.munmap(meta.ptr[0..meta.size]);
        handle_table[decode_slot(h)] = null;
        _ = @atomicRmw(u32, &active_vmos, .Sub, 1, .monotonic);
    }
}

export fn zx_vmo_write_native(h: u32, offset: usize, buffer: [*]const u8, size: usize) i32 {
    const shard = shard_for(h);
    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();
    const meta = lookup_valid(h) orelse return -1;
    if (offset + size > meta.size) return -1;
    @memcpy(meta.ptr[offset .. offset + size], buffer[0..size]);
    return 0;
}

export fn zx_vmo_read_native(h: u32, offset: usize, buffer: [*]u8, size: usize) i32 {
    const shard = shard_for(h);
    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();
    const meta = lookup_valid(h) orelse return -1;
    if (offset + size > meta.size) return -1;
    @memcpy(buffer[0..size], meta.ptr[offset .. offset + size]);
    return 0;
}

export fn zx_vmo_get_info(h: u32, out_info: *VmoInfo) i32 {
    const shard = shard_for(h);
    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();
    const meta = lookup_valid(h) orelse return -1;
    out_info.* = .{ .handle = h, .size = meta.size, .committed_pages = meta.size / PAGE_SIZE, .is_cow = if (meta.is_cow_child) 1 else 0, .parent_handle = meta.parent_handle };
    return 0;
}

export fn zx_vmo_calculate_entropy(h: u32) f32 {
    const shard = shard_for(h);
    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();
    const meta = lookup_valid(h) orelse return 0.0;
    var counts = [_]u32{0} ** 256;
    for (meta.ptr[0..meta.size]) |byte| counts[byte] += 1;
    var entropy: f32 = 0.0;
    const size_f: f32 = @floatFromInt(meta.size);
    for (counts) |count| {
        if (count > 0) {
            const p = @as(f32, @floatFromInt(count)) / size_f;
            entropy -= p * std.math.log2(p);
        }
    }
    return entropy / 8.0;
}

export fn zx_vmo_get_all_handles(out_handles: [*]u32, max_count: usize) usize {
    var count: usize = 0;
    var s: usize = 0;
    while (s < SHARD_COUNT and count < max_count) : (s += 1) {
        shard_mutexes[s].lock();
        defer shard_mutexes[s].unlock();
        const base = s * SLOTS_PER_SHARD;
        var i: usize = base;
        while (i < base + SLOTS_PER_SHARD and count < max_count) : (i += 1) {
            if (handle_table[i]) |meta| {
                if (meta.magic == SENTINEL_MAGIC) {
                    const slot: u32 = @intCast(i);
                    out_handles[count] = slot | (meta.generation << GENERATION_SHIFT);
                    count += 1;
                }
            }
        }
    }
    return count;
}

export fn muscle_get_active_vmos() u32 { return @atomicLoad(u32, &active_vmos, .monotonic); }
export fn muscle_get_next_handle() u32 { return @atomicLoad(u32, &next_handle, .monotonic); }
export fn set_substrate_load(load: f32) void { shard_mutexes[0].lock(); defer shard_mutexes[0].unlock(); current_load = @max(0.0, @min(1.0, load)); }
export fn set_stress_mode(enabled: bool) void { shard_mutexes[0].lock(); defer shard_mutexes[0].unlock(); stress_mode = enabled; }
export fn extract_thermal_pulse_raw() i32 {
    shard_mutexes[0].lock();
    defer shard_mutexes[0].unlock();
    const multiplier: f32 = if (stress_mode) 2.5 else 1.0;
    const temp = (current_load * 40.0 * multiplier) + 30.0;
    return @intFromFloat(temp * 1000.0);
}
ZIG_EOF
echo "    ✔ muscle.zig"

echo "[ 7/13 ] system_monitor.rs — wiring up sysinfo..."
cat > bins/ide_backend/src/system_monitor.rs << 'RUST_EOF'
use sysinfo::{Components, System};

pub struct Vitals {
    pub cpu_load:     f32,
    pub ram_gb:       f32,
    pub total_ram_gb: f32,
    pub temperature:  f32,
}

pub struct SystemMonitor {
    sys:        System,
    components: Components,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self { sys, components: Components::new_with_refreshed_list() }
    }

    pub fn get_vitals(&mut self) -> Vitals {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.components.refresh(true);

        let cpu_load     = self.sys.global_cpu_usage();
        let ram_gb       = self.sys.used_memory()  as f32 / 1_073_741_824.0;
        let total_ram_gb = self.sys.total_memory() as f32 / 1_073_741_824.0;

        let temps: Vec<f32> = self.components.iter().filter_map(|c| c.temperature()).collect();
        let temperature = if temps.is_empty() { 0.0 } else { temps.iter().sum::<f32>() / temps.len() as f32 };

        Vitals { cpu_load, ram_gb, total_ram_gb, temperature }
    }
}

impl Default for SystemMonitor { fn default() -> Self { Self::new() } }
RUST_EOF
echo "    ✔ system_monitor.rs"

echo "[ 8/13 ] auth.rs — warning on malformed TTL env var..."
cat > bins/ide_backend/src/auth.rs << 'RUST_EOF'
use anyhow::{anyhow, Result};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const MIN_KEY_BYTES: usize  = 32;
const DEFAULT_TTL_SECS: u64 = 86_400;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims { pub sub: String, pub exp: usize, pub iat: usize }

fn load_key() -> Result<String> {
    let key = std::env::var("NEXUS_AUTH_KEY").map_err(|_| anyhow!("NEXUS_AUTH_KEY env var is not set"))?;
    if key.len() < MIN_KEY_BYTES { return Err(anyhow!("NEXUS_AUTH_KEY is too short ({} bytes). Minimum is {} bytes.", key.len(), MIN_KEY_BYTES)); }
    Ok(key)
}

fn token_ttl() -> u64 {
    match std::env::var("NEXUS_TOKEN_TTL_SECS") {
        Ok(v) => match v.parse::<u64>() {
            Ok(secs) => secs,
            Err(_) => { tracing::warn!("[AUTH] NEXUS_TOKEN_TTL_SECS='{}' is not a valid u64. Using default {}s. Fix your .env file.", v, DEFAULT_TTL_SECS); DEFAULT_TTL_SECS }
        },
        Err(_) => DEFAULT_TTL_SECS,
    }
}

pub fn issue_sovereign_token() -> Result<String> {
    let key = load_key()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let claims = Claims { sub: "architect".to_owned(), iat: now as usize, exp: (now + token_ttl()) as usize };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(key.as_bytes())).map_err(|e| anyhow!("JWT_GEN_FAIL: {}", e))
}

#[derive(Debug, PartialEq)]
pub enum TokenVerdict { Valid, Expired, InvalidSignature, MissingKey }

pub fn verify_token_detailed(token: &str) -> Result<TokenVerdict> {
    let key = load_key()?;
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 0;
    match decode::<Claims>(token, &DecodingKey::from_secret(key.as_bytes()), &validation) {
        Ok(_) => Ok(TokenVerdict::Valid),
        Err(e) => Ok(match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => TokenVerdict::Expired,
            _ => TokenVerdict::InvalidSignature,
        }),
    }
}

pub fn verify_token(token: &str) -> Result<bool> {
    match verify_token_detailed(token)? {
        TokenVerdict::Valid => Ok(true),
        TokenVerdict::Expired => { tracing::warn!("[AUTH] Token rejected: expired"); Ok(false) }
        TokenVerdict::InvalidSignature => { tracing::warn!("[AUTH] Token rejected: invalid signature"); Ok(false) }
        TokenVerdict::MissingKey => Ok(false),
    }
}
RUST_EOF
echo "    ✔ auth.rs"

echo "[ 9/13 ] server.rs — auto-injecting auth guard to /api/fs/write..."
python3 -c '
import os, re
path = "bins/ide_backend/src/server.rs"
if os.path.exists(path):
    with open(path, "r") as f: data = f.read()
    pattern = re.compile(r"pub async fn start_server\(state: Arc<AppState>\)\s*\{.*?\n\}", re.DOTALL)
    suture = """pub async fn start_server(state: Arc<AppState>) {
    use axum::middleware::from_fn;
    use crate::security::sovereign_auth_guard;

    let authed = Router::new()
        .route("/api/task", post(unified_gateway))
        .route("/api/fs/write", post(fs_write_handler))
        .layer(from_fn(sovereign_auth_guard));

    let app = Router::new()
        .merge(authed)
        .route("/api/metrics", get(metrics_handler))
        .route("/api/fs/read", get(fs_read_handler))
        .route("/api/fs/list", get(fs_list_handler))
        .route("/api/vitals", get(crate::vitals_handler::handle_vitals))
        .route("/api/cluster/nodes", get(crate::cluster_api::list_nodes))
        .route("/ws/telemetry", get(crate::ws_bridge::ws_telemetry_handler))
        .fallback(static_handler)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("\\x1b[32m>> [SERVER] Listening on http://0.0.0.0:3000\\x1b[0m");
    axum::serve(listener, app).await.unwrap();
}"""
    if pattern.search(data):
        with open(path, "w") as f: f.write(pattern.sub(suture, data))
        print("    ✔ server.rs spliced successfully.")
    else:
        print("    [FAULT] Could not locate start_server in server.rs")
else:
    print("    [SKIP] server.rs not found.")
'

echo "[ 10/13 ] bins/cli/src/main.rs — removing unused import..."
cat > bins/cli/src/main.rs << 'RUST_EOF'
use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use nexus_runtime_core::{
    cluster::ClusterMesh,
    distributed_scheduler::DistributedScheduler,
    orchestrator::Orchestrator,
    shared_state::VmoState,
    task_manager::send_task,
};
use nexus_substrate::memory::MemoryPalace;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("nexus=info".parse().unwrap())).init();
    println!("\x1b[36m==================================================\x1b[0m");
    println!("\x1b[1;36m>> NEXUS SINGULARITY v3.0 — SOVEREIGN CLI ONLINE\x1b[0m");
    println!("\x1b[36m==================================================\x1b[0m");

    let api_key = std::env::var("GEMINI_API_KEY").expect("Fatal: GEMINI_API_KEY not found in .env file.");
    let (tx, mut rx) = broadcast::channel::<String>(256);
    let memory       = Arc::new(Mutex::new(MemoryPalace::new("nexus_cli.db").await?));
    let vmo_state    = Arc::new(Mutex::new(VmoState::default()));
    let cluster      = Arc::new(ClusterMesh::new());
    let dist         = Arc::new(DistributedScheduler::new(cluster));
    let orchestrator = Arc::new(Orchestrator::new());
    let client       = reqwest::Client::new();

    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&msg) {
                let msg_type = parsed["type"].as_str().unwrap_or("LOG");
                let payload  = parsed["payload"].as_str().unwrap_or("");
                match msg_type {
                    "NEURAL_THOUGHT" => println!("\n\x1b[35m[CORTEX]\x1b[0m {}", payload),
                    "TASK_STATUS"    => println!("\x1b[33m[SYS]\x1b[0m {}",    payload),
                    "LOG_EVENT"      => println!("\x1b[90m[EVENT]\x1b[0m {}",   payload),
                    _                => {}
                }
            } else {
                println!("\x1b[90m[RAW]\x1b[0m {}", msg);
            }
        }
    });

    println!("\x1b[32m>> Neural Uplink Established. Substrate Ready.\x1b[0m");
    println!("\x1b[90mType '/help' for commands, or enter a directive.\x1b[0m");
    let stdin = io::stdin();

    loop {
        print!("\n\x1b[1;32mnexus>\x1b[0m ");
        io::stdout().flush()?;
        let mut input = String::new();
        stdin.read_line(&mut input)?;
        let command = input.trim();
        if command.is_empty() { continue; }

        match command {
            "/exit" | "/quit" => { println!("\x1b[31m>> Severing Uplink. Goodbye.\x1b[0m"); break; }
            "/help" => {
                println!("\x1b[36mCommands:\x1b[0m");
                println!("  /status    — Active VMOs + RAM usage");
                println!("  /clear     — Purge SQLite Memory Palace");
                println!("  /exit      — Shutdown CLI");
                println!("  <text>     — Submit directive to the Neural Agent");
            }
            "/status" => {
                let active = unsafe { nexus_substrate::kernel::ffi::muscle_get_active_vmos() };
                println!("\x1b[36m[SUBSTRATE]\x1b[0m Active VMOs: {}", active);
                if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                    for line in status.lines() {
                        if line.starts_with("VmRSS:") || line.starts_with("VmPeak:") { println!("\x1b[36m[MEM]\x1b[0m {}", line.trim()); }
                    }
                }
            }
            "/clear" => {
                let mem = memory.lock().await;
                if let Err(e) = mem.purge_all().await { println!("\x1b[31m[ERROR]\x1b[0m Memory purge failed: {}", e); } 
                else { println!("\x1b[32m[MEMORY]\x1b[0m Neural context eradicated."); }
            }
            directive => {
                println!("\x1b[36m>> Engaging Neural Routing...\x1b[0m");
                let response = send_task(orchestrator.clone(), &client, &api_key, directive, memory.clone(), vmo_state.clone(), tx.clone(), dist.clone()).await;
                println!("\n\x1b[1;32m[MISSION COMPLETE]\x1b[0m {}", response);
            }
        }
    }
    Ok(())
}
RUST_EOF
echo "    ✔ bins/cli/src/main.rs"

echo "[ 11/13 ] logic_engine.rs — stub detection + hallucination risk flag..."
cat > crates/runtime_core/src/logic_engine.rs << 'RUST_EOF'
use std::collections::HashMap;
use tracing::warn;

pub struct LogicEngine;

impl LogicEngine {
    pub fn is_action_safe(directive: &str) -> bool {
        let forbidden = ["rm -rf /", "mkfs", "shutdown", "chmod 777", "dd if=/dev/zero", "> /dev/sda", "wget | bash", "curl | bash", "wget | sh", "curl | sh", "| bash", "| sh"];
        let d_lower = directive.to_lowercase();
        !forbidden.iter().any(|&p| d_lower.contains(p))
    }

    pub fn is_stub_response(obs: &str) -> bool { obs.contains("[STUB_OK]") || obs.contains("STUB_OK") }

    pub fn detect_hallucination_risk(agent_output: &str) -> Option<String> {
        let hallucination_phrases = ["successfully deployed", "has been fully integrated", "all targeted architectural goals", "all milestones have been met", "deployment was successful"];
        let lower = agent_output.to_lowercase();
        for phrase in hallucination_phrases {
            if lower.contains(phrase) { return Some(format!("[HALLUCINATION_RISK] Output contains presumptive success language: '{}'. Verify this is grounded in actual observation data, not fabricated.", phrase)); }
        }
        None
    }

    pub fn detect_deadlock(observations: &[String]) -> bool {
        if observations.len() < 3 { return false; }
        let mut read_counts: HashMap<String, usize> = HashMap::new();
        for obs in observations {
            if let Some(after) = obs.strip_prefix("[READ_OK] ") {
                let path = after.split_whitespace().next().unwrap_or("").to_string();
                if !path.is_empty() { *read_counts.entry(path).or_insert(0) += 1; }
            }
        }
        for (path, count) in &read_counts {
            if *count >= 3 {
                let file_progress = observations.iter().any(|obs| { (obs.contains("[WRITE_OK]") || obs.contains("[DIFF_PATCH_OK]") || obs.contains("[PATCH_OK]")) && obs.contains(path) });
                if !file_progress { warn!("[LOGIC] Localized deadlock: {} read {} times, zero write progress.", path, count); return true; }
            }
        }
        let mut last_cmd_sig  = String::new();
        let mut cmd_rep_count = 0usize;
        for obs in observations {
            if let Some(start) = obs.find("[CMD:") {
                let sig: String = obs[start..].lines().filter(|l| !l.contains("Finished") && !l.contains("Blocking waiting")).collect::<Vec<&str>>().join("\n");
                if sig == last_cmd_sig && !sig.is_empty() { cmd_rep_count += 1; } else { last_cmd_sig  = sig; cmd_rep_count = 1; }
            }
        }
        if cmd_rep_count >= 2 { warn!("[LOGIC] Deadlock: time-invariant command repetition detected."); return true; }
        false
    }
}
RUST_EOF
echo "    ✔ logic_engine.rs"

echo "[ 12/13 ] Cargo.toml — promoting shared deps to workspace to prevent drift..."
cat > Cargo.toml << 'TOML_EOF'
[workspace]
members = [
    "crates/runtime_core", "crates/identity", "crates/memory_graph",
    "crates/economy", "crates/protocol", "crates/mesh",
    "crates/payments", "crates/marketplace", "crates/billing",
    "bins/cli", "bins/node", "bins/ide_backend"
]
resolver = "2"

[workspace.package]
edition = "2021"
version = "3.0.0"

[workspace.dependencies]
tokio              = { version = "1.44", features = ["full", "io-util"] }
serde              = { version = "1.0",  features = ["derive"] }
serde_json         = "1.0"
anyhow             = "1.0"
reqwest            = { version = "0.12", features = ["json", "stream"] }
axum               = { version = "0.7",  features = ["ws", "macros"] }
sqlx               = { version = "0.8",  features = ["sqlite", "runtime-tokio", "tls-rustls"] }
tracing            = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
once_cell          = "1.19"
parking_lot        = "0.12"
async-trait        = "0.1"
base64             = "0.22"
uuid               = { version = "1.8", features = ["v4", "serde"] }
futures-util       = "0.3"
tower-http         = { version = "0.5", features = ["fs", "cors", "trace"] }
dotenvy            = "0.15"
bitflags           = "2.4"
jsonwebtoken       = "9.3"
sysinfo            = "0.31"
mime_guess         = "2.0"
rust-embed         = "8.0"

[profile.dev]
opt-level     = 0
debug         = true
incremental   = true
codegen-units = 256
panic         = "abort"

[profile.release]
opt-level     = 3
lto           = "thin"
codegen-units = 1
strip         = "symbols"
TOML_EOF
echo "    ✔ Cargo.toml"

echo "[ 12.5/13 ] Sweeping child Cargo.toml files for workspace inheritance..."
python3 -c '
import os, re
deps = ["tokio", "serde", "serde_json", "anyhow", "reqwest", "axum", "sqlx", "tracing", "tracing-subscriber", "once_cell", "parking_lot", "async-trait", "base64", "uuid", "futures-util", "tower-http", "dotenvy", "bitflags", "jsonwebtoken", "sysinfo", "mime_guess", "rust-embed"]

for root_dir in ["crates", "bins"]:
    if not os.path.exists(root_dir): continue
    for subdir in os.listdir(root_dir):
        toml_path = os.path.join(root_dir, subdir, "Cargo.toml")
        if os.path.exists(toml_path):
            with open(toml_path, "r") as f: content = f.read()
            new_content = content
            for dep in deps:
                pattern = re.compile(rf"^{dep}\s*=\s*([\"\x27][^\"\x27]+[\"\x27]|\{{(?:(?!\bworkspace\b).)*\}})", re.MULTILINE)
                new_content = pattern.sub(f"{dep} = {{ workspace = true }}", new_content)
            if new_content != content:
                with open(toml_path, "w") as f: f.write(new_content)
                print(f"    ✔ Migrated dependencies in {toml_path}")
'

echo "[ 13/13 ] Aggressive FFI / Target Cleanup..."
rm -rf target/
rm -f *.so
echo "    ✔ Linker caches purged."

echo ""
echo -e "\x1b[32m>> UPGRADE SEQUENCE COMPLETE.\x1b[0m"
