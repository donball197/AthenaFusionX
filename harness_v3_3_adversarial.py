#!/usr/bin/env python3
"""
AthenaFusionX Benchmark Harness v3.3 (Adversarial & Semantic Generalization Edition)
Rigorous, Pre-Registered Edge AI Benchmark with OLS Latency Decomposition & Atomic Persistence.
"""

import json
import time
import statistics
import requests
from datetime import datetime

SERVER_URL = "http://192.168.4.223:9379/v1/chat/completions"
MODEL_NAME = "gemma-4-e4b-it"
RUNS_PER_TEST = 3

session = requests.Session()

def parse_payload_json(raw_text):
    text = raw_text.strip()
    if "<payload>" in text:
        start = text.find("<payload>") + len("<payload>")
        end = text.rfind("</payload>")
        text = text[start:end if end != -1 else len(text)].strip()
    if text.startswith("```json"):
        text = text[7:]
    elif text.startswith("```"):
        text = text[3:]
    if text.endswith("```"):
        text = text[:-3]
    return json.loads(text.strip())

# The frozen stepwise sentinel gate template (Unchanged from v3.2)
def gate_prompt(objective, evidence):
    return (
        "Task: Execution sentinel audit.\n"
        "Procedure:\n"
        "1. List every deliverable the Objective requires.\n"
        "2. For each deliverable, state whether the Evidence explicitly confirms it by name.\n"
        "3. A deliverable with no explicit confirmation is missing. If the evidence shows a "
        "failure ([ACTION FAILED], error, non-zero exit, or [WRITE_FAIL]), nothing is confirmed.\n\n"
        "Example:\n"
        "Objective: Create build.zip and notes.txt.\n"
        "Evidence: [WRITE_OK] notes.txt written.\n"
        "Deliverables: build.zip, notes.txt\n"
        "build.zip: not mentioned in evidence -> missing\n"
        "notes.txt: confirmed\n"
        '<payload>{"missing_items": ["build.zip"], "verdict": "NO"}</payload>\n\n'
        "Now audit this task. Write your Step 1-2 lines first, then output ONLY the final "
        "JSON inside <payload></payload> tags.\n"
        "Objective: " + objective + "\n"
        "Evidence: " + evidence
    )

# ----------------------------------------------------------------------
# PRE-REGISTERED EVALUATION SUITE (v3.3)
# ----------------------------------------------------------------------
TESTS = [
    # --- DOMAIN 0: PROMPT-GROUNDED LATENCY FLOOR ---
    {
        "id": "T00_probe_latency_floor",
        "domain": "Latency Decomposition",
        "prompt": "Reply with exactly: OK",
        "blind_status": "N/A (Probe)",
        "validate": lambda c: "OK" in c
    },

    # --- DOMAIN 1: CORE SENTINEL GATES (v3.1 Baseline) ---
    {
        "id": "T01_gate_completed_objective",
        "domain": "Sentinel Core",
        "prompt": gate_prompt("Create report.md.", "[WRITE_OK] report.md written (1420 bytes)."),
        "blind_status": "Baseline",
        "validate": lambda c: parse_payload_json(c).get("verdict") == "YES"
    },
    {
        "id": "T02_gate_missing_deliverable",
        "domain": "Sentinel Core",
        "prompt": gate_prompt("Create audit.json and summary.md.", "[WRITE_OK] audit.json written."),
        "blind_status": "Baseline",
        "validate": lambda c: parse_payload_json(c).get("verdict") == "NO"
    },
    {
        "id": "T03_gate_failed_command_trap",
        "domain": "Sentinel Core",
        "prompt": gate_prompt("Compile release binary.", "[ACTION FAILED] exit status: 101 error: could not compile."),
        "blind_status": "Baseline",
        "validate": lambda c: parse_payload_json(c).get("verdict") == "NO"
    },

    # --- DOMAIN 2: AST SECURITY TRIAGE ---
    {
        "id": "T04_triage_sqli_raw_concat",
        "domain": "AST Security",
        "prompt": (
            "Analyze the PHP snippet below for SQL_INJECTION vulnerability.\n"
            "Rubric:\n"
            "- Return 'EXPLOITABLE' if untrusted input is concatenated into the query without parameterization.\n"
            "- Return 'SAFE' if parameterized with prepared statements.\n"
            "- Return 'UNKNOWN' if query provenance cannot be determined.\n\n"
            "Format: <payload>{\"state\": \"EXPLOITABLE\" | \"SAFE\" | \"UNKNOWN\", \"severity\": \"High\" | \"Medium\" | \"Low\"}</payload>\n\n"
            "```php\n"
            "$q = 'SELECT * FROM users WHERE id = ' . $_GET['id'];\n"
            "$r = mysqli_query($con, $q);\n"
            "```"
        ),
        "blind_status": "Baseline",
        "validate": lambda c: parse_payload_json(c).get("state") == "EXPLOITABLE"
    },
    {
        "id": "T05_triage_sqli_prepared_stmt",
        "domain": "AST Security",
        "prompt": (
            "Analyze the PHP snippet below for SQL_INJECTION vulnerability.\n"
            "Rubric:\n"
            "- Return 'EXPLOITABLE' if untrusted input is concatenated into the query without parameterization.\n"
            "- Return 'SAFE' if parameterized with prepared statements.\n"
            "- Return 'UNKNOWN' if query provenance cannot be determined.\n\n"
            "Format: <payload>{\"state\": \"EXPLOITABLE\" | \"SAFE\" | \"UNKNOWN\", \"severity\": \"High\" | \"Medium\" | \"Low\"}</payload>\n\n"
            "```php\n"
            "$stmt = $con->prepare('SELECT * FROM users WHERE id = ?');\n"
            "$stmt->bind_param('i', $_GET['id']);\n"
            "$stmt->execute();\n"
            "```"
        ),
        "blind_status": "Baseline",
        "validate": lambda c: parse_payload_json(c).get("state") == "SAFE"
    },
    {
        "id": "T06_triage_unknown_provenance",
        "domain": "AST Security",
        "prompt": (
            "Analyze the PHP snippet below for SQL_INJECTION vulnerability.\n"
            "Rubric:\n"
            "- Return 'EXPLOITABLE' only if code demonstrably concatenates unsanitized input into the query.\n"
            "- Return 'SAFE' only if demonstrably parameterized.\n"
            "- Return 'UNKNOWN' if the variable's source/construction is not visible in the snippet.\n\n"
            "Format: <payload>{\"state\": \"EXPLOITABLE\" | \"SAFE\" | \"UNKNOWN\", \"severity\": \"High\" | \"Medium\" | \"Low\"}</payload>\n\n"
            "```php\n"
            "$res = mysqli_query($con, $internal_query);\n"
            "```"
        ),
        "blind_status": "Baseline",
        "validate": lambda c: parse_payload_json(c).get("state") == "UNKNOWN"
    },

    # --- DOMAIN 3: EXTRACTION & REMEDIATION ---
    {
        "id": "T07_extract_parameter_mapping",
        "domain": "Code Extraction",
        "prompt": (
            "Code snippet:\n"
            "$stmt->bind_param('ssi', $user, $pass, $role_id);\n"
            "Extract all bound variable names. Output only the variable names separated by commas."
        ),
        "blind_status": "Baseline",
        "validate": lambda c: "$user" in c and "$pass" in c and "$role_id" in c
    },
    {
        "id": "T08_remediation_diff_anchor",
        "domain": "Code Remediation",
        "prompt": (
            "Role: Security Remediation Engine.\n"
            "Task: Rewrite the PHP snippet to use a mysqli prepared statement ($con->prepare, bind_param, execute, get_result).\n"
            "Rules:\n"
            "1. Output ONLY the raw PHP replacement code inside <payload></payload> tags.\n"
            "2. Do NOT invent variable names. Use only $id.\n\n"
            "Snippet:\n"
            "```php\n"
            "$q = 'SELECT * FROM users WHERE id = ' . $id;\n"
            "$r = mysqli_query($con, $q);\n"
            "```"
        ),
        "blind_status": "Baseline",
        "validate": lambda c: (
            "prepare(" in c and "bind_param" in c and "?" in c 
            and "$id" in c and ". $id" not in c and "execute(" in c
        )
    },

    # --- DOMAIN 4: STRUCTURAL GENERALIZATION ---
    {
        "id": "T09_gate_name_in_failure_trap",
        "domain": "Structural Generalization",
        "prompt": gate_prompt(
            "Create metrics.log and app.bin.",
            "[WRITE_OK] app.bin written (4096 bytes).\n[WRITE_FAIL] metrics.log: disk full (0 bytes written)."
        ),
        "blind_status": "Rubric-Assisted ([WRITE_FAIL] explicit)",
        "validate": lambda c: parse_payload_json(c).get("verdict") == "NO"
    },
    {
        "id": "T10_gate_three_item_generalization",
        "domain": "Structural Generalization",
        "prompt": gate_prompt(
            "Create config.json, schema.sql, and seed.py.",
            "[WRITE_OK] config.json written.\n[WRITE_OK] seed.py written."
        ),
        "blind_status": "Genuinely Blind",
        "validate": lambda c: parse_payload_json(c).get("verdict") == "NO" and "schema.sql" in parse_payload_json(c).get("missing_items", [])
    },
    {
        "id": "T11_gate_overdelivery_acceptance",
        "domain": "Structural Generalization",
        "prompt": gate_prompt(
            "Create license.txt.",
            "[WRITE_OK] license.txt written.\n[WRITE_OK] bonus_readme.txt written."
        ),
        "blind_status": "Genuinely Blind",
        "validate": lambda c: parse_payload_json(c).get("verdict") == "YES"
    },
    {
        "id": "T12_gate_substring_shadow_trap",
        "domain": "Structural Generalization",
        "prompt": gate_prompt(
            "Create final_report.md.",
            "[WRITE_OK] report.md written (2048 bytes)."
        ),
        "blind_status": "Genuinely Blind",
        "validate": lambda c: parse_payload_json(c).get("verdict") == "NO"
    },
    {
        "id": "T13_gate_action_crash_generalization",
        "domain": "Structural Generalization",
        "prompt": gate_prompt(
            "Deploy migration script.",
            "[ACTION FAILED] runtime error: connection refused on port 5432."
        ),
        "blind_status": "Partial ([ACTION FAILED] in rubric)",
        "validate": lambda c: parse_payload_json(c).get("verdict") == "NO"
    },

    # --- DOMAIN 5: ADVERSARIAL SEMANTIC & STATE REASONING (Strictly Blind) ---
    {
        "id": "T14_gate_untagged_semantic_failure",
        "domain": "Adversarial Semantic",
        "prompt": gate_prompt(
            "Install nginx.",
            "[LOG] 10:14:01 FATAL: unable to acquire package lock. Subprocess aborted."
        ),
        "blind_status": "Genuinely Blind (No rubric marker)",
        "validate": lambda c: parse_payload_json(c).get("verdict") == "NO"
    },
    {
        "id": "T15_gate_synonym_success_transfer",
        "domain": "Adversarial Semantic",
        "prompt": gate_prompt(
            "Build app.bin.",
            "[BUILD OK] app.bin built (1.2 MB executable binary generated)."
        ),
        "blind_status": "Genuinely Blind (Novel success marker)",
        "validate": lambda c: parse_payload_json(c).get("verdict") == "YES"
    },
    {
        "id": "T16_gate_future_intent_plan_trap",
        "domain": "Adversarial Semantic",
        "prompt": gate_prompt(
            "Create summary.md.",
            "[PLAN] Next step: create summary.md.\n[STATUS] Ready to execute."
        ),
        "blind_status": "Genuinely Blind (Unfulfilled intent)",
        "validate": lambda c: parse_payload_json(c).get("verdict") == "NO"
    },
    {
        "id": "T17_gate_state_revocation_trap",
        "domain": "Adversarial Semantic",
        "prompt": gate_prompt(
            "Create temp.bin.",
            "[WRITE_OK] temp.bin written (1024 bytes).\n[DELETE_OK] temp.bin deleted (cleanup)."
        ),
        "blind_status": "Genuinely Blind (Contradictory state)",
        "validate": lambda c: parse_payload_json(c).get("verdict") == "NO"
    }
]

# ----------------------------------------------------------------------
# INFERENCE & ATOMIC TELEMETRY ENGINE
# ----------------------------------------------------------------------
def run_single_inference(test):
    payload = {
        "model": MODEL_NAME,
        "messages": [{"role": "user", "content": test["prompt"]}],
        "temperature": 0.0
    }

    start = time.perf_counter()
    resp = session.post(SERVER_URL, json=payload, timeout=45)
    wall_time = time.perf_counter() - start
    
    resp.raise_for_status()
    data = resp.json()
    
    content = data["choices"][0]["message"]["content"].strip()
    usage = data.get("usage", {})
    gen_tokens = usage.get("completion_tokens", len(content.split()))
    
    effective_tps = gen_tokens / max(wall_time, 0.001)

    passed = False
    try:
        passed = test["validate"](content)
    except Exception:
        passed = False
        
    return {
        "passed": passed,
        "wall_time_s": wall_time,
        "effective_tps": effective_tps,
        "gen_tokens": gen_tokens,
        "content": content
    }

def main():
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    results_filename = f"benchmark_results_{timestamp}.json"
    
    print("=" * 85)
    print(f">> AthenaFusionX Benchmark Harness v3.3 (Target: {MODEL_NAME})")
    print(f">> Pre-Registered Semantic & Adversarial Suite | Total Tests: {len(TESTS)}")
    print("=" * 85)
    
    suite_results = []
    
    def persist_current_state():
        with open(results_filename, "w") as f:
            json.dump({
                "timestamp": timestamp,
                "model": MODEL_NAME,
                "runs_per_test": RUNS_PER_TEST,
                "tasks": suite_results
            }, f, indent=2)

    try:
        for i, test in enumerate(TESTS, 1):
            print(f"\n[{i:02d}/{len(TESTS):02d}] Running: {test['id']} [{test['domain']}]...")
            print(f"       Classification: {test['blind_status']}")
            
            run_data = []
            for r in range(RUNS_PER_TEST):
                try:
                    res = run_single_inference(test)
                    run_data.append(res)
                except Exception as e:
                    print(f"    [!] Run {r+1} Error: {e}")
                    run_data.append({"passed": False, "wall_time_s": 45.0, "effective_tps": 0.0, "gen_tokens": 0, "content": "ERROR"})
            
            passes = sum(1 for r in run_data if r["passed"])
            passed = (passes == RUNS_PER_TEST)
            
            unique_outputs = set(r["content"] for r in run_data)
            deterministic = (len(unique_outputs) == 1)
            det_flag = "det:YES" if deterministic else "det:NO"
            
            med_wall = statistics.median(r["wall_time_s"] for r in run_data)
            med_tps = statistics.median(r["effective_tps"] for r in run_data)
            med_tok = statistics.median(r["gen_tokens"] for r in run_data)
            
            status = f"PASS ({passes}/{RUNS_PER_TEST})" if passed else f"FAIL ({passes}/{RUNS_PER_TEST})"
            color = "\033[92m" if passed else "\033[91m"
            print(f"    -> Result: {color}{status}\033[0m | Med Wall: {med_wall:.2f}s | Out: {med_tok:.0f} tok | Eff: {med_tps:.1f} tps | {det_flag}")
            
            sample = run_data[0]["content"].replace("\n", "\\n")
            snippet = sample if len(sample) <= 140 else sample[:140] + "..."
            print(f"       [run1]: {repr(snippet)}")

            suite_results.append({
                "id": test["id"],
                "domain": test["domain"],
                "blind_status": test["blind_status"],
                "passed": passed,
                "deterministic": deterministic,
                "median_wall_s": med_wall,
                "median_effective_tps": med_tps,
                "median_tokens": med_tok,
                "runs": run_data
            })
            persist_current_state()
            
    except KeyboardInterrupt:
        print("\n[!] Execution interrupted by user. Saved partial results.")

    # Compute OLS linear fit (wall_time = intercept + slope * tokens)
    valid_pts = [(r["median_tokens"], r["median_wall_s"]) for r in suite_results if r["domain"] != "Latency Decomposition" and r["median_tokens"] > 0]
    
    if len(valid_pts) >= 2:
        x_vals, y_vals = zip(*valid_pts)
        mean_x, mean_y = statistics.mean(x_vals), statistics.mean(y_vals)
        num = sum((x - mean_x) * (y - mean_y) for x, y in zip(x_vals, y_vals))
        den = sum((x - mean_x) ** 2 for x in x_vals)
        slope = num / den if den != 0 else 0.0
        intercept = mean_y - slope * mean_x
        decode_tps = (1.0 / slope) if slope > 0 else 0.0
    else:
        intercept, decode_tps = 0.0, 0.0

    eval_tests = [r for r in suite_results if r["domain"] != "Latency Decomposition"]
    total_passed = sum(1 for r in eval_tests if r["passed"])
    pass_rate = (total_passed / len(eval_tests)) * 100 if eval_tests else 0.0
    
    probe_latency = next((r["median_wall_s"] for r in suite_results if r["id"] == "T00_probe_latency_floor"), 0.0)

    # Summary
    print("\n" + "=" * 85)
    print(">> PRE-REGISTERED BENCHMARK SUMMARY (v3.3)")
    print("=" * 85)
    print(f"Evaluation Accuracy          : {total_passed}/{len(eval_tests)} ({pass_rate:.1f}%)")
    print(f"Deterministic Consistency    : {sum(1 for r in suite_results if r['deterministic'])}/{len(suite_results)} tasks (det:YES)")
    print(f"Empirical Overhead Floor (T00): {probe_latency:.2f}s")
    print(f"OLS Linear Fit Decomposition : Overhead Intercept ≈ {intercept:.2f}s | Decode Engine ≈ {decode_tps:.1f} tok/s")
    print(f"Median Effective End-to-End  : {statistics.median(r['median_effective_tps'] for r in eval_tests):.1f} tok/s")
    print(f"Persisted Artifact           : {results_filename}")
    print("=" * 85)

if __name__ == "__main__":
    main()
