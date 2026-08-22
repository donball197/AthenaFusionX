import re
from pathlib import Path

MAIN = Path("apps/nexus_ast_tool/src/main.rs")
CLIENT = Path("apps/nexus_ast_tool/src/client.rs")

main_code = MAIN.read_text()
client_code = CLIENT.read_text()

# ============================================================
# MAIN.RS FIXES
# ============================================================

# 1. Counters
main_code = re.sub(
    r'let mut report_md = String::from\("# 🛡️ Nexus Singularity Bug Bounty Report\\n\\n"\);\s*let mut exploitable_count = 0;',
    r'''let mut report_md = String::from("# 🛡️ Nexus Singularity Bug Bounty Report\\n\\n");
            let mut actionable_count = 0;
            let mut fixed_count = 0;
            let mut failed_fix_count = 0;
            let mut unknown_count = 0;
            let mut safe_count = 0;''',
    main_code
)

# 2. Triage Classification
main_code = re.sub(
    r'Ok\(\(is_exploitable, severity, reason\)\) => \{\s*if is_exploitable == "EXPLOITABLE" \|\| is_exploitable == "true" \{\s*exploitable_count \+= 1;\s*println!\("        \[🔴 EXPLOITABLE\] Severity: \{\}", severity\);\s*println!\("        \[🧠 AI Reasoning\] \{\}", reason\);',
    r'''Ok((is_exploitable, severity, reason)) => {
                            let classification = match is_exploitable.trim().to_uppercase().as_str() {
                                "EXPLOITABLE" | "TRUE" => "EXPLOITABLE",
                                "SAFE" | "FALSE" => "SAFE",
                                "UNKNOWN" => "UNKNOWN",
                                other => {
                                    println!("        [⚠️ TRIAGE POLICY] Invalid model state '{}'; forcing UNKNOWN.", other);
                                    "UNKNOWN"
                                }
                            };

                            if classification == "EXPLOITABLE" {
                                actionable_count += 1;
                                println!("        [🔴 EXPLOITABLE] Severity: {}", severity);
                                println!("        [🧠 AI Reasoning] {}", reason);''',
    main_code
)

# 3. Else Branches (UNKNOWN and SAFE)
main_code = re.sub(
    r'\} else \{\s*println!\("        \[🟢 FALSE POSITIVE\] Safe\.\\n        \[🧠 AI Reasoning\] \{\}", reason\);\s*\}',
    r'''} else if classification == "UNKNOWN" {
                                unknown_count += 1;
                                println!("        [🟡 UNKNOWN] Requires manual review.\n        [🧠 AI Reasoning] {}", reason);
                            } else {
                                safe_count += 1;
                                println!("        [🟢 FALSE POSITIVE] Safe.\n        [🧠 AI Reasoning] {}", reason);
                            }''',
    main_code
)

# 4. Remove patch_is_safe bypass
main_code = re.sub(
    r'let patch_is_safe = replace_block\.contains[^;]+;\s*if syntax_ok && \(ast_ok \|\| patch_is_safe\) \{',
    r'''if syntax_ok && ast_ok {
                                                    fixed_count += 1;''',
    main_code
)

# 5. Patch Gen Errors mapped to failed_fix_count
main_code = re.sub(
    r'Err\(e\) => println!\("        \[❌ Patch Gen Error\] \{\}", e\),',
    r'''Err(e) => {
                                        failed_fix_count += 1;
                                        println!("        [❌ Patch Gen Error] {}", e);
                                        report_md.push_str(&format!("### Remediation Status: REMEDIATION_FAILED\n- **Reason:** Patch generation failed: {}\n\n", e));
                                    },''',
    main_code
)

# 6. Mismatched SEARCH blocks mapped to failed_fix_count
main_code = re.sub(
    r'if !matched_exact \{\s*println!\("        \[❌ Patch Rejected\] SEARCH block not found exactly in file\. Reverting\."\);\s*report_md\.push_str\(&format!\(\s*"### Recommended Patch \(auto-heal attempted, REJECTED: SEARCH block mismatch, reverted\)\\n```\{\}\\n\{\}\\n```\\n\\n",\s*lang_str\.to_lowercase\(\), replace_block\s*\)\);\s*\} else \{',
    r'''if !matched_exact {
                                                failed_fix_count += 1;
                                                println!("        [❌ Patch Rejected] SEARCH block not found exactly in file. Reverting.");
                                                report_md.push_str(&format!(
                                                    "### Remediation Status: REMEDIATION_FAILED\n- **Reason:** SEARCH block mismatch\n\n### Rejected Patch\n```{} \n{}\n```\n\n",
                                                    lang_str.to_lowercase(), replace_block
                                                ));
                                            } else {''',
    main_code
)

# 7. Final Report Output
main_code = re.sub(
    r'if generate_report && exploitable_count > 0 \{\s*let _ = fs::write\("bounty_report\.md", &report_md\);\s*println!\("\\n==> \[Report Generator\] 📄 Successfully generated \'bounty_report\.md\' with \{\} actionable bugs!", exploitable_count\);\s*\}',
    r'''if generate_report && actionable_count > 0 {
                    let _ = fs::write("bounty_report.md", &report_md);
                    println!("\n==> [Report Generator] 📄 'bounty_report.md' generated.");
                    println!("    ↳ 🚨 Actionable Findings: {}", actionable_count);
                    println!("    ↳ ✅ Auto-Healed (Fixed): {}", fixed_count);
                    println!("    ↳ ❌ Remediation Failed:  {}", failed_fix_count);
                    println!("    ↳ 🟡 Unknown / Review:    {}", unknown_count);
                    println!("    ↳ 🟢 Safe (False Pos):    {}", safe_count);
                }''',
    main_code
)

# ============================================================
# CLIENT.RS FIXES
# ============================================================

# 8. Client Triage Contract
client_code = re.sub(
    r'3\. If you must make ANY ASSUMPTION about external data validation or session trust, return \\"UNKNOWN\\"\\.\n\s*Return ONLY a JSON object wrapped in <payload></payload> tags like this:\\n\{\{\\"state\\": \\"EXPLOITABLE\\", \\"severity\\": \\"High\\", \\"reason\\": \\"\.\.\.\\"\}\}",',
    r'''3. If you must make ANY ASSUMPTION about external data validation, session trust, provenance, or sanitization, return \"UNKNOWN\".\n\
             4. \"SAFE\" is permitted ONLY when the supplied source proves the sink is mitigated without assumptions.\n\
             5. The state MUST be exactly one of: \"EXPLOITABLE\", \"SAFE\", \"UNKNOWN\".\n\
             6. Never return \"FALSE POSITIVE\", \"TRUE\", \"FALSE\", or any other state.\n\
             Return ONLY a JSON object wrapped in <payload></payload> tags like this:\n{{\"state\": \"EXPLOITABLE\", \"severity\": \"High\", \"reason\": \"...\"}}",''',
    client_code
)

# 9. Smart Fallback for `Missing <payload>` (Triage block)
client_code = re.sub(
    r'let start = text\s*\.find\("<payload>"\)\s*\.ok_or_else\(\|\| anyhow!\("Missing <payload>"\)\)\?\s*\+\ 9;\s*let end = text\s*\.rfind\("</payload>"\)\s*\.ok_or_else\(\|\| anyhow!\("Missing </payload>"\)\)\?;',
    r'''let start = text.find("<payload>").map(|i| i + 9).unwrap_or_else(|| text.find("{").unwrap_or(0));
        let end = text.rfind("</payload>").unwrap_or_else(|| text.rfind("}").map(|i| i + 1).unwrap_or(text.len()));''',
    client_code
)

# 10. Smart Fallback for `Missing <payload>` (Auto-Heal Generator block)
client_code = re.sub(
    r'let start = text\s*\.find\("<payload>"\)\s*\.ok_or_else\(\|\| anyhow!\("Model response missing <payload> tag"\)\)\?\s*\+\ 9;\s*let end = text\s*\.rfind\("</payload>"\)\s*\.ok_or_else\(\|\| anyhow!\("Missing </payload> tag"\)\)\?;',
    r'''let start = text.find("<payload>").map(|i| i + 9).unwrap_or(0);
        let end = text.rfind("</payload>").unwrap_or(text.len());''',
    client_code
)

MAIN.write_text(main_code)
CLIENT.write_text(client_code)
print("✅ Applied the ultimate resilience patch!")
