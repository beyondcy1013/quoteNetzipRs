#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_tmp_dir="$script_dir/../.tmp"
mkdir -p "$project_tmp_dir"
tmp_dir="$(mktemp -d "$project_tmp_dir/full-push-success.XXXXXX")"
trap 'rm -rf "$tmp_dir"; rmdir "$project_tmp_dir" 2>/dev/null || true' EXIT

mkdir -p "$tmp_dir/bin"
cat >"$tmp_dir/bin/date" <<'EOF'
#!/usr/bin/env bash
counter_file="${NETZIP_TEST_DATE_COUNTER:?}"
counter=0
if [[ -f "$counter_file" ]]; then
    counter="$(cat "$counter_file")"
fi
counter=$((counter + 1))
printf '%s' "$counter" >"$counter_file"
if (( counter < 3 )); then
    printf '1:140000\n'
else
    printf '1:150101\n'
fi
EOF
cat >"$tmp_dir/bin/curl" <<'EOF'
#!/usr/bin/env bash
if [[ "$*" == *'/api/codes/worklist'* ]]; then
    printf '{"payload":{"as_of_date":"2026-07-27","freshness":{"required_quote_trade_date":"2026-07-27"}}}\n'
else
    printf '{"success":true,"published_count":1}\n'
fi
EOF
cat >"$tmp_dir/bin/sleep" <<'EOF'
#!/usr/bin/env bash
printf 'sleep %s\n' "$*" >>"${NETZIP_TEST_SLEEP_LOG:?}"
EOF
chmod +x "$tmp_dir/bin/date" "$tmp_dir/bin/curl" "$tmp_dir/bin/sleep"

export PATH="$tmp_dir/bin:$PATH"
export NETZIP_TEST_DATE_COUNTER="$tmp_dir/date-counter"
export NETZIP_TEST_SLEEP_LOG="$tmp_dir/sleep.log"
export NETZIP_FULL_PUSH_ONCE=1

bash "$script_dir/run-full-push.sh" >/dev/null

if [[ -s "$NETZIP_TEST_SLEEP_LOG" ]]; then
    echo "successful full push unexpectedly slept before the next round" >&2
    cat "$NETZIP_TEST_SLEEP_LOG" >&2
    exit 1
fi
