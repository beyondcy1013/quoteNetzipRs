#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
source_binary="$project_dir/target/release/quoteNetzipRs"
install_dir="/home/bin/netzip"
install_binary="$install_dir/quoteNetzipRs"
legacy_install_binary="$install_dir/netzip_service"
unit_source="$project_dir/deploy/quote-netzip-rs-supplement.service"
unit_target="/etc/systemd/system/quote-netzip-rs-supplement.service"
retired_unit_target="/etc/systemd/system/quote-netzip-rs.service"
push_script_source="$project_dir/scripts/run-full-push.sh"
push_script_target="$install_dir/run-full-push.sh"
push_service_source="$project_dir/deploy/quote-netzip-rs-full-push.service"
push_service_target="/etc/systemd/system/quote-netzip-rs-full-push.service"
push_timer_target="/etc/systemd/system/quote-netzip-rs-full-push.timer"

test -x "$source_binary"
test -f "$unit_source"
test -f "$push_script_source"
test -f "$push_service_source"
install -d -m 0755 "$install_dir"
if [[ -f "$legacy_install_binary" ]]; then
    mv -f "$legacy_install_binary" "$legacy_install_binary.retired"
fi
if [[ -f "$install_binary" ]]; then
    mv -f "$install_binary" "$install_binary.bak"
fi
install -m 0755 "$source_binary" "$install_binary"
install -m 0755 "$push_script_source" "$push_script_target"
install -m 0644 "$unit_source" "$unit_target"
install -m 0644 "$push_service_source" "$push_service_target"
systemctl disable --now netzip-rs-full-push.timer netzip-rs-full-push.service 2>/dev/null || true
systemctl disable --now netzip-rs.service 2>/dev/null || true
systemctl disable --now netzip-full-push.timer 2>/dev/null || true
systemctl stop quote-netzip-rs-full-push.service 2>/dev/null || true
systemctl disable --now quote-netzip-rs.service 2>/dev/null || true
systemctl stop netzip-rs-full-push.service netzip-rs.service netzip-full-push.service netzip-service.service 2>/dev/null || true
rm -f /etc/systemd/system/netzip-rs-full-push.timer \
    /etc/systemd/system/netzip-rs-full-push.service \
    /etc/systemd/system/netzip-rs.service \
    /etc/systemd/system/netzip-full-push.timer \
    /etc/systemd/system/netzip-full-push.service \
    /etc/systemd/system/netzip-service.service \
    "$retired_unit_target" \
    "$push_timer_target"
systemctl daemon-reload
systemctl enable quote-netzip-rs-supplement.service
systemctl restart quote-netzip-rs-supplement.service
if [[ "${NETZIP_INSTALL_ENABLE_FULL_PUSH:-1}" == "1" ]]; then
    systemctl enable quote-netzip-rs-full-push.service
    systemctl restart quote-netzip-rs-full-push.service
else
    systemctl disable --now quote-netzip-rs-full-push.service
fi
systemctl is-active --quiet quote-netzip-rs-supplement.service
if [[ "${NETZIP_INSTALL_ENABLE_FULL_PUSH:-1}" == "1" ]]; then
    systemctl is-active --quiet quote-netzip-rs-full-push.service
else
    ! systemctl is-active --quiet quote-netzip-rs-full-push.service
fi
test ! -e "$push_timer_target"
curl -fsS --max-time 5 http://127.0.0.1:16893/health >/dev/null
