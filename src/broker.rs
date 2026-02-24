use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use serde::Serialize;

#[derive(Debug)]
pub struct Mission {
    pub path: PathBuf,
    pub objective: String,
}

#[derive(Serialize)]
pub struct GraphNode {
    pub id: String,
    pub group: String,
    pub summary: String,
    pub size: u64,
}

#[derive(Serialize)]
pub struct GraphLink {
    pub source: String,
    pub target: String,
}

#[derive(Serialize)]
pub struct NeuralGraph {
    pub nodes: Vec<GraphNode>,
    pub links: Vec<GraphLink>,
}

pub struct KnowledgeBroker;

impl KnowledgeBroker {
    pub async fn get_graph(state: Arc<crate::server::AppState>) -> NeuralGraph {
        let memories = state.memory.load_all();
        let mut nodes = Vec::new();
        let mut links = Vec::new();

        for mem in memories.iter().filter(|m| m.prompt.starts_with("FOUNDATION:")) {
            let path = mem.prompt.replace("FOUNDATION: ", "");
            let group = if path.ends_with(".rs") { "rust" } else { "svelte" };
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(500);

            nodes.push(GraphNode {
                id: path.clone(),
                group: group.to_string(),
                summary: mem.solution_code.clone(),
                size,
            });

            if path != "src/main.rs" && group == "rust" {
                links.push(GraphLink { source: "src/main.rs".into(), target: path.clone() });
            }
        }
        NeuralGraph { nodes, links }
    }

    pub async fn deep_index(state: Arc<crate::server::AppState>) -> anyhow::Result<String> {
        let mut entries = Vec::new();
        Self::read_src(Path::new("src"), &mut entries);
        Self::read_src(Path::new("ui/src"), &mut entries);

        for path in entries {
            let content = fs::read_to_string(&path)?;
            let (summary, _) = state.agent.prepare_and_run(
                &state.client, 
                &state.api_key, 
                &format!("Summarize purpose of {}", path), 
                crate::neural_agent::ContextPacket { file_content: Some(content), ..Default::default() }, 
                0.0
            ).await?;
            state.memory.remember(format!("FOUNDATION: {}", path), summary, "INDEXED".into(), None);
        }
        Ok("Deep Index Complete".into())
    }

    fn read_src(path: &Path, entries: &mut Vec<String>) {
        if let Ok(rd) = fs::read_dir(path) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let name = p.file_name().unwrap().to_string_lossy();
                    if name != "target" && name != "node_modules" && name != "dist" { 
                        Self::read_src(&p, entries); 
                    }
                } else if p.extension().map_or(false, |ext| ext == "rs" || ext == "svelte") {
                    entries.push(p.to_string_lossy().to_string());
                }
            }
        }
    }

    pub fn get_unclaimed_mission(node_id: &str) -> Option<Mission> {
        let missions = fs::read_dir("missions").ok()?;
        for entry in missions.flatten() {
            let path = entry.path();
            if path.is_dir() { continue; }
            let file_name = path.file_name()?.to_str()?;
            let claim_path = format!("claims/{}.lock", file_name);
            if !Path::new(&claim_path).exists() {
                if fs::write(&claim_path, node_id).is_ok() {
                    let objective = fs::read_to_string(&path).unwrap_or_default();
                    return Some(Mission { path, objective });
                }
            }
        }
        None
    }
}
