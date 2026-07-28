#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_tmp_dir="$script_dir/../.tmp"
mkdir -p "$project_tmp_dir"
tmp_dir="$(mktemp -d "$project_tmp_dir/full-push-idle.XXXXXX")"
trap 'rm -rf "$tmp_dir"; rmdir "$project_tmp_dir" 2>/dev/null || true' EXIT

mkdir -p "$tmp_dir/bin"
cat >"$tmp_dir/bin/date" <<'EOF'
#!/usr/bin/env bash
printf '1:120000\n'
EOF
cat >"$tmp_dir/bin/curl" <<'EOF'
#!/usr/bin/env bash
printf 'unexpected curl: %s\n' "$*" >>"${NETZIP_TEST_CURL_LOG:?}"
exit 1
EOF
cat >"$tmp_dir/bin/sleep" <<'EOF'
#!/usr/bin/env bash
printf 'sleep %s\n' "$*" >>"${NETZIP_TEST_SLEEP_LOG:?}"
/usr/bin/sleep 0.05
EOF
chmod +x "$tmp_dir/bin/date" "$tmp_dir/bin/curl" "$tmp_dir/bin/sleep"

export PATH="$tmp_dir/bin:$PATH"
export NETZIP_TEST_CURL_LOG="$tmp_dir/curl.log"
export NETZIP_TEST_SLEEP_LOG="$tmp_dir/sleep.log"

set +e
timeout 0.3 bash "$script_dir/run-full-push.sh" >"$tmp_dir/output.log" 2>&1
status=$?
set -e

if [[ "$status" != "124" ]]; then
    echo "persistent full push exited outside the trading window: status=$status" >&2
    cat "$tmp_dir/output.log" >&2
    exit 1
fi
grep -Fq 'online and idle: outside trading window' "$tmp_dir/output.log"
test -s "$NETZIP_TEST_SLEEP_LOG"
test ! -e "$NETZIP_TEST_CURL_LOG"
