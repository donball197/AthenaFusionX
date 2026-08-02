pub struct BuildMemoryBridge;

impl BuildMemoryBridge {
    pub fn record(lesson: &str) -> String {
        format!("Build lesson recorded: {}", lesson)
    }
}
