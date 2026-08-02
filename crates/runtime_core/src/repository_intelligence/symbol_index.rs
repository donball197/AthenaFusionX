use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
}

impl Ord for Span {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.line
            .cmp(&other.line)
            .then_with(|| self.start.cmp(&other.start))
            .then_with(|| self.end.cmp(&other.end))
    }
}

impl PartialOrd for Span {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub id: SymbolId,
    pub name: String,
    pub kind: String,
    pub span: Span,
    pub file_path: String,
}

#[derive(Debug, Clone)]
pub struct SymbolReference {
    pub symbol_id: SymbolId,
    pub span: Span,
    pub file_path: String,
}

#[derive(Debug, Default, Clone)]
pub struct SymbolIndex {
    pub symbols: HashMap<String, SymbolInfo>,
    pub references: Vec<SymbolReference>,
    pub span_map: BTreeMap<Span, SymbolId>,
}

impl SymbolIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_symbol(&mut self, info: SymbolInfo) {
        self.span_map.insert(info.span.clone(), info.id.clone());
        self.symbols.insert(info.id.0.clone(), info);
    }

    pub fn add_reference(&mut self, reference: SymbolReference) {
        self.references.push(reference);
    }

    pub fn get_symbol(&self, id: &SymbolId) -> Option<&SymbolInfo> {
        self.symbols.get(&id.0)
    }

    pub fn find_by_span(&self, span: &Span) -> Option<&SymbolId> {
        self.span_map.get(span)
    }
}
