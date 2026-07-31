#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_tmp_dir="$script_dir/../.tmp"
mkdir -p "$project_tmp_dir"
tmp_dir="$(mktemp -d "$project_tmp_dir/full-push-native.XXXXXX")"
trap 'rm -rf "$tmp_dir"; rmdir "$project_tmp_dir" 2>/dev/null || true' EXIT

mkdir -p "$tmp_dir/bin"
cat >"$tmp_dir/bin/date" <<'EOF'
#!/usr/bin/env bash
printf '1:140000\n'
EOF
cat >"$tmp_dir/bin/curl" <<'EOF'
#!/usr/bin/env bash
if [[ "$*" == *'/api/codes/worklist'* ]]; then
    printf '{"payload":{"as_of_date":"2026-07-31","freshness":{"required_quote_trade_date":"2026-07-31"}}}\n'
else
    printf '%s\n' "$*" >"${NETZIP_TEST_PUBLISH_ARGS:?}"
    printf '{"success":true,"published_records":10}\n200'
fi
EOF
cat >"$tmp_dir/bin/sleep" <<'EOF'
#!/usr/bin/env bash
exit 99
EOF
chmod +x "$tmp_dir/bin/date" "$tmp_dir/bin/curl" "$tmp_dir/bin/sleep"

export PATH="$tmp_dir/bin:$PATH"
export NETZIP_TEST_PUBLISH_ARGS="$tmp_dir/publish-args.log"
export NETZIP_FULL_PUSH_MODE=push
export NETZIP_NATIVE_PUSH_SESSION_SECS=120
export NETZIP_NATIVE_PUSH_AUDIT_INTERVAL_SECS=30
export NETZIP_FULL_PUSH_ONCE=1

bash "$script_dir/run-full-push.sh" >"$tmp_dir/output.log" 2>&1

grep -Fq '/api/hqw/push-worklist' "$NETZIP_TEST_PUBLISH_ARGS"
grep -Fq '"duration_secs":120' "$NETZIP_TEST_PUBLISH_ARGS"
grep -Fq '"audit_interval_secs":30' "$NETZIP_TEST_PUBLISH_ARGS"
grep -Fq '"publish":true' "$NETZIP_TEST_PUBLISH_ARGS"
grep -Fq -- '--max-time 180' "$NETZIP_TEST_PUBLISH_ARGS"
! grep -Fq 'worker_count' "$NETZIP_TEST_PUBLISH_ARGS"
