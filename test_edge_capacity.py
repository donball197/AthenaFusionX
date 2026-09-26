import time
import requests
import json

ENDPOINT = "http://192.168.4.223:9379/v1/chat/completions"
HEADERS = {
    "Content-Type": "application/json",
    "Authorization": "Bearer local-s23-device"
}
MODEL = "gemma-4-e4b-it"
TARGET_FILE = "crates/runtime_core/src/cortical_loop.rs"

with open(TARGET_FILE, "r", encoding="utf-8") as f:
    all_lines = f.readlines()

test_sizes = [50, 100, 150]

print("==================================================")
print(">> TESTING S23 EDGE CAPACITY WITH LIL GEMMA")
print(f">> Target File: {TARGET_FILE} (Total: {len(all_lines)} lines)")
print(f">> Endpoint:    {ENDPOINT}")
print(f">> Model:       {MODEL}")
print("==================================================\n")

for count in test_sizes:
    # Extract chunk starting around line 1700 (compiler critic logic)
    chunk = "".join(all_lines[1700 : 1700 + count])
    char_len = len(chunk)
    approx_tokens = char_len // 4

    # The exact prompt structure cortical_loop sends to the evaluator
    system_prompt = (
        "You are the Epistemic Evaluator. Evaluate whether this physical evidence "
        "advances the strategy: 'Inspect the compiler critic failure branch'. "
        "State your critique and confidence modifier between -0.50 and +0.50."
    )
    user_prompt = (
        f"ACTION TAKEN:\nread_file_chunk({count} lines)\n\n"
        f"PHYSICAL EVIDENCE:\n{chunk}\n\n"
        "Provide a concise evaluation."
    )

    payload = {
        "model": MODEL,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ],
        "temperature": 0.2,
        "max_tokens": 120
    }

    print(f"▶ Testing Chunk: {count} lines (~{char_len} chars, ~{approx_tokens} tokens)...")
    start_t = time.time()
    try:
        res = requests.post(ENDPOINT, headers=HEADERS, json=payload, timeout=30)
        elapsed_ms = int((time.time() - start_t) * 1000)

        if res.status_code == 200:
            data = res.json()
            reply = data["choices"][0]["message"]["content"].strip()
            # Clean up line breaks for display
            clean_reply = " ".join(reply.splitlines()[:2])
            print(f"  • Status : 200 OK in {elapsed_ms}ms")
            print(f"  • Gemma  : \"{clean_reply[:120]}...\"\n")
        else:
            print(f"  • Status : FAILED (HTTP {res.status_code}): {res.text}\n")
    except Exception as e:
        print(f"  • Error  : {e}\n")

print("==================================================")
print(">> BENCHMARK COMPLETE (0 source files modified)")
print("==================================================")
