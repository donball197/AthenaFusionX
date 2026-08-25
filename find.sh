cd ~/AthenaFusionX

echo "=================================================="
echo "1. ENVIRONMENT: WHICH PROVIDERS ARE CONFIGURED?"
echo "=================================================="

for v in GEMINI_API_KEY NVIDIA_API_KEY NEXUS_AUTH_KEY NEXUS_API_ENDPOINT NEXUS_PLANNER_MODEL; do
    if [[ -n "${!v:-}" ]]; then
        case "$v" in
            *_KEY)
                echo "$v=SET (length ${#!v})"
                ;;
            *)
                echo "$v=${!v}"
                ;;
        esac
    else
        echo "$v=NOT_SET"
    fi
done

echo
echo "=================================================="
echo "2. PLANNER MODEL/CASCADE CONFIG"
echo "=================================================="

sed -n '1,140p' crates/nexus_dag_orchestrator/src/planner.rs

echo
echo "=================================================="
echo "3. WHERE CASCADES ARE CREATED"
echo "=================================================="

rg -n -C 5 \
  'light_cascade|heavy_cascade|NEXUS_PLANNER_MODEL|HybridDispatcher::new|dispatch_cascade|process_task' \
  crates --glob '!target/**'

echo
echo "=================================================="
echo "4. ALL MODEL NAMES IN CONFIG"
echo "=================================================="

rg -n \
  'nvidia/|nemotron|gemini-|gemma|llama|qwen|deepseek|MODEL|CASCADE' \
  .env .env.* crates \
  --glob '!target/**' \
  2>/dev/null || true

echo
echo "=================================================="
echo "5. PROVIDER ROUTING IMPLEMENTATIONS"
echo "=================================================="

rg -n -C 12 \
  'integrate\.api\.nvidia\.com|generativelanguage\.googleapis\.com|NVIDIA_API_KEY|GEMINI_API_KEY|NEXUS_API_ENDPOINT' \
  crates \
  --glob '!target/**' \
  --glob '!*.bak.*'

echo
echo "=================================================="
echo "6. DISPATCHER TEST SOURCE"
echo "=================================================="

sed -n '1,160p' crates/nexus_edge_minion/tests/dispatcher_integration.rs
