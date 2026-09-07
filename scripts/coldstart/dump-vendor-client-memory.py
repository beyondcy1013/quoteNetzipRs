#!/usr/bin/env python3
"""Dump writable private memory regions of a process into one file + index.

usage: dump_wjf_mem.py <pid> <out_prefix>
writes <out_prefix>.bin and <out_prefix>.index.json ([{start,end,offset,perms,path}])
"""
import json
import sys

pid = int(sys.argv[1])
prefix = sys.argv[2]
regions = []
with open(f"/proc/{pid}/maps") as maps:
    for line in maps:
        parts = line.split()
        addr, perms = parts[0], parts[1]
        path = parts[5] if len(parts) > 5 else ""
        if "r" not in perms or "w" not in perms:
            continue
        if path.startswith("[vvar]") or path.startswith("[vsyscall]"):
            continue
        start, end = (int(x, 16) for x in addr.split("-"))
        regions.append((start, end, perms, path))

index = []
offset = 0
with open(f"/proc/{pid}/mem", "rb", 0) as mem, open(prefix + ".bin", "wb") as out:
    for start, end, perms, path in regions:
        try:
            mem.seek(start)
            data = mem.read(end - start)
        except (OSError, OverflowError) as error:
            index.append({"start": start, "end": end, "perms": perms, "path": path, "error": str(error)})
            continue
        out.write(data)
        index.append({"start": start, "end": end, "perms": perms, "path": path, "offset": offset, "len": len(data)})
        offset += len(data)
with open(prefix + ".index.json", "w") as fh:
    json.dump(index, fh)
print(f"pid={pid} regions={len(regions)} bytes={offset}")
