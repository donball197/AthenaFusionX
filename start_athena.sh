#!/bin/bash
PHONE_IP="192.168.4.223"
PHONE_USER="donball197"
PHONE_PORT="8022"

echo "[1/3] Cleaning up old processes..."
pkill -f "serve_embeddings.py"
pkill -f "ssh.*-R 9380"

echo "[2/3] Starting LiteRT embedding server..."
cd ~/AthenaFusionX
nohup /root/granite-embed-venv/bin/python serve_embeddings.py > ~/AthenaFusionX/server.log 2>&1 &
sleep 2

echo "[3/3] Opening reverse tunnel to Android..."
ssh -f -N \
  -o ExitOnForwardFailure=yes \
  -o ServerAliveInterval=30 \
  -o ServerAliveCountMax=3 \
  -R 9380:127.0.0.1:9380 \
  -p $PHONE_PORT \
  $PHONE_USER@$PHONE_IP

if [ $? -eq 0 ]; then
    echo "=========================================="
    echo " AthenaFusionX Bridge is ACTIVE!"
    echo " Endpoint: http://127.0.0.1:9380"
    echo "=========================================="
else
    echo "[ERROR] Failed to establish tunnel. Is Termux running sshd?"
fi
