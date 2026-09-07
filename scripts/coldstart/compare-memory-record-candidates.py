#!/usr/bin/env python3
"""Compare bounded 311B candidate scans from two Wine memory snapshots.

Inputs are the JSON files produced by ``scan-internal-records.py``.  The
report is evidence-only: offsets are snapshot-file offsets, never protocol
addresses, and a changed candidate is not treated as a decoder match.
"""
import json
import sys


def region_for(regions, offset):
    for region in regions:
        if region["offset"] <= offset < region["offset"] + region["len"]:
            return region
    raise ValueError(f"snapshot offset {offset} is outside the region index")


def region_counts(records, regions):
    counts = {}
    for record in records.values():
        region = region_for(regions, record["addr_off"])
        key = f"0x{region['start']:08x}"
        item = counts.setdefault(key, {"candidates": 0, "file_offset": region["offset"], "length": region["len"]})
        item["candidates"] += 1
    return dict(sorted(counts.items(), key=lambda item: (-item[1]["candidates"], item[0])))


def main() -> int:
    if len(sys.argv) != 6:
        print("usage: compare-memory-record-candidates.py BEFORE BEFORE_INDEX AFTER AFTER_INDEX OUT", file=sys.stderr)
        return 2
    before = json.load(open(sys.argv[1]))
    before_regions = json.load(open(sys.argv[2]))
    after = json.load(open(sys.argv[3]))
    after_regions = json.load(open(sys.argv[4]))
    common = sorted(before.keys() & after.keys())
    changed = [key for key in common if before[key]["raw"] != after[key]["raw"]]
    fields = (
        "ts", "open", "high", "low", "close", "volume", "amount",
        "last_close", "ladder_prices", "ladder_volumes",
    )
    field_changes = {field: sum(before[key][field] != after[key][field] for key in common) for field in fields}
    out = {
        "evidence_only": True,
        "candidate_record_length": 0x137,
        "before_unique_keys": len(before),
        "after_unique_keys": len(after),
        "common_keys": len(common),
        "changed_common_keys": len(changed),
        "before_only_keys": len(before.keys() - after.keys()),
        "after_only_keys": len(after.keys() - before.keys()),
        "stable_snapshot_offsets": sum(before[key]["addr_off"] == after[key]["addr_off"] for key in common),
        "field_change_counts": field_changes,
        "before_region_candidates": region_counts(before, before_regions),
        "after_region_candidates": region_counts(after, after_regions),
        "limitations": [
            "scan offsets are offsets within the snapshot file, not stable virtual addresses",
            "heuristic candidates are not proof of 311B slot identity",
            "no payload-to-candidate same-record join is asserted",
        ],
    }
    with open(sys.argv[5], "w") as handle:
        json.dump(out, handle, indent=2, sort_keys=True)
        handle.write("\n")
    print(json.dumps({k: out[k] for k in ("before_unique_keys", "after_unique_keys", "common_keys", "changed_common_keys")}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
