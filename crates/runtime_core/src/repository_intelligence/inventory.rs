//! Inventory module for repository intelligence.

use serde::{Deserialize, Serialize};

/// Represents the inventory status of a repository component.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryItem {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub active: bool,
}

impl Default for InventoryItem {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "default_item".to_string(),
            kind: "generic".to_string(),
            active: true,
        }
    }
}
