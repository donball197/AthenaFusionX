pub fn get_light_models() -> Vec<String> {
    let mut cascade = Vec::new();
    if let Ok(m) = std::env::var("NEXUS_LIGHT_MODEL") {
        if !m.trim().is_empty() {
            cascade.push(m.trim().to_string());
        }
    }
    cascade.push("gemini-3.1-flash-lite".to_string());
    cascade.push("gemma-4-26b-a4b-it".to_string());

    // Deduplicate list
    let mut unique = Vec::new();
    for m in cascade {
        if !unique.contains(&m) {
            unique.push(m);
        }
    }
    unique
}

pub fn get_heavy_models() -> Vec<String> {
    let mut cascade = Vec::new();
    if let Ok(m) = std::env::var("NEXUS_HEAVY_MODEL") {
        if !m.trim().is_empty() {
            cascade.push(m.trim().to_string());
        }
    }
    cascade.push("gemma-4-31b-it".to_string());
    cascade.push("gemini-3.1-flash-lite".to_string());

    let mut unique = Vec::new();
    for m in cascade {
        if !unique.contains(&m) {
            unique.push(m);
        }
    }
    unique
}
