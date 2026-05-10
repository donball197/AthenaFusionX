use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;

pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> Value;
    
    /// Executes the tool using the strictly validated JSON payload.
    /// In Phase 2, this will bridge to WASI capabilities.
    fn execute(&self, input: Value) -> Result<String>;
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get_system_prompt_schema(&self) -> String {
        let mut schemas = Vec::new();
        for tool in self.tools.values() {
            schemas.push(serde_json::json!({
                "name": tool.name(),
                "description": tool.description(),
                "parameters": tool.input_schema()
            }));
        }
        format!("Available Capabilities:\n{}", serde_json::to_string_pretty(&schemas).unwrap())
    }

    pub fn dispatch(&self, tool_name: &str, input: Value) -> Result<String> {
        if let Some(tool) = self.tools.get(tool_name) {
            // Note: Schema validation logic against `input` will be injected here.
            tool.execute(input)
        } else {
            Err(anyhow!("CAPABILITY_DENIED: Tool '{}' is not registered or agent lacks clearance.", tool_name))
        }
    }
}
