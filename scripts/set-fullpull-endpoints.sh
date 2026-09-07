#!/usr/bin/env bash
set -euo pipefail

mode="${1:-}"
defaults_path="${NETZIP_DEFAULTS_PATH:-/etc/default/netzip-rs}"
failover_endpoints="${NETZIP_FAILOVER_ENDPOINTS:-}"

case "$mode" in
    failover|default) ;;
    *)
        echo "usage: $0 {failover|default}" >&2
        exit 2
        ;;
esac

defaults_dir="$(dirname "$defaults_path")"
mkdir -p "$defaults_dir"
tmp_file="$(mktemp "$defaults_dir/.netzip-rs-endpoints.XXXXXX")"
trap 'rm -f "$tmp_file"' EXIT

if [[ -f "$defaults_path" ]]; then
    awk '!/^NETZIP_TRANSITION_7709_ENDPOINTS=/' "$defaults_path" >"$tmp_file"
fi

if [[ "$mode" == "failover" ]]; then
    if [[ -z "$failover_endpoints" ]]; then
        echo "NETZIP_FAILOVER_ENDPOINTS is required and must contain authenticated Netzip full-pull endpoints" >&2
        exit 2
    fi
    if [[ "$failover_endpoints" != *,* ]]; then
        echo "NETZIP_FAILOVER_ENDPOINTS must contain at least two endpoints" >&2
        exit 2
    fi
    printf 'NETZIP_TRANSITION_7709_ENDPOINTS=%s\n' "$failover_endpoints" >>"$tmp_file"
fi

install -m 0644 "$tmp_file" "$defaults_path"

if [[ "${NETZIP_SKIP_RESTART:-0}" != "1" ]]; then
    systemctl stop quote-netzip-rs-full-push.service
    systemctl restart quote-netzip-rs-supplement.service
    systemctl start quote-netzip-rs-full-push.service
    systemctl is-active --quiet quote-netzip-rs-supplement.service
    systemctl is-active --quiet quote-netzip-rs-full-push.service
fi

if [[ "$mode" == "failover" ]]; then
    grep -Fx "NETZIP_TRANSITION_7709_ENDPOINTS=$failover_endpoints" "$defaults_path"
else
    if grep -q '^NETZIP_TRANSITION_7709_ENDPOINTS=' "$defaults_path"; then
        echo "endpoint override remains in $defaults_path" >&2
        exit 1
    fi
    echo "NETZIP_TRANSITION_7709_ENDPOINTS=<required-explicit-authenticated-pool>"
fi
