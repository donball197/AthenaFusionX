use crate::graph::AstFile;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub fn read_map(path: &Path) -> Result<Vec<AstFile>, Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let files: Vec<AstFile> = serde_json::from_reader(reader)?;
    Ok(files)
}
