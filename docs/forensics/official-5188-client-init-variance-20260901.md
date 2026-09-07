# Official 5188 Client Initialization Variance

Status: **exploration / needs-verification**  
Date: 2026-09-01 (Asia/Shanghai)  
Account class: `formal`

## Evidence

- Capture: `diagnostics/20260831-netzip-windows-vs-rust/captures/vendor_pm.pcapng`
- SHA-256: `b9d3b2b4a3e766ee45fd453b6ee745611328d56bd665982ce5c37284ab0a2ad3`
- Extractor: `analysis/extract_5188_payloads.py --direction client --full-frame`
- Machine-readable summary: `official-5188-client-init-variance-20260901.json`

The extraction reconstructed 72 client frames. Ten flows contained the full
`3 x 3610` followed by `3 x 2d10` shape. Optional `0710` appeared once and
`2a10` appeared on seven of those flows. Four separate probe-like flows carried
only a stable `2e10` frame.

## Confirmed Observations

| Wire kind | Count | Payload lengths | Unique full-frame hashes |
|---|---:|---|---:|
| `3610` | 30 | 67, 94, 95 | 12 |
| `2d10` | 30 | 32 | 6 |
| `0710` | 1 | 12 | 1 |
| `2a10` | 7 | 742, 6154 | 7 |
| `2e10` | 4 | 24 | 1 |

The 95-byte `3610` frame differs on every complete connection. Its differences
cover a broad byte range, not just the header length or metadata. The 94-byte
and 67-byte `3610` frames are stable across these ten flows. The three `2d10`
frames occur as two distinct three-frame sets. Every `2a10` frame is distinct;
six contain 1024 opaque six-byte entries and one contains 122.

## Disposition

The evidence rejects static replay of one captured initialization sequence as a
runtime implementation. At least part of `3610`, `2d10` and `2a10` is bound to
the connection, current session, market partition or subscription assignment.
Captured frames remain valid shape fixtures only. Rust must generate the
sequence from the current formal authentication result and current connection
assignment before a live 5188 session can be accepted.

Raw extracted frames are not added to source control. They remain derivable
from the registered capture, avoiding a second untracked copy of potentially
session-bearing bytes.

## Next Discriminating Test

Capture two fresh formal-account logins with synchronized Wine callbacks. For
each connection, compare the changing `3610` byte range and `2d10` sets against
the authentication response, endpoint assignment, local connection index and
`2a10` subscription range. Promote a constructor rule only when one candidate
predicts the bytes in the second independent login.
