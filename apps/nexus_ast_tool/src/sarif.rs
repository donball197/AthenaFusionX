//! SARIF 2.1.0 and structured JSON report emitters.

use crate::polyglot::{SecurityFinding, SecurityRule};
use anyhow::Result;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct AuditFindingRecord {
    pub finding: SecurityFinding,
    pub triage_state: String,
    pub triage_reason: Option<String>,
    pub model_severity: Option<String>,
    pub remediation_status: String,
    pub remediation_detail: Option<String>,
    pub poc_verified: Option<bool>,
}

fn sarif_level(severity: &str) -> &'static str {
    match severity.trim().to_ascii_lowercase().as_str() {
        "critical" | "high" => "error",
        "medium" => "warning",
        _ => "note",
    }
}

fn normalize_uri(path: &Path, root: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let s = rel.to_string_lossy().replace('\\', "/");
    s.trim_start_matches("./").to_string()
}

pub fn render_sarif(
    records: &[AuditFindingRecord],
    rules: &[SecurityRule],
    scan_root: &Path,
) -> Result<String> {
    let mut rule_map: BTreeMap<String, Value> = BTreeMap::new();
    for rule in rules {
        rule_map.entry(rule.id.clone()).or_insert_with(|| {
            json!({
                "id": rule.id,
                "shortDescription": { "text": rule.description },
                "defaultConfiguration": {
                    "level": sarif_level(&rule.severity)
                },
                "properties": {
                    "severity": rule.severity,
                    "language": rule.language
                }
            })
        });
    }

    let results: Vec<Value> = records
        .iter()
        .map(|rec| {
            let f = &rec.finding;
            let uri = normalize_uri(&f.path, scan_root);
            let mut msg = format!("{} detected", f.pattern_name);
            if let Some(reason) = &rec.triage_reason {
                msg = format!("{}: {}", f.pattern_name, reason);
            } else if !f.taint_path.is_empty() {
                msg = format!("{} (taint path: {})", f.pattern_name, f.taint_path.join(" -> "));
            }

            let mut thread_locs: Vec<Value> = f
                .taint_locations
                .iter()
                .map(|loc| {
                    json!({
                        "location": {
                            "physicalLocation": {
                                "artifactLocation": { "uri": normalize_uri(&loc.path, scan_root) },
                                "region": { "startLine": loc.line.max(1) }
                            },
                            "message": { "text": loc.message }
                        }
                    })
                })
                .collect();

            thread_locs.push(json!({
                "location": {
                    "physicalLocation": {
                        "artifactLocation": { "uri": uri },
                        "region": {
                            "startLine": f.sink_start_line.max(1),
                            "endLine": f.sink_end_line.max(f.sink_start_line.max(1))
                        }
                    },
                    "message": { "text": format!("Dangerous sink `{}`", f.pattern_name) }
                }
            }));

            let mut obj = json!({
                "ruleId": f.pattern_name,
                "level": sarif_level(&f.severity),
                "message": { "text": msg },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": uri },
                        "region": {
                            "startLine": f.sink_start_line.max(1),
                            "endLine": f.sink_end_line.max(f.sink_start_line.max(1)),
                            "snippet": { "text": f.patch_snippet }
                        }
                    }
                }],
                "properties": {
                    "triageState": rec.triage_state,
                    "modelSeverity": rec.model_severity,
                    "remediationStatus": rec.remediation_status,
                    "remediationDetail": rec.remediation_detail,
                    "pocVerified": rec.poc_verified,
                    "taintPath": f.taint_path
                }
            });

            if !f.taint_locations.is_empty() {
                obj["codeFlows"] = json!([{
                    "threadFlows": [{
                        "locations": thread_locs
                    }]
                }]);
            }

            if rec.triage_state == "SAFE" {
                obj["suppressions"] = json!([{
                    "kind": "external",
                    "justification": rec.triage_reason.clone().unwrap_or_else(|| "Classified SAFE by AI triage".to_string())
                }]);
            }

            obj
        })
        .collect();

    let driver_rules: Vec<Value> = rule_map.into_values().collect();

    let sarif = json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "nexus-ast-tool",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/AthenaFusionX",
                    "rules": driver_rules
                }
            },
            "results": results
        }]
    });

    Ok(serde_json::to_string_pretty(&sarif)?)
}

pub fn render_json(records: &[AuditFindingRecord], summary: Value) -> Result<String> {
    let payload = json!({
        "schema_version": 1,
        "tool": "nexus-ast-tool",
        "summary": summary,
        "findings": records
    });
    Ok(serde_json::to_string_pretty(&payload)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callgraph::TaintLocation;
    use crate::polyglot::SupportedLanguage;
    use std::path::PathBuf;

    #[test]
    fn generates_valid_sarif_2_1_with_code_flows() {
        let rec = AuditFindingRecord {
            finding: SecurityFinding {
                language: SupportedLanguage::Python,
                pattern_name: "SQL_INJECTION".to_string(),
                severity: "High".to_string(),
                path: PathBuf::from("/tmp/app/routes/signup.py"),
                start_line: 12,
                end_line: 20,
                source_snippet: "session.execute(text(f\"...\"))".to_string(),
                patch_snippet: "text(f\"...\")".to_string(),
                sink_start_line: 16,
                sink_end_line: 18,
                sink_text: "text(f\"...\")".to_string(),
                taint_path: vec!["do_signup -> validate_token".to_string()],
                taint_locations: vec![TaintLocation {
                    path: PathBuf::from("/tmp/app/routes/signup.py"),
                    line: 48,
                    message: "Tainted argument passed from do_signup()".to_string(),
                }],
            },
            triage_state: "EXPLOITABLE".to_string(),
            triage_reason: Some("Unsanitized SQL interpolation".to_string()),
            model_severity: Some("High".to_string()),
            remediation_status: "FIXED".to_string(),
            remediation_detail: None,
            poc_verified: Some(true),
        };

        let out = render_sarif(&[rec], &[], Path::new("/tmp/app")).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["version"], "2.1.0");
        assert_eq!(
            v["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "routes/signup.py"
        );
        assert!(v["runs"][0]["results"][0]["codeFlows"][0]["threadFlows"][0]["locations"]
            .as_array()
            .unwrap()
            .len()
            >= 2);
    }
}
