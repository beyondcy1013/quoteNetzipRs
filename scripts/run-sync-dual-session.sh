#!/usr/bin/env bash
# Synchronized dual-session capture: official 网际风.exe (168) + netzip-driver-hub
# live steady-state clone, same wall-clock window, 5188-only.
#
# Usage (Git Bash on Windows, from quoteNetzipRs/):
#   scripts/run-sync-dual-session.sh <RUN_DIR> [STEADY_SECS]
# Example:
#   scripts/run-sync-dual-session.sh diagnostics/20260907-sync-open 300
#
# Preconditions:
#   - Official 网际风.exe logged in and HOLDING remote :5188 connections
#     (checked; if missing, re-login the official client manually — this
#     script never starts/stops it).
#   - Mon-Fri 09:30-14:00 recommended (168 free account is rejected 14:00-15:00).
#   - pktmon requires an elevated shell.
#
# After the run, byte-parity compare + active-market decode stats:
#   python scripts/extract-official-5188-partition-summary.py RUN_DIR/all-5188.ip.pcap RUN_DIR/clone-2a10-summary.json
#   CARGO_TARGET_DIR=<isolated> cargo build --release -p netzipapi-rust-demo --example official_5188_extract
#   target/release/examples/official_5188_extract RUN_DIR/all-5188.ip.pcap RUN_DIR/extract-active
#   (per-partition entry compare: reuse the 20260904-live-168-evening method,
#    bins vs live streams matched by set overlap; failures bucket by mask class)

set -u

RUN="${1:?usage: run-sync-dual-session.sh <RUN_DIR> [STEADY_SECS]}"
STEADY="${2:-300}"
REPO="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$RUN"

echo "== preflight: official 5188 sessions"
OFFICIAL_PID=$(tasklist //FI "IMAGENAME eq 网际风.exe" //FO CSV 2>/dev/null | tail -n +2 | head -1 | cut -d'"' -f4 || true)
if [ -z "${OFFICIAL_PID}" ]; then
  echo "ABORT: 网际风.exe is not running; start and log it in manually, then rerun."
  exit 1
fi
ESTAB=$(netstat -ano | grep "ESTABLISHED" | grep ":5188 " | grep -c " ${OFFICIAL_PID}\$" || true)
echo "official pid=$OFFICIAL_PID established_5188=${ESTAB:-0}"
if [ "${ESTAB:-0}" -eq 0 ]; then
  echo "ABORT: official has no established :5188 sessions; re-login the official client, then rerun."
  exit 1
fi

echo "== capture start (5188-only)"
pktmon filter remove all >/dev/null 2>&1
pktmon filter add -p 5188 >/dev/null 2>&1
pktmon start --capture --pkt-size 0 -f "$RUN/all-5188.etl" >/dev/null
date +%s%3N > "$RUN/start_ms"

cd "$REPO"
CARGO_TARGET_DIR=target-zcode NETZIP_LIVE_ACCEPTANCE=1 NETZIP_NATIVE_5188_INIT=1 \
NETZIP_LIVE_STEADY_SECS="$STEADY" \
NETZIP_TEST_ACCOUNT="${NETZIP_TEST_ACCOUNT:?env}" NETZIP_TEST_PASSWORD="${NETZIP_TEST_PASSWORD:?env}" \
cargo test -p netzip-driver-hub live_native_driver_steady_state_with_test_account -- --ignored --nocapture 2>&1 | tee "$RUN/steady-state.log"
TEST_RC=${PIPESTATUS[0]}

date +%s%3N > "$RUN/end_ms"
pktmon stop >/dev/null 2>&1
pktmon filter remove all >/dev/null 2>&1
pktmon etl2pcap "$RUN/all-5188.etl" -o "$RUN/all-5188.pcapng" >/dev/null 2>&1
python - "$RUN/all-5188.pcapng" "$RUN/all-5188.ip.pcap" <<'PYEOF'
import struct, sys
src, dst = sys.argv[1], sys.argv[2]
data = open(src, 'rb').read()
off = 0; out = open(dst, 'wb'); out.write(struct.pack('<IHHiIII', 0xa1b2c3d4, 2, 4, 0, 0, 65535, 101))
count = 0
while off + 12 < len(data):
    btype, blen = struct.unpack_from('<II', data, off)
    if blen < 12 or off + blen > len(data):
        break
    if btype == 6:
        caplen = struct.unpack_from('<I', data, off + 20)[0]
        raw = data[off + 28:off + 28 + caplen]
        idx = raw.find(b'\x45\x00', 0, 20)
        if idx >= 0 and len(raw) - idx >= 20:
            ip = raw[idx:]
            ihl = (ip[0] & 0xf) * 4
            tl = struct.unpack_from('>H', ip, 2)[0]
            if tl <= len(ip):
                out.write(struct.pack('<IIII', 0, 0, tl, tl)); out.write(ip[:tl]); count += 1
    off += blen
out.close()
print(f"rewrote {count} raw-IPv4 packets -> {dst}")
PYEOF

echo "== done (steady-state test rc=$TEST_RC)"
echo "== 2a10 entry-level compare (clone uplink vs official 5188 streams in same capture)"
if [ -n "${OFFICIAL_EXTRACT_DIR:-}" ] && [ -d "$OFFICIAL_EXTRACT_DIR" ]; then
  python scripts/compare-2a10-sync.py "$RUN/all-5188.ip.pcap" "$OFFICIAL_EXTRACT_DIR" "$RUN/2a10-sync-verdict.json" || true
else
  echo "OFFICIAL_EXTRACT_DIR not set; extract official 2a10 bins from the official-only capture first, then:"
  echo "  python scripts/compare-2a10-sync.py $RUN/all-5188.ip.pcap <OFFICIAL_EXTRACT_DIR> $RUN/2a10-sync-verdict.json"
fi
echo "== active-market 2704 stats"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}" cargo build --release -p netzipapi-rust-demo --example official_5188_extract >/dev/null 2>&1 \
  && "${CARGO_TARGET_DIR:-target}/release/examples/official_5188_extract" "$RUN/all-5188.ip.pcap" "$RUN/extract-active" \
  && python scripts/2704-capture-stats.py "$RUN/all-5188.ip.pcap" "$RUN/extract-active" "$RUN/2704-active-stats.json" "$RUN/steady-state.log" \
  || echo "2704 stats skipped (build extractor first: official_5188_extract example)"
echo "== semantic 2a10 compare (optional; requires OFFICIAL_EXTRACT_DIR and CLONE_EXTRACT_DIR)"
if [ -n "${OFFICIAL_EXTRACT_DIR:-}" ] && [ -d "$OFFICIAL_EXTRACT_DIR" ] && [ -n "${CLONE_EXTRACT_DIR:-}" ] && [ -d "$CLONE_EXTRACT_DIR" ]; then
  python scripts/compare-2a10-sync.py "$OFFICIAL_EXTRACT_DIR" "$CLONE_EXTRACT_DIR" "$RUN/2a10-semantic-verdict.json" "$OFFICIAL_EXTRACT_DIR" "$CLONE_EXTRACT_DIR" || true
else
  echo "set OFFICIAL_EXTRACT_DIR and CLONE_EXTRACT_DIR to emit raw+0104-code semantic parity"
fi
