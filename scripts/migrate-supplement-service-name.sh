#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
new_unit="quote-netzip-rs-supplement.service"
old_unit="quote-netzip-rs.service"
push_unit="quote-netzip-rs-full-push.service"

test -x /home/bin/netzip/quoteNetzipRs
install -m 0644 "$project_dir/deploy/$new_unit" "/etc/systemd/system/$new_unit"
install -m 0644 "$project_dir/deploy/$push_unit" "/etc/systemd/system/$push_unit"

systemctl stop "$push_unit" 2>/dev/null || true
systemctl disable --now "$old_unit" 2>/dev/null || true
rm -f "/etc/systemd/system/$old_unit"
systemctl daemon-reload
systemctl enable --now "$new_unit"
systemctl enable --now "$push_unit"

systemctl is-active --quiet "$new_unit"
systemctl is-active --quiet "$push_unit"
! systemctl list-unit-files --no-legend "$old_unit" 2>/dev/null | grep -Fq "$old_unit"
curl -fsS --max-time 5 http://127.0.0.1:16893/health >/dev/null
