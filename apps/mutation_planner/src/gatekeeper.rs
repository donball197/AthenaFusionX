use crate::telemetry::ImpactReport;

pub enum ActionPlan {
    AutoPatch(String),
    ReviewRequired(String),
    HumanApproval(String),
}

pub fn evaluate_risk(report: &ImpactReport, prompt: &str) -> ActionPlan {
    let base_context = format!(
        "Target: {}\nBlast Radius: {} dependents.\nUser Prompt: {}",
        report.target_node, report.total_dependents, prompt
    );

    match report.risk_level.as_str() {
        "LOW" => ActionPlan::AutoPatch(format!(
            "[CONSTRAINT: AUTO-PATCH]\nGenerate direct patch instructions.\nContext:\n{}",
            base_context
        )),
        "MEDIUM" => ActionPlan::ReviewRequired(format!(
            "[CONSTRAINT: REVIEW-REQUIRED]\nAnalyze side effects before proposing patch.\nContext:\n{}",
            base_context
        )),
        _ => ActionPlan::HumanApproval(format!(
            "[CONSTRAINT: HUMAN-GATE]\nHalt patching. Generate EVOLUTION_REPORT.md detailing risks.\nContext:\n{}",
            base_context
        )),
    }
}
