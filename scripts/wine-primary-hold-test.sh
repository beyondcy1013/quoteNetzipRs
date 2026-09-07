#!/usr/bin/env bash
set -u
RUN="${1:?usage: wine-primary-hold-test.sh OUT_DIR}"
mkdir -p "$RUN/callbacks"
sudo -n systemctl set-environment QUOTENETZIPWINE_INIT_LOGIN_MODULES=none
sudo -n tcpdump -i any -s 0 -w "$RUN/full-6100-7100-5188.pcap" \
    'tcp port 6100 or tcp port 7100 or tcp port 5188' >/dev/null 2>&1 &
echo $! > "$RUN/capture.pid"
sudo -n systemctl start quoteNetzipWine-wine-supervisor.service
for i in $(seq 1 150); do
    curl -fsS --max-time 2 http://127.0.0.1:28787/health >/dev/null 2>&1 && break
    sleep 1
done
for i in $(seq 1 180); do
    now_ms=$(date +%s%3N)
    response=$(curl -fsS --max-time 3 'http://127.0.0.1:28787/api/v1/events?limit=100' || true)
    if [ -n "$response" ]; then
        jq -c --arg capture_ms "$now_ms" '.data[]? | {sequence,timestamp_ms,channel,form,return_code,text}' \
            <<<"$response" >> "$RUN/callbacks/events.jsonl"
    fi
    sleep 1
done
CAP_PID=$(cat "$RUN/capture.pid")
sudo -n kill -INT "$CAP_PID" 2>/dev/null || true
wait "$CAP_PID" 2>/dev/null || true
curl -fsS http://127.0.0.1:28787/api/v1/status > "$RUN/wine-status-after.json" || true
echo "done $RUN"
