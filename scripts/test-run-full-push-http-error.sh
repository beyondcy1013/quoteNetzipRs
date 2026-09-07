#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_tmp_dir="$script_dir/../.tmp"
mkdir -p "$project_tmp_dir"
tmp_dir="$(mktemp -d "$project_tmp_dir/full-push-http-error.XXXXXX")"
trap 'rm -rf "$tmp_dir"; rmdir "$project_tmp_dir" 2>/dev/null || true' EXIT

mkdir -p "$tmp_dir/bin"
cat >"$tmp_dir/bin/date" <<'EOF'
#!/usr/bin/env bash
if [[ "${1:-}" == "+%s" ]]; then
    printf '1000\n'
    exit 0
fi
printf '1:140000\n'
EOF
cat >"$tmp_dir/bin/curl" <<'EOF'
#!/usr/bin/env bash
if [[ "$*" == *'/api/fullpull/official-5188/status'* ]]; then
    printf '%s\n' '{"authenticated":true,"control_session_retained":true,"initialized":true,"connection_count":10,"slots":[0,1,2,3,4,5,6,7,8,9],"shadows":[{"running":true,"last_error":null},{"running":true,"last_error":null},{"running":true,"last_error":null},{"running":true,"last_error":null},{"running":true,"last_error":null},{"running":true,"last_error":null},{"running":true,"last_error":null},{"running":true,"last_error":null},{"running":true,"last_error":null},{"running":true,"last_error":null}]}'
    exit 0
fi
if [[ "$*" == *'/api/codes/worklist'* ]]; then
    printf '{"payload":{"as_of_date":"2026-07-30","freshness":{"required_quote_trade_date":"2026-07-30"}}}\n'
    exit 0
fi

body='{"error":"7709 fallback full-push batch 3/5 failed after 3 fresh-session attempts: captured failure"}'
if [[ "$*" == *'--write-out'* || "$*" == *' -w '* ]]; then
    printf '%s\n500' "$body"
    exit 0
fi
printf '%s\n' "$body"
exit 22
EOF
cat >"$tmp_dir/bin/sleep" <<'EOF'
#!/usr/bin/env bash
printf 'sleep %s\n' "$*" >>"${NETZIP_TEST_SLEEP_LOG:?}"
/usr/bin/sleep 0.05
EOF
chmod +x "$tmp_dir/bin/date" "$tmp_dir/bin/curl" "$tmp_dir/bin/sleep"

export PATH="$tmp_dir/bin:$PATH"
export NETZIP_TEST_SLEEP_LOG="$tmp_dir/sleep.log"

set +e
timeout 0.3 bash "$script_dir/run-full-push.sh" >"$tmp_dir/output.log" 2>&1
status=$?
set -e

if [[ "$status" != "124" ]]; then
    echo "persistent full push exited after HTTP 500: status=$status" >&2
    cat "$tmp_dir/output.log" >&2
    exit 1
fi
grep -Fq 'netzip full push failed: stage=push-worklist http_status=500' "$tmp_dir/output.log"
grep -Fq '7709 fallback full-push batch 3/5 failed after 3 fresh-session attempts: captured failure' "$tmp_dir/output.log"
grep -Fq 'sleep 5' "$NETZIP_TEST_SLEEP_LOG"
