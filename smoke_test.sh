#!/usr/bin/env bash
cd "$(dirname "$0")" || exit 1
BIN=target/debug/nexus-ide-backend
echo "building..."
cargo +nightly build -p nexus_ide_backend 2>&1 | tail -3
[ -x "$BIN" ] || { echo "no binary at $BIN"; exit 1; }
KEY=$(grep '^NEXUS_AUTH_KEY=' .env | cut -d= -f2- | tr -d "\"'\r")
U=http://127.0.0.1:3000

pkill -f nexus-ide-backend 2>/dev/null; sleep 2
"$BIN" > audit_out/daemon.log 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null' EXIT
echo "waiting for daemon (up to 600s)..."
for i in $(seq 1 600); do
  curl -sf $U/health >/dev/null && { echo "up after ${i}s"; break; }
  kill -0 $PID 2>/dev/null || { echo "!! daemon died at boot:"; tail -30 audit_out/daemon.log; exit 1; }
  sleep 1
done
curl -sf $U/health >/dev/null || { echo "!! never came up:"; tail -30 audit_out/daemon.log; exit 1; }

chk(){ printf '%-42s %s (want %s)\n' "$1" "$2" "$3"; }
code(){ curl -s -o /dev/null -w '%{http_code}' "$@"; }

chk "health"                       "$(code $U/health)" 200
chk "login wrong pw"               "$(code -X POST $U/api/auth/login -H 'content-type: application/json' -d '{"password":"x"}')" 401
chk "login empty pw"               "$(code -X POST $U/api/auth/login -H 'content-type: application/json' -d '{"password":""}')" 401
TOKEN=$(curl -s -X POST $U/api/auth/login -H 'content-type: application/json' -d "{\"password\":\"$KEY\"}" | python3 -c 'import sys,json;print(json.load(sys.stdin).get("token",""))')
chk "login correct (token len>0)"  "${#TOKEN}" ">0"
chk "metrics no token"             "$(code $U/api/metrics)" 401
chk "metrics with token"           "$(code $U/api/metrics -H "Authorization: Bearer $TOKEN")" 200

WS=(-H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==' --max-time 2)
chk "ws no token"                  "$(code "${WS[@]}" $U/ws/telemetry)" 401
chk "ws bad token"                 "$(code "${WS[@]}" "$U/ws/telemetry?token=bad")" 401
chk "ws good token"                "$(code "${WS[@]}" "$U/ws/telemetry?token=$TOKEN")" 101

A=(-H "Authorization: Bearer $TOKEN")
chk "fs read ../../etc/passwd"     "$(code "${A[@]}" "$U/api/fs/read?path=../../etc/passwd")" "4xx"
chk "fs read /etc/passwd"          "$(code "${A[@]}" "$U/api/fs/read?path=/etc/passwd")" "4xx"
chk "fs write ../escape.txt"       "$(code "${A[@]}" -X POST $U/api/fs/write -H 'content-type: application/json' -d '{"path":"../escape_test.txt","content":"x"}')" "4xx"
ls ../escape_test.txt 2>/dev/null && { echo "!! TRAVERSAL WRITE SUCCEEDED"; rm -f ../escape_test.txt; }
echo "daemon log: audit_out/daemon.log"
