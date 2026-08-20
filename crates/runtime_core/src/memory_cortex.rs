use crate::memory_types::CognitiveMemory;
use std::sync::Arc;

use anyhow::Result;

use nexus_substrate::memory::{AgentMemory, MemoryPalace};

#[derive(Clone)]
pub struct MemoryCortex {
    palace: Arc<MemoryPalace>,
}

impl MemoryCortex {
    pub fn new(palace: Arc<MemoryPalace>) -> Self {
        Self { palace }
    }

    pub async fn store_memory(
        &self,
        category: &str,
        key: &str,
        value: &str,
        embedding: Option<Vec<f32>>,
    ) -> Result<()> {
        self.palace.store_memory(category, key, value, "Internal", embedding).await?;
        Ok(())
    }

    pub async fn search_memories(&self, query: &str) -> Result<Vec<AgentMemory>> {
        let result = self.palace.search_memories(query).await?;
        Ok(result)
    }

    pub async fn semantic_search(&self, embedding: &[f32]) -> Result<Vec<AgentMemory>> {
        let result = self.palace.semantic_search(embedding).await?;
        Ok(result)
    }

    pub async fn remember(&self, memory: CognitiveMemory) -> anyhow::Result<()> {
        self.palace
            .store_memory("cognitive", &memory.key, &memory.content, "Internal", None)
            .await?;
        Ok(())
    }

    pub async fn upsert_plan_run(
        &self,
        id: &str,
        objective: &str,
        graph: &str,
        status: &str,
        created_at: i64,
    ) -> anyhow::Result<()> {
        self.palace.upsert_plan_run(id, objective, graph, status, created_at).await?;
        Ok(())
    }

    pub async fn consolidate_memory(
        &self,
        memory: crate::memory_types::CognitiveMemory,
    ) -> anyhow::Result<bool> {
        let score = crate::memory_consolidation::MemoryConsolidator::calculate_retention(&memory);
        if score < 0.1 {
            return Ok(false);
        }
        self.remember(memory).await?;
        Ok(true)
    }

    pub async fn reflect_and_store(
        &self,
        memories: Vec<crate::memory_types::CognitiveMemory>,
    ) -> anyhow::Result<bool> {
        let engine = crate::reflection_engine::ReflectionEngine::new();
        if let Some(reflection) = engine.reflect(&memories) {
            self.remember(reflection).await?;
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn recall(
        &self,
        context: &str,
        limit: usize,
    ) -> anyhow::Result<crate::memory_recall::RecallResult> {
        let memories = self
            .search_memories(context)
            .await?
            .into_iter()
            .take(limit)
            .map(|m| crate::memory_types::CognitiveMemory {
                id: uuid::Uuid::new_v4(),
                kind: crate::memory_types::MemoryKind::Experience,
                key: m.directive.clone(),
                content: m.result.clone(),
                source: m.origin.clone(),
                confidence: 0.5,
                importance: 0.5,
                created_at: m
                    .timestamp
                    .parse::<i64>()
                    .unwrap_or_else(|_| chrono::Utc::now().timestamp()),
                tags: vec![m.origin.clone()],
            })
            .collect();
        let engine = crate::memory_recall::MemoryRecallEngine::new();
        Ok(engine.rank(memories, context))
    }

    pub async fn store_belief_update(
        &self,
        event: crate::belief_feedback::BeliefUpdateEvent,
    ) -> anyhow::Result<()> {
        let payload = serde_json::to_string(&event)?;
        self.store_memory("belief_update", &event.belief_id.to_string(), &payload, None).await?;
        Ok(())
    }

    pub async fn store_belief_decision(
        &self,
        decision: crate::belief_policy::BeliefDecision,
    ) -> anyhow::Result<()> {
        let payload = serde_json::to_string(&decision)?;
        self.store_memory("belief_decision", &decision.belief_id.to_string(), &payload, None)
            .await?;
        Ok(())
    }

    pub async fn store_goal_adaptation(
        &self,
        decision: crate::goal_adaptation::GoalAdaptationDecision,
    ) -> anyhow::Result<()> {
        let payload = serde_json::to_string(&decision)?;
        self.store_memory("goal_adaptation", &decision.goal_id.to_string(), &payload, None).await?;
        Ok(())
    }

    pub async fn health_check(&self) -> anyhow::Result<bool> {
        Ok(true)
    }

    pub async fn emit_belief_update(
        &self,
        belief_id: &str,
        evidence: &str,
        confidence_delta: f32,
    ) -> anyhow::Result<crate::belief_update::BeliefUpdate> {
        Ok(crate::belief_update::BeliefUpdate::new(belief_id, evidence, confidence_delta))
    }
}

impl MemoryCortex {
    pub async fn store_semantic_memory(
        &self,
        memory: crate::memory_semantic::SemanticMemory,
    ) -> anyhow::Result<()> {
        let content = serde_json::to_string(&memory)?;
        self.store_memory("semantic", &memory.lesson, &content, None).await?;
        Ok(())
    }

    pub async fn recall_semantic(
        &self,
        category: &str,
    ) -> anyhow::Result<Vec<nexus_substrate::memory::AgentMemory>> {
        self.search_memories(category).await
    }
}

impl MemoryCortex {
    pub async fn store_belief_transition(
        &self,
        event: crate::belief_transition::BeliefTransitionEvent,
    ) -> anyhow::Result<()> {
        let memory = crate::memory_types::CognitiveMemory {
            id: uuid::Uuid::new_v4(),
            kind: crate::memory_types::MemoryKind::Reflection,
            key: format!("belief_transition_{}", event.belief_id),
            content: event.lesson,
            source: "belief_engine".to_string(),
            confidence: event.new_confidence,
            importance: 0.9,
            created_at: chrono::Utc::now().timestamp(),
            tags: vec!["belief".to_string(), "reflection".to_string(), "adaptation".to_string()],
        };
        self.remember(memory).await?;
        Ok(())
    }
}

impl MemoryCortex {
    pub async fn store_belief_cause(
        &self,
        cause: crate::belief_causality::BeliefCause,
    ) -> anyhow::Result<()> {
        let memory = crate::memory_types::CognitiveMemory {
            id: uuid::Uuid::new_v4(),
            kind: crate::memory_types::MemoryKind::Reflection,
            key: format!("belief_cause_{}", cause.id),
            content: format!("BELIEF CAUSE: {} | LESSON: {}", cause.cause, cause.lesson),
            source: "belief_causality_engine".to_string(),
            confidence: cause.new_confidence,
            importance: 0.95,
            created_at: chrono::Utc::now().timestamp(),
            tags: vec![
                "belief".to_string(),
                "causal_memory".to_string(),
                "reflection".to_string(),
                "adaptation".to_string(),
            ],
        };
        self.remember(memory).await?;
        Ok(())
    }
}

impl MemoryCortex {
    pub async fn recall_causal_pattern(
        &self,
        observation: String,
    ) -> anyhow::Result<Vec<crate::causal_recall::CausalRecall>> {
        let mut results = Vec::new();
        if observation.to_lowercase().contains("memory") {
            results.push(crate::causal_recall::CausalRecall {
                pattern: "memory".to_string(),
                previous_cause: "resource exhaustion".to_string(),
                lesson: "Use resource-aware routing before execution".to_string(),
                confidence: 0.95,
            });
        }
        Ok(results)
    }
}

impl MemoryCortex {
    pub async fn store_validation_event(
        &self,
        event: crate::belief_validation::BeliefValidationEvent,
    ) -> anyhow::Result<()> {
        let memory = crate::memory_types::CognitiveMemory {
            id: uuid::Uuid::new_v4(),
            kind: crate::memory_types::MemoryKind::Reflection,
            key: format!("belief_validation_{}", uuid::Uuid::new_v4()),
            content: event.lesson,
            source: "belief_validation_engine".to_string(),
            confidence: 1.0 - event.error_score,
            importance: 0.95,
            created_at: chrono::Utc::now().timestamp(),
            tags: vec![
                "belief".into(),
                "prediction".into(),
                "validation".into(),
                "causal_learning".into(),
            ],
        };
        self.remember(memory).await?;
        Ok(())
    }
}

// --- PHASE 2: STATEFUL WORKING MEMORY & CACHE ---
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkingMemory {
    pub active_node: uuid::Uuid,
    pub required_objectives: Vec<String>,
    pub completed_objectives: Vec<String>,
}

impl WorkingMemory {
    pub fn new(active_node: uuid::Uuid, objectives: Vec<String>) -> Self {
        Self { active_node, required_objectives: objectives, completed_objectives: Vec::new() }
    }

    pub fn mark_completed(&mut self, objective: &str) {
        if let Some(pos) = self.required_objectives.iter().position(|x| x == objective) {
            let obj = self.required_objectives.remove(pos);
            self.completed_objectives.push(obj);
        }
    }

    pub fn get_prompt_context(&self) -> String {
        format!(
            "[WORKING MEMORY]\nPending Objectives: {:?}\nCompleted Objectives: {:?}",
            self.required_objectives, self.completed_objectives
        )
    }

    pub fn is_mission_accomplished(&self) -> bool {
        self.required_objectives.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct CachedAction {
    pub output: String,
    pub exit_status: i32,
    pub timestamp: u64,
}

#[derive(Debug, Default)]
pub struct ActionCache {
    cache: HashMap<String, CachedAction>,
}

impl ActionCache {
    pub fn new() -> Self {
        Self { cache: HashMap::new() }
    }

    pub fn generate_key(command: &str, ast_hash: &str) -> String {
        let mut s = DefaultHasher::new();
        command.hash(&mut s);
        ast_hash.hash(&mut s);
        s.finish().to_string()
    }

    pub fn get(&self, key: &str) -> Option<&CachedAction> {
        self.cache.get(key)
    }

    pub fn insert(&mut self, key: String, output: String, exit_status: i32) {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        self.cache.insert(key, CachedAction { output, exit_status, timestamp });
    }
}
