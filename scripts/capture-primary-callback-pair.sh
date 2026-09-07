#!/usr/bin/env bash
set -euo pipefail

out_dir="${1:-diagnostics/$(date +%Y%m%d)-live-pair}"
duration="${DURATION_SECONDS:-60}"
wine_api="${WINE_API:-http://127.0.0.1:28787}"
mkdir -p "$out_dir/callbacks"
stamp="$(date +%Y%m%dT%H%M%S)"
pcap="$out_dir/wine-primary-sync-$stamp.pcap"
events="$out_dir/callbacks/wine-events-primary-sync-$stamp.jsonl"
status_snapshot="$out_dir/wine-status-primary-sync-$stamp.json"

# Preserve the effective vendor provenance beside the packet/callback pair.
# Callback volume alone does not establish that the primary full-push route was used.
curl -fsS --max-time 3 "$wine_api/api/v1/status" >"$status_snapshot" ||
    printf '{"status":"unavailable","captured_at":"%s"}\n' "$(date --iso-8601=seconds)" >"$status_snapshot"

cleanup() {
    if [[ -n "${tcpdump_pid:-}" ]] && kill -0 "$tcpdump_pid" 2>/dev/null; then
        kill -INT "$tcpdump_pid" 2>/dev/null || true
        wait "$tcpdump_pid" 2>/dev/null || true
    fi
    if [[ -s "$events" ]]; then
        tmp_events="${events}.dedup"
        # Keep one compact JSON object per line so downstream evidence tools
        # can consume the capture as conventional JSONL.
        jq -c -s 'unique_by(.sequence) | sort_by(.sequence) | .[]' "$events" > "$tmp_events"
        mv -f "$tmp_events" "$events"
    fi
}
trap cleanup EXIT INT TERM

: > "$events"
# Include authentication, official quote endpoints, and the vendor's local
# proxy ports so callback provenance can be distinguished from cached output.
tcpdump -i any -s 0 -w "$pcap" 'tcp port 6100 or tcp port 7100 or tcp port 5188 or (tcp portrange 16801-16805)' >/dev/null 2>&1 &
tcpdump_pid=$!
deadline=$((SECONDS + duration))
while ((SECONDS < deadline)); do
    now_ms="$(date +%s%3N)"
    response="$(curl -fsS --max-time 3 "$wine_api/api/v1/events?limit=200" || true)"
    if [[ -n "$response" ]]; then
        jq -c --arg capture_ms "$now_ms" '.data[]? | select(.form == "股票数据" or .channel == "system" or .form == "提示信息") | . + {capture_polled_at_ms: ($capture_ms|tonumber)}' \
            <<<"$response" >> "$events"
    fi
    sleep 1
done

cleanup
sha256sum "$pcap" "$events" "$status_snapshot"
