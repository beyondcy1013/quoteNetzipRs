#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
source_binary="$project_dir/target/release/netzip_service"
install_dir="/home/bin/netzip"
install_binary="$install_dir/netzip_service"
unit_source="$project_dir/deploy/netzip-rs.service"
unit_target="/etc/systemd/system/netzip-rs.service"
push_script_source="$project_dir/scripts/run-full-push.sh"
push_script_target="$install_dir/run-full-push.sh"
push_service_source="$project_dir/deploy/netzip-rs-full-push.service"
push_service_target="/etc/systemd/system/netzip-rs-full-push.service"
push_timer_target="/etc/systemd/system/netzip-rs-full-push.timer"

test -x "$source_binary"
test -f "$unit_source"
test -f "$push_script_source"
test -f "$push_service_source"
install -d -m 0755 "$install_dir"
if [[ -f "$install_binary" ]]; then
    mv -f "$install_binary" "$install_binary.bak"
fi
install -m 0755 "$source_binary" "$install_binary"
install -m 0755 "$push_script_source" "$push_script_target"
install -m 0644 "$unit_source" "$unit_target"
install -m 0644 "$push_service_source" "$push_service_target"
systemctl disable --now netzip-rs-full-push.timer 2>/dev/null || true
systemctl disable --now netzip-full-push.timer 2>/dev/null || true
systemctl stop netzip-full-push.service netzip-service.service 2>/dev/null || true
rm -f /etc/systemd/system/netzip-full-push.timer \
    /etc/systemd/system/netzip-full-push.service \
    /etc/systemd/system/netzip-service.service \
    "$push_timer_target"
systemctl daemon-reload
systemctl enable netzip-rs.service
systemctl restart netzip-rs.service
if [[ "${NETZIP_INSTALL_ENABLE_FULL_PUSH:-1}" == "1" ]]; then
    systemctl enable netzip-rs-full-push.service
    systemctl restart netzip-rs-full-push.service
else
    systemctl disable --now netzip-rs-full-push.service
fi
systemctl is-active --quiet netzip-rs.service
if [[ "${NETZIP_INSTALL_ENABLE_FULL_PUSH:-1}" == "1" ]]; then
    systemctl is-active --quiet netzip-rs-full-push.service
else
    ! systemctl is-active --quiet netzip-rs-full-push.service
fi
test ! -e "$push_timer_target"
curl -fsS --max-time 5 http://127.0.0.1:16893/health >/dev/null
