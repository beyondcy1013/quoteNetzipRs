#!/usr/bin/env bash
set -euo pipefail

defaults_path="${NETZIP_DEFAULTS_PATH:-/etc/default/netzip-rs}"
defaults_dir="$(dirname "$defaults_path")"
mkdir -p "$defaults_dir"
tmp_file="$(mktemp "$defaults_dir/.netzip-rs.XXXXXX")"
trap 'rm -f "$tmp_file"' EXIT

if [[ -f "$defaults_path" ]]; then
    awk '!/^(NETZIP_FULL_PUSH_MODE|NETZIP_NATIVE_PUSH_PUBLISH_ENABLE|NETZIP_NATIVE_PUSH_SESSION_SECS|NETZIP_NATIVE_PUSH_AUDIT_INTERVAL_SECS)=/' \
        "$defaults_path" >"$tmp_file"
fi
cat >>"$tmp_file" <<'EOF'
NETZIP_FULL_PUSH_MODE=push
NETZIP_NATIVE_PUSH_PUBLISH_ENABLE=1
NETZIP_NATIVE_PUSH_SESSION_SECS=240
NETZIP_NATIVE_PUSH_AUDIT_INTERVAL_SECS=30
EOF
install -m 0644 "$tmp_file" "$defaults_path"

if [[ "${NETZIP_SKIP_RESTART:-0}" != "1" ]]; then
    systemctl restart netzip-rs.service
    systemctl restart netzip-rs-full-push.service
    systemctl is-active --quiet netzip-rs.service
    systemctl is-active --quiet netzip-rs-full-push.service
fi

grep -E '^(NETZIP_FULL_PUSH_MODE|NETZIP_NATIVE_PUSH_PUBLISH_ENABLE|NETZIP_NATIVE_PUSH_SESSION_SECS|NETZIP_NATIVE_PUSH_AUDIT_INTERVAL_SECS)=' "$defaults_path"
