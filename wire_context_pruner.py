import os

cl_path = "crates/runtime_core/src/cortical_loop.rs"
with open(cl_path, "r") as f:
    cl_data = f.read()

old_loop = """                let mut evidence_history = String::new();
                for edge in &self.graph.edges {
                    if edge.target == h_id && (edge.relation == EdgeRelation::Supports || edge.relation == EdgeRelation::Contradicts) {
                        if let Some(obs) = self.graph.nodes.get(&edge.source) {
                            evidence_history.push_str(&format!("- [{:?}] {}\\n", edge.relation, obs.content));
                        }
                    }
                }
                if evidence_history.is_empty() { evidence_history = "None yet.".to_string(); }"""

new_loop = """                // --- ⚡ HEURISTIC CONTEXT PRUNING (API STARVATION) ---
                let mut evidence_history = String::new();
                let mut relevant_obs = Vec::new();
                
                for edge in &self.graph.edges {
                    if edge.target == h_id && (edge.relation == EdgeRelation::Supports || edge.relation == EdgeRelation::Contradicts) {
                        if let Some(obs) = self.graph.nodes.get(&edge.source) {
                            relevant_obs.push((edge.relation.clone(), obs.content.clone()));
                        }
                    }
                }

                // Keep failures (we must learn from mistakes) + the 3 most recent successes
                let mut filtered_obs = Vec::new();
                let mut success_count = 0;
                
                for (relation, content) in relevant_obs.into_iter().rev() {
                    let is_failure = content.contains("FAILED") || content.contains("STDERR:") || relation == EdgeRelation::Contradicts;
                    if is_failure {
                        filtered_obs.push((relation, content));
                    } else if success_count < 3 {
                        filtered_obs.push((relation, content));
                        success_count += 1;
                    }
                }

                // Re-reverse so chronological order is maintained, and cap at 3000 chars total
                filtered_obs.reverse();
                for (relation, content) in filtered_obs {
                    let truncated = if content.len() > 800 {
                        format!("{}... [TRUNCATED]", &content[..800])
                    } else {
                        content
                    };
                    evidence_history.push_str(&format!("- [{:?}] {}\\n", relation, truncated));
                    if evidence_history.len() > 3000 { break; }
                }

                if evidence_history.is_empty() { evidence_history = "None yet.".to_string(); }
                // --- END PRUNING ---"""

cl_data = cl_data.replace(old_loop, new_loop)

with open(cl_path, "w") as f:
    f.write(cl_data)

print("[✓] Heuristic Context Pruner wired into Cortical Loop.")
