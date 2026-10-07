#!/usr/bin/env bash
# Question 1: runs a host, a client that digs, and a client that joins late. All headless.
set -u
cd "$(dirname "$0")"
GODOT="${GODOT:-../bin/godot.linuxbsd.editor.x86_64}"
mkdir -p logs
"$GODOT" --headless --path . -- --host > logs/host.log 2>&1 &
HOST=$!
sleep 2
"$GODOT" --headless --path . -- --join --dig > logs/client.log 2>&1 &
sleep 8
"$GODOT" --headless --path . -- --join > logs/late.log 2>&1 &
( sleep 60; kill $HOST 2>/dev/null ) &
wait $HOST; CODE=$?
grep -E '^(PASS|FAIL|RESULT|editable)' logs/host.log
grep -hE 'ERROR|SCRIPT' logs/*.log | sort | uniq -c | head -15
pkill -P $$ 2>/dev/null
exit $CODE
