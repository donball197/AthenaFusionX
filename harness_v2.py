#!/usr/bin/env python3
"""
AthenaFusionX Benchmark Harness v2.1
Strict, Schema-Framed, Edge AI Evaluation Protocol.
"""

import json
import time
import statistics
import requests

SERVER_URL = "http://192.168.4.223:9379/v1/chat/completions"
MODEL_NAME = "gemma-4-e4b-it"
RUNS_PER_TEST = 3

session = requests.Session()

def parse_json_safely(raw_text):
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

# ----------------------------------------------------------------------
# TEST SUITE DEFINITIONS
# ----------------------------------------------------------------------
TESTS = [
    # --- DOMAIN 1: ADVERSARIAL SENTINEL GATES ---
    {
        "id": "T01_gate_completed_objective",
        "domain": "Sentinel Gate",
        "max_tokens": 32,
        "prompt": "Task: You are an execution sentinel. Is the task completely satisfied?\nObjective: Create report.md.\nEvidence: [WRITE_OK] report.md written (1420 bytes).\nReturn JSON: {\"verdict\": \"YES\" | \"NO\"}",
        "validate": lambda content: parse_json_safely(content).get("verdict") == "YES"
    },
    {
        "id": "T02_gate_missing_deliverable",
        "domain": "Sentinel Gate",
        "max_tokens": 32,
        "prompt": "Task: You are an execution sentinel. If ANY required artifact is missing, answer NO.\nObjective: Create audit.json and summary.md.\nEvidence: [WRITE_OK] audit.json written.\nIs the objective fully satisfied?\nReturn JSON: {\"verdict\": \"YES\" | \"NO\"}",
        "validate": lambda content: parse_json_safely(content).get("verdict") == "NO"
    },
    {
        "id": "T03_gate_failed_command_trap",
        "domain": "Sentinel Gate",
        "max_tokens": 32,
        "prompt": "Task: You are an execution sentinel.\nObjective: Compile release binary.\nEvidence: [ACTION FAILED] exit status: 101 error: could not compile.\nIs the objective satisfied?\nReturn JSON: {\"verdict\": \"YES\" | \"NO\"}",
        "validate": lambda content: parse_json_safely(content).get("verdict") == "NO"
    },

    # --- DOMAIN 2: AST SECURITY TRIAGE ---
    {
        "id": "T04_triage_sqli_raw_concat",
        "domain": "AST Security",
        "max_tokens": 64,
        "prompt": "Analyze for SQL_INJECTION:\n```php\n$q = 'SELECT * FROM users WHERE id = ' . $_GET['id'];\n$r = mysqli_query($con, $q);\n```\nReturn JSON in <payload></payload>:\n<payload>{\"state\": \"EXPLOITABLE\", \"severity\": \"High\", \"reason\": \"SQL injection via concatenation\"}</payload>",
        "validate": lambda content: parse_json_safely(content).get("state") == "EXPLOITABLE"
    },
    {
        "id": "T05_triage_sqli_prepared_stmt",
        "domain": "AST Security",
        "max_tokens": 64,
        "prompt": "Analyze for SQL_INJECTION:\n```php\n$stmt = $con->prepare('SELECT * FROM users WHERE id = ?');\n$stmt->bind_param('i', $_GET['id']);\n$stmt->execute();\n```\nReturn JSON in <payload></payload>:\n<payload>{\"state\": \"SAFE\", \"severity\": \"Low\", \"reason\": \"Parameterized prepared statement\"}</payload>",
        "validate": lambda content: parse_json_safely(content).get("state") == "SAFE"
    },
    {
        "id": "T06_triage_unknown_provenance",
        "domain": "AST Security",
        "max_tokens": 64,
        "prompt": "Analyze for SQL_INJECTION without assuming $internal_query origin:\n```php\n$res = mysqli_query($con, $internal_query);\n```\nReturn JSON in <payload></payload>:\n<payload>{\"state\": \"UNKNOWN\", \"severity\": \"Medium\", \"reason\": \"Origin unknown\"}</payload>",
        "validate": lambda content: parse_json_safely(content).get("state") == "UNKNOWN"
    },

    # --- DOMAIN 3: CODE EXTRACTION & STRUCTURED REPAIR ---
    {
        "id": "T07_extract_parameter_mapping",
        "domain": "Code Extraction",
        "max_tokens": 64,
        "prompt": "Code: $stmt->bind_param('ssi', $user, $pass, $role_id);\nList the 3 variables bound above, separated by commas:",
        "validate": lambda content: "$user" in content and "$pass" in content and "$role_id" in content
    },
    {
        "id": "T08_remediation_diff_anchor",
        "domain": "Code Remediation",
        "max_tokens": 96,
        "prompt": "Role: Security Remediation Engine.\nTask: Rewrite to safe prepared statement:\n```php\n$q = 'SELECT * FROM users WHERE id = ' . $id;\n$r = mysqli_query($con, $q);\n```\nOutput ONLY inside <payload></payload> tags:\n<payload>$stmt = $con->prepare(\"SELECT * FROM users WHERE id = ?\");\n$stmt->bind_param(\"i\", $id);\n$stmt->execute();\n$r = $stmt->get_result();</payload>",
        "validate": lambda content: "?" in content and "prepare" in content and "bind_param" in content
    }
]

# ----------------------------------------------------------------------
# BENCHMARK ENGINE
# ----------------------------------------------------------------------
def run_single_inference(test):
    payload = {
        "model": MODEL_NAME,
        "messages": [{"role": "user", "content": test["prompt"]}],
        "max_tokens": test["max_tokens"],
        "temperature": 0.0
    }

    start = time.perf_counter()
    resp = session.post(SERVER_URL, json=payload, timeout=45)
    wall_time = time.perf_counter() - start
    
    resp.raise_for_status()
    data = resp.json()
    
    content = data["choices"][0]["message"]["content"].strip()
    gen_tokens = data.get("usage", {}).get("completion_tokens", len(content.split()))
    
    dec_tps = gen_tokens / max(wall_time, 0.001)
    prompt_ms = 0.0
    
    passed = False
    try:
        passed = test["validate"](content)
    except Exception:
        passed = False
        
    return {
        "passed": passed,
        "wall_time_s": wall_time,
        "decode_tps": dec_tps,
        "prompt_ms": prompt_ms,
        "content": content
    }

def main():
    print("=" * 70)
    print(f">> AthenaFusionX Benchmark Harness v2.1 (Target: {MODEL_NAME})")
    print(f">> Runs Per Task: {RUNS_PER_TEST} | Protocol: Structured Verification")
    print("=" * 70)
    
    results = []
    
    for i, test in enumerate(TESTS, 1):
        print(f"\n[{i:02d}/{len(TESTS):02d}] Running: {test['id']} ({test['domain']})...")
        
        run_data = []
        for r in range(RUNS_PER_TEST):
            try:
                res = run_single_inference(test)
                run_data.append(res)
            except Exception as e:
                print(f"    [!] Run {r+1} Error: {e}")
                run_data.append({"passed": False, "wall_time_s": 45.0, "decode_tps": 0.0, "prompt_ms": 0.0, "content": "ERROR"})
        
        passed = (sum(1 for r in run_data if r["passed"]) >= (RUNS_PER_TEST / 2))
        med_wall = statistics.median(r["wall_time_s"] for r in run_data)
        med_tps = statistics.median(r["decode_tps"] for r in run_data)
        
        status = "PASS" if passed else "FAIL"
        color = "\033[92m" if passed else "\033[91m"
        print(f"    -> Result: {color}{status}\033[0m | Avg Wall: {med_wall:.2f}s | Speed: {med_tps:.1f} tok/s")
        
        if not passed:
            sample_content = run_data[0]["content"]
            snippet = sample_content if len(sample_content) <= 120 else sample_content[:120] + "..."
            print(f"       [Sample Output]: {repr(snippet)}")

        results.append({
            "id": test["id"],
            "domain": test["domain"],
            "passed": passed,
            "wall_s": med_wall,
            "tps": med_tps
        })

    # Summary
    print("\n" + "=" * 70)
    print(">> FINAL BENCHMARK SUMMARY")
    print("=" * 70)
    
    total_passed = sum(1 for r in results if r["passed"])
    pass_rate = (total_passed / len(results)) * 100
    print(f"Overall Accuracy: {total_passed}/{len(results)} ({pass_rate:.1f}%)")
    print(f"Median Generation Speed: {statistics.median(r['tps'] for r in results):.1f} tok/s")
    print("=" * 70)

if __name__ == "__main__":
    main()
