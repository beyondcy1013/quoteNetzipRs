#!/usr/bin/env python3
"""Among leftover OEM joins, is the 2704 close already nonzero?

Product first-print should follow same-session 2704, not leftover callbacks.
This scores incoming and merged close on the auction leftover/live split.

usage:
  classify-leftover-2704-close.py EXTRACT CALLBACK META_0903
"""
from __future__ import annotations

import json
import os
import sys
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone

_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _DIR)
from importlib.machinery import SourceFileLoader

seed = SourceFileLoader(
    "seed_last_close_and_preopen",
    os.path.join(_DIR, "seed-last-close-and-preopen.py"),
).load_module()
ident = SourceFileLoader(
    "disambiguate_586_batch_identity",
    os.path.join(_DIR, "disambiguate-586-batch-identity.py"),
).load_module()

TZ8 = timezone(timedelta(hours=8))
GATE = int(datetime(2026, 9, 4, 9, 25, 0, tzinfo=TZ8).timestamp())


def pick_later(cands: list[dict]) -> dict:
    return max(cands, key=lambda q: (q["seq"], q["ts_ms"]))


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: classify-leftover-2704-close.py EXTRACT CALLBACK META_0903",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903 = sys.argv[1:]
    metadata = seed.load_index_metadata(meta_0903)
    morning = seed.load_largest_0104(
        "diagnostics/20260904-live-rust/ten-slot-0909/extract-strict"
    )
    rows, _ = seed.load_decoded(extract_dir)
    rows = sorted(rows, key=lambda r: r["order"])

    cb_by = defaultdict(list)
    for q in ident.stream_callback_quotes(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)

    states: dict[tuple[str, str], seed.Public] = {}
    hits = Counter()
    samples = []
    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None:
            continue
        code_key = (row["market"], meta["code"])
        morn = morning.get(code_key)
        last_i32 = morn["last_0104"] if morn else 0
        state = states.get(code_key)
        if state is None:
            state = seed.Public(last_i32)
            states[code_key] = state
        incoming = seed.i32(row["rec"], 0x10)
        seed.merge(state, row)
        cands = cb_by.get((row["market"], meta["code"], row["ts"]), [])
        if not cands:
            continue
        hits["joined"] += 1
        picked = pick_later(cands)
        live = picked["price"] != 0
        side = "live" if live else "leftover"
        when = "before_0925" if row["ts"] < GATE else "after_0925"
        hits[side] += 1
        hits[f"{side}_{when}"] += 1
        if incoming == 0:
            hits[f"{side}_in0"] += 1
        else:
            hits[f"{side}_in_nz"] += 1
        if state.close == 0:
            hits[f"{side}_mg0"] += 1
        else:
            hits[f"{side}_mg_nz"] += 1
        if (not live) and incoming != 0 and len(samples) < 6:
            samples.append(
                {
                    "code": meta["code"],
                    "when": when,
                    "incoming_close": incoming,
                    "merged_close": state.close,
                    "cb_price": picked["price"],
                }
            )

    report = {
        "schema": "quoteNetzipRs.official_5188_leftover_2704_close.v1",
        "extract_dir": extract_dir,
        "joined": hits["joined"],
        "counts": dict(hits),
        "samples_leftover_oem_but_2704_nonzero": samples,
        "note": (
            "If leftover OEM seconds already have a nonzero 2704 close, do not "
            "treat leftover callbacks as the first-print gate. Publish from "
            "same-session 2704; leftover OEM is not the OemState target."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
