#!/usr/bin/env python3
"""Read-only comparison of ladder failure traces against clean traces."""
import json, os, re, sys

root = sys.argv[1] if len(sys.argv) > 1 else r"Z:/stock/quoteNetzipRs/diagnostics/20260907-close-window-144507"
report = os.path.join(root, "ladder-volume-e8-03-2d-compare.json")
out = os.path.join(root, "ladder-bit-span-diff-analysis.json")
data = json.load(open(report, encoding="utf-8"))
extract = os.path.join(root, "extract")

def trace(name):
    if not name:
        return None
    path = os.path.join(extract, name)
    if not os.path.exists(path):
        return None
    with open(path, encoding="utf-8") as f:
        return json.load(f)

def key(r):
    return (r.get("mask_class"), r.get("header"), r.get("ladder_layout"), r.get("volume_mask"))

clean = []
missing_clean = []
for s in data.get("successful_trace_samples", []):
    rows = trace(s.get("trace"))
    if rows is None:
        missing_clean.append(s.get("trace"))
        continue
    r = next((x for x in rows if x.get("record_index") == s["record_index"]), None)
    if r:
        clean.append({"source": s, "record": r})
clean_by_key = {}
for item in clean:
    clean_by_key.setdefault(key(item["record"]), []).append(item)

results = []
missing_failed = []
for s in data.get("failed_samples", []):
    rows = trace(s.get("trace"))
    if rows is None:
        missing_failed.append(s.get("trace"))
        continue
    r = next((x for x in rows if f"value record {x.get('record_index')}" in s.get("error", "") and x.get("stage") == "ladder_volumes"), None)
    if r is None:
        m = re.search(r"value record (\d+)", s.get("error", ""))
        if m:
            ordinal = int(m.group(1))
            r = next((x for x in rows if x.get("record_index") == ordinal - 1), None)
    if r is None:
        continue
    peers = clean_by_key.get(key(r), [])
    results.append({
        "failed_trace": s["trace"], "failed_record_index": r.get("record_index"),
        "failed": {k: r.get(k) for k in ("mask_class","header","ladder_layout","volume_mask","baseline_mode","anchor","record_start","header_end","timestamp_end","prefix_values_end","ladder_move_bits","ladder_values_bits","ladder_volumes_end","ladder_anchor_value","stage")},
        "clean_peer_count": len(peers),
        "clean_peers": [{"trace": p["source"]["trace"], "record_index": p["record"].get("record_index"), "spans": {k:p["record"].get(k) for k in ("baseline_mode","record_start","header_end","timestamp_end","prefix_values_end","ladder_move_bits","ladder_values_bits","ladder_volumes_end","ladder_anchor_value","stage")}} for p in peers[:5]],
        "error": s.get("error"),
    })

summary = {"schema":"netzip.2704-ladder-bit-span-diff.v1", "policy":"read-only diagnostic; no decoder/token/mask/state changes", "failed_samples_analyzed":len(results), "same_key_clean_peer_samples":sum(bool(x["clean_peer_count"]) for x in results), "missing_clean_traces":sorted(set(x for x in missing_clean if x)), "missing_failed_traces":sorted(set(x for x in missing_failed if x)), "results":results}
json.dump(summary, open(out,"w",encoding="utf-8"), ensure_ascii=False, indent=2)
print(json.dumps({k:summary[k] for k in summary if k != "results"}, ensure_ascii=False))
print(out)
