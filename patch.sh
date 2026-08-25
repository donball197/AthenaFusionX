#!/usr/bin/env bash
# AthenaFusionX — evidence markers + Bayesian calibration + loop fixes A/B
# Run from workspace root:  bash install_upgrades.sh
set -euo pipefail

ROOT="$(pwd)"
CORE="$ROOT/crates/runtime_core/src"
test -d "$CORE" || { echo "error: run from AthenaFusionX root (missing $CORE)"; exit 1; }

# ============================================================
# 1. evidence_markers.rs
# ============================================================
cat << 'EOF' > "$CORE/evidence_markers.rs"
//! Shared marker predicates over the free-form evidence log.
//! Keep local_evidence as String; query it through these helpers only.

/// Successful substrate mutation markers produced by the executor.
pub fn has_mutation_marker(evidence: &str) -> bool {
    evidence.lines().any(|l| {
        l.starts_with("[WRITE_OK]")
            || l.starts_with("[PATCH_FUNCTION_OK]")
            || l.starts_with("[REPLACE_LINES_OK]")
            || l.starts_with("[DIFF_PATCH_OK]")
    })
}

/// Successful recon / read / search / command markers.
pub fn has_recon_marker(evidence: &str) -> bool {
    evidence.lines().any(|l| {
        l.starts_with("[READ_OK]")
            || l.starts_with("[READ_CHUNK_OK]")
            || l.starts_with("[SEARCH_CODE_OK]")
            || l.starts_with("[CMD_OK]")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_detects_write_and_patch() {
        assert!(has_mutation_marker("[WRITE_OK] foo.rs\n"));
        assert!(has_mutation_marker(
            "[PATCH_FUNCTION_OK] Updated function 'x' in a.rs\n"
        ));
        assert!(!has_mutation_marker("[READ_OK] foo.rs\n"));
    }

    #[test]
    fn recon_detects_read_search_cmd() {
        assert!(has_recon_marker("[READ_OK] a\n"));
        assert!(has_recon_marker(
            "[SEARCH_CODE_OK] Path: crates | Query: 'x'\n"
        ));
        assert!(has_recon_marker("[CMD_OK] Status: 0\n"));
        assert!(!has_recon_marker("[WRITE_OK] a\n"));
    }
}
EOF
echo "OK  evidence_markers.rs"

# ============================================================
# 2. bayesian_calibration.rs
# ============================================================
cat << 'EOF' > "$CORE/bayesian_calibration.rs"
//! Bayesian calibration of hypothesis belief from tool evidence and LLM modifiers.
//!
//! logit(posterior) = logit(prior) + log(P(E|H)/P(E|¬H)) + clamped LLM LLR

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[inline]
fn logit(p: f32) -> f32 {
    let p = p.clamp(1e-6, 1.0 - 1e-6);
    (p / (1.0 - p)).ln()
}

#[inline]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceClass {
    MutationSuccess,
    MutationFailure,
    ReconSuccess,
    Halt,
    TestSuccess,
    TestFailure,
    CompileOk,
    CompileFail,
    ActionBlocked,
    Unknown,
}

impl EvidenceClass {
    pub fn classify(blob: &str) -> Self {
        let t = blob.trim_start();
        if t.contains("[MUTATION_REVERTED]") || t.contains("[COMPILE_FAIL]") {
            if t.contains("[COMPILE_FAIL]") {
                return EvidenceClass::CompileFail;
            }
            return EvidenceClass::MutationFailure;
        }
        if t.starts_with("[COMPILE_OK]") || t.contains("\n[COMPILE_OK]") {
            return EvidenceClass::CompileOk;
        }
        if t.starts_with("[WRITE_OK]")
            || t.starts_with("[DIFF_PATCH_OK]")
            || t.starts_with("[PATCH_FUNCTION_OK]")
            || t.starts_with("[REPLACE_LINES_OK]")
        {
            return EvidenceClass::MutationSuccess;
        }
        if t.starts_with("[TEST_OK]") {
            return EvidenceClass::TestSuccess;
        }
        if t.starts_with("[TEST_FAILED]") {
            return EvidenceClass::TestFailure;
        }
        if t.starts_with("[HALT]") {
            return EvidenceClass::Halt;
        }
        if t.starts_with("[ACTION_GATE_BLOCKED]") || t.starts_with("[ACTION FAILED]") {
            return EvidenceClass::ActionBlocked;
        }
        if t.starts_with("[READ_OK]")
            || t.starts_with("[READ_CHUNK_OK]")
            || t.starts_with("[SEARCH_CODE_OK]")
            || t.starts_with("[CMD_OK]")
            || t.starts_with("[SEARCH_OK]")
            || t.starts_with("[CACHE_HIT]")
        {
            return EvidenceClass::ReconSuccess;
        }
        EvidenceClass::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LikelihoodTable {
    pub p_e_given_h: HashMap<String, f32>,
    pub p_e_given_not_h: HashMap<String, f32>,
}

impl Default for LikelihoodTable {
    fn default() -> Self {
        use EvidenceClass::*;
        let mut p_h = HashMap::new();
        let mut p_nh = HashMap::new();
        let rows: &[(EvidenceClass, f32, f32)] = &[
            (MutationSuccess, 0.85, 0.08),
            (MutationFailure, 0.12, 0.55),
            (CompileOk, 0.80, 0.15),
            (CompileFail, 0.10, 0.60),
            (TestSuccess, 0.82, 0.12),
            (TestFailure, 0.15, 0.55),
            (ReconSuccess, 0.55, 0.45),
            (Halt, 0.70, 0.25),
            (ActionBlocked, 0.20, 0.40),
            (Unknown, 0.50, 0.50),
        ];
        for (c, a, b) in rows {
            p_h.insert(format!("{c:?}"), *a);
            p_nh.insert(format!("{c:?}"), *b);
        }
        Self {
            p_e_given_h: p_h,
            p_e_given_not_h: p_nh,
        }
    }
}

impl LikelihoodTable {
    fn lr(&self, class: EvidenceClass) -> f32 {
        let key = format!("{class:?}");
        let p_h = self
            .p_e_given_h
            .get(&key)
            .copied()
            .unwrap_or(0.5)
            .clamp(1e-4, 1.0 - 1e-4);
        let p_nh = self
            .p_e_given_not_h
            .get(&key)
            .copied()
            .unwrap_or(0.5)
            .clamp(1e-4, 1.0 - 1e-4);
        p_h / p_nh
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifierConfig {
    pub scale: f32,
    pub max_abs_llr: f32,
}

impl Default for ModifierConfig {
    fn default() -> Self {
        Self {
            scale: 2.0,
            max_abs_llr: 1.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationReport {
    pub prior: f32,
    pub posterior: f32,
    pub evidence_class: EvidenceClass,
    pub llr_marker: f32,
    pub llr_modifier: f32,
    pub llr_total: f32,
}

impl CalibrationReport {
    pub fn edge_strength(&self) -> f32 {
        let a = self.llr_total.abs();
        (1.0 - (-a).exp()).clamp(0.05, 0.95)
    }

    pub fn supports(&self) -> bool {
        self.llr_total >= 0.0
    }
}

#[derive(Debug, Clone)]
pub struct BayesianCalibrator {
    pub table: LikelihoodTable,
    pub modifier: ModifierConfig,
    pub shrinkage: f32,
}

impl Default for BayesianCalibrator {
    fn default() -> Self {
        Self {
            table: LikelihoodTable::default(),
            modifier: ModifierConfig::default(),
            shrinkage: 0.05,
        }
    }
}

impl BayesianCalibrator {
    pub fn calibrate(
        &self,
        prior_belief: f32,
        evidence_blob: &str,
        llm_modifier: Option<f32>,
    ) -> CalibrationReport {
        let prior = prior_belief.clamp(1e-6, 1.0 - 1e-6);
        let class = EvidenceClass::classify(evidence_blob);
        let llr_marker = self.table.lr(class).ln();
        let llr_modifier = match llm_modifier {
            Some(m) => {
                let m = m.clamp(-0.5, 0.5);
                (m * self.modifier.scale)
                    .clamp(-self.modifier.max_abs_llr, self.modifier.max_abs_llr)
            }
            None => 0.0,
        };
        let llr_total = llr_marker + llr_modifier;
        let mut posterior = sigmoid(logit(prior) + llr_total);
        if self.shrinkage > 0.0 {
            posterior = posterior * (1.0 - self.shrinkage) + 0.5 * self.shrinkage;
        }
        CalibrationReport {
            prior,
            posterior: posterior.clamp(0.0, 1.0),
            evidence_class: class,
            llr_marker,
            llr_modifier,
            llr_total,
        }
    }

    pub fn update_belief(
        &self,
        prior_belief: f32,
        evidence_blob: &str,
        llm_modifier: Option<f32>,
    ) -> f32 {
        self.calibrate(prior_belief, evidence_blob, llm_modifier)
            .posterior
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_success_raises_belief() {
        let cal = BayesianCalibrator::default();
        let r = cal.calibrate(0.5, "[DIFF_PATCH_OK] Applied changes to foo.py", Some(0.25));
        assert!(r.posterior > 0.5, "posterior={}", r.posterior);
        assert_eq!(r.evidence_class, EvidenceClass::MutationSuccess);
        assert!(r.supports());
    }

    #[test]
    fn compile_fail_lowers_belief() {
        let cal = BayesianCalibrator::default();
        let r = cal.calibrate(0.8, "[COMPILE_FAIL] type error", Some(-0.2));
        assert!(r.posterior < 0.8, "posterior={}", r.posterior);
        assert!(!r.supports());
    }

    #[test]
    fn recon_is_weak() {
        let cal = BayesianCalibrator::default();
        let r = cal.calibrate(0.5, "[READ_OK] foo.py\nCONTENT...", Some(0.05));
        assert!(
            r.posterior > 0.48 && r.posterior < 0.70,
            "posterior={}",
            r.posterior
        );
    }

    #[test]
    fn successive_patches_converge() {
        let cal = BayesianCalibrator::default();
        let mut b = 0.5_f32;
        for _ in 0..3 {
            b = cal.update_belief(
                b,
                "[PATCH_FUNCTION_OK] Updated function 'f' in a.rs",
                Some(0.2),
            );
        }
        assert!(b > 0.85, "belief after 3 patches={}", b);
    }
}
EOF
echo "OK  bayesian_calibration.rs"

# ============================================================
# 3. contract_status.rs (helpers + compound-objective tests)
# ============================================================
cat << 'EOF' > "$CORE/contract_status.rs"
use crate::evidence_markers::{has_mutation_marker, has_recon_marker};

#[derive(Debug, Clone, PartialEq)]
pub enum ContractStatus {
    Satisfied,
    RejectedMutation,
    PartialEvidence,
    NoEvidence,
}

pub struct ContractEvaluator;

impl ContractEvaluator {
    pub fn evaluate(
        is_mutation_task: bool,
        is_write_task: bool,
        local_evidence: &str,
        current_belief: f32,
        mission: &str,
    ) -> ContractStatus {
        let has_mutation = has_mutation_marker(local_evidence);
        let has_recon = has_recon_marker(local_evidence);

        if (is_mutation_task || is_write_task) && !has_mutation {
            return if has_recon {
                ContractStatus::PartialEvidence
            } else {
                ContractStatus::NoEvidence
            };
        }

        // FIX B companion: mutation marker + high belief counts as satisfied
        // even if compound keyword scan is incomplete (e.g. belief 0.90–0.94).
        if has_mutation && current_belief >= 0.90 {
            return ContractStatus::Satisfied;
        }

        if Self::is_compound_objective_satisfied(mission, local_evidence) {
            if current_belief >= 0.95 {
                return ContractStatus::Satisfied;
            }
            return ContractStatus::PartialEvidence;
        }

        if has_mutation || has_recon {
            return if current_belief >= 0.95 {
                ContractStatus::Satisfied
            } else {
                ContractStatus::PartialEvidence
            };
        }

        ContractStatus::NoEvidence
    }

    fn is_compound_objective_satisfied(mission: &str, evidence: &str) -> bool {
        let mission_lower = mission.to_lowercase();
        let objective_part = mission_lower
            .lines()
            .find(|line| line.starts_with("objective:"))
            .unwrap_or(mission_lower.as_str());

        let evidence_lower = evidence.to_lowercase();

        let and_segments: Vec<&str> = objective_part
            .split(" and ")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if and_segments.is_empty() {
            return false;
        }

        and_segments.iter().all(|segment| {
            let or_parts: Vec<&str> = segment
                .split(" or ")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();

            if or_parts.len() <= 1 {
                segment
                    .split_whitespace()
                    .filter(|w| w.len() > 3)
                    .any(|w| evidence_lower.contains(w))
            } else {
                or_parts.iter().any(|part| {
                    part.split_whitespace()
                        .filter(|w| w.len() > 3)
                        .any(|w| evidence_lower.contains(w))
                })
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compound(mission: &str, evidence: &str) -> bool {
        ContractEvaluator::is_compound_objective_satisfied(mission, evidence)
    }

    #[test]
    fn and_requires_all_segments() {
        let mission = "objective: clone repository and write summary document";
        assert!(!compound(mission, "cloned the repository successfully"));
        assert!(compound(
            mission,
            "cloned the repository and wrote the summary document"
        ));
    }

    #[test]
    fn or_requires_any_part() {
        let mission = "objective: use patch_function or diff_patch on the target";
        assert!(compound(
            mission,
            "[PATCH_FUNCTION_OK] Updated function 'foo' in target.rs"
        ));
        assert!(!compound(mission, "[READ_OK] target.rs\n"));
    }

    #[test]
    fn mutation_high_belief_satisfied() {
        let status = ContractEvaluator::evaluate(
            true,
            false,
            "[DIFF_PATCH_OK] Applied changes to server.py\n",
            0.92,
            "objective: fix command injection",
        );
        assert_eq!(status, ContractStatus::Satisfied);
    }

    #[test]
    fn mutation_task_without_marker_is_partial_or_none() {
        let status = ContractEvaluator::evaluate(
            true,
            false,
            "[READ_OK] server.py\n",
            0.99,
            "objective: fix command injection",
        );
        assert_eq!(status, ContractStatus::PartialEvidence);
    }
}
EOF
echo "OK  contract_status.rs"

# ============================================================
# 4. Register modules in lib.rs
# ============================================================
for mod in evidence_markers bayesian_calibration; do
  if grep -q "pub mod $mod" "$CORE/lib.rs" 2>/dev/null; then
    echo "SKIP mod $mod (already in lib.rs)"
  else
    echo "pub mod $mod;" >> "$CORE/lib.rs"
    echo "OK  pub mod $mod; appended to lib.rs"
  fi
done

# ============================================================
# 5. Patch cortical_loop.rs (A: anti-loop, B: early complete, Bayes in evaluate)
# ============================================================
python3 << 'PY'
from pathlib import Path
import re
import sys

path = Path("crates/runtime_core/src/cortical_loop.rs")
text = path.read_text()
n = 0

# --- B1 strategize early complete after Satisfied block ---
s_anchor = '''        if status == ContractStatus::Satisfied {
            self.log("\\x1b[1;36m>> [CORTEX OMNI-GUARD] Contract mathematically satisfied. Stripping LLM authority and forcing deterministic node completion.\\x1b[0m".to_string());
            return Ok(CortexState::Complete { belief: current_belief, evidence: session.collect_evidence(h_id) });
        }'''

s_insert = '''        if status == ContractStatus::Satisfied {
            self.log("\\x1b[1;36m>> [CORTEX OMNI-GUARD] Contract mathematically satisfied. Stripping LLM authority and forcing deterministic node completion.\\x1b[0m".to_string());
            return Ok(CortexState::Complete { belief: current_belief, evidence: session.collect_evidence(h_id) });
        }

        // FIX B: mutation marker + high belief → force completion
        if (is_mutation_task || is_write_task)
            && crate::evidence_markers::has_mutation_marker(&session.local_evidence)
            && current_belief >= 0.90
        {
            self.log(format!(
                "\\x1b[1;36m>> [CORTEX] Mutation marker acquired with high belief ({:.2}). Forcing completion.\\x1b[0m",
                current_belief
            ));
            return Ok(CortexState::Complete {
                belief: current_belief.max(0.95),
                evidence: session.collect_evidence(h_id),
            });
        }'''

if "Mutation marker acquired with high belief" not in text:
    if s_anchor in text:
        text = text.replace(s_anchor, s_insert, 1)
        n += 1
        print("OK  B1 strategize early-complete")
    else:
        print("WARN B1 strategize anchor not found")
else:
    print("SKIP B1 already present")

# --- B2 critic early complete ---
c_anchor = '''        if status == ContractStatus::Satisfied {
            self.log("\\x1b[1;36m>> [SUBGOAL_COMPLETE] Evidence is sufficient. Forcing completion.\\x1b[0m".to_string());
            return Ok(CortexState::Complete { belief, evidence: session.collect_evidence(session.best_hypothesis_id.unwrap_or_default()) });
        }'''

c_insert = '''        if status == ContractStatus::Satisfied {
            self.log("\\x1b[1;36m>> [SUBGOAL_COMPLETE] Evidence is sufficient. Forcing completion.\\x1b[0m".to_string());
            return Ok(CortexState::Complete { belief, evidence: session.collect_evidence(session.best_hypothesis_id.unwrap_or_default()) });
        }

        // FIX B: mutation marker + high belief → complete (critic path)
        if (is_mutation_task || is_write_task)
            && crate::evidence_markers::has_mutation_marker(&session.local_evidence)
            && belief >= 0.90
        {
            self.log(format!(
                "\\x1b[1;36m>> [SUBGOAL_COMPLETE] Mutation marker + belief {:.2}. Forcing completion.\\x1b[0m",
                belief
            ));
            return Ok(CortexState::Complete {
                belief: belief.max(0.95),
                evidence: session.collect_evidence(session.best_hypothesis_id.unwrap_or_default()),
            });
        }'''

if "Mutation marker + belief" not in text:
    if c_anchor in text:
        text = text.replace(c_anchor, c_insert, 1)
        n += 1
        print("OK  B2 critic early-complete")
    else:
        print("WARN B2 critic anchor not found")
else:
    print("SKIP B2 already present")

# --- contract_still_unmet helper (idempotent) ---
old_unmet = re.compile(
    r'let contract_still_unmet = \(is_mutation_task \|\| is_write_task\) && '
    r'!session\.local_evidence\.lines\(\)\.any\(\|l\| l\.starts_with\("\[WRITE_OK\]"\) \|\| '
    r'l\.starts_with\("\[PATCH_FUNCTION_OK\]"\) \|\| l\.starts_with\("\[REPLACE_LINES_OK\]"\) \|\| '
    r'l\.starts_with\("\[DIFF_PATCH_OK\]"\)\);'
)
new_unmet = (
    "let contract_still_unmet =\n"
    "                    (is_mutation_task || is_write_task) && "
    "!crate::evidence_markers::has_mutation_marker(&session.local_evidence);"
)
text2, c = old_unmet.subn(new_unmet, text, count=1)
if c:
    text = text2
    n += 1
    print("OK  contract_still_unmet -> has_mutation_marker")
elif "has_mutation_marker(&session.local_evidence)" in text:
    print("SKIP contract_still_unmet already using helper")
else:
    print("WARN contract_still_unmet pattern not found (may already be patched)")

# --- A: anti-loop ---
anti_pat = re.compile(
    r'[ \t]*// ⚡ CONSECUTIVE RECON LIMIT\n'
    r'[ \t]*// Allow reading/searching the same target up to 2 times\.\n'
    r'[ \t]*// On the 3rd attempt, hard-block and force a pivot\.\n'
    r'[ \t]*let attempt_count = session\.action_attempt_count\(&action_key\)\.unwrap_or\(0\);\n'
    r'[ \t]*if attempt_count >= 2 \{.*?'
    r'session\.record_action_attempt\(&action_key\);',
    re.S,
)

anti_new = '''                // ⚡ CONSECUTIVE RECON LIMIT (FIX A)
                // Base limit 2; after mutation marker, recon limit 4 for post-patch verify.
                let attempt_count = session.action_attempt_count(&action_key).unwrap_or(0);
                let is_recon = matches!(
                    action,
                    AgentAction::ReadFile { .. }
                        | AgentAction::ReadFileChunk { .. }
                        | AgentAction::SearchCode { .. }
                        | AgentAction::SearchMemory { .. }
                );
                let mutation_done =
                    crate::evidence_markers::has_mutation_marker(&session.local_evidence);
                let limit: u32 = if is_recon && mutation_done { 4 } else { 2 };

                if attempt_count >= limit {
                    self.log(format!("\\x1b[1;31m>> [ANTI-LOOP] Action [{}] attempted {} times (limit {}). FORCING STRATEGY PIVOT.\\x1b[0m", action_key, attempt_count, limit));
                    let tool_names = crate::policy::TOOL_CONTRACTS.iter().map(|t| t.name).collect::<Vec<_>>().join(", ");
                    let veto_msg = format!("[CRITICAL VETO] LOOP DETECTED! You have repeatedly proposed the exact same action. You are forbidden from proposing this action again. YOU MUST PIVOT YOUR STRATEGY by using a DIFFERENT tool. Available tools: [{}].", tool_names);
                    if let Some(h_id) = session.best_hypothesis_id {
                        let obs_id = session.graph.add_node(NodeKind::Observation, &veto_msg, 1.0, 95);
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.99);
                        session.graph.propagate_beliefs();
                    }
                    return Ok(CortexState::Strategize);
                }
                session.record_action_attempt(&action_key);'''

if "is_recon && mutation_done" not in text:
    text3, c3 = anti_pat.subn(anti_new, text, count=1)
    if c3:
        text = text3
        n += 1
        print("OK  A anti-loop post-mutation limit")
    else:
        print("WARN A anti-loop block not matched — patch manually")
else:
    print("SKIP A already present")

# --- Bayes in handle_evaluate after record_evidence ---
bayes_anchor = "session.record_evidence(&evidence, &eval, h_id);"
bayes_insert = '''session.record_evidence(&evidence, &eval, h_id);

                // Bayesian calibration: marker LR + capped LLM modifier
                if let Some(node) = session.graph.nodes.get_mut(&h_id) {
                    let report = crate::bayesian_calibration::BayesianCalibrator::default()
                        .calibrate(node.belief_score, &evidence, Some(eval.confidence_modifier));
                    node.belief_score = report.posterior;
                    self.log(format!(
                        "\\x1b[1;32m>> [BAYES] {:?} {:.2} → {:.2} (llr={:.2})\\x1b[0m",
                        report.evidence_class, report.prior, report.posterior, report.llr_total
                    ));
                }'''

if "[BAYES]" not in text:
    if bayes_anchor in text:
        text = text.replace(bayes_anchor, bayes_insert, 1)
        n += 1
        print("OK  Bayes wire-in handle_evaluate")
    else:
        print("WARN Bayes anchor record_evidence not found")
else:
    print("SKIP Bayes already present")

path.write_text(text)
print(f"cortical_loop.rs: {n} change group(s) applied")
if n == 0:
    print("NOTE: zero automatic edits — check WARN lines; apply manual snippets if needed")
PY

# ============================================================
# 6. Verify
# ============================================================
echo ""
echo "==> cargo test (markers + bayes + contract_status)"
cargo test -p runtime_core evidence_markers:: bayesian_calibration:: contract_status:: -- --nocapture

echo ""
echo "==> cargo check"
cargo check -p runtime_core

echo ""
echo "==> cargo clippy (advisory)"
cargo clippy -p runtime_core -- -W clippy::all || true

echo ""
echo "DONE. Re-run: ./nexus.sh  then  /mutate ping_ip ..."
