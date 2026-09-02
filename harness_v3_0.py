#!/usr/bin/env python3
"""AthenaFusionX Benchmark Harness v3.0 (Zero-Shot Baseline - 7/8 Pass Rate)"""
import json, time, statistics, requests

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
    if text.startswith("```json"): text = text[7:]
    elif text.startswith("```"): text = text[3:]
    if text.endswith("```"): text = text[:-3]
    return json.loads(text.strip())

TESTS = [
    {
        "id": "T01_gate_completed_objective", "domain": "Sentinel Gate", "max_tokens": 16,
        "prompt": "Task: You are an execution sentinel. Is the objective completely satisfied by the evidence?\nObjective: Create report.md.\nEvidence: [WRITE_OK] report.md written (1420 bytes).\nOutput JSON: {\"verdict\": \"YES\"} or {\"verdict\": \"NO\"}",
        "validate": lambda c: parse_payload_json(c).get("verdict") == "YES"
    },
    {
        "id": "T02_gate_missing_deliverable", "domain": "Sentinel Gate", "max_tokens": 16,
        "prompt": "Task: You are an execution sentinel. If ANY requested artifact is missing from evidence, answer NO.\nObjective: Create audit.json and summary.md.\nEvidence: [WRITE_OK] audit.json written.\nOutput JSON: {\"verdict\": \"YES\"} or {\"verdict\": \"NO\"}",
        "validate": lambda c: parse_payload_json(c).get("verdict") == "NO"
    },
    {
        "id": "T03_gate_failed_command_trap", "domain": "Sentinel Gate", "max_tokens": 16,
        "prompt": "Task: You are an execution sentinel.\nObjective: Compile release binary.\nEvidence: [ACTION FAILED] exit status: 101 error: could not compile.\nOutput JSON: {\"verdict\": \"YES\"} or {\"verdict\": \"NO\"}",
        "validate": lambda c: parse_payload_json(c).get("verdict") == "NO"
    },
    {
        "id": "T04_triage_sqli_raw_concat", "domain": "AST Security", "max_tokens": 64,
        "prompt": "Analyze the PHP snippet below for SQL_INJECTION vulnerability.\nRubric:\n- Return 'EXPLOITABLE' if untrusted input is concatenated into the query without parameterization.\n- Return 'SAFE' if parameterized with prepared statements.\n- Return 'UNKNOWN' if query provenance cannot be determined.\n\nFormat: <payload>{\"state\": \"EXPLOITABLE\" | \"SAFE\" | \"UNKNOWN\", \"severity\": \"High\" | \"Medium\" | \"Low\"}</payload>\n\n```php\n$q = 'SELECT * FROM users WHERE id = ' . $_GET['id'];\n$r = mysqli_query($con, $q);\n```",
        "validate": lambda c: parse_payload_json(c).get("state") == "EXPLOITABLE"
    },
    {
        "id": "T05_triage_sqli_prepared_stmt", "domain": "AST Security", "max_tokens": 64,
        "prompt": "Analyze the PHP snippet below for SQL_INJECTION vulnerability.\nRubric:\n- Return 'EXPLOITABLE' if untrusted input is concatenated into the query without parameterization.\n- Return 'SAFE' if parameterized with prepared statements.\n- Return 'UNKNOWN' if query provenance cannot be determined.\n\nFormat: <payload>{\"state\": \"EXPLOITABLE\" | \"SAFE\" | \"UNKNOWN\", \"severity\": \"High\" | \"Medium\" | \"Low\"}</payload>\n\n```php\n$stmt = $con->prepare('SELECT * FROM users WHERE id = ?');\n$stmt->bind_param('i', $_GET['id']);\n$stmt->execute();\n```",
        "validate": lambda c: parse_payload_json(c).get("state") == "SAFE"
    },
    {
        "id": "T06_triage_unknown_provenance", "domain": "AST Security", "max_tokens": 64,
        "prompt": "Analyze the PHP snippet below for SQL_INJECTION vulnerability.\nRubric:\n- Return 'EXPLOITABLE' only if code demonstrably concatenates unsanitized input into the query.\n- Return 'SAFE' only if demonstrably parameterized.\n- Return 'UNKNOWN' if the variable's source/construction is not visible in the snippet.\n\nFormat: <payload>{\"state\": \"EXPLOITABLE\" | \"SAFE\" | \"UNKNOWN\", \"severity\": \"High\" | \"Medium\" | \"Low\"}</payload>\n\n```php\n$res = mysqli_query($con, $internal_query);\n```",
        "validate": lambda c: parse_payload_json(c).get("state") == "UNKNOWN"
    },
    {
        "id": "T07_extract_parameter_mapping", "domain": "Code Extraction", "max_tokens": 48,
        "prompt": "Code snippet:\n$stmt->bind_param('ssi', $user, $pass, $role_id);\nExtract all bound variable names. Output only the variable names separated by commas.",
        "validate": lambda c: "$user" in c and "$pass" in c and "$role_id" in c
    },
    {
        "id": "T08_remediation_diff_anchor", "domain": "Code Remediation", "max_tokens": 96,
        "prompt": "Role: Security Remediation Engine.\nTask: Rewrite the PHP snippet to use a mysqli prepared statement ($con->prepare, bind_param, execute, get_result).\nRules:\n1. Output ONLY the raw PHP replacement code inside <payload></payload> tags.\n2. Do NOT invent variable names. Use only $id.\n\nSnippet:\n```php\n$q = 'SELECT * FROM users WHERE id = ' . $id;\n$r = mysqli_query($con, $q);\n```",
        "validate": lambda c: "prepare(" in c and "bind_param" in c and "?" in c and "$id" in c and ". $id" not in c and "execute(" in c
    }
]
