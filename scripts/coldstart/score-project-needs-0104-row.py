#!/usr/bin/env python3
"""product 311B→OEM: project() needs the same-session 0104 row, not just i32.

Runtime 19:48 stores only (market, index) -> previous_close i32 and never
calls OemState.project. merge() copies OHLC/book but not 0x12b.
to_public_quote requires six ASCII digits, a name, and price_scale from
the matched 0104 row (price_scale_hint = 10 ** opaque_tail[0]).

This scores four last_close publishers against Wine OEM without editing
official_5188.rs / runtime / callback_parity.

usage:
  score-project-needs-0104-row.py EXTRACT CALLBACK META_0903 MORNING_0104 \\
    NIGHT_CALLBACK NIGHT_0104
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


def six_digit(code: str) -> bool:
    return len(code) == 6 and code.isdigit()


def load_largest_0104_full(directory: str) -> dict[tuple[str, str], dict]:
    by_market: dict[str, list] = defaultdict(list)
    for name in os.listdir(directory):
        if not name.endswith("0104.code-table.json"):
            continue
        table = json.loads(open(os.path.join(directory, name), "rb").read())
        market = bytes(table["market"]).decode()
        by_market[market].append((len(table["records"]), name, table))
    out: dict[tuple[str, str], dict] = {}
    for market, items in by_market.items():
        items.sort(reverse=True)
        _, _, table = items[0]
        for rec in table["records"]:
            code = rec.get("code") or ""
            if not code:
                continue
            tail = bytes(rec["opaque_tail"])
            places = tail[0] if tail else 0
            scale = float(10**places) if 0 < places <= 6 else 0.0
            out[(market, code)] = {
                "index": rec["symbol_index"],
                "name": rec.get("name") or "",
                "places": places,
                "scale": scale,
                "last_0104": seed.i32(tail, 11) if len(tail) >= 15 else 0,
            }
    return out


def table_census(rows: dict[tuple[str, str], dict]) -> dict:
    places = Counter()
    prefixes = Counter()
    n = len(rows)
    six = 0
    scale100 = 0
    no_scale = 0
    for (market, code), row in rows.items():
        places[str(row["places"])] += 1
        if six_digit(code):
            six += 1
        else:
            prefixes[code[:3] if len(code) >= 3 else code] += 1
        if row["scale"] == 100.0:
            scale100 += 1
        if row["scale"] <= 0:
            no_scale += 1
    return {
        "rows": n,
        "six_digit": six,
        "not_six_digit": n - six,
        "scale_eq_100": scale100,
        "scale_ne_100": n - scale100 - no_scale,
        "no_scale": no_scale,
        "places": dict(places),
        "not_six_digit_prefix_top": dict(prefixes.most_common(8)),
    }


def pick_later(cands: list[dict]) -> dict:
    return max(cands, key=lambda q: (q["seq"], q["ts_ms"]))


def last_oem_by_code(callback_jsonl: str) -> dict[tuple[str, str], dict]:
    best: dict[tuple[str, str], dict] = {}
    with open(callback_jsonl, "r", encoding="utf-8") as fh:
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
            ts_ms = int(ev.get("timestamp_ms") or 0)
            for q in batch.get("quotes") or []:
                key = (q["market"], q["code"])
                row = {
                    "seq": int(seq or 0),
                    "ts_ms": ts_ms,
                    "name": q.get("name") or "",
                    "last_close": float(q.get("last_close") or 0),
                    "code": q["code"],
                    "market": q["market"],
                }
                prev = best.get(key)
                if prev is None or (row["seq"], row["ts_ms"]) >= (
                    prev["seq"],
                    prev["ts_ms"],
                ):
                    best[key] = row
    return best


def score_unique(
    oem: dict[tuple[str, str], dict],
    table: dict[tuple[str, str], dict],
) -> dict:
    hits = Counter()
    samples_scale = []
    samples_name = []
    for key, q in oem.items():
        hits["oem"] += 1
        if not six_digit(key[1]):
            hits["oem_not_six_digit"] += 1
        row = table.get(key)
        if row is None:
            hits["missing_0104"] += 1
            continue
        hits["present"] += 1
        if row["name"] == q["name"]:
            hits["name_exact"] += 1
        elif len(samples_name) < 6:
            samples_name.append(
                {
                    "code": key[1],
                    "0104": row["name"],
                    "oem": q["name"],
                }
            )
        if row["scale"] <= 0:
            hits["no_scale"] += 1
            continue
        seeded = seed.rust_scaled(row["last_0104"], row["scale"])
        hard100 = seed.rust_scaled(row["last_0104"], 100.0)
        zero = 0.0
        if seed.same_f32(seeded, q["last_close"]):
            hits["project_0104"] += 1
        if seed.same_f32(hard100, q["last_close"]):
            hits["hardcoded_100"] += 1
        else:
            hits["hardcoded_100_miss"] += 1
            if row["scale"] != 100.0 and len(samples_scale) < 8:
                samples_scale.append(
                    {
                        "code": key[1],
                        "places": row["places"],
                        "scale": row["scale"],
                        "0104": seeded,
                        "hard100": hard100,
                        "oem": q["last_close"],
                    }
                )
        if seed.same_f32(zero, q["last_close"]):
            hits["no_project_zero"] += 1
    n = hits["present"] or 1
    return {
        "oem_codes": hits["oem"],
        "present_in_0104": hits["present"],
        "missing_0104": hits["missing_0104"],
        "oem_not_six_digit": hits["oem_not_six_digit"],
        "name_exact": hits["name_exact"],
        "project_0104_last_close": hits["project_0104"],
        "hardcoded_scale_100": hits["hardcoded_100"],
        "hardcoded_100_miss": hits["hardcoded_100_miss"],
        "no_project_zero_hits": hits["no_project_zero"],
        "no_scale": hits["no_scale"],
        "rates": {
            "project_0104": round(hits["project_0104"] / n, 6),
            "hardcoded_100": round(hits["hardcoded_100"] / n, 6),
            "name_exact": round(hits["name_exact"] / n, 6),
        },
        "samples_hardcoded_100_miss": samples_scale,
        "samples_name_miss": samples_name,
    }


def main() -> int:
    if len(sys.argv) != 7:
        print(
            "usage: score-project-needs-0104-row.py EXTRACT CALLBACK META_0903 "
            "MORNING_0104 NIGHT_CALLBACK NIGHT_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, morning_dir, night_cb, night_0104 = (
        sys.argv[1:]
    )
    morning = load_largest_0104_full(morning_dir)
    night_table = load_largest_0104_full(night_0104)
    meta_0903 = seed.load_index_metadata(meta_0903)
    rows, _ = seed.load_decoded(extract_dir)

    cb_by = defaultdict(list)
    for q in ident.stream_callback_quotes(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)

    sticky: dict[tuple[str, int], int] = {}
    join = Counter()
    samples_sticky = []
    samples_resolver = []
    for row in rows:
        key_idx = (row["market"], row["index"])
        incoming = seed.i32(row["rec"], 0x12B)
        if incoming != 0:
            sticky[key_idx] = incoming
        meta = meta_0903.get(key_idx)
        if meta is None:
            continue
        cands = cb_by.get((row["market"], meta["code"], row["ts"]), [])
        if not cands:
            continue
        join["joined"] += 1
        picked = pick_later(cands)
        scale = float(meta.get("scale") or 0)
        morn = morning.get((row["market"], meta["code"]))
        if scale <= 0 or morn is None or morn.get("scale", 0) <= 0:
            join["no_scale"] += 1
            continue
        project = seed.rust_scaled(morn["last_0104"], morn["scale"])
        no_project = 0.0
        resolver = seed.rust_scaled(incoming, scale)
        sticky_v = seed.rust_scaled(sticky.get(key_idx, 0), scale)
        hard100 = seed.rust_scaled(morn["last_0104"], 100.0)
        if seed.same_f32(project, picked["last_close"]):
            join["project_0104"] += 1
        if seed.same_f32(no_project, picked["last_close"]):
            join["oemstate_record_without_project"] += 1
        if seed.same_f32(resolver, picked["last_close"]):
            join["resolver_full_overwrite_0x12b"] += 1
        elif len(samples_resolver) < 6:
            samples_resolver.append(
                {
                    "code": meta["code"],
                    "incoming_0x12b": resolver,
                    "oem": picked["last_close"],
                    "0104": project,
                }
            )
        if seed.same_f32(sticky_v, picked["last_close"]):
            join["counterfactual_merge_copy_0x12b"] += 1
        elif len(samples_sticky) < 6:
            samples_sticky.append(
                {
                    "code": meta["code"],
                    "sticky_0x12b": sticky_v,
                    "oem": picked["last_close"],
                    "0104": project,
                }
            )
        if seed.same_f32(hard100, picked["last_close"]):
            join["hardcoded_100"] += 1

    auction_oem = last_oem_by_code(callback_jsonl)
    night_oem = last_oem_by_code(night_cb)

    n = join["joined"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_project_needs_0104_row.v1",
        "crate_facts": {
            "OemState.merge_does_not_copy_0x12b": True,
            "OemState.project_writes_previous_close_to_0x12b": True,
            "to_public_quote_requires_six_ascii_digits_and_0104_scale": True,
            "runtime_stores_only_previous_close_i32": True,
            "runtime_never_calls_project": True,
            "resolver_update_from_decoded_overwrites_full_record_including_0x12b": True,
            "seed_code_tables_skips_existing_index_keys": True,
        },
        "morning_0104_census": table_census(morning),
        "night_0104_census": table_census(night_table),
        "auction_unique_oem_vs_morning_0104": score_unique(auction_oem, morning),
        "night_unique_oem_vs_night_0104": score_unique(night_oem, night_table),
        "auction_exact_second_join": {
            "joined": join["joined"],
            "project_0104": join["project_0104"],
            "oemstate_record_without_project": join["oemstate_record_without_project"],
            "resolver_full_overwrite_0x12b": join["resolver_full_overwrite_0x12b"],
            "counterfactual_merge_copy_nonzero_0x12b": join[
                "counterfactual_merge_copy_0x12b"
            ],
            "hardcoded_scale_100": join["hardcoded_100"],
            "rates": {
                "project_0104": round(join["project_0104"] / n, 6),
                "without_project": round(
                    join["oemstate_record_without_project"] / n, 6
                ),
                "resolver_0x12b": round(join["resolver_full_overwrite_0x12b"] / n, 6),
                "sticky_0x12b": round(join["counterfactual_merge_copy_0x12b"] / n, 6),
                "hardcoded_100": round(join["hardcoded_100"] / n, 6),
            },
            "samples_resolver_miss": samples_resolver,
            "samples_sticky_miss": samples_sticky,
        },
        "product_wiring": [
            "Call OemState.project(code, name, price_scale_hint) from the same-session 0104 row.",
            "Do not to_public_quote(oem_states.record) — 0x12b stays 0 after merge.",
            "Do not to_public_quote(resolver.record) after 2704 — update_from_decoded overwrites 0x12b.",
            "Do not add 0x12b to merge()'s nonzero-copy list — leftover overnight residue would stick.",
            "Do not hardcode scale=100; 0104 opaque_tail[0] is per-row.",
            "Reconnect must drop oem_states AND rebuild the 0104 row map (code/name/scale), not only previous_close i32.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
