#!/usr/bin/env bash
set -euo pipefail

systemctl disable --now quote-netzip-rs-full-push.service
systemctl disable --now quote-netzip-rs-full-push.timer 2>/dev/null || true

if systemctl is-active --quiet quote-netzip-rs-full-push.service; then
  echo "quote-netzip-rs-full-push.service is still active" >&2
  exit 1
fi

echo "NetzipRs native Rust full push paused: persistent service is inactive"
