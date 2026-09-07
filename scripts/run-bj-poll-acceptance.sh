#!/usr/bin/env bash
set -euo pipefail

service_addr="${NETZIP_SERVICE_ADDR:-127.0.0.1:16893}"
duration_secs="${NETZIP_BJ_ACCEPTANCE_DURATION_SECS:-15}"
output_path="${NETZIP_BJ_ACCEPTANCE_OUTPUT:-docs/logs/bj-poll-acceptance.json}"

mkdir -p "$(dirname "$output_path")"
response="$(curl --noproxy '*' -fsS --max-time "$((duration_secs + 60))" \
    -X POST "http://$service_addr/api/hqw/push-worklist" \
    -H 'Content-Type: application/json' \
    -d "{\"duration_secs\":$duration_secs,\"audit_interval_secs\":30,\"bj_poll_interval_secs\":3,\"publish\":true}")"
printf '%s\n' "$response" | jq . | tee "$output_path"

jq -e '
    .success == true and
    .publish == true and
    .shard_count == 53 and
    .bj_poll_symbols > 0 and
    .bj_poll_runs >= 3 and
    .bj_poll_failures == 0 and
    .reader_failures == 0
' <<<"$response" >/dev/null
