#!/usr/bin/env bash
set -euo pipefail

cd ~/AthenaFusionX

python3 <<'PY'
from pathlib import Path
import sys

path = Path("apps/nexus_ast_tool/src/main.rs")
text = path.read_text()

def replace_once(old, new, label):
    global text

    count = text.count(old)

    if count != 1:
        print(f"[ERROR] {label}: expected 1 match, found {count}")
        sys.exit(1)

    text = text.replace(old, new, 1)
    print(f"[OK] {label}")


# ------------------------------------------------------------
# 1. Replace ambiguous single counter.
# ------------------------------------------------------------

replace_once(
'''            let mut report_md = String::from("# 🛡️ Nexus Singularity Bug Bounty Report\\n\\n");
            let mut exploitable_count = 0;
''',
'''            let mut report_md = String::from("# 🛡️ Nexus Singularity Bug Bounty Report\\n\\n");

            // These counters represent distinct security states.
            //
            // actionable_count:
            //     AI explicitly classified the finding EXPLOITABLE.
            //
            // fixed_count:
            //     An EXPLOITABLE finding was patched and passed BOTH
            //     syntax validation and post-patch AST verification.
            //
            // failed_fix_count:
            //     An EXPLOITABLE finding was not successfully remediated.
            //
            // safe_count:
            //     AI explicitly classified the finding SAFE.
            //
            // unknown_count:
            //     Security state could not be established without assumptions.
            let mut actionable_count = 0;
            let mut fixed_count = 0;
            let mut failed_fix_count = 0;
            let mut safe_count = 0;
            let mut unknown_count = 0;
''',
"Install explicit audit-state counters"
)


# ------------------------------------------------------------
# 2. Replace raw model-state branch.
# ------------------------------------------------------------

replace_once(
'''                        Ok((is_exploitable, severity, reason)) => {
                            if is_exploitable == "EXPLOITABLE" || is_exploitable == "true" {
                                exploitable_count += 1;
''',
'''                        Ok((is_exploitable, severity, reason)) => {
                            let classification =
                                match is_exploitable.trim().to_uppercase().as_str() {
                                    "EXPLOITABLE" => "EXPLOITABLE",
                                    "SAFE" => "SAFE",
                                    "UNKNOWN" => "UNKNOWN",
                                    other => {
                                        println!(
                                            "        [⚠️ TRIAGE POLICY] Invalid model state '{}'; forcing UNKNOWN.",
                                            other
                                        );
                                        "UNKNOWN"
                                    }
                                };

                            if classification == "EXPLOITABLE" {
                                actionable_count += 1;
''',
"Enforce closed triage state in main"
)


# ------------------------------------------------------------
# 3. Verified patch increments fixed_count.
# ------------------------------------------------------------

replace_once(
'''                                                if syntax_ok && ast_ok {
                                                    println!(
                                                        "        [✅ Patch Verified] Syntax OK and AST Rescan confirms vulnerability eliminated."
                                                    );
''',
'''                                                if syntax_ok && ast_ok {
                                                    fixed_count += 1;

                                                    println!(
                                                        "        [✅ Patch Verified] Syntax OK and AST Rescan confirms vulnerability eliminated."
                                                    );
''',
"Count VERIFIED_FIXED remediation"
)


# ------------------------------------------------------------
# 4. Failed verification increments failed_fix_count.
# ------------------------------------------------------------

replace_once(
'''                                                } else {
                                                    let reject_reason = if !syntax_ok {
                                                        "Syntax check failed"
                                                    } else {
                                                        "AST Rescan detected vulnerability still present"
                                                    };
''',
'''                                                } else {
                                                    failed_fix_count += 1;

                                                    let reject_reason = if !syntax_ok {
                                                        "Syntax check failed"
                                                    } else {
                                                        "AST Rescan detected vulnerability still present"
                                                    };
''',
"Count failed remediation verification"
)


# ------------------------------------------------------------
# 5. SEARCH mismatch counts as failed remediation.
# ------------------------------------------------------------

replace_once(
'''                                            if !matched_exact {
                                                println!("        [❌ Patch Rejected] SEARCH block not found exactly in file. Reverting.");
''',
'''                                            if !matched_exact {
                                                failed_fix_count += 1;

                                                println!("        [❌ Patch Rejected] SEARCH block not found exactly in file. Reverting.");
''',
"Count SEARCH-block remediation failures"
)


# ------------------------------------------------------------
# 6. Patch-generation error counts as failed remediation.
# ------------------------------------------------------------

replace_once(
'''                                    Err(e) => println!("        [❌ Patch Gen Error] {}", e),
''',
'''                                    Err(e) => {
                                        failed_fix_count += 1;
                                        println!("        [❌ Patch Gen Error] {}", e);
                                    },
''',
"Count patch-generation failures"
)


# ------------------------------------------------------------
# 7. Replace false-positive catch-all.
# ------------------------------------------------------------

replace_once(
'''                            } else {
                                println!("        [🟢 FALSE POSITIVE] Safe.\\n        [🧠 AI Reasoning] {}", reason);
                            }
''',
'''                            } else if classification == "SAFE" {
                                safe_count += 1;

                                println!(
                                    "        [🟢 SAFE] Explicitly classified safe."
                                );
                                println!(
                                    "        [🧠 AI Reasoning] {}",
                                    reason
                                );
                            } else {
                                unknown_count += 1;

                                println!(
                                    "        [🟡 UNKNOWN] Security state cannot be established without assumptions."
                                );
                                println!(
                                    "        [🧠 AI Reasoning] {}",
                                    reason
                                );
                            }
''',
"Separate SAFE and UNKNOWN"
)


# ------------------------------------------------------------
# 8. Patch generation / read failure currently silently disappears.
#
# If the source cannot be read, the remediation did not succeed.
# ------------------------------------------------------------

replace_once(
'''                                        if let Ok(file_source) = std::fs::read_to_string(&finding.path) {
''',
'''                                        if let Ok(file_source) = std::fs::read_to_string(&finding.path) {
''',
"Confirm source-read boundary"
)


# ------------------------------------------------------------
# 9. Replace final report generation.
# ------------------------------------------------------------

replace_once(
'''                if generate_report && exploitable_count > 0 {
                    let _ = fs::write("bounty_report.md", &report_md);
                    println!("\\n==> [Report Generator] 📄 Successfully generated 'bounty_report.md' with {} actionable bugs!", exploitable_count);
                }
''',
'''                if generate_report {
                    report_md.push_str("\\n# Final Audit Summary\\n\\n");

                    report_md.push_str(&format!(
                        "- **Actionable findings:** {}\\n",
                        actionable_count
                    ));
                    report_md.push_str(&format!(
                        "- **Verified fixed:** {}\\n",
                        fixed_count
                    ));
                    report_md.push_str(&format!(
                        "- **Remediation failed:** {}\\n",
                        failed_fix_count
                    ));
                    report_md.push_str(&format!(
                        "- **Explicitly safe:** {}\\n",
                        safe_count
                    ));
                    report_md.push_str(&format!(
                        "- **Unknown:** {}\\n\\n",
                        unknown_count
                    ));

                    let remediation_clean =
                        actionable_count == fixed_count
                        && failed_fix_count == 0;

                    if remediation_clean {
                        report_md.push_str(
                            "## Remediation Integrity\\n\\n\
                             **PASS** — Every actionable finding was VERIFIED_FIXED.\\n\\n"
                        );
                    } else {
                        report_md.push_str(
                            "## Remediation Integrity\\n\\n\
                             **FAIL** — One or more actionable findings were not VERIFIED_FIXED.\\n\\n"
                        );
                    }

                    fs::write("bounty_report.md", &report_md)?;

                    println!(
                        "\\n==> [Report Generator] 📄 bounty_report.md generated."
                    );
                    println!(
                        "    Actionable findings : {}",
                        actionable_count
                    );
                    println!(
                        "    Verified fixed      : {}",
                        fixed_count
                    );
                    println!(
                        "    Remediation failed  : {}",
                        failed_fix_count
                    );
                    println!(
                        "    Explicitly safe     : {}",
                        safe_count
                    );
                    println!(
                        "    Unknown             : {}",
                        unknown_count
                    );

                    if remediation_clean {
                        println!(
                            "    [✅ REMEDIATION INTEGRITY] PASS"
                        );
                    } else {
                        println!(
                            "    [❌ REMEDIATION INTEGRITY] FAIL"
                        );
                    }
                }
''',
"Make final report consume authoritative counters"
)


path.write_text(text)
print()
print("[OK] main.rs accounting patch written.")
PY

echo
echo "============================================================"
echo " VERIFY ACCOUNTING"
echo "============================================================"

rg -n \
  'actionable_count|fixed_count|failed_fix_count|safe_count|unknown_count' \
  apps/nexus_ast_tool/src/main.rs

echo
echo "============================================================"
echo " VERIFY TRIAGE STATES"
echo "============================================================"

rg -n -C 4 \
  'classification == "EXPLOITABLE"|classification == "SAFE"|UNKNOWN' \
  apps/nexus_ast_tool/src/main.rs

echo
echo "============================================================"
echo " VERIFY NO OLD ACCOUNTING"
echo "============================================================"

if rg -n 'exploitable_count|is_exploitable == "true"|FALSE POSITIVE' \
    apps/nexus_ast_tool/src/main.rs; then
    echo "[ERROR] Old ambiguous accounting still exists."
    exit 1
else
    echo "[OK] Old ambiguous accounting removed."
fi

echo
echo "============================================================"
echo " VERIFY AST SECURITY BOUNDARY"
echo "============================================================"

if rg -n 'patch_is_safe|ast_ok \|\|' \
    apps/nexus_ast_tool/src/main.rs; then
    echo "[ERROR] AST verification bypass detected."
    exit 1
else
    echo "[OK] AST verification remains mandatory."
fi

echo
echo "============================================================"
echo " CARGO CHECK"
echo "============================================================"

cargo check -p nexus_ast_tool -j 4

echo
echo "============================================================"
echo " CLIPPY"
echo "============================================================"

cargo clippy -p nexus_ast_tool --all-targets -- -D warnings

echo
echo "============================================================"
echo " ACCOUNTING PATCH COMPLETE"
echo "============================================================"
