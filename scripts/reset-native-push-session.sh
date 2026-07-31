#!/usr/bin/env bash
set -euo pipefail

systemctl stop netzip-rs-full-push.service
systemctl restart netzip-rs.service
systemctl start netzip-rs-full-push.service

systemctl is-active --quiet netzip-rs.service
systemctl is-active --quiet netzip-rs-full-push.service
