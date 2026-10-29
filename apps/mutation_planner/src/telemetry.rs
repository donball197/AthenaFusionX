use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
pub struct ImpactReport {
    pub target_node: String,
    pub total_dependents: usize,
    pub risk_level: String,
    pub impacted_nodes: Vec<String>,
}

pub fn load_impact<P: AsRef<Path>>(path: P) -> Result<ImpactReport, Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let report: ImpactReport = serde_json::from_reader(reader)?;
    Ok(report)
}
