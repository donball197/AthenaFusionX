#[derive(Debug, Clone)]
pub struct LocatedFunction {
    pub file_path: String,

    pub symbol: String,

    pub start_line: usize,

    pub end_line: usize,

    pub confidence: f32,
}

pub struct FunctionLocator;

impl FunctionLocator {
    pub fn locate(symbol: &str) -> Option<LocatedFunction> {
        Some(LocatedFunction {
            file_path: String::new(),
            symbol: symbol.to_string(),
            start_line: 0,
            end_line: 0,
            confidence: 0.0,
        })
    }
}
