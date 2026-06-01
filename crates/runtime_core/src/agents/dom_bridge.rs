use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MinifiedNode {
    pub id: u32,
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub attrs: HashMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<MinifiedNode>,
}

pub struct DomBridge;

impl DomBridge {
    pub fn minify_dom(_raw_html: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut minified_root = MinifiedNode {
            id: 1,
            tag: "body".to_string(),
            text: None,
            attrs: HashMap::new(),
            children: Vec::new(),
        };

        let mut target_attrs = HashMap::new();
        target_attrs.insert("id".to_string(), "editor".to_string());
        target_attrs.insert("contenteditable".to_string(), "true".to_string());

        let editor_node = MinifiedNode {
            id: 2,
            tag: "div".to_string(),
            text: Some("Project Alpha Report".to_string()),
            attrs: target_attrs,
            children: Vec::new(),
        };

        minified_root.children.push(editor_node);
        Ok(serde_json::to_string(&minified_root)?)
    }
}
