#!/usr/bin/env bash
set -euo pipefail

systemctl stop quote-netzip-rs-full-push.service
systemctl restart quote-netzip-rs-supplement.service
systemctl start quote-netzip-rs-full-push.service

systemctl is-active --quiet quote-netzip-rs-supplement.service
systemctl is-active --quiet quote-netzip-rs-full-push.service
