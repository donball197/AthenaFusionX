use crate::nexus_error::NexusError;

pub struct DiffEngine;

impl DiffEngine {
    pub fn apply(original: &str, search: &str, replace: &str) -> Result<String, NexusError> {
        let clean_search = search.trim();
        if clean_search.is_empty() { return Ok(original.to_string()); }
        
        // STRICT PROTOCOL: Exact match only to prevent fuzzy data corruption
        if original.contains(search) { 
            return Ok(original.replace(search, replace)); 
        }

        Err(NexusError::Patch("PATTERN_NOT_FOUND: Could not locate exact search string in file.".into()))
    }

    pub fn unified_diff(_old: &str, _new: &str, path: &str) -> String {
        format!("--- a/{}\n+++ b/{}\n[MODIFICATION DETECTED]", path, path)
    }
}
