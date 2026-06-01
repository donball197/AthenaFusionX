use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ConceptNode {
    pub id: String,
    pub associative_tags: Vec<String>,
    pub confidence_weight: f32,
    pub insight_payload: String,
}

pub struct CognitiveGraph {
    pub lattice: HashMap<String, ConceptNode>,
}

impl CognitiveGraph {
    pub fn ignite() -> Self {
        Self { lattice: HashMap::new() }
    }

    /// Stores a stateful memory of an operational result
    pub fn encode_experience(&mut self, tags: Vec<&str>, insight: &str, success: bool) -> String {
        // Simple hash ID generation for universal ARM64 compatibility
        let id = format!("mem_node_{}", self.lattice.len()); 
        
        // Penalize the weight of failed logic so the agent learns to avoid it
        let confidence_weight = if success { 0.95 } else { 0.15 }; 
        
        let node = ConceptNode {
            id: id.clone(),
            associative_tags: tags.into_iter().map(|s| s.to_string()).collect(),
            confidence_weight,
            insight_payload: insight.to_string(),
        };
        
        self.lattice.insert(id.clone(), node);
        id
    }

    /// Retrieves contextual memory before the agent attempts a new task
    pub fn query_latent_context(&self, active_context: &str) -> Vec<ConceptNode> {
        let mut resonant_nodes: Vec<ConceptNode> = self.lattice.values()
            .filter(|n| n.associative_tags.iter().any(|tag| active_context.contains(tag)))
            .cloned()
            .collect();
            
        // Sort by highest confidence to feed the agent the most reliable past experiences
        resonant_nodes.sort_by(|a, b| b.confidence_weight.partial_cmp(&a.confidence_weight).unwrap());
        resonant_nodes
    }
}
