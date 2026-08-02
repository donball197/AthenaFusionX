use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// --- Cognitive Schema Definitions ---

#[derive(Deserialize)]
struct TestFixture {
    scenario_id: String,
    description: String,
    inputs: Inputs,
    expected_outputs: ExpectedOutputs,
}

#[derive(Deserialize)]
struct Inputs {
    target_mutation: Value,
    mock_graph: Value,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct ExpectedOutputs {
    impact_classification: String,
    critical_blast_radius: Vec<String>,
    low_impact_radius: Vec<String>,
}

// --- Execution Harness ---

#[test]
fn validate_cognitive_impact_scenarios() {
    let fixtures_dir = Path::new("tests/fixtures");

    assert!(fixtures_dir.exists(), "Fixtures directory missing: {:?}", fixtures_dir.canonicalize());

    for entry in fs::read_dir(fixtures_dir).expect("Failed to read fixtures directory") {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            run_scenario(&path);
        }
    }
}

fn run_scenario(fixture_path: &PathBuf) {
    let fixture_data = fs::read_to_string(fixture_path).expect("Failed to read fixture file");
    let fixture: TestFixture = serde_json::from_str(&fixture_data)
        .unwrap_or_else(|e| panic!("Failed to parse JSON in {:?}: {}", fixture_path, e));

    println!(">> Running Cognitive Scenario [{}]: {}", fixture.scenario_id, fixture.description);

    let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let mutation_map_path = temp_dir.path().join("mutation_map.json");
    let symbol_graph_path = temp_dir.path().join("symbol_graph.json");
    let impact_output_path = temp_dir.path().join("impact.json");

    fs::write(&mutation_map_path, serde_json::to_string(&fixture.inputs.target_mutation).unwrap())
        .expect("Failed to write mock mutation map");
    fs::write(&symbol_graph_path, serde_json::to_string(&fixture.inputs.mock_graph).unwrap())
        .expect("Failed to write mock symbol graph");

    // 2. Invoke the compiled binary as a Black Box
    let status = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "impact_analyzer",
            "--",
            "--mutation-map",
            mutation_map_path.to_str().unwrap(),
            "--symbol-graph",
            symbol_graph_path.to_str().unwrap(),
            "--output",
            impact_output_path.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to execute impact_analyzer binary");

    assert!(
        status.success(),
        "[{}] impact_analyzer crashed or returned non-zero exit code",
        fixture.scenario_id
    );

    // 3. Read and Assert the Output
    let output_data = fs::read_to_string(&impact_output_path)
        .expect("impact_analyzer failed to produce impact.json");
    let actual_output: ExpectedOutputs =
        serde_json::from_str(&output_data).expect("Failed to parse impact_analyzer output JSON");

    // 4. Mathematical Graph Validation
    assert_eq!(
        actual_output.impact_classification, fixture.expected_outputs.impact_classification,
        "[{}] Impact classification mismatch",
        fixture.scenario_id
    );

    for critical_node in &fixture.expected_outputs.critical_blast_radius {
        assert!(
            actual_output.critical_blast_radius.contains(critical_node),
            "[{}] Missing critical blast radius node: {}",
            fixture.scenario_id,
            critical_node
        );
    }

    for low_node in &fixture.expected_outputs.low_impact_radius {
        assert!(
            actual_output.low_impact_radius.contains(low_node),
            "[{}] Missing low impact radius node: {}",
            fixture.scenario_id,
            low_node
        );
    }

    println!(">> [OK] Scenario {} passed.", fixture.scenario_id);
}
