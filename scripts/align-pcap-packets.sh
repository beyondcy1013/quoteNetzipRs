#!/usr/bin/env bash
set -euo pipefail

pcap=${1:?usage: $0 PCAP [SERVICE_URL]}
service_url=${2:-http://127.0.0.1:16893}

[[ -f "$pcap" ]] || { echo "pcap not found: $pcap" >&2; exit 2; }
pcap=$(realpath "$pcap")

request() {
  local path=$1
  curl -fsS --max-time 30 -X POST "$service_url$path" \
    -H 'content-type: application/json' \
    --data-binary "$(jq -cn --arg path "$pcap" '{path:$path}')"
}

echo "pcap=$pcap"
echo "-- normalized application-bearing streams --"
request /api/debug/pcap-summary | jq -r '
  .flows[] | select(.wire_payload_bytes > 0) |
  [.src, .dst, .total_packets, .unique_packets, .duplicate_packets,
   .wire_payload_bytes, ((.samples // []) | map(.netpacket_prefix // "") | join(" || "))] | @tsv'

echo "-- 5188 reconstructed frames --"
request /api/debug/pcap-5188-summary | jq -r '
  .flows[] |
  [.src, .dst, .frame_count, .client_frames, .server_frames,
   .unknown_frames, (.kind_counts | tojson), (.payload_hint_counts | tojson)] | @tsv'
