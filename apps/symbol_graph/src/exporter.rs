use crate::graph::SerializableGraph;
use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

pub fn write_graph(path: &Path, graph: &SerializableGraph) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, graph)?;
    Ok(())
}
