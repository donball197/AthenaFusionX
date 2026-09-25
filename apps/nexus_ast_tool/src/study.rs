use crate::polyglot::{SupportedLanguage, TreeSitterParseStatus};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use tree_sitter::{Parser, Query, QueryCursor};
use walkdir::WalkDir;

#[derive(Debug, Serialize)]
pub struct StudyReport {
    pub schema_version: u32,
    pub root: String,
    pub files: Vec<StudyFile>,
    pub functions: Vec<StudyFunction>,
    pub entities: Vec<StudyEntity>,
    pub language_counts: BTreeMap<String, usize>,
    pub tree_sitter_diagnostics: Vec<TreeSitterDiagnostic>,
    pub read_failures: Vec<ReadFailure>,
    pub manifests: Vec<String>,
    pub manifest_details: Vec<StudyManifest>,
}

#[derive(Debug, Serialize)]
pub struct StudyManifest {
    pub path: String,
    pub format: String,
    pub package_name: Option<String>,
    pub package_version: Option<String>,
    pub dependencies: Vec<StudyDependency>,
}

#[derive(Debug, Serialize)]
pub struct StudyDependency {
    pub name: String,
    pub kind: String,
    pub requirement: Option<String>,
    pub optional: bool,
    pub features: Vec<String>,
    pub workspace: bool,
    pub path: Option<String>,
    pub git: Option<String>,
    pub branch: Option<String>,
    pub tag: Option<String>,
    pub package: Option<String>,
    pub workspace_resolved: Option<WorkspaceDependency>,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceDependency {
    pub version: Option<String>,
    pub features: Vec<String>,
    pub default_features: Option<bool>,
    pub optional: Option<bool>,
    pub path: Option<String>,
    pub git: Option<String>,
    pub branch: Option<String>,
    pub tag: Option<String>,
    pub package: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StudyFile {
    pub path: String,
    pub language: String,
    pub bytes: usize,
    pub lines: usize,
    pub parse_status: ParseStatus,
}

#[derive(Debug, Serialize)]
pub struct StudyFunction {
    pub path: String,
    pub language: String,
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Serialize)]
pub struct StudyEntity {
    pub path: String,
    pub language: String,
    pub kind: String,
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Serialize)]
pub enum ParseStatus {
    Clean,
    TreeSitterError,
    ParserUnavailable,
    NoParseTree,
}

#[derive(Debug, Serialize)]
pub struct TreeSitterDiagnostic {
    pub path: String,
    pub language: String,
}

#[derive(Debug, Serialize)]
pub struct ReadFailure {
    pub path: String,
    pub error: String,
}

pub fn run_study_report(root: &Path) -> Result<StudyReport> {
    let root = root
        .canonicalize()
        .with_context(|| format!("Cannot resolve study path: {}", root.display()))?;

    if !root.is_dir() {
        anyhow::bail!("Study path is not a directory: {}", root.display());
    }

    build_study_report(&root)
}

fn build_study_report(root: &Path) -> Result<StudyReport> {
    let workspace_dependencies = load_workspace_dependencies(root);

    let mut report = StudyReport {
        schema_version: 1,
        root: root.display().to_string(),
        files: Vec::new(),
        functions: Vec::new(),
        entities: Vec::new(),
        language_counts: BTreeMap::new(),
        tree_sitter_diagnostics: Vec::new(),
        read_failures: Vec::new(),
        manifests: Vec::new(),
        manifest_details: Vec::new(),
    };

    for entry in WalkDir::new(root).follow_links(false).into_iter().filter_map(|entry| entry.ok()) {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        if is_ignored(path) {
            continue;
        }

        if is_manifest(path) {
            let relative = relative_path(root, path);
            report.manifests.push(relative.clone());

            if path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml") {
                if let Ok(source) = fs::read_to_string(path) {
                    if let Some(mut manifest) = parse_cargo_manifest(&source, &relative) {
                        for dependency in &mut manifest.dependencies {
                            if dependency.workspace {
                                dependency.workspace_resolved = workspace_dependencies
                                    .get(&dependency.name)
                                    .and_then(workspace_dependency);
                            }
                        }

                        report.manifest_details.push(manifest);
                    }
                }
            }
        }

        let Some(language) = SupportedLanguage::from_path(path) else {
            continue;
        };

        let language_name = format!("{language:?}");

        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                report.read_failures.push(ReadFailure {
                    path: relative_path(root, path),
                    error: error.to_string(),
                });
                continue;
            }
        };

        let parse_status = match language.tree_sitter_parse_status(&source) {
            TreeSitterParseStatus::Clean => ParseStatus::Clean,
            TreeSitterParseStatus::TreeSitterError => ParseStatus::TreeSitterError,
            TreeSitterParseStatus::ParserUnavailable => ParseStatus::ParserUnavailable,
            TreeSitterParseStatus::NoParseTree => ParseStatus::NoParseTree,
        };

        if !matches!(parse_status, ParseStatus::Clean) {
            report.tree_sitter_diagnostics.push(TreeSitterDiagnostic {
                path: relative_path(root, path),
                language: language_name.clone(),
            });
        }

        *report.language_counts.entry(language_name.clone()).or_insert(0) += 1;

        let relative = relative_path(root, path);

        if matches!(parse_status, ParseStatus::Clean) {
            report.functions.extend(discover_functions(&source, language, &relative));

            report.entities.extend(discover_entities(&source, language, &relative));
        }

        report.files.push(StudyFile {
            path: relative,
            language: language_name,
            bytes: source.len(),
            lines: source.lines().count(),
            parse_status,
        });
    }

    report.files.sort_by(|a, b| a.path.cmp(&b.path));
    report.functions.sort_by(|a, b| {
        (a.path.as_str(), a.start_line, a.name.as_str()).cmp(&(
            b.path.as_str(),
            b.start_line,
            b.name.as_str(),
        ))
    });
    report.entities.sort_by(|a, b| {
        (a.path.as_str(), a.start_line, a.kind.as_str(), a.name.as_str()).cmp(&(
            b.path.as_str(),
            b.start_line,
            b.kind.as_str(),
            b.name.as_str(),
        ))
    });
    report.tree_sitter_diagnostics.sort_by(|a, b| a.path.cmp(&b.path));
    report.read_failures.sort_by(|a, b| a.path.cmp(&b.path));
    report.manifests.sort();
    report.manifest_details.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(report)
}

pub fn run_study(root: &Path, output: &Path) -> Result<()> {
    let root = root
        .canonicalize()
        .with_context(|| format!("Cannot resolve study path: {}", root.display()))?;

    if !root.is_dir() {
        anyhow::bail!("Study path is not a directory: {}", root.display());
    }

    fs::create_dir_all(output)
        .with_context(|| format!("Cannot create output directory: {}", output.display()))?;

    let report = build_study_report(&root)?;

    let repository_json = serde_json::to_string_pretty(&report)?;

    fs::write(output.join("repository.json"), format!("{repository_json}\n"))?;

    fs::write(
        output.join("files.json"),
        format!("{}\n", serde_json::to_string_pretty(&report.files)?),
    )?;

    fs::write(
        output.join("functions.json"),
        format!("{}\n", serde_json::to_string_pretty(&report.functions)?),
    )?;

    fs::write(
        output.join("entities.json"),
        format!("{}\n", serde_json::to_string_pretty(&report.entities)?),
    )?;

    fs::write(
        output.join("dependencies.json"),
        format!("{}\n", serde_json::to_string_pretty(&report.manifest_details)?),
    )?;

    let evidence = crate::evidence::build_evidence_packets(&report);
    let evidence_dir = output.join("evidence");
    fs::create_dir_all(&evidence_dir)?;

    fs::write(
        evidence_dir.join("overview.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&crate::evidence::EvidencePacket {
                schema_version: evidence.schema_version,
                root: evidence.root.clone(),
                packet: &evidence.overview,
            })?
        ),
    )?;

    fs::write(
        evidence_dir.join("architecture.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&crate::evidence::EvidencePacket {
                schema_version: evidence.schema_version,
                root: evidence.root.clone(),
                packet: &evidence.architecture,
            })?
        ),
    )?;

    fs::write(
        evidence_dir.join("hotspots.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&crate::evidence::EvidencePacket {
                schema_version: evidence.schema_version,
                root: evidence.root.clone(),
                packet: &evidence.hotspots,
            })?
        ),
    )?;

    fs::write(
        evidence_dir.join("dependencies.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&crate::evidence::EvidencePacket {
                schema_version: evidence.schema_version,
                root: evidence.root.clone(),
                packet: &evidence.dependencies,
            })?
        ),
    )?;

    fs::write(
        evidence_dir.join("parse_health.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&crate::evidence::EvidencePacket {
                schema_version: evidence.schema_version,
                root: evidence.root.clone(),
                packet: &evidence.parse_health,
            })?
        ),
    )?;

    fs::write(
        evidence_dir.join("symbols.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&crate::evidence::EvidencePacket {
                schema_version: evidence.schema_version,
                root: evidence.root.clone(),
                packet: &evidence.symbols,
            })?
        ),
    )?;

    fs::write(output.join("report.md"), render_markdown(&report))?;

    println!("==> [Study] Repository: {}", root.display());
    println!("==> [Study] Source files: {}", report.files.len());
    println!("==> [Study] Tree-sitter diagnostics: {}", report.tree_sitter_diagnostics.len());
    println!("==> [Study] Read failures: {}", report.read_failures.len());
    println!("==> [Study] Reports: {}", output.display());

    Ok(())
}

fn discover_functions(
    source: &str,
    language: SupportedLanguage,
    relative: &str,
) -> Vec<StudyFunction> {
    let mut parser = Parser::new();
    let ts_lang = language.tree_sitter_language();

    if parser.set_language(&ts_lang).is_err() {
        return Vec::new();
    }

    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let Ok(query) = Query::new(&ts_lang, language.symbol_query()) else {
        return Vec::new();
    };

    let mut cursor = QueryCursor::new();
    let mut functions = Vec::new();

    for query_match in cursor.matches(&query, tree.root_node(), source.as_bytes()) {
        for capture in query_match.captures {
            let Ok(name) = capture.node.utf8_text(source.as_bytes()) else {
                continue;
            };

            let range = capture.node.range();

            functions.push(StudyFunction {
                path: relative.to_string(),
                language: format!("{language:?}"),
                name: name.to_string(),
                start_line: range.start_point.row + 1,
                end_line: range.end_point.row + 1,
            });
        }
    }

    functions
}

fn discover_entities(
    source: &str,
    language: SupportedLanguage,
    relative: &str,
) -> Vec<StudyEntity> {
    let mut parser = Parser::new();
    let ts_lang = language.tree_sitter_language();

    if parser.set_language(&ts_lang).is_err() {
        return Vec::new();
    }

    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut entities = Vec::new();
    collect_entities(tree.root_node(), source, language, relative, &mut entities);

    entities
}

fn collect_entities(
    node: tree_sitter::Node<'_>,
    source: &str,
    language: SupportedLanguage,
    relative: &str,
    entities: &mut Vec<StudyEntity>,
) {
    let kind = node.kind();

    let entity_kind = match language {
        SupportedLanguage::Rust => match kind {
            "struct_item" => Some("struct"),
            "enum_item" => Some("enum"),
            "trait_item" => Some("trait"),
            "impl_item" => Some("impl"),
            "mod_item" => Some("module"),
            _ => None,
        },
        SupportedLanguage::Python => match kind {
            "class_definition" => Some("class"),
            _ => None,
        },
        SupportedLanguage::JavaScript | SupportedLanguage::TypeScript => match kind {
            "class_declaration" => Some("class"),
            "interface_declaration" => Some("interface"),
            _ => None,
        },
        SupportedLanguage::C | SupportedLanguage::Cpp => match kind {
            "struct_specifier" => Some("struct"),
            "class_specifier" => Some("class"),
            _ => None,
        },
        SupportedLanguage::Go => match kind {
            "type_spec" => Some("type"),
            _ => None,
        },
        SupportedLanguage::PHP => match kind {
            "class_declaration" => Some("class"),
            "interface_declaration" => Some("interface"),
            "trait_declaration" => Some("trait"),
            _ => None,
        },
        SupportedLanguage::Zig => None,
    };

    if let Some(entity_kind) = entity_kind {
        if let Some(name) = entity_name(node, source, language, entity_kind) {
            let range = node.range();

            entities.push(StudyEntity {
                path: relative.to_string(),
                language: format!("{language:?}"),
                kind: entity_kind.to_string(),
                name,
                start_line: range.start_point.row + 1,
                end_line: range.end_point.row + 1,
            });
        }
    }

    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        collect_entities(child, source, language, relative, entities);
    }
}

fn entity_name(
    node: tree_sitter::Node<'_>,
    source: &str,
    language: SupportedLanguage,
    entity_kind: &str,
) -> Option<String> {
    if let Some(name_node) = node.child_by_field_name("name") {
        return name_node.utf8_text(source.as_bytes()).ok().map(str::to_owned);
    }

    if matches!(language, SupportedLanguage::Rust) && entity_kind == "impl" {
        let type_node =
            node.child_by_field_name("type").or_else(|| node.child_by_field_name("trait"));

        if let Some(type_node) = type_node {
            let type_name = type_node.utf8_text(source.as_bytes()).ok()?;
            return Some(format!("impl {type_name}"));
        }

        return Some("impl".to_string());
    }

    Some(format!("<anonymous {entity_kind}>"))
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

fn is_ignored(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(".git")
                | Some("target")
                | Some("node_modules")
                | Some(".venv")
                | Some(".nexus_venv")
                | Some("zig_muscle")
                | Some("substrate")
                | Some(".zig-cache")
                | Some("venv")
                | Some("__pycache__")
                | Some("nexus_study_report")
        )
    })
}

fn is_manifest(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(
            "Cargo.toml"
                | "package.json"
                | "pyproject.toml"
                | "requirements.txt"
                | "go.mod"
                | "build.zig"
                | "composer.json"
                | "CMakeLists.txt"
        )
    )
}

fn parse_cargo_manifest(source: &str, relative: &str) -> Option<StudyManifest> {
    let document: toml::Value = source.parse().ok()?;

    let package = document.get("package").and_then(toml::Value::as_table);

    let package_name = package
        .and_then(|table| table.get("name"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned);

    let package_version = package
        .and_then(|table| table.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned);

    let mut dependencies = Vec::new();

    for (section, kind) in
        [("dependencies", "normal"), ("dev-dependencies", "dev"), ("build-dependencies", "build")]
    {
        let Some(table) = document.get(section).and_then(toml::Value::as_table) else {
            continue;
        };

        for (name, value) in table {
            let (requirement, optional, features, workspace, path, git, branch, tag, package) =
                match value {
                    toml::Value::String(requirement) => (
                        Some(requirement.clone()),
                        false,
                        Vec::new(),
                        false,
                        None,
                        None,
                        None,
                        None,
                        None,
                    ),
                    toml::Value::Table(table) => {
                        let requirement =
                            table.get("version").and_then(toml::Value::as_str).map(str::to_owned);

                        let optional =
                            table.get("optional").and_then(toml::Value::as_bool).unwrap_or(false);

                        let features = table
                            .get("features")
                            .and_then(toml::Value::as_array)
                            .map(|values| {
                                values
                                    .iter()
                                    .filter_map(toml::Value::as_str)
                                    .map(str::to_owned)
                                    .collect()
                            })
                            .unwrap_or_default();

                        let workspace =
                            table.get("workspace").and_then(toml::Value::as_bool).unwrap_or(false);

                        let path =
                            table.get("path").and_then(toml::Value::as_str).map(str::to_owned);

                        let git = table.get("git").and_then(toml::Value::as_str).map(str::to_owned);

                        let branch =
                            table.get("branch").and_then(toml::Value::as_str).map(str::to_owned);

                        let tag = table.get("tag").and_then(toml::Value::as_str).map(str::to_owned);

                        let package =
                            table.get("package").and_then(toml::Value::as_str).map(str::to_owned);

                        (
                            requirement,
                            optional,
                            features,
                            workspace,
                            path,
                            git,
                            branch,
                            tag,
                            package,
                        )
                    }
                    _ => (None, false, Vec::new(), false, None, None, None, None, None),
                };

            dependencies.push(StudyDependency {
                name: name.clone(),
                kind: kind.to_string(),
                requirement,
                optional,
                features,
                workspace,
                path,
                git,
                branch,
                tag,
                package,
                workspace_resolved: None,
            });
        }
    }

    dependencies.sort_by(|a, b| {
        (a.kind.as_str(), a.name.as_str()).cmp(&(b.kind.as_str(), b.name.as_str()))
    });

    Some(StudyManifest {
        path: relative.to_string(),
        format: "Cargo".to_string(),
        package_name,
        package_version,
        dependencies,
    })
}

fn load_workspace_dependencies(root: &Path) -> BTreeMap<String, toml::Value> {
    let mut current = Some(root);

    while let Some(directory) = current {
        let manifest_path = directory.join("Cargo.toml");

        if let Ok(source) = fs::read_to_string(&manifest_path) {
            if let Ok(document) = source.parse::<toml::Value>() {
                if document.get("workspace").is_some() {
                    return document
                        .get("workspace")
                        .and_then(toml::Value::as_table)
                        .and_then(|workspace| workspace.get("dependencies"))
                        .and_then(toml::Value::as_table)
                        .map(|dependencies| {
                            dependencies
                                .iter()
                                .map(|(name, value)| (name.clone(), value.clone()))
                                .collect()
                        })
                        .unwrap_or_default();
                }
            }
        }

        current = directory.parent();
    }

    BTreeMap::new()
}

fn workspace_dependency(value: &toml::Value) -> Option<WorkspaceDependency> {
    match value {
        toml::Value::String(version) => Some(WorkspaceDependency {
            version: Some(version.clone()),
            features: Vec::new(),
            default_features: None,
            optional: None,
            path: None,
            git: None,
            branch: None,
            tag: None,
            package: None,
        }),
        toml::Value::Table(table) => Some(WorkspaceDependency {
            version: table.get("version").and_then(toml::Value::as_str).map(str::to_owned),
            features: table
                .get("features")
                .and_then(toml::Value::as_array)
                .map(|values| {
                    values.iter().filter_map(toml::Value::as_str).map(str::to_owned).collect()
                })
                .unwrap_or_default(),
            default_features: table.get("default-features").and_then(toml::Value::as_bool),
            optional: table.get("optional").and_then(toml::Value::as_bool),
            path: table.get("path").and_then(toml::Value::as_str).map(str::to_owned),
            git: table.get("git").and_then(toml::Value::as_str).map(str::to_owned),
            branch: table.get("branch").and_then(toml::Value::as_str).map(str::to_owned),
            tag: table.get("tag").and_then(toml::Value::as_str).map(str::to_owned),
            package: table.get("package").and_then(toml::Value::as_str).map(str::to_owned),
        }),
        _ => None,
    }
}

fn render_markdown(report: &StudyReport) -> String {
    let mut markdown = String::new();

    let mut entity_counts = BTreeMap::new();

    for entity in &report.entities {
        *entity_counts.entry(entity.kind.as_str()).or_insert(0usize) += 1;
    }

    let total_source_files = report.files.len();

    let total_bytes: usize = report.files.iter().map(|file| file.bytes).sum();

    let total_lines: usize = report.files.iter().map(|file| file.lines).sum();

    let clean_files =
        report.files.iter().filter(|file| matches!(file.parse_status, ParseStatus::Clean)).count();

    let parse_health = if total_source_files == 0 {
        100.0
    } else {
        (clean_files as f64 / total_source_files as f64) * 100.0
    };

    let mut functions_by_file = BTreeMap::new();

    for function in &report.functions {
        *functions_by_file.entry(function.path.as_str()).or_insert(0usize) += 1;
    }

    let mut entities_by_file = BTreeMap::new();

    for entity in &report.entities {
        *entities_by_file.entry(entity.path.as_str()).or_insert(0usize) += 1;
    }

    let mut largest_files: Vec<&StudyFile> = report.files.iter().collect();
    largest_files.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.path.cmp(&b.path)));

    let mut function_hotspots: Vec<(&str, usize)> =
        functions_by_file.iter().map(|(path, count)| (*path, *count)).collect();

    function_hotspots.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

    let mut structural_hotspots: Vec<(&str, usize)> =
        entities_by_file.iter().map(|(path, count)| (*path, *count)).collect();

    structural_hotspots.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

    let total_dependencies: usize =
        report.manifest_details.iter().map(|manifest| manifest.dependencies.len()).sum();

    let workspace_dependencies: usize = report
        .manifest_details
        .iter()
        .flat_map(|manifest| manifest.dependencies.iter())
        .filter(|dependency| dependency.workspace)
        .count();

    let resolved_workspace_dependencies: usize = report
        .manifest_details
        .iter()
        .flat_map(|manifest| manifest.dependencies.iter())
        .filter(|dependency| dependency.workspace && dependency.workspace_resolved.is_some())
        .count();

    markdown.push_str("# Repository Study\n\n");
    markdown.push_str(&format!("**Root:** `{}`\n\n", report.root));
    markdown.push_str(&format!("**Schema version:** `{}`\n\n", report.schema_version));

    markdown.push_str("## Executive Summary\n\n");
    markdown.push_str(&format!(
        "This deterministic study indexed **{} source files**, **{} functions**, and **{} structural entities** across **{} languages**. ",
        total_source_files,
        report.functions.len(),
        report.entities.len(),
        report.language_counts.len()
    ));
    markdown.push_str(&format!(
        "The source inventory contains approximately **{} lines** and **{} bytes**. ",
        total_lines, total_bytes
    ));
    markdown.push_str(&format!(
        "Tree-sitter parse health is **{:.1}%**, with **{} diagnostic file(s)** and **{} read failure(s)**. ",
        parse_health,
        report.tree_sitter_diagnostics.len(),
        report.read_failures.len()
    ));
    markdown.push_str(&format!(
        "The dependency inventory contains **{} dependency records**, including **{} workspace dependencies**, of which **{} are resolved** from the workspace declaration.\n\n",
        total_dependencies,
        workspace_dependencies,
        resolved_workspace_dependencies
    ));

    markdown.push_str("## Repository Metrics\n\n");
    markdown.push_str("| Metric | Value |\n|---|---:|\n");
    markdown.push_str(&format!("| Source files | {} |\n", total_source_files));
    markdown.push_str(&format!("| Total lines | {} |\n", total_lines));
    markdown.push_str(&format!("| Total bytes | {} |\n", total_bytes));
    markdown.push_str(&format!("| Functions | {} |\n", report.functions.len()));
    markdown.push_str(&format!("| Structural entities | {} |\n", report.entities.len()));
    markdown.push_str(&format!("| Languages | {} |\n", report.language_counts.len()));
    markdown.push_str(&format!("| Dependencies | {} |\n", total_dependencies));
    markdown.push_str(&format!("| Workspace dependencies | {} |\n", workspace_dependencies));
    markdown.push_str(&format!(
        "| Workspace dependencies resolved | {} |\n",
        resolved_workspace_dependencies
    ));
    markdown.push_str(&format!("| Parse health | {:.1}% |\n", parse_health));
    markdown.push_str(&format!(
        "| Tree-sitter diagnostics | {} |\n",
        report.tree_sitter_diagnostics.len()
    ));
    markdown.push_str(&format!("| Read failures | {} |\n", report.read_failures.len()));
    markdown.push_str("| Compiler validation | Not performed |\n\n");

    markdown.push_str("## Languages\n\n");
    markdown.push_str("| Language | Files |\n|---|---:|\n");

    for (language, count) in &report.language_counts {
        markdown.push_str(&format!("| {language} | {count} |\n"));
    }

    markdown.push_str("\n## Largest Files\n\n");
    markdown.push_str("| Rank | File | Bytes | Lines |\n|---:|---|---:|---:|\n");

    for (index, file) in largest_files.iter().take(10).enumerate() {
        markdown.push_str(&format!(
            "| {} | `{}` | {} | {} |\n",
            index + 1,
            file.path,
            file.bytes,
            file.lines
        ));
    }

    if largest_files.is_empty() {
        markdown.push_str("| - | None detected | - | - |\n");
    }

    markdown.push_str("\n## Function Hotspots\n\n");
    markdown.push_str("| Rank | File | Functions |\n|---:|---|---:|\n");

    for (index, (path, count)) in function_hotspots.iter().take(10).enumerate() {
        markdown.push_str(&format!("| {} | `{}` | {} |\n", index + 1, path, count));
    }

    if function_hotspots.is_empty() {
        markdown.push_str("| - | None detected | - |\n");
    }

    markdown.push_str("\n## Structural Hotspots\n\n");
    markdown.push_str("| Rank | File | Entities |\n|---:|---|---:|\n");

    for (index, (path, count)) in structural_hotspots.iter().take(10).enumerate() {
        markdown.push_str(&format!("| {} | `{}` | {} |\n", index + 1, path, count));
    }

    if structural_hotspots.is_empty() {
        markdown.push_str("| - | None detected | - |\n");
    }

    markdown.push_str("\n## Structural Entities\n\n");
    markdown.push_str(&format!("- Entities discovered: {}\n\n", report.entities.len()));

    markdown.push_str("| Kind | Count |\n|---|---:|\n");

    for (kind, count) in &entity_counts {
        markdown.push_str(&format!("| {kind} | {count} |\n"));
    }

    markdown.push_str("\n## Dependency Architecture\n\n");
    markdown.push_str(&format!("- Dependency records: {}\n", total_dependencies));
    markdown.push_str(&format!("- Workspace dependencies: {}\n", workspace_dependencies));
    markdown.push_str(&format!(
        "- Workspace dependencies resolved: {}\n\n",
        resolved_workspace_dependencies
    ));

    for manifest in &report.manifest_details {
        markdown.push_str(&format!("### `{}` ({})\n\n", manifest.path, manifest.format));

        if let Some(name) = &manifest.package_name {
            markdown.push_str(&format!("- Package: `{name}`\n"));
        }

        if let Some(version) = &manifest.package_version {
            markdown.push_str(&format!("- Version: `{version}`\n"));
        }

        markdown
            .push_str("\n| Dependency | Kind | Workspace | Requirement |\n|---|---|---:|---|\n");

        for dependency in &manifest.dependencies {
            let requirement = dependency
                .requirement
                .as_deref()
                .or_else(|| {
                    dependency
                        .workspace_resolved
                        .as_ref()
                        .and_then(|resolved| resolved.version.as_deref())
                })
                .unwrap_or("-");

            markdown.push_str(&format!(
                "| `{}` | {} | {} | `{}` |\n",
                dependency.name, dependency.kind, dependency.workspace, requirement
            ));
        }

        markdown.push('\n');
    }

    if report.manifest_details.is_empty() {
        markdown.push_str("No parsed manifest details detected.\n\n");
    }

    markdown.push_str("## Workspace Dependencies\n\n");

    let mut printed_workspace_dependency = false;

    for manifest in &report.manifest_details {
        for dependency in &manifest.dependencies {
            if !dependency.workspace {
                continue;
            }

            printed_workspace_dependency = true;

            let resolved = dependency.workspace_resolved.as_ref();

            let version = resolved.and_then(|value| value.version.as_deref()).unwrap_or("-");

            let features = resolved
                .map(|value| {
                    if value.features.is_empty() {
                        "-".to_owned()
                    } else {
                        value.features.join(", ")
                    }
                })
                .unwrap_or_else(|| "-".to_owned());

            markdown.push_str(&format!(
                "- `{}` → version `{}`; features: `{}`\n",
                dependency.name, version, features
            ));
        }
    }

    if !printed_workspace_dependency {
        markdown.push_str("None detected.\n");
    }

    markdown.push('\n');

    markdown.push_str("## Parse Health\n\n");
    markdown.push_str(&format!("- Clean source files: {} / {}\n", clean_files, total_source_files));
    markdown.push_str(&format!("- Parse health: {:.1}%\n\n", parse_health));

    markdown.push_str("## Manifests\n\n");

    for manifest in &report.manifests {
        markdown.push_str(&format!("- `{manifest}`\n"));
    }

    if report.manifests.is_empty() {
        markdown.push_str("- None detected\n");
    }

    markdown.push_str("\n## Tree-sitter Diagnostics\n\n");

    if report.tree_sitter_diagnostics.is_empty() {
        markdown.push_str("None detected.\n");
    } else {
        for failure in &report.tree_sitter_diagnostics {
            markdown.push_str(&format!("- `{}` ({})\n", failure.path, failure.language));
        }
    }

    markdown.push_str("\n## Read Failures\n\n");

    if report.read_failures.is_empty() {
        markdown.push_str("None detected.\n");
    } else {
        for failure in &report.read_failures {
            markdown.push_str(&format!("- `{}`: {}\n", failure.path, failure.error));
        }
    }

    markdown.push_str("\n## Deterministic Observations\n\n");

    if total_source_files > 0 {
        markdown.push_str(&format!(
            "- The repository contains {} source files across {} detected languages.\n",
            total_source_files,
            report.language_counts.len()
        ));
    }

    if !function_hotspots.is_empty() {
        markdown.push_str(&format!(
            "- The highest function-count file is `{}` with {} discovered functions.\n",
            function_hotspots[0].0, function_hotspots[0].1
        ));
    }

    if !structural_hotspots.is_empty() {
        markdown.push_str(&format!(
            "- The highest structural-entity file is `{}` with {} entities.\n",
            structural_hotspots[0].0, structural_hotspots[0].1
        ));
    }

    if !largest_files.is_empty() {
        markdown.push_str(&format!(
            "- The largest indexed file is `{}` at {} bytes and {} lines.\n",
            largest_files[0].path, largest_files[0].bytes, largest_files[0].lines
        ));
    }

    if report.tree_sitter_diagnostics.is_empty() {
        markdown.push_str("- No Tree-sitter diagnostics were detected.\n");
    } else {
        markdown.push_str(&format!(
            "- {} source file(s) produced Tree-sitter diagnostics and should be reviewed separately from compiler validation.\n",
            report.tree_sitter_diagnostics.len()
        ));
    }

    if report.read_failures.is_empty() {
        markdown.push_str("- No source-read failures were detected.\n");
    } else {
        markdown.push_str(&format!(
            "- {} source-read failure(s) were detected.\n",
            report.read_failures.len()
        ));
    }

    markdown
}
