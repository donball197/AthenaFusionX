#!/usr/bin/env bash

# phase8c_validation_v2.sh
# AthenaFusionX v5 - Phase 8C Validation
# Matches current cognition API

set -euo pipefail

ROOT="${1:-.}"

TEST_DIR="$ROOT/crates/runtime_core/tests"

echo ">> Phase 8C Validation v2"

mkdir -p "$TEST_DIR"

cat > "$TEST_DIR/cognition_tests.rs" <<'EOF'
#![cfg(feature = "cognition_v5")]

use nexus_runtime_core::cognition::traits::*;
use nexus_runtime_core::cognition::reflection_engine::ReflectionEngine;
use nexus_runtime_core::cognition::event_bus::EventBus;
use nexus_runtime_core::cognition::belief_engine::BeliefEngine;
use nexus_runtime_core::cognition::prediction_engine::PredictionEngine;

use uuid::Uuid;


#[tokio::test]
async fn test_reflection_success_path() {

    let bus = EventBus::new();

    let engine = ReflectionEngine::new(bus);


    let outcome = MissionOutcome {
        mission_id: Uuid::new_v4(),
        success: true,
        evidence_summary: "completed".into(),
        failures: vec![],
        successful_strategies: vec![
            "adaptive_execution".into()
        ],
        lessons: vec![],
    };


    let result = engine
        .reflect(&outcome)
        .await
        .expect("reflection failed");


    assert!(
        !result.is_empty(),
        "successful reflection should produce lessons"
    );
}



#[tokio::test]
async fn test_reflection_failure_path() {

    let bus = EventBus::new();

    let engine = ReflectionEngine::new(bus);


    let outcome = MissionOutcome {
        mission_id: Uuid::new_v4(),
        success: false,
        evidence_summary: "failed".into(),
        failures: vec![
            "timeout".into()
        ],
        successful_strategies: vec![],
        lessons: vec![],
    };


    let result = engine
        .reflect(&outcome)
        .await
        .expect("reflection failed");


    assert!(
        result.iter().any(|x| x.contains("timeout")),
        "failure should propagate into reflection"
    );
}



#[test]
fn test_belief_update_cycle() {

    let engine = BeliefEngine::new();

    let id = Uuid::new_v4();


    let initial = engine.get_belief(id);

    let updated = engine.update_belief(id, 1.0);


    assert!(
        updated > initial,
        "belief should increase after positive evidence"
    );
}



#[test]
fn test_prediction_engine() {

    let engine = PredictionEngine::new();

    let id = Uuid::new_v4();


    let prediction =
        engine.generate_prediction(
            id,
            "file_written",
            0.1
        );


    engine.record_prediction(prediction);


    let result =
        engine.evaluate_prediction(
            id,
            "file_written successfully"
        );


    assert_eq!(result.accuracy,1.0);
    assert_eq!(result.error,0.0);
}
EOF


echo ">> Created:"
echo "$TEST_DIR/cognition_tests.rs"


echo ">> Running integration test"

cargo test \
-p nexus_runtime_core \
--test cognition_tests \
--features cognition_v5


echo ">> Phase 8C validation complete"
