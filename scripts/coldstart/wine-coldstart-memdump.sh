#!/usr/bin/env bash
# Cold-restart Wine while capturing 5188 traffic + callbacks, then dump the
# vendor client's (网际风.exe) memory shortly after the initial 2704 dump.
set -uo pipefail
cd /home/codes/stock/quoteNetzipRs
stamp="$(date +%Y%m%dT%H%M%S)"
out="diagnostics/20260904-cold-start/wine-night-coldstart-$stamp"
mkdir -p "$out"
old_vendor_pid="$(pgrep -f '网际风.exe' | head -n1 || true)"
echo "old_vendor_pid=$old_vendor_pid out=$out" | tee "$out/run.log"

DURATION_SECONDS=300 bash scripts/capture-primary-callback-pair.sh "$out" >>"$out/capture.log" 2>&1 &
capture_pid=$!
sleep 8
echo "$(date +%T) restarting supervisor" | tee -a "$out/run.log"
systemctl restart quoteNetzipWine-wine-supervisor.service
echo "$(date +%T) restart returned rc=$?" | tee -a "$out/run.log"

new_pid=""
for _ in $(seq 1 120); do
    for p in $(pgrep -f '网际风.exe' || true); do
        if [[ "$p" != "$old_vendor_pid" ]]; then new_pid="$p"; fi
    done
    [[ -n "$new_pid" ]] && break
    sleep 1
done
echo "$(date +%T) new_vendor_pid=$new_pid" | tee -a "$out/run.log"
if [[ -z "$new_pid" ]]; then
    wait "$capture_pid"
    exit 1
fi
t0=$SECONDS
for delay in 45 90 150; do
    while (( SECONDS - t0 < delay )); do sleep 1; done
    ts="$(date +%T)"
    python3 scripts/coldstart/dump-vendor-client-memory.py "$new_pid" "$out/wjf-mem-t${delay}" | tee -a "$out/run.log"
    echo "$ts mem dump t+${delay}s done" | tee -a "$out/run.log"
    curl -fsS --max-time 3 http://127.0.0.1:28787/api/v1/status >"$out/wine-status-t${delay}.json" 2>/dev/null || true
done
wait "$capture_pid"
echo "$(date +%T) capture finished" | tee -a "$out/run.log"
ls -la "$out" >>"$out/run.log"
