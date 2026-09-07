#!/usr/bin/env python3
"""Coverage of the 19:36 leftover×live special case vs always max-sequence.

Mirrors examples/official_5188_callback_parity.rs business-ts:
same second; if exactly 2 quotes, one price==0, one !=0, identical last_close
bits → pick nonzero max sequence; else wall-clock nearest.
Does not edit owned files.

usage:
  score-join-1936-special-case.py EXTRACT CALLBACK META_0903
"""
from __future__ import annotations

import json
import os
import sys
from collections import Counter, defaultdict

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


def pick_nearest(cands: list[dict], completed: int) -> dict:
    return min(
        cands,
        key=lambda q: (abs(q["ts_ms"] * 1000 - completed), q["ts_ms"], q["seq"]),
    )


def pick_max_seq(cands: list[dict]) -> dict:
    return max(cands, key=lambda q: (q["seq"], q["ts_ms"]))


def leftover_live_shape(cands: list[dict]) -> bool:
    if len(cands) != 2:
        return False
    zeros = sum(1 for q in cands if q["price"] == 0.0)
    if zeros != 1:
        return False
    bits = {seed.f32_bits(q["last_close"]) for q in cands}
    return len(bits) == 1


def pick_1936(cands: list[dict], completed: int) -> dict:
    if leftover_live_shape(cands):
        live = [q for q in cands if q["price"] != 0.0]
        return pick_max_seq(live)
    return pick_nearest(cands, completed)


def shape_name(cands: list[dict]) -> str:
    n = len(cands)
    if n == 1:
        return "unique"
    zeros = sum(1 for q in cands if q["price"] == 0.0)
    lives = n - zeros
    if n == 2 and zeros == 1 and lives == 1:
        bits = {seed.f32_bits(q["last_close"]) for q in cands}
        return "leftover_x_live_same_last" if len(bits) == 1 else "leftover_x_live_diff_last"
    if n == 2 and zeros == 2:
        return "leftover_x_leftover"
    if n == 2 and lives == 2:
        return "live_x_live"
    return f"other_n{n}_{zeros}zero_{lives}live"


def qid(q: dict) -> tuple:
    return (q["seq"], q["ts_ms"], seed.f32_bits(q["price"]))


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: score-join-1936-special-case.py EXTRACT CALLBACK META_0903",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903 = sys.argv[1:]
    metadata = seed.load_index_metadata(meta_0903)
    rows, _ = seed.load_decoded(extract_dir)
    cb_by = defaultdict(list)
    for q in ident.stream_callback_quotes(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)

    shapes = Counter()
    disagree = Counter()
    leftover_picked = Counter()
    samples = []
    n = 0
    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None:
            continue
        cands = cb_by.get((row["market"], meta["code"], row["ts"]), [])
        if not cands:
            continue
        n += 1
        shape = shape_name(cands)
        shapes[shape] += 1
        nearest = pick_nearest(cands, int(row["completed"]))
        max_seq = pick_max_seq(cands)
        rust = pick_1936(cands, int(row["completed"]))
        if qid(rust) != qid(max_seq):
            disagree["rust1936_vs_max_seq"] += 1
            disagree[f"rust1936_vs_max_seq__{shape}"] += 1
        if qid(nearest) != qid(max_seq):
            disagree["nearest_vs_max_seq"] += 1
            disagree[f"nearest_vs_max_seq__{shape}"] += 1
        if qid(rust) != qid(nearest):
            disagree["rust1936_vs_nearest"] += 1
            disagree[f"rust1936_vs_nearest__{shape}"] += 1
        leftover_picked["nearest_leftover"] += int(nearest["price"] == 0.0)
        leftover_picked["max_seq_leftover"] += int(max_seq["price"] == 0.0)
        leftover_picked["rust1936_leftover"] += int(rust["price"] == 0.0)
        if (
            shape != "leftover_x_live_same_last"
            and qid(nearest) != qid(max_seq)
            and len(samples) < 8
        ):
            samples.append(
                {
                    "code": meta["code"],
                    "shape": shape,
                    "n": len(cands),
                    "prices": [q["price"] for q in cands],
                    "seqs": [q["seq"] for q in cands],
                    "nearest_seq": nearest["seq"],
                    "max_seq": max_seq["seq"],
                    "nearest_price": nearest["price"],
                    "max_seq_price": max_seq["price"],
                }
            )

    report = {
        "schema": "quoteNetzipRs.official_5188_join_1936_special_case.v1",
        "extract_dir": extract_dir,
        "joined": n,
        "shapes": dict(shapes),
        "leftover_picks": dict(leftover_picked),
        "disagreements": dict(disagree),
        "samples_non_special_nearest_vs_max_seq": samples,
        "note": (
            "rust1936 is the 19:36 parity-example special case. "
            "Product rule remains: same second + same last_close → max sequence. "
            "Special case only fires on leftover×live size 2."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
