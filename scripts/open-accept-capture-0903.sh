#!/usr/bin/env bash
set -u
RUN="${1:?usage: ... OUT_DIR}"
DUR="${2:-1500}"          # 默认25分钟 (1500s)，覆盖 09:16->09:41
mkdir -p "$RUN/callbacks"
sudo -n tcpdump -i any -s 0 -w "$RUN/full-5188.pcap" 'tcp port 5188' >/dev/null 2>&1 &
echo $! > "$RUN/capture.pid"
# 记起始
date +%s%3N > "$RUN/start_ms"
deadline=$(( $(date +%s) + DUR ))
while [ "$(date +%s)" -lt "$deadline" ]; do
  now_ms=$(date +%s%3N)
  response=$(curl -fsS --max-time 3 'http://127.0.0.1:28787/api/v1/events?limit=400' || true)
  if [ -n "$response" ]; then
    jq -c --arg cm "$now_ms" '.data[]? | . + {capture_polled_at_ms:($cm|tonumber)}' <<<"$response" \
      >> "$RUN/callbacks/full.jsonl"
  fi
  sleep 1
done
CAP_PID=$(cat "$RUN/capture.pid"); sudo -n kill -INT "$CAP_PID" 2>/dev/null || true; wait "$CAP_PID" 2>/dev/null || true
date +%s%3N > "$RUN/end_ms"
curl -fsS http://127.0.0.1:28787/api/v1/status > "$RUN/wine-status-end.json" 2>/dev/null || true
echo "done $RUN $(date '+%F %T')"
