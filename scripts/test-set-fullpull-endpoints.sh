#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

defaults="$tmp_dir/netzip-rs"
cat >"$defaults" <<'EOF'
NETZIP_FULL_PUSH_MODE=push
NETZIP_TRANSITION_7709_ENDPOINTS=old.example:7709
NETZIP_NATIVE_PUSH_PUBLISH_ENABLE=1
EOF

NETZIP_DEFAULTS_PATH="$defaults" NETZIP_SKIP_RESTART=1 \
    NETZIP_FAILOVER_ENDPOINTS='198.51.100.1:7709,198.51.100.2:7709' \
        bash "$script_dir/set-fullpull-endpoints.sh" failover
grep -Fxq 'NETZIP_TRANSITION_7709_ENDPOINTS=198.51.100.1:7709,198.51.100.2:7709' "$defaults"
test "$(grep -c '^NETZIP_TRANSITION_7709_ENDPOINTS=' "$defaults")" -eq 1
grep -Fxq 'NETZIP_FULL_PUSH_MODE=push' "$defaults"
grep -Fxq 'NETZIP_NATIVE_PUSH_PUBLISH_ENABLE=1' "$defaults"

NETZIP_DEFAULTS_PATH="$defaults" NETZIP_SKIP_RESTART=1 \
    bash "$script_dir/set-fullpull-endpoints.sh" default
! grep -q '^NETZIP_TRANSITION_7709_ENDPOINTS=' "$defaults"
grep -Fxq 'NETZIP_FULL_PUSH_MODE=push' "$defaults"
grep -Fxq 'NETZIP_NATIVE_PUSH_PUBLISH_ENABLE=1' "$defaults"

cat >"$tmp_dir/systemctl" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$ENDPOINT_TEST_LOG"
EOF
chmod +x "$tmp_dir/systemctl"

ENDPOINT_TEST_LOG="$tmp_dir/calls" NETZIP_DEFAULTS_PATH="$defaults" \
    NETZIP_FAILOVER_ENDPOINTS='198.51.100.1:7709,198.51.100.2:7709' \
    PATH="$tmp_dir:$PATH" bash "$script_dir/set-fullpull-endpoints.sh" failover

cat >"$tmp_dir/expected" <<'EOF'
stop quote-netzip-rs-full-push.service
restart quote-netzip-rs-supplement.service
start quote-netzip-rs-full-push.service
is-active --quiet quote-netzip-rs-supplement.service
is-active --quiet quote-netzip-rs-full-push.service
EOF
cmp "$tmp_dir/expected" "$tmp_dir/calls"
