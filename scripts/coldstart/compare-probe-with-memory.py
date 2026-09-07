#!/usr/bin/env python3
"""Compare probe-decoded 2704 records with Wine's in-memory records.

usage: cmp_mem.py <probe_dump.json> <mem_records.json>
"""
import json
import sys
import collections

dump = json.load(open(sys.argv[1]))
mem = json.load(open(sys.argv[2]))
fields = ["ts", "open", "high", "low", "close", "volume", "amount", "last_close"]
stats = collections.defaultdict(lambda: collections.Counter())
missing = 0
shown = 0
by_frame = collections.defaultdict(list)
for r in dump:
    by_frame[(r["src"], r["dst"], r["frame_index"], r["frame_us"])].append(r)
for key, recs in by_frame.items():
    recs.sort(key=lambda r: r["bit_start"])
    for pos, r in enumerate(recs):
        m = mem.get(f"{r['market']}:{r['idx']}")
        if m is None:
            missing += 1
            continue
        bucket = "first" if pos == 0 else ("second" if pos == 1 else "later")
        stats[bucket]["n"] += 1
        all_ok = True
        for f in fields:
            ok = r[f] == m[f]
            stats[bucket][f] += ok
            all_ok &= ok
        # ladder: our probe only dumps bid1/ask1
        lp = m["ladder_prices"]
        stats[bucket]["bid1"] += r["bid1"] == lp[4]
        stats[bucket]["ask1"] += r["ask1"] == lp[5]
        stats[bucket]["all8"] += all_ok
        if not all_ok and shown < 6 and pos <= 1:
            shown += 1
            print(f"MISMATCH pos={pos} {r['market']}:{r['code']} {r['name']} mask=0x{r['mask']:02x} bits={r['bits']}")
            print("   ours:", {f: r[f] for f in fields}, "bid1/ask1", r["bid1"], r["ask1"])
            print("   wine:", {f: m[f] for f in fields}, "bid1/ask1", lp[4], lp[5])
print(f"frames={len(by_frame)} records={len(dump)} no_mem_record={missing}")
for b in ("first", "second", "later"):
    c = stats[b]
    n = c["n"] or 1
    print(f"{b:>6}: n={c['n']} " + " ".join(f"{f}={c[f]}" for f in fields + ["bid1", "ask1", "all8"]))
