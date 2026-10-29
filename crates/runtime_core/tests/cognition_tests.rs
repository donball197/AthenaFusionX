use nexus_runtime_core::cognition::belief_engine::BeliefEngine;
use nexus_runtime_core::cognition::prediction_engine::PredictionEngine;
use nexus_runtime_core::cognition::reflection_engine::ReflectionEngine;
use nexus_runtime_core::cognition::traits::{MissionOutcome, Reflector};
use uuid::Uuid;

#[test]
fn test_v5_belief_engine_decay() {
    let engine = BeliefEngine::new();
    let val1 = engine.update_belief("gemini_api_healthy", -0.2);
    assert_eq!(val1, 0.3); // 0.5 - 0.2
    let val2 = engine.get_belief("gemini_api_healthy");
    assert_eq!(val2, 0.3);
}

#[test]
fn test_v5_prediction_engine_nearest_neighbor() {
    let engine = PredictionEngine::new();
    let id1 = Uuid::new_v4();

    // Baseline prediction
    let pred1 = engine.generate_prediction(id1, "read_file src/main.rs");
    assert_eq!(pred1.confidence, 0.5);

    // Log a successful hit
    let eval = engine.evaluate_prediction(id1, 0.05, true);
    assert_eq!(eval.accuracy, 1.0);

    // Nearest neighbor signature (read_file_rs) should increase confidence
    let id2 = Uuid::new_v4();
    let pred2 = engine.generate_prediction(id2, "read_file lib/utils.rs");
    assert!(pred2.confidence > 0.5, "Confidence should scale on repeated success");
}

#[tokio::test]
async fn test_v5_reflection_engine_lessons() {
    let engine = ReflectionEngine::new();
    let outcome = MissionOutcome {
        mission_id: Uuid::new_v4(),
        success: false,
        evidence_summary: "Wrote unescaped JSON to disk".to_string(),
        failures: vec!["File corruption".to_string()],
        successful_strategies: vec![],
        raw_telemetry: "".to_string(),
        failed_strategy_id: None,
    };

    let report = engine.reflect(&outcome).await.unwrap();
    assert!(!report.lessons.is_empty());
    assert!(report.lessons[0].contains("File corruption"));
}
