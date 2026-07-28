#!/usr/bin/env bash
set -euo pipefail

systemctl enable --now netzip-rs-full-push.service

if ! systemctl is-active --quiet netzip-rs-full-push.service; then
  echo "netzip-rs-full-push.service failed to become active" >&2
  exit 1
fi

systemctl show netzip-rs-full-push.service \
  -p ActiveState \
  -p SubState \
  -p MainPID \
  -p ExecMainStartTimestamp
