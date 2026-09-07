#!/usr/bin/env bash
set -euo pipefail

systemctl enable --now quote-netzip-rs-full-push.service

if ! systemctl is-active --quiet quote-netzip-rs-full-push.service; then
  echo "quote-netzip-rs-full-push.service failed to become active" >&2
  exit 1
fi

systemctl show quote-netzip-rs-full-push.service \
  -p ActiveState \
  -p SubState \
  -p MainPID \
  -p ExecMainStartTimestamp
