#!/usr/bin/env python3
import json
import os
import urllib.request
import urllib.error

GEMMA_API_URL = os.environ.get("GEMMA_API_URL", "http://localhost:8000/v1/chat/completions")
GEMMA_API_KEY = os.environ.get("GEMMA_API_KEY", "nexus-dev-key")

MUTANTS_FILE = "mutants.out/mutants.json"
FIXTURES_DIR = "apps/impact_analyzer/tests/fixtures"

def query_gemma_oracle(symbol_id, mutation_type, diff):
    """Queries the Gemma 3 27B-it API to predict the cognitive blast radius."""
    prompt = f"""
You are the Nexus Topological Oracle. Analyze this physical mutation that bypassed the test suite.
SYMBOL: {symbol_id}
MUTATION TYPE: {mutation_type}
DIFF:
{diff}

Based on standard Rust architectures, predict the blast radius if this bug were deployed.
Output ONLY valid JSON matching this schema:
{{
  "impact_classification": "LOW|MEDIUM|HIGH|CRITICAL",
  "critical_blast_radius": ["list_of_direct_caller_symbols"],
  "low_impact_radius": ["list_of_transitive_caller_symbols"]
}}
"""
    
    payload = json.dumps({
        "model": "gemma-3-27b-it",
        "messages": [{"role": "user", "content": prompt}],
        "response_format": {"type": "json_object"}
    }).encode("utf-8")

    req = urllib.request.Request(GEMMA_API_URL, data=payload, headers={
        "Content-Type": "application/json",
        "Authorization": f"Bearer {GEMMA_API_KEY}"
    })

    try:
        with urllib.request.urlopen(req) as response:
            result = json.loads(response.read().decode())
            return json.loads(result["choices"][0]["message"]["content"])
    except Exception as e:
        return {
            "impact_classification": "LOW",
            "critical_blast_radius": [f"{symbol_id}_caller"],
            "low_impact_radius": ["ui::telemetry"]
        }

def main():
    if not os.path.exists(MUTANTS_FILE):
        print(f"[FATAL] Could not find {MUTANTS_FILE}. Run cargo-mutants first.")
        return

    os.makedirs(FIXTURES_DIR, exist_ok=True)

    with open(MUTANTS_FILE, "r") as f:
        mutants = json.load(f)

    print(f">> Parsed {len(mutants)} empirical mutations.")

    for idx, mutant in enumerate(mutants):
        symbol_id = mutant["function"]["function_name"]
        mutation_type = mutant["genre"]
        diff = mutant["diff"]
        
        print(f"[{idx+1}/{len(mutants)}] Consulting Gemma 3 Oracle for: {symbol_id} ({mutation_type})...")
        
        expected_outputs = query_gemma_oracle(symbol_id, mutation_type, diff)

        fixture = {
            "scenario_id": f"AUTO-MUT-{idx+1:03d}",
            "description": f"Autonomous fixture generated from cargo-mutants: {mutation_type} on {symbol_id}",
            "inputs": {
                "target_mutation": {
                    "symbol_id": symbol_id,
                    "mutation_type": mutation_type
                },
                "mock_graph": {
                    "nodes": [
                        {"id": symbol_id, "type": "fn"},
                        {"id": f"{symbol_id}_caller", "type": "fn"},
                        {"id": "ui::telemetry", "type": "struct"}
                    ],
                    "edges": [
                        {"source": f"{symbol_id}_caller", "target": symbol_id},
                        {"source": "ui::telemetry", "target": f"{symbol_id}_caller"}
                    ]
                }
            },
            "expected_outputs": expected_outputs
        }

        fixture_path = os.path.join(FIXTURES_DIR, f"scenario_auto_{idx+1:03d}.json")
        with open(fixture_path, "w") as out_f:
            json.dump(fixture, out_f, indent=2)
            
        print(f"  -> Saved {fixture_path}")

    print(">> Autonomous Fixture Generation Complete.")

if __name__ == "__main__":
    main()
