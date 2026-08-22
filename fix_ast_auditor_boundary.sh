#!/usr/bin/env bash
set -euo pipefail

cd ~/AthenaFusionX

MAIN="apps/nexus_ast_tool/src/main.rs"
CLIENT="apps/nexus_ast_tool/src/client.rs"

echo "============================================================"
echo " NEXUS AST TOOL — SECURITY TRIAGE / REMEDIATION BOUNDARY"
echo "============================================================"

[[ -f "$MAIN" ]] || { echo "[ERROR] Missing $MAIN"; exit 1; }
[[ -f "$CLIENT" ]] || { echo "[ERROR] Missing $CLIENT"; exit 1; }

python3 <<'PY'
import re
from pathlib import Path
import sys

MAIN = Path("apps/nexus_ast_tool/src/main.rs")
CLIENT = Path("apps/nexus_ast_tool/src/client.rs")

main = MAIN.read_text()
client = CLIENT.read_text()

def replace_once(text, old, new, label, path):
    count = text.count(old)
    if count != 1:
        print(f"[ERROR] {label}\n        Expected exactly 1 match, found {count}\n        File: {path}")
        sys.exit(1)
    print(f"[OK] {label}")
    return text.replace(old, new, 1)


# ============================================================
# 1. Replace ambiguous exploitable_count with explicit counters.
# ============================================================
old = '''            let mut report_md = String::from("# 🛡️ Nexus Singularity Bug Bounty Report\\n\\n");
            let mut exploitable_count = 0;'''

new = '''            let mut report_md = String::from("# 🛡️ Nexus Singularity Bug Bounty Report\\n\\n");

            let mut actionable_count = 0;
            let mut fixed_count = 0;
            let mut failed_fix_count = 0;
            let mut unknown_count = 0;
            let mut safe_count = 0;'''

main = replace_once(main, old, new, "Introduce explicit security-state counters", MAIN)


# ============================================================
# 2. Replace the unsafe AI state branch.
# ============================================================
old = '''                        Ok((is_exploitable, severity, reason)) => {
                            if is_exploitable == "EXPLOITABLE" || is_exploitable == "true" {
                                exploitable_count += 1;
                                println!("        [🔴 EXPLOITABLE] Severity: {}", severity);
                                println!("        [🧠 AI Reasoning] {}", reason);'''

new = '''                        Ok((is_exploitable, severity, reason)) => {
                            let classification = match is_exploitable.trim().to_uppercase().as_str() {
                                "EXPLOITABLE" => "EXPLOITABLE",
                                "SAFE" => "SAFE",
                                "UNKNOWN" => "UNKNOWN",
                                other => {
                                    println!("        [⚠️ TRIAGE POLICY] Invalid model state '{}'; forcing UNKNOWN.", other);
                                    "UNKNOWN"
                                }
                            };

                            if classification == "EXPLOITABLE" {
                                actionable_count += 1;
                                println!("        [🔴 EXPLOITABLE] Severity: {}", severity);
                                println!("        [🧠 AI Reasoning] {}", reason);'''

main = replace_once(main, old, new, "Make triage classification explicit", MAIN)


# ============================================================
# 3. Add explicit UNKNOWN and SAFE else-branches
# ============================================================
old = '''                            } else {
                                println!("        [🟢 FALSE POSITIVE] Safe.\\n        [🧠 AI Reasoning] {}", reason);
                            }'''

new = '''                            } else if classification == "UNKNOWN" {
                                unknown_count += 1;
                                println!("        [🟡 UNKNOWN] Requires manual review.\\n        [🧠 AI Reasoning] {}", reason);
                            } else {
                                safe_count += 1;
                                println!("        [🟢 FALSE POSITIVE] Safe.\\n        [🧠 AI Reasoning] {}", reason);
                            }'''

main = replace_once(main, old, new, "Add explicit UNKNOWN branch", MAIN)


# ============================================================
# 4. SEARCH Mismatch Failure (Using Regex to bypass invisible newlines)
# ============================================================
pattern = r'if !matched_exact \{.*?lang_str\.to_lowercase\(\), replace_block\s*\)\);\s*\} else \{'

new_search_mismatch = '''if !matched_exact {
                                                failed_fix_count += 1;
                                                println!("        [❌ Patch Rejected] SEARCH block not found exactly in file. Reverting.");
                                                report_md.push_str(&format!(
                                                    "### Remediation Status: REMEDIATION_FAILED\\n- **Reason:** SEARCH block mismatch\\n\\n### Rejected Patch\\n```{} \\n{}\\n```\\n\\n",
                                                    lang_str.to_lowercase(), replace_block
                                                ));
                                            } else {'''

if re.search(pattern, main, re.DOTALL):
    main = re.sub(pattern, new_search_mismatch, main, count=1, flags=re.DOTALL)
    print("[OK] Account for SEARCH mismatch failures via Regex")
else:
    print("[ERROR] Failed to match SEARCH mismatch block via Regex")
    sys.exit(1)


# ============================================================
# 5. Remove patch verification bypass & add verified accounting
# ============================================================
old = '''                                                let patch_is_safe = replace_block.contains("subprocess.run") || replace_block.contains("subprocess.check_output") || replace_block.contains("mysqli_prepare") || replace_block.contains("run(");
                                                if syntax_ok && (ast_ok || patch_is_safe) {
                                                    println!("        [✅ Patch Verified] Syntax OK and AST Rescan confirms vulnerability eliminated.");
                                                    report_md.push_str("### Recommended Patch (Auto-Healed)\\n");
                                                    report_md.push_str(&format!("```{} \\n{}\\n```\\n\\n", lang_str.to_lowercase(), replace_block));
                                                } else {
                                                    let reject_reason = if !syntax_ok { "Syntax check failed" } else { "AST Rescan detected vulnerability still present" };
                                                    println!("        [❌ Patch Rejected] {}. Reverting.", reject_reason);
                                                    let _ = std::fs::write(&finding.path, &file_source);
                                                    report_md.push_str(&format!(
                                                        "### Recommended Patch (auto-heal attempted, REJECTED: {}, reverted)\\n```{} \\n{}\\n```\\n\\n",
                                                        reject_reason, lang_str.to_lowercase(), replace_block
                                                    ));
                                                }'''

new = '''                                                if syntax_ok && ast_ok {
                                                    fixed_count += 1;
                                                    println!("        [✅ Patch Verified] Syntax OK and AST Rescan confirms vulnerability eliminated.");
                                                    report_md.push_str("### Remediation Status: VERIFIED_FIXED\\n### Verified Patch\\n");
                                                    report_md.push_str(&format!("```{} \\n{}\\n```\\n\\n", lang_str.to_lowercase(), replace_block));
                                                } else {
                                                    failed_fix_count += 1;
                                                    let reject_reason = if !syntax_ok { "Syntax check failed" } else { "AST Rescan detected vulnerability still present" };
                                                    println!("        [❌ Patch Rejected] {}. Reverting.", reject_reason);
                                                    let _ = std::fs::write(&finding.path, &file_source);
                                                    report_md.push_str(&format!(
                                                        "### Remediation Status: REMEDIATION_FAILED\\n- **Reason:** {}\\n\\n### Rejected Patch\\n```{} \\n{}\\n```\\n\\n",
                                                        reject_reason, lang_str.to_lowercase(), replace_block
                                                    ));
                                                }'''

main = replace_once(main, old, new, "Remove patch bypass & handle AST failures", MAIN)


# ============================================================
# 6. Patch Gen Errors
# ============================================================
old = '''                                    Err(e) => println!("        [❌ Patch Gen Error] {}", e),'''

new = '''                                    Err(e) => {
                                        failed_fix_count += 1;
                                        println!("        [❌ Patch Gen Error] {}", e);
                                        report_md.push_str(&format!("### Remediation Status: REMEDIATION_FAILED\\n- **Reason:** Patch generation failed: {}\\n\\n", e));
                                    },'''

main = replace_once(main, old, new, "Account for patch-generation failures", MAIN)


# ============================================================
# 7. Final Report Output (Consuming all variables)
# ============================================================
old = '''                if generate_report && exploitable_count > 0 {
                    let _ = fs::write("bounty_report.md", &report_md);
                    println!("\\n==> [Report Generator] 📄 Successfully generated 'bounty_report.md' with {} actionable bugs!", exploitable_count);
                }'''

new = '''                if generate_report && actionable_count > 0 {
                    let _ = fs::write("bounty_report.md", &report_md);
                    println!("\\n==> [Report Generator] 📄 'bounty_report.md' generated.");
                    println!("    ↳ 🚨 Actionable Findings: {}", actionable_count);
                    println!("    ↳ ✅ Auto-Healed (Fixed): {}", fixed_count);
                    println!("    ↳ ❌ Remediation Failed:  {}", failed_fix_count);
                    println!("    ↳ 🟡 Unknown / Review:    {}", unknown_count);
                    println!("    ↳ 🟢 Safe (False Pos):    {}", safe_count);
                }'''

main = replace_once(main, old, new, "Update final report printout to consume counters", MAIN)


# ============================================================
# CLIENT-SIDE HARDENING
# ============================================================
old_client = '''             3. If you must make ANY ASSUMPTION about external data validation or session trust, return "UNKNOWN".\\n\\
             Return ONLY a JSON object wrapped in <payload></payload> tags like this:\\n{{"state": "EXPLOITABLE", "severity": "High", "reason": "..."}}",'''

new_client = '''             3. If you must make ANY ASSUMPTION about external data validation, session trust, provenance, or sanitization, return "UNKNOWN".\\n\\
             4. "SAFE" is permitted ONLY when the supplied source proves the sink is mitigated without assumptions.\\n\\
             5. The state MUST be exactly one of: "EXPLOITABLE", "SAFE", "UNKNOWN".\\n\\
             6. Never return "FALSE POSITIVE", "TRUE", "FALSE", or any other state.\\n\\
             Return ONLY a JSON object wrapped in <payload></payload> tags like this:\\n{{"state": "EXPLOITABLE", "severity": "High", "reason": "..."}}",'''

client = replace_once(client, old_client, new_client, "Harden triage state contract", CLIENT)


MAIN.write_text(main)
CLIENT.write_text(client)

print("\n[OK] All boundaries secured.")
PY
