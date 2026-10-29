use crate::study::{ParseStatus, StudyReport};
use serde::Serialize;
use std::collections::BTreeMap;

const EVIDENCE_SCHEMA_VERSION: u32 = 1;
const HOTSPOT_LIMIT: usize = 10;

#[derive(Debug, Serialize)]
pub struct EvidencePacket<T> {
    pub schema_version: u32,
    pub root: String,
    pub packet: T,
}

#[derive(Debug, Serialize)]
pub struct EvidencePacketSet {
    pub schema_version: u32,
    pub root: String,
    pub overview: OverviewPacket,
    pub architecture: ArchitecturePacket,
    pub hotspots: HotspotsPacket,
    pub dependencies: DependenciesPacket,
    pub parse_health: ParseHealthPacket,
    pub symbols: SymbolsPacket,
}

#[derive(Debug, Serialize)]
pub struct OverviewPacket {
    pub source_files: usize,
    pub total_lines: usize,
    pub total_bytes: usize,
    pub functions: usize,
    pub structural_entities: usize,
    pub languages: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
pub struct ArchitecturePacket {
    pub entity_counts: BTreeMap<String, usize>,
    pub structural_hotspots: Vec<FileCount>,
}

#[derive(Debug, Serialize)]
pub struct HotspotsPacket {
    pub largest_files: Vec<FileMetrics>,
    pub function_hotspots: Vec<FileCount>,
}

#[derive(Debug, Serialize)]
pub struct DependenciesPacket {
    pub total_dependencies: usize,
    pub workspace_dependencies: usize,
    pub resolved_workspace_dependencies: usize,
    pub manifests: Vec<DependencyManifest>,
}

#[derive(Debug, Serialize)]
pub struct DependencyManifest {
    pub path: String,
    pub format: String,
    pub package_name: Option<String>,
    pub package_version: Option<String>,
    pub dependencies: Vec<DependencyRecord>,
}

#[derive(Debug, Serialize)]
pub struct DependencyRecord {
    pub name: String,
    pub kind: String,
    pub requirement: Option<String>,
    pub workspace: bool,
    pub workspace_resolved: bool,
    pub resolved_version: Option<String>,
    pub features: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ParseHealthPacket {
    pub source_files: usize,
    pub clean_files: usize,
    pub parse_health_percent: f64,
    pub diagnostics: Vec<ParseDiagnostic>,
    pub read_failures: usize,
}

#[derive(Debug, Serialize)]
pub struct ParseDiagnostic {
    pub path: String,
    pub language: String,
}

#[derive(Debug, Serialize)]
pub struct SymbolsPacket {
    pub function_count: usize,
    pub entity_count: usize,
    pub sample_functions: Vec<SymbolRecord>,
    pub sample_entities: Vec<SymbolRecord>,
}

#[derive(Debug, Serialize)]
pub struct SymbolRecord {
    pub path: String,
    pub language: String,
    pub kind: String,
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Serialize)]
pub struct FileMetrics {
    pub path: String,
    pub bytes: usize,
    pub lines: usize,
}

#[derive(Debug, Serialize)]
pub struct FileCount {
    pub path: String,
    pub count: usize,
}

pub fn build_evidence_packets(report: &StudyReport) -> EvidencePacketSet {
    let total_lines = report.files.iter().map(|file| file.lines).sum();
    let total_bytes = report.files.iter().map(|file| file.bytes).sum();

    let clean_files =
        report.files.iter().filter(|file| matches!(file.parse_status, ParseStatus::Clean)).count();

    let parse_health_percent = if report.files.is_empty() {
        100.0
    } else {
        (clean_files as f64 / report.files.len() as f64) * 100.0
    };

    let mut entity_counts = BTreeMap::new();
    let mut functions_by_file = BTreeMap::new();
    let mut entities_by_file = BTreeMap::new();

    for function in &report.functions {
        *functions_by_file.entry(function.path.as_str()).or_insert(0usize) += 1;
    }

    for entity in &report.entities {
        *entity_counts.entry(entity.kind.as_str()).or_insert(0usize) += 1;

        *entities_by_file.entry(entity.path.as_str()).or_insert(0usize) += 1;
    }

    let mut largest_files: Vec<FileMetrics> = report
        .files
        .iter()
        .map(|file| FileMetrics { path: file.path.clone(), bytes: file.bytes, lines: file.lines })
        .collect();

    largest_files.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.path.cmp(&b.path)));
    largest_files.truncate(HOTSPOT_LIMIT);

    let function_hotspots = sorted_file_counts(functions_by_file);
    let structural_hotspots = sorted_file_counts(entities_by_file);

    let total_dependencies =
        report.manifest_details.iter().map(|manifest| manifest.dependencies.len()).sum();

    let workspace_dependencies = report
        .manifest_details
        .iter()
        .flat_map(|manifest| manifest.dependencies.iter())
        .filter(|dependency| dependency.workspace)
        .count();

    let resolved_workspace_dependencies = report
        .manifest_details
        .iter()
        .flat_map(|manifest| manifest.dependencies.iter())
        .filter(|dependency| dependency.workspace && dependency.workspace_resolved.is_some())
        .count();

    let manifests = report
        .manifest_details
        .iter()
        .map(|manifest| DependencyManifest {
            path: manifest.path.clone(),
            format: manifest.format.clone(),
            package_name: manifest.package_name.clone(),
            package_version: manifest.package_version.clone(),
            dependencies: manifest
                .dependencies
                .iter()
                .map(|dependency| {
                    let resolved = dependency.workspace_resolved.as_ref();

                    DependencyRecord {
                        name: dependency.name.clone(),
                        kind: dependency.kind.clone(),
                        requirement: dependency.requirement.clone(),
                        workspace: dependency.workspace,
                        workspace_resolved: resolved.is_some(),
                        resolved_version: resolved.and_then(|value| value.version.clone()),
                        features: resolved
                            .map(|value| value.features.clone())
                            .unwrap_or_else(|| dependency.features.clone()),
                    }
                })
                .collect(),
        })
        .collect();

    let diagnostics = report
        .tree_sitter_diagnostics
        .iter()
        .map(|diagnostic| ParseDiagnostic {
            path: diagnostic.path.clone(),
            language: diagnostic.language.clone(),
        })
        .collect();

    let sample_functions = report
        .functions
        .iter()
        .take(HOTSPOT_LIMIT)
        .map(|function| SymbolRecord {
            path: function.path.clone(),
            language: function.language.clone(),
            kind: "function".to_string(),
            name: function.name.clone(),
            start_line: function.start_line,
            end_line: function.end_line,
        })
        .collect();

    let sample_entities = report
        .entities
        .iter()
        .take(HOTSPOT_LIMIT)
        .map(|entity| SymbolRecord {
            path: entity.path.clone(),
            language: entity.language.clone(),
            kind: entity.kind.clone(),
            name: entity.name.clone(),
            start_line: entity.start_line,
            end_line: entity.end_line,
        })
        .collect();

    EvidencePacketSet {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        root: report.root.clone(),
        overview: OverviewPacket {
            source_files: report.files.len(),
            total_lines,
            total_bytes,
            functions: report.functions.len(),
            structural_entities: report.entities.len(),
            languages: report.language_counts.clone(),
        },
        architecture: ArchitecturePacket {
            entity_counts: entity_counts
                .into_iter()
                .map(|(kind, count)| (kind.to_string(), count))
                .collect(),
            structural_hotspots,
        },
        hotspots: HotspotsPacket { largest_files, function_hotspots },
        dependencies: DependenciesPacket {
            total_dependencies,
            workspace_dependencies,
            resolved_workspace_dependencies,
            manifests,
        },
        parse_health: ParseHealthPacket {
            source_files: report.files.len(),
            clean_files,
            parse_health_percent,
            diagnostics,
            read_failures: report.read_failures.len(),
        },
        symbols: SymbolsPacket {
            function_count: report.functions.len(),
            entity_count: report.entities.len(),
            sample_functions,
            sample_entities,
        },
    }
}

fn sorted_file_counts(counts: BTreeMap<&str, usize>) -> Vec<FileCount> {
    let mut values: Vec<FileCount> = counts
        .into_iter()
        .map(|(path, count)| FileCount { path: path.to_string(), count })
        .collect();

    values.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.path.cmp(&b.path)));

    values.truncate(HOTSPOT_LIMIT);
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::study::{ParseStatus, StudyFile};

    #[test]
    fn evidence_packets_are_deterministic() {
        let report = StudyReport {
            schema_version: 1,
            root: "/repo".to_string(),
            files: vec![StudyFile {
                path: "src/lib.rs".to_string(),
                language: "Rust".to_string(),
                bytes: 100,
                lines: 10,
                parse_status: ParseStatus::Clean,
            }],
            functions: Vec::new(),
            entities: Vec::new(),
            language_counts: BTreeMap::from([("Rust".to_string(), 1)]),
            tree_sitter_diagnostics: Vec::new(),
            read_failures: Vec::new(),
            manifests: Vec::new(),
            manifest_details: Vec::new(),
        };

        let first = serde_json::to_string(&build_evidence_packets(&report)).unwrap();
        let second = serde_json::to_string(&build_evidence_packets(&report)).unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn evidence_packets_preserve_core_counts() {
        let report = StudyReport {
            schema_version: 1,
            root: "/repo".to_string(),
            files: vec![
                StudyFile {
                    path: "src/a.rs".to_string(),
                    language: "Rust".to_string(),
                    bytes: 100,
                    lines: 10,
                    parse_status: ParseStatus::Clean,
                },
                StudyFile {
                    path: "src/b.rs".to_string(),
                    language: "Rust".to_string(),
                    bytes: 200,
                    lines: 20,
                    parse_status: ParseStatus::TreeSitterError,
                },
            ],
            functions: Vec::new(),
            entities: Vec::new(),
            language_counts: BTreeMap::from([("Rust".to_string(), 2)]),
            tree_sitter_diagnostics: Vec::new(),
            read_failures: Vec::new(),
            manifests: Vec::new(),
            manifest_details: Vec::new(),
        };

        let packets = build_evidence_packets(&report);

        assert_eq!(packets.overview.source_files, 2);
        assert_eq!(packets.overview.total_lines, 30);
        assert_eq!(packets.overview.total_bytes, 300);
        assert_eq!(packets.parse_health.clean_files, 1);
        assert_eq!(packets.parse_health.parse_health_percent, 50.0);
        assert_eq!(packets.hotspots.largest_files[0].path, "src/b.rs");
    }
}
