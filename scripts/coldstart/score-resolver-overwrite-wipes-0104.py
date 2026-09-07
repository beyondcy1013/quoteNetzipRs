#!/usr/bin/env python3
"""2704 full-record overwrite vs 0104 metadata that project() still needs.

from_code_table_metadata writes code at 0xe5, amount_mode at 0x11f, and
opaque_tail at 0x120. resolver.update_from_decoded replaces the whole 311B
record. Name is never in the 311-byte value record.

Does not edit official_5188.rs.

usage:
  score-resolver-overwrite-wipes-0104.py NIGHT_PROBE NIGHT_0104 EXTRACT META_0903
"""
from __future__ import annotations

import json
import os
import sys
from collections import Counter

_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _DIR)
from importlib.machinery import SourceFileLoader

seed = SourceFileLoader(
    "seed_last_close_and_preopen",
    os.path.join(_DIR, "seed-last-close-and-preopen.py"),
).load_module()
proj = SourceFileLoader(
    "score_project_needs_0104_row",
    os.path.join(_DIR, "score-project-needs-0104-row.py"),
).load_module()


def rec_code(rec: bytes) -> str:
    raw = rec[0xE5:0xEB]
    if len(raw) == 6 and all(48 <= b <= 57 for b in raw):
        return raw.decode("ascii")
    return ""


def rec_amount_mode(rec: bytes) -> int:
    return rec[0x11F]


def rec_places(rec: bytes) -> int:
    return rec[0x120]


def rec_last(rec: bytes) -> int:
    return seed.i32(rec, 0x12B)


def probe_bytes(row: dict) -> bytes | None:
    raw_hex = row.get("record_hex") or ""
    if len(raw_hex) < 0x137 * 2:
        return None
    return bytes.fromhex(raw_hex[: 0x137 * 2])


def load_probe(path: str) -> dict[tuple[str, str], dict]:
    best: dict[tuple[str, str], dict] = {}
    for row in json.loads(open(path, "rb").read()):
        code = row.get("code") or ""
        market = row.get("market") or ""
        if not code:
            continue
        key = (market, code)
        prev = best.get(key)
        if prev is None or int(row.get("frame_index") or 0) >= int(
            prev.get("frame_index") or 0
        ):
            best[key] = row
    return best


def score_records(label: str, items: list[tuple[bytes, dict, str]]) -> dict:
    """items: (rec, 0104_row, identity_code)"""
    hits = Counter()
    samples = []
    for rec, table, ident in items:
        hits["n"] += 1
        code = rec_code(rec)
        if not code:
            hits["code_empty"] += 1
        elif code == ident:
            hits["code_matches_identity"] += 1
        else:
            hits["code_wrong"] += 1
            if len(samples) < 6:
                samples.append(
                    {"ident": ident, "rec_code": code, "index": table.get("index")}
                )
        if rec_amount_mode(rec) == int(table.get("amount_mode") or 0):
            hits["amount_mode_eq_0104"] += 1
        if rec_places(rec) == int(table.get("places") or 0):
            hits["places_eq_0104"] += 1
        if rec_last(rec) == int(table.get("last_0104") or 0):
            hits["last_eq_0104"] += 1
        if rec[0x120:0x137] == bytes(table.get("tail") or [])[:23]:
            hits["full_tail_eq_0104"] += 1
    n = hits["n"] or 1
    return {
        "label": label,
        "n": hits["n"],
        "code_empty": hits["code_empty"],
        "code_matches_identity": hits["code_matches_identity"],
        "code_wrong": hits["code_wrong"],
        "amount_mode_eq_0104": hits["amount_mode_eq_0104"],
        "places_eq_0104": hits["places_eq_0104"],
        "last_eq_0104": hits["last_eq_0104"],
        "full_tail_eq_0104": hits["full_tail_eq_0104"],
        "rates": {
            "code_present": round(hits["code_matches_identity"] / n, 6),
            "places": round(hits["places_eq_0104"] / n, 6),
            "last": round(hits["last_eq_0104"] / n, 6),
        },
        "samples_code_wrong": samples,
    }


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-resolver-overwrite-wipes-0104.py NIGHT_PROBE NIGHT_0104 "
            "EXTRACT META_0903",
            file=sys.stderr,
        )
        return 2
    night_probe, night_0104, extract_dir, meta_0903 = sys.argv[1:]
    night_table = proj.load_largest_0104_full(night_0104)
    # load_largest_0104_full doesn't keep amount_mode/tail; use seed loader too
    night_seed = seed.load_largest_0104(night_0104)
    for key, row in night_table.items():
        extra = night_seed.get(key) or {}
        row["amount_mode"] = extra.get("amount_mode")
        row["tail"] = extra.get("tail")
        row["last_0104"] = extra.get("last_0104", row.get("last_0104"))
        row["places"] = extra.get("places", row.get("places"))

    probe = load_probe(night_probe)
    night_items = []
    star_prefix_vs_mode = Counter()
    for key, row in probe.items():
        table = night_table.get(key)
        rec = probe_bytes(row)
        if table is None or rec is None:
            continue
        night_items.append((rec, table, key[1]))
        if key[1].startswith(("688", "689")):
            star_prefix_vs_mode[str(table.get("amount_mode"))] += 1

    meta = seed.load_index_metadata(meta_0903)
    rows, _ = seed.load_decoded(extract_dir)
    auction_items = []
    auction_code_vs_0903 = Counter()
    seen_idx = set()
    last_by_idx: dict[tuple[str, int], bytes] = {}
    for row in rows:
        last_by_idx[(row["market"], row["index"])] = row["rec"]
    for key_idx, rec in last_by_idx.items():
        table = meta.get(key_idx)
        if table is None:
            continue
        ident = table["code"]
        auction_items.append((rec, table, ident))
        code = rec_code(rec)
        if not code:
            auction_code_vs_0903["empty"] += 1
        elif code == ident:
            auction_code_vs_0903["eq_0903"] += 1
        else:
            auction_code_vs_0903["ne_0903"] += 1
        seen_idx.add(key_idx)

    report = {
        "schema": "quoteNetzipRs.official_5188_resolver_overwrite_wipes_0104.v1",
        "crate_facts": {
            "name_not_in_311B": True,
            "from_code_table_metadata_writes_code_amount_mode_tail": True,
            "update_from_decoded_replaces_whole_record": True,
            "OemState.merge_does_not_copy_code_or_tail": True,
            "to_public_quote_requires_caller_code_name_scale": True,
            "star_lots_uses_code_prefix_argument_not_amount_mode": True,
        },
        "night_same_session_probe_311B": score_records("night", night_items),
        "auction_last_2704_per_index": score_records("auction", auction_items),
        "auction_code_field": dict(auction_code_vs_0903),
        "star_688_689_0104_amount_mode": dict(star_prefix_vs_mode),
        "unique_auction_indexes_scored": len(seen_idx),
        "product_wiring": [
            "Keep a side map of the same-session 0104 row: code, name, price_scale_hint, previous_close i32, amount_mode.",
            "Do not recover name from OemState.record or resolver.record — it is never stored in 311B.",
            "Do not recover scale from resolver after 2704 unless the incoming record preserved 0x120.",
            "STAR lots in to_public_quote use the code argument prefix 688/689, so a wrong code also wrong-lots.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
