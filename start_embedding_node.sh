#!/usr/bin/env bash
set -u

EMBED_HOST="100.115.92.204"
EMBED_PORT="9380"
EMBED_URL="http://${EMBED_HOST}:${EMBED_PORT}"

S23_HOST="192.168.4.223"
S23_PORT="8022"
S23_USER="donball197"
TUNNEL_PORT="9381"

echo "=================================================="
echo "ATHENAFUSIONX — DUET EMBEDDING NODE"
echo "=================================================="
echo

echo "[1/5] Checking Granite embedding service..."

if curl -fsS --connect-timeout 2 --max-time 4 \
    "http://127.0.0.1:${EMBED_PORT}/health" >/tmp/athena_embedding_health.json 2>/dev/null
then
    echo "[PASS] Granite embedding service already running."
else
    echo "[INFO] Granite service is not running."
    echo "[INFO] Starting serve_embeddings.py..."

    nohup python3 serve_embeddings.py \
        >/tmp/athena_embedding_node.log 2>&1 &

    sleep 3

    if ! curl -fsS --connect-timeout 2 --max-time 5 \
        "http://127.0.0.1:${EMBED_PORT}/health" \
        >/tmp/athena_embedding_health.json 2>/dev/null
    then
        echo "[FAIL] Granite embedding service did not become healthy."
        echo
        echo "Last log output:"
        tail -40 /tmp/athena_embedding_node.log 2>/dev/null || true
        exit 1
    fi

    echo "[PASS] Granite embedding service started."
fi

cat /tmp/athena_embedding_health.json
echo

echo "[2/5] Verifying Duet network endpoint..."

if curl -fsS --connect-timeout 2 --max-time 4 \
    "${EMBED_URL}/health" >/dev/null
then
    echo "[PASS] ${EMBED_URL}/health"
else
    echo "[FAIL] Duet network endpoint ${EMBED_URL} is unavailable."
    exit 1
fi

echo
echo "[3/5] Verifying passwordless SSH to S23..."

if ssh -p "${S23_PORT}" \
    -o BatchMode=yes \
    -o PasswordAuthentication=no \
    -o ConnectTimeout=5 \
    "${S23_USER}@${S23_HOST}" \
    'echo SSH_OK' 2>/dev/null | grep -q '^SSH_OK$'
then
    echo "[PASS] SSH key authentication."
else
    echo "[FAIL] Passwordless SSH to S23 failed."
    echo
    echo "Expected:"
    echo "  ${S23_USER}@${S23_HOST}:${S23_PORT}"
    exit 1
fi

echo
echo "[4/5] Checking reverse tunnel..."

if pgrep -af -- "-R ${TUNNEL_PORT}:${EMBED_HOST}:${EMBED_PORT}" >/dev/null 2>&1
then
    echo "[PASS] Reverse tunnel already running."
else
    echo "[INFO] Starting reverse SSH tunnel..."

    ssh -f -N \
        -o ExitOnForwardFailure=yes \
        -o ServerAliveInterval=30 \
        -o ServerAliveCountMax=3 \
        -R "${TUNNEL_PORT}:${EMBED_HOST}:${EMBED_PORT}" \
        -p "${S23_PORT}" \
        "${S23_USER}@${S23_HOST}"

    sleep 1

    if pgrep -af -- "-R ${TUNNEL_PORT}:${EMBED_HOST}:${EMBED_PORT}" >/dev/null 2>&1
    then
        echo "[PASS] Reverse tunnel established."
    else
        echo "[FAIL] Reverse tunnel process not detected."
        exit 1
    fi
fi

echo
echo "[5/5] Finalizing Duet node..."

echo "[PASS] DUET EMBEDDING NODE READY"
echo
echo "Granite:     ${EMBED_URL}"
echo "Tunnel:      S23 127.0.0.1:${TUNNEL_PORT}"
echo "Model:       granite-311m-edge"
echo "Dimension:   384"
echo
echo "Temporary log:"
echo "  /tmp/athena_embedding_node.log"
echo
