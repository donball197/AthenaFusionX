/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


// src/json_parser.rs - v4.4.0 (Explicit Error Feedback)
use crate::agent_protocol::AgentAction;

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>, String> {
    let mut block = response.to_string();
    
    if let (Some(s), Some(e)) = (response.find("<ACTION>"), response.find("</ACTION>")) {
        block = response[s + 8..e].trim().to_string();
    } else {
        return Err("PARSE_ERROR: Missing <ACTION>[...]</ACTION> tags.".into());
    }
    
    block = block.replace("```json", "").replace("```", "").trim().to_string();
    
    if let (Some(s), Some(e)) = (block.find('['), block.rfind(']')) {
        let pure_json = &block[s..=e];
        serde_json::from_str::<Vec<AgentAction>>(pure_json)
            .map_err(|e| format!("SCHEMA_ERROR: {} - Verify field names (e.g., 'url' for scrape, 'path' for read).", e))
    } else {
        Err("PARSE_ERROR: No JSON array found inside ACTION tags.".into())
    }
}
