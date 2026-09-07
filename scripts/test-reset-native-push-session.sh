#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

cat >"$tmp_dir/systemctl" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$RESET_TEST_LOG"
EOF
chmod +x "$tmp_dir/systemctl"

RESET_TEST_LOG="$tmp_dir/calls" PATH="$tmp_dir:$PATH" \
  bash "$script_dir/reset-native-push-session.sh"

cat >"$tmp_dir/expected" <<'EOF'
stop quote-netzip-rs-full-push.service
restart quote-netzip-rs-supplement.service
start quote-netzip-rs-full-push.service
is-active --quiet quote-netzip-rs-supplement.service
is-active --quiet quote-netzip-rs-full-push.service
EOF
cmp "$tmp_dir/expected" "$tmp_dir/calls"
