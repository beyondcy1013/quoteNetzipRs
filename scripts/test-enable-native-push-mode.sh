#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_tmp_dir="$script_dir/../.tmp"
mkdir -p "$project_tmp_dir"
tmp_dir="$(mktemp -d "$project_tmp_dir/enable-native-push.XXXXXX")"
trap 'rm -rf "$tmp_dir"; rmdir "$project_tmp_dir" 2>/dev/null || true' EXIT

defaults="$tmp_dir/netzip-rs"
cat >"$defaults" <<'EOF'
NETZIP_QUOTE_GATEWAY_ADDR=127.0.0.1:16886
NETZIP_FULL_PUSH_MODE=poll
NETZIP_NATIVE_PUSH_SESSION_SECS=60
EOF

NETZIP_DEFAULTS_PATH="$defaults" NETZIP_SKIP_RESTART=1 \
    bash "$script_dir/enable-native-push-mode.sh"
NETZIP_DEFAULTS_PATH="$defaults" NETZIP_SKIP_RESTART=1 \
    bash "$script_dir/enable-native-push-mode.sh"

grep -Fxq 'NETZIP_QUOTE_GATEWAY_ADDR=127.0.0.1:16886' "$defaults"
grep -Fxq 'NETZIP_FULL_PUSH_MODE=push' "$defaults"
grep -Fxq 'NETZIP_NATIVE_PUSH_PUBLISH_ENABLE=1' "$defaults"
grep -Fxq 'NETZIP_NATIVE_PUSH_SESSION_SECS=240' "$defaults"
grep -Fxq 'NETZIP_NATIVE_PUSH_AUDIT_INTERVAL_SECS=30' "$defaults"
test "$(grep -c '^NETZIP_FULL_PUSH_MODE=' "$defaults")" -eq 1
test "$(grep -c '^NETZIP_NATIVE_PUSH_PUBLISH_ENABLE=' "$defaults")" -eq 1
