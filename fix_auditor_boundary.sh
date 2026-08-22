#!/usr/bin/env bash
set -euo pipefail

cd ~/AthenaFusionX

MAIN="apps/nexus_ast_tool/src/main.rs"
CLIENT="apps/nexus_ast_tool/src/client.rs"

echo "============================================================"
echo " NEXUS AST TOOL — TRIAGE / VERIFICATION / REPORT FIX"
echo "============================================================"
echo

[[ -f "$MAIN" ]] || { echo "[ERROR] $MAIN not found"; exit 1; }
[[ -f "$CLIENT" ]] || { echo "[ERROR] $CLIENT not found"; exit 1; }

python3 <<'PYEOF'
from pathlib import Path
import sys

main_path = Path("apps/nexus_ast_tool/src/main.rs")
client_path = Path("apps/nexus_ast_tool/src/client.rs")

main = main_path.read_text()
client = client_path.read_text()

changes = 0

def replace_exact(text, old, new, label, path):
    global changes

    count = text.count(old)

    if count != 1:
        print(f"[ERROR] {label}: expected exactly 1 match in {path}, found {count}")
        sys.exit(1)

    print(f"[OK] {label}")
    changes += 1
    return text.replace(old, new, 1)


# ============================================================
# 1. Add explicit UNKNOWN handling.
#
# Current architecture:
#
#     if EXPLOITABLE
#         ...
#     else
#         FALSE POSITIVE
#
# That incorrectly converts UNKNOWN -> FALSE POSITIVE.
# ============================================================

old = '''                            if is_exploitable == "EXPLOITABLE" || is_exploitable == "true" {
                                exploitable_count += 1;
                                println!("        [🔴 EXPLOITABLE] Severity: {}", severity);
                                println!("        [🧠 AI Reasoning] {}", reason);

                                // Add to Markdown Report
'''

new = '''                            let classification = match is_exploitable.trim().to_uppercase().as_str() {
                                "EXPLOITABLE" | "TRUE" => "EXPLOITABLE",
                                "SAFE" | "FALSE" | "FALSE_POSITIVE" => "SAFE",
                                "UNKNOWN" => "UNKNOWN",
                                other => {
                                    println!(
                                        "        [⚠️ TRIAGE POLICY] Unrecognized AI state '{}'; forcing UNKNOWN.",
                                        other
                                    );
                                    "UNKNOWN"
                                }
                            };

                            if classification == "EXPLOITABLE" {
                                println!("        [🔴 EXPLOITABLE] Severity: {}", severity);
                                println!("        [🧠 AI Reasoning] {}", reason);

                                // Add to Markdown Report
'''

main = replace_exact(
    main,
    old,
    new,
    "Replace unsafe triage branch",
    str(main_path),
)


# ============================================================
# 2. Prevent SAFE when the model's reasoning indicates an
#    unresolved assumption.
#
# This is intentionally deterministic.
# ============================================================

old = '''                            } else {
                                println!("        [🟢 FALSE POSITIVE] Safe.\\n        [🧠 AI Reasoning] {}", reason);
                            }
'''

new = '''                            } else if classification == "UNKNOWN" {
                                println!(
                                    "        [🟡 UNKNOWN] Security state cannot be established without assumptions."
                                );
                                println!("        [🧠 AI Reasoning] {}", reason);
                            } else {
                                println!(
                                    "        [🟢 FALSE POSITIVE] Safe.\\n        [🧠 AI Reasoning] {}",
                                    reason
                                );
                            }
'''

main = replace_exact(
    main,
    old,
    new,
    "Add explicit UNKNOWN branch",
    str(main_path),
)


# ============================================================
# 3. Remove the patch_is_safe verification bypass.
#
# BEFORE:
#
#   syntax_ok && (ast_ok || patch_is_safe)
#
# A patch could therefore pass despite AST verification failing.
#
# AFTER:
#
#   syntax_ok && ast_ok
# ============================================================

old = '''                                                let patch_is_safe = replace_block.contains("subprocess.run") || replace_block.contains("subprocess.check_output") || replace_block.contains("mysqli_prepare") || replace_block.contains("run(");
                                                if syntax_ok && (ast_ok || patch_is_safe) {
'''

new = '''                                                // SECURITY BOUNDARY:
                                                // Never allow a heuristic string such as
                                                // "mysqli_prepare" to bypass the AST verifier.
                                                //
                                                // Syntax validity alone is insufficient.
                                                // The original vulnerability must disappear
                                                // from the post-patch security scan.
                                                if syntax_ok && ast_ok {
'''

main = replace_exact(
    main,
    old,
    new,
    "Remove patch_is_safe verification bypass",
    str(main_path),
)


# ============================================================
# 4. Count actionable findings only AFTER final triage.
#
# The old code incremented exploitable_count before any patch
# was attempted.
#
# Keep the report count as "actionable findings", but make it
# count only authoritative EXPLOITABLE states.
# ============================================================

old = '''                                report_md.push_str(&format!("### Description\\n{}\\n\\n", reason));
                                report_md.push_str(&format!("### Vulnerable Code\\n```{}\\n{}\\n```\\n\\n", lang_str.to_lowercase(), finding.source_snippet));
'''

new = '''                                report_md.push_str(&format!("### Description\\n{}\\n\\n", reason));
                                report_md.push_str(&format!(
                                    "### Vulnerable Code\\n```{}\\n{}\\n```\\n\\n",
                                    lang_str.to_lowercase(),
                                    finding.source_snippet
                                ));

                                // This is a triage count, NOT a remediation-success count.
                                // Remediation status is recorded separately below.
                                exploitable_count += 1;
'''

main = replace_exact(
    main,
    old,
    new,
    "Move exploitable count into authoritative branch",
    str(main_path),
)


# ============================================================
# 5. The previous increment must disappear.
# ============================================================

old = '''                            if is_exploitable == "EXPLOITABLE" || is_exploitable == "true" {
                                exploitable_count += 1;
                                println!("        [🔴 EXPLOITABLE] Severity: {}", severity);
'''

# This old block should no longer exist after step 1.
if old in main:
    print("[ERROR] Old exploitable-count increment still exists.")
    sys.exit(1)


# ============================================================
# 6. Fix report wording so "actionable" does not imply "fixed".
#
# We retain the existing report format but make the final console
# statement explicit.
# ============================================================

old = '''                    println!("\\n==> [Report Generator] 📄 Successfully generated 'bounty_report.md' with {} actionable bugs!", exploitable_count);
'''

new = '''                    println!(
                        "\\n==> [Report Generator] 📄 Successfully generated 'bounty_report.md' with {} actionable findings.\\n\
                         ==> [Report Generator] ℹ️ Actionable count is NOT a remediation-success count.",
                        exploitable_count
                    );
'''

main = replace_exact(
    main,
    old,
    new,
    "Clarify report count semantics",
    str(main_path),
)


# ============================================================
# CLIENT-SIDE HARDENING
#
# The prompt already says UNKNOWN, but make the contract stronger:
# - exact enum
# - no prose outside JSON
# - assumptions must force UNKNOWN
# ============================================================

old = '''             3. If you must make ANY ASSUMPTION about external data validation or session trust, return "UNKNOWN".\\n\
             Return ONLY a JSON object wrapped in <payload></payload> tags like this:\\n{{"state": "EXPLOITABLE", "severity": "High", "reason": "..."}}",
'''

new = '''             3. If you must make ANY ASSUMPTION about external data validation, session trust, provenance, or sanitization, return "UNKNOWN".\\n\
             4. "SAFE" is permitted ONLY when the supplied source proves the sink is mitigated without assumptions.\\n\
             5. The state MUST be exactly one of: "EXPLOITABLE", "SAFE", "UNKNOWN".\\n\
             6. Never return "FALSE POSITIVE", "TRUE", "FALSE", or any other state.\\n\
             Return ONLY a JSON object wrapped in <payload></payload> tags like this:\\n{{"state": "EXPLOITABLE", "severity": "High", "reason": "..."}}",
'''

client = replace_exact(
    client,
    old,
    new,
    "Harden triage state contract",
    str(client_path),
)


# ============================================================
# Write changes
# ============================================================

main_path.write_text(main)
client_path.write_text(client)

print()
print(f"[OK] Applied {changes} surgical changes.")
PYEOF

echo
echo "============================================================"
echo " VERIFYING SOURCE"
echo "============================================================"

echo
echo "=== UNKNOWN handling ==="
rg -n -C 4 \
    'classification == "UNKNOWN"|Unrecognized AI state|\[🟡 UNKNOWN\]' \
    "$MAIN"

echo
echo "=== verification boundary ==="
rg -n -C 5 \
    'SECURITY BOUNDARY|syntax_ok && ast_ok|patch_is_safe' \
    "$MAIN"

echo
echo "=== triage contract ==="
rg -n -C 5 \
    'SAFE.*UNKNOWN|state.*EXPLOITABLE.*SAFE.*UNKNOWN|FALSE POSITIVE' \
    "$CLIENT"

echo
echo "============================================================"
echo " CARGO CHECK"
echo "============================================================"

cargo check -p nexus_ast_tool

echo
echo "============================================================"
echo " CLIPPY"
echo "============================================================"

cargo clippy -p nexus_ast_tool --all-targets -- -D warnings

echo
echo "============================================================"
echo " PATCH COMPLETE"
echo "============================================================"
echo
echo "The auditor now enforces:"
echo
echo "  EXPLOITABLE -> actionable"
echo "  SAFE        -> safe only when explicitly proven"
echo "  UNKNOWN     -> unresolved / assumption required"
echo
echo "And:"
echo
echo "  syntax_ok && ast_ok"
echo "       = required for Patch Verified"
echo
echo "The old heuristic:"
echo
echo "  ast_ok || patch_is_safe"
echo
echo "has been removed."
echo
