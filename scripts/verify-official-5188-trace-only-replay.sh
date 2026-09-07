#!/usr/bin/env bash
# Trace-only offline replay gates. Does not deploy, restart, or enable publication.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
EXTRACT="$ROOT/target/release/examples/official_5188_extract"

summarize_2704() {
  python3 - "$1" <<'PY'
import json, collections, sys
manifest = json.load(open(sys.argv[1]))
frames = [row for row in manifest if row.get("wire_kind") == "2704"]
stages = collections.Counter()
clean = 0
partial = 0
failed = 0
omitted_frames = 0
omitted_records = 0
residue5 = 0
missing_fresh = 0
for row in frames:
    error = row.get("decoded_value_error")
    traces = bool(row.get("decoded_value_trace_file"))
    omitted = int(row.get("omitted_tail_records") or 0)
    if omitted:
        omitted_frames += 1
        omitted_records += omitted
    if row.get("decoded_value_residue_bit_offset") == 5 and error:
        residue5 += 1
    if error is None:
        clean += 1
    elif traces:
        partial += 1
    else:
        failed += 1
    if error:
        text = str(error)
        if "baseline_mode=missing_fresh" in text:
            missing_fresh += 1
        if "stage=" in text:
            stages[text.split("stage=", 1)[1].split()[0]] += 1
print(json.dumps({
    "frames": len(frames),
    "clean": clean,
    "partial": partial,
    "failed": failed,
    "error": partial + failed,
    "omitted_frames": omitted_frames,
    "omitted_records": omitted_records,
    "residue_offset_5_failures": residue5,
    "missing_fresh": missing_fresh,
    "ladder_volumes": stages.get("ladder_volumes", 0),
    "accumulators": stages.get("accumulators", 0),
    "ohlc": stages.get("ohlc", 0),
    "ladder_values": stages.get("ladder_values", 0),
    "stages": dict(stages),
}, sort_keys=True))
PY
}

assert_json() {
  python3 - "$1" "$2" <<'PY'
import json, sys
got = json.loads(sys.argv[1])
want = json.loads(sys.argv[2])
missing = {key: (got.get(key), value) for key, value in want.items() if got.get(key) != value}
if missing:
    raise SystemExit(f"replay mismatch: {missing}")
print("ok", want)
PY
}

echo "== full-init replay =="
rm -rf diagnostics/20260907-shadow-deploy/extract-full-init-trace-v1
"$EXTRACT" \
  captures/20260907-shadow-deploy/official-5188-restart-full-init.pcap \
  diagnostics/20260907-shadow-deploy/extract-full-init-trace-v1 \
  - \
  diagnostics/20260907-shadow-deploy/extract-full-init-lifecycle-v2 \
  222.85.139.177:5188 \
  strict
FULL_INIT="$(summarize_2704 diagnostics/20260907-shadow-deploy/extract-full-init-trace-v1/manifest.json)"
echo "$FULL_INIT"
assert_json "$FULL_INIT" '{"frames":26722,"clean":22776,"error":3946,"omitted_frames":665,"omitted_records":1399,"ladder_volumes":1518}'

echo "== close-window official pcap replay =="
rm -rf diagnostics/20260907-close-window-144507/extract
"$EXTRACT" \
  captures/20260907-close-window-144507/official-5188-10m.pcap \
  diagnostics/20260907-close-window-144507/extract \
  - \
  diagnostics/20260907-shadow-deploy/extract-full-init-lifecycle-v2 \
  222.85.139.177:5188 \
  strict
CLOSE="$(summarize_2704 diagnostics/20260907-close-window-144507/extract/manifest.json)"
echo "$CLOSE"
assert_json "$CLOSE" '{"frames":101765,"clean":82149,"error":19616,"omitted_frames":4363,"omitted_records":9392,"ladder_volumes":7030,"accumulators":3191,"ohlc":2739,"ladder_values":2602}'

echo "== Monday 168 clone fixture replay =="
rm -rf diagnostics/20260907-sync-open/extract-strict-trace-v1
"$EXTRACT" \
  diagnostics/20260907-sync-open/sync-run2-5188.pcap \
  diagnostics/20260907-sync-open/extract-strict-trace-v1 \
  - \
  diagnostics/20260907-sync-open/extract-wt-clone \
  192.168.3.2:51470 \
  strict
CLONE="$(summarize_2704 diagnostics/20260907-sync-open/extract-strict-trace-v1/manifest.json)"
echo "$CLONE"
assert_json "$CLONE" '{"frames":184,"clean":149,"error":35}'

echo "== ladder-volume trace diff =="
python3 scripts/analyze-official-5188-ladder-volume-trace-diff.py \
  diagnostics/20260907-close-window-144507/extract \
  diagnostics/20260907-close-window-144507/ladder-volume-trace-diff-v1.json \
  --full-init-extract diagnostics/20260907-shadow-deploy/extract-full-init-trace-v1 \
  --max-samples 3
sha256sum \
  diagnostics/20260907-close-window-144507/ladder-volume-trace-diff-v1.json \
  captures/20260907-close-window-144507/official-5188-10m.pcap
echo "publication=disabled"
