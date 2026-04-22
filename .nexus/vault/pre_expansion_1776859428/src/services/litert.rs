pub struct LogicAgent;
impl LogicAgent {
    pub fn wake_cortex(path: &str) -> anyhow::Result<()> {
        println!(">> [LITERT_CORTEX] Mapping model: {}", path);
        Ok(())
    }
}
