#!/usr/bin/env bash
#
# 09:25 call-auction absolute-baseline capture for Rust 2704 decoder parity.
#
# Rationale (2026-09-03 observed): during 09:15-09:24 the Wine quote_batch
# carries price=0.0; at 09:25:00 the call auction fixes and Wine receives the
# first real price for ~5800 symbols. That first-priced transmission is an
# ABSOLUTE frame (not a uses_baseline delta), which is exactly the baseline
# the Rust 2704 decoder needs. A mid-session capture only contains
# uses_baseline=true deltas whose decode against a pseudo baseline silently
# yields wrong values (600259->0.06 vs 78.98), so the 09:25 window must be
# captured with a fresh pcap.
#
# Usage:
#   open-auction-baseline-capture.sh OUT_DIR [START_HHMM] [END_HHMM]
#   START_HHMM/END_HHMM default 0924 / 0935.
#
set -u
OUT="${1:?usage: open-auction-baseline-capture.sh OUT_DIR [START_HHMM] [END_HHMM]}"
START="${2:-0924}"
END="${3:-0935}"
mkdir -p "$OUT/callbacks"

# Wait until clock >= START
now_hm="$(date +%H%M)"
while [ "$now_hm" -lt "$START" ]; do
    sleep 5
    now_hm="$(date +%H%M)"
done

date +%s%3N > "$OUT/start_ms"
tcpdump -i any -s 0 -w "$OUT/auction-5188.pcap" 'tcp port 5188' >/dev/null 2>&1 &
echo $! > "$OUT/capture.pid"

now_hm="$(date +%H%M)"
while [ "$now_hm" -lt "$END" ]; do
    cm="$(date +%s%3N)"
    resp="$(curl -fsS --max-time 3 'http://127.0.0.1:28787/api/v1/events?limit=400' || true)"
    if [ -n "$resp" ]; then
        jq -c --arg cm "$cm" '.data[]? | . + {capture_polled_at_ms:($cm|tonumber)}' \
            <<<"$resp" >> "$OUT/callbacks/full.jsonl"
    fi
    sleep 1
    now_hm="$(date +%H%M)"
done

cap_pid="$(cat "$OUT/capture.pid")"
kill -INT "$cap_pid" 2>/dev/null || true
wait "$cap_pid" 2>/dev/null || true
date +%s%3N > "$OUT/end_ms"
echo "done $OUT $(date '+%F %T')"
