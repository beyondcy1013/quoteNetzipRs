#!/usr/bin/env python3
"""Morning login 0104 last vs night 311B +0x10 close.

The 09:15 0104 last already matches auction OEM last_close 6183/6183.
Night dump close hits 4914/4928. This checks whether morning 0104 is the
same integer as night close, or the official settlement that repairs the
14 last-trade residuals.

Does not touch official_5188.rs.
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


def load_all_0104_tables(directory: str) -> dict[tuple[str, str], list[dict]]:
    by_code: dict[tuple[str, str], list[dict]] = defaultdict(list)
    for name in sorted(os.listdir(directory)):
        if not name.endswith("0104.code-table.json"):
            continue
        table = json.loads(open(os.path.join(directory, name), "rb").read())
        market = bytes(table["market"]).decode()
        for rec in table["records"]:
            code = rec.get("code") or ""
            if not code:
                continue
            tail = bytes(rec["opaque_tail"])
            places = tail[0] if tail else 0
            last = seed.i32(tail, 11) if len(tail) >= 15 else 0
            by_code[(market, code)].append(
                {
                    "file": name,
                    "index": rec["symbol_index"],
                    "places": places,
                    "last": last,
                }
            )
    return by_code


def stability(rows: list[dict]) -> dict:
    lasts = {row["last"] for row in rows}
    indexes = {row["index"] for row in rows}
    return {
        "tables": len(rows),
        "unique_last": len(lasts),
        "unique_index": len(indexes),
        "stable_last": len(lasts) == 1,
        "stable_index": len(indexes) == 1,
    }


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: morning-0104-vs-night-close.py MORNING_EXTRACT CALLBACK NIGHT_0104 PROBE",
            file=sys.stderr,
        )
        return 2
    morning_dir, callback_jsonl, night_0104_dir, probe_json = sys.argv[1:]
    morning_tables = load_all_0104_tables(morning_dir)
    morning = seed.load_largest_0104(morning_dir)
    night_0104 = seed.load_largest_0104(night_0104_dir)
    probe = seed.load_probe_by_code(probe_json)

    lasts = {}
    with open(callback_jsonl, "rb") as fh:
        seen = set()
        for line in fh:
            ev = json.loads(line)
            seq = ev.get("sequence")
            if seq in seen:
                continue
            seen.add(seq)
            batch = ev.get("quote_batch") or {}
            if batch.get("schema") != "quoteNetzipWine.quote_batch.v1":
                continue
            for q in batch.get("quotes") or []:
                key = (q["market"], q["code"])
                lc = float(q.get("last_close") or 0)
                if lc:
                    lasts.setdefault(key, Counter())[lc] += 1
    mode = {k: v.most_common(1)[0][0] for k, v in lasts.items()}

    stable_last = 0
    unstable_last = 0
    unstable_samples = []
    for key, rows in morning_tables.items():
        st = stability(rows)
        if st["stable_last"]:
            stable_last += 1
        else:
            unstable_last += 1
            if len(unstable_samples) < 8:
                unstable_samples.append(
                    {
                        "market": key[0],
                        "code": key[1],
                        "lasts": [row["last"] for row in rows],
                        "files": [row["file"] for row in rows],
                    }
                )

    hits = Counter()
    miss_night_hit_morn = []
    miss_both = []
    equal_int = 0
    unequal_int = []
    extra_morn_only = Counter()
    for key, cb in mode.items():
        hits["callback"] += 1
        morn = morning.get(key)
        night = probe.get(key)
        if morn and morn["scale"] > 0:
            hits["morn_present"] += 1
            proj = seed.rust_scaled(morn["last_0104"], morn["scale"])
            if seed.same_f32(proj, cb):
                hits["morn_hit"] += 1
        else:
            extra_morn_only["missing"] += 1
        if night is not None:
            hits["night_present"] += 1
            scale = (morn or night_0104.get(key) or {}).get("scale") or 0
            if scale <= 0:
                hits["night_no_scale"] += 1
                continue
            night_close = seed.rust_scaled(int(night["close"]), scale)
            night_12b = seed.rust_scaled(int(night["last_close"]), scale)
            if seed.same_f32(night_close, cb):
                hits["night_close_hit"] += 1
            if seed.same_f32(night_12b, cb):
                hits["night_12b_hit"] += 1
            if morn:
                if int(night["close"]) == morn["last_0104"]:
                    equal_int += 1
                else:
                    unequal_int.append(
                        {
                            "market": key[0],
                            "code": key[1],
                            "night_close": int(night["close"]),
                            "morn_last": morn["last_0104"],
                            "night_12b": int(night["last_close"]),
                            "cb": cb,
                            "scale": scale,
                            "morn_hit": seed.same_f32(
                                seed.rust_scaled(morn["last_0104"], morn["scale"]), cb
                            ),
                            "night_close_hit": seed.same_f32(night_close, cb),
                        }
                    )
            if not seed.same_f32(night_close, cb):
                row = {
                    "market": key[0],
                    "code": key[1],
                    "night_close": int(night["close"]),
                    "night_12b": int(night["last_close"]),
                    "morn_last": None if morn is None else morn["last_0104"],
                    "cb": cb,
                    "morn_hit": False
                    if morn is None
                    else seed.same_f32(
                        seed.rust_scaled(morn["last_0104"], morn["scale"]), cb
                    ),
                }
                if row["morn_hit"]:
                    miss_night_hit_morn.append(row)
                else:
                    miss_both.append(row)
        else:
            extra_morn_only[key[0]] += 1
            if morn and morn["scale"] > 0 and seed.same_f32(
                seed.rust_scaled(morn["last_0104"], morn["scale"]), cb
            ):
                hits["callback_without_night_dump_morn_hit"] += 1

    sh603059 = {
        "morning": morning.get(("SH", "603059")),
        "night_0104": night_0104.get(("SH", "603059")),
        "probe": None
        if ("SH", "603059") not in probe
        else {
            "close": probe[("SH", "603059")]["close"],
            "last_close": probe[("SH", "603059")]["last_close"],
            "code": probe[("SH", "603059")].get("code"),
        },
        "callback": mode.get(("SH", "603059")),
    }

    report = {
        "schema": "quoteNetzipRs.official_5188_morning_0104_vs_night_close.v1",
        "morning_dir": morning_dir,
        "callback_codes": hits["callback"],
        "sh603059": sh603059,
        "login_table_stability": {
            "codes": len(morning_tables),
            "stable_last": stable_last,
            "unstable_last": unstable_last,
            "unstable_samples": unstable_samples,
        },
        "vs_callback": {
            "morning_0104": {
                "present": hits["morn_present"],
                "hits": hits["morn_hit"],
                "rate": round(hits["morn_hit"] / hits["morn_present"], 6)
                if hits["morn_present"]
                else None,
            },
            "night_close_+0x10": {
                "present": hits["night_present"],
                "hits": hits["night_close_hit"],
                "rate": round(hits["night_close_hit"] / hits["night_present"], 6)
                if hits["night_present"]
                else None,
            },
            "night_0x12b": {
                "present": hits["night_present"],
                "hits": hits["night_12b_hit"],
            },
        },
        "morning_vs_night_close_integer": {
            "equal": equal_int,
            "unequal": len(unequal_int),
            "unequal_all": unequal_int,
        },
        "night_close_miss_morning_hit": {
            "n": len(miss_night_hit_morn),
            "rows": miss_night_hit_morn,
        },
        "night_close_and_morning_both_miss": {
            "n": len(miss_both),
            "rows": miss_both[:12],
        },
        "callback_without_night_dump": {
            "by_market": {
                k: v for k, v in extra_morn_only.items() if k != "missing"
            },
            "morning_still_hits": hits["callback_without_night_dump_morn_hit"],
        },
        "note": (
            "If morning 0104 last != night +0x10 on the 14 residuals, next-day "
            "OEM last_close is official 0104 settlement, not last trade."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
