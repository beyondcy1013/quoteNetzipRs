# Wine Binary Symbol Audit

Status: **exploration / evidence-building**  
Date: 2026-09-01

## Inputs

- `/home/codes/third_party/quoteNetzipWine/Stock.dll`
- `/home/codes/third_party/quoteNetzipWine/Stock.dat`
- `/home/codes/third_party/quoteNetzipWine/Stock64.dll`
- `/home/codes/third_party/quoteNetzipWine/系统/Stockdrv.dll`

## Observation

`objdump -x` exposes only the expected DLL control surface (`Start`, `Ask`,
`Stop`). Wide-string inspection finds `ZSTD` markers, but no stable exported or
named symbols for `5188`, `0x2704`, or quote-field decoders. This confirms that
the 5188 business codec is implemented behind the vendor DLL boundary rather
than in the open Wine wrapper.

## Disposition

Confirmed as a boundary fact; **not** evidence for any particular compression
format or field layout. The Rust implementation must therefore derive inner
objects from reassembled captures and callback correlation, not from symbol
names or the existing 7709 parser.

## Login/ABK/ACK static path

- `confirmed`: staged `Stock.dat` is itself a PE32 DLL exporting
  `Start/Ask/Stop`. Its PDB path identifies `ThsStock/Stock.pdb`, and retained
  strings include `Socket/DzhServer.cpp`, `SSLogInRePack`, `m_intLogInRet`,
  `SFLogInPack=ABK`, `&ack=`, `penc`, and `hypenc`.
- `confirmed`: the handler around `0x10070abc` requires numeric kind `0x1031`
  (wire `3110`). The branch at `0x10070b05` compares `ABK`; the alternative at
  `0x10070c3d` compares `ACK`. Both feed shared instance-state and
  formatting/packing calls.
- `confirmed`: function `0x10074200` references the `&ack=` literal, allocates
  a roughly 4 KiB local buffer, copies caller data and invokes the shared
  packing helper. Name it only `ack_string_pack_helper` in research notes;
  calling it encryption, hashing or compression is `needs-verification`.
- `confirmed`: `0x10074200` has one direct call site at `0x100380f3`. The
  caller passes the session buffer at instance offset `+0x6c261c`; it does not
  pass the immediately preceding 75-byte `3610` response field.
- `confirmed`: the `0x1031` handler at `0x10038120` has an `ACK` branch at
  `0x1003821a`. That branch copies its caller-supplied payload into the same
  `+0x6c261c` buffer and terminates it before later session processing. This
  identifies a receive/store/forward lifecycle for the opaque ACK data.
- `confirmed`: the only observed caller at `0x1002ed71` passes the reassembled
  frame object as argument 1, the value returned by its `0x1000e0b0` accessor
  as argument 2, and the caller's original argument 3 as argument 3.
  `0x1000e0b0` returns `8 + dword[object+2]`, the complete 8-byte-header plus
  declared-payload frame length; `0x1000e0d0` separately returns `object+8`.
  In the ACK branch, argument 1 is the source pointer and argument 2 is the
  exact byte count supplied to `memcpy(session+0x6c261c, source, count)`.
  Therefore the session buffer receives the complete second `3110` application
  frame: 639 bytes for a 631-byte payload or 644 bytes for a 636-byte payload,
  followed by a NUL terminator.
- `confirmed`: `ack_string_pack_helper` does not consume that whole stored
  frame as text. It calls the payload accessor (`object+8`), copies the declared
  payload into a zeroed local buffer, then passes it through a
  `MultiByteToWideChar` wrapper that first computes C `strlen`. In all ten
  `vendor_pm` flows, the 631-byte variant has its first NUL at payload offset
  64 and the 636-byte variant at offset 111. The effective `&ack=` source is
  therefore exactly the 64- or 111-byte pre-NUL prefix. Treating all 631/636
  payload bytes as ACK text input is `rejected`.
- `confirmed`: `vendor_pm` has two server-to-client wire-`3110` frames per
  complete 5188 flow and one wire-`3210` frame. Across ten flows, `3110`
  payloads are 48 bytes x10 plus 631 bytes x4 / 636 bytes x6; `3210` payloads
  are 466 bytes x8 / 467 bytes x2. These are initialization-control envelopes,
  not decoded ACK blob bytes.
- `rejected`: `ack_string_pack_helper` derives the 1379/1387-byte data from
  the short control response at the call site. The data already resides in
  per-session state before that helper runs.
- `needs-verification`: which `3110`/`3210` payload or downstream transform
  yields the 1379/1387 bytes. The length groups do not permit a direct-copy
  claim, and no opaque bytes are promoted to runtime defaults.
- `confirmed`: the control-capture response objects align with this path. The
  login, ABK and ACK response `数据` fields are complete `3610` frames of 103,
  102 and 75 bytes respectively, with matching stage name and `应答编号`.

These addresses belong to staged `Stock.dat` SHA-256
`9f6ea44ffd78bd40c2ea0fce0312524d0d78f95ac2dea1bc33ff9579fb3947c9`.
They must be re-based or re-located by string cross-reference for another
binary revision.

## Next discriminating step

Bounded disassembly of `0x10073f60` adds a confirmed call boundary: the
function initializes a wide-character work buffer, formats the caller input
through `0x100742f0`, invokes `0x100075f0` with a bounded `0x3a8` conversion
length, and then passes the resulting object to `0x1007b7f0`. The ACK helper
`0x10074200` supplies the payload accessor result and the `&ack=` template to
this path. This confirms a structured text/object packing chain; it does not
prove encryption, hashing, or a direct byte transform. The exact field
construction and variant selector remain `needs-verification`.

The callee at `0x1007b7f0` is a thin instance dispatcher: it forwards its two
arguments to `0x1002c3f0` with an object/state base at instance offset
`+0xe3c5a0`. No byte-wise transform or fixed 1379/1387 length is present in
this wrapper. This narrows the remaining search to the object method behind
`0x1002c3f0`; the wrapper itself is not a candidate ACK encoder.

Inspection of `0x1002c3f0` shows the forwarded method is stateful session
setup/dispatch rather than a byte encoder: it checks instance state at
`+0x2d4`, acquires a session object at `+0x64d4c4`, sets a `0xbb8` timeout,
invokes an indirect transport operation, and handles failure cleanup. No
1379/1387-sized buffer or payload copy occurs in this method. The ACK value
construction therefore remains upstream in the formatter/object path.

Read-only data cross-references further identify the formatter templates:
`0x10105d54` is the NUL-terminated ASCII template `MAC=%s`, while
`0x10105d7c` is the ASCII literal `&ack=`. The adjacent UTF-16 data contains
the `NetCardMac`/MAC field labels and short stage markers. These literals
confirm that the same generic object formatter serves both network-card
identity and ACK fields; they do not expose the opaque ACK value or its
variant-selection rule.

Trace the generic packer at `0x10073f60` from the confirmed 64/111-byte
second-`3110` pre-NUL prefix to the resulting 1379/1387-byte control value.
Identify the code-page conversion and control-object field construction. Test only
bounded structural relationships first; do not export opaque payloads.
Separately, use a captured long-lived 5188 session and a
concurrently timestamped Wine `OEM_REPORT` callback for business decoding.
Keep failed candidates as negative evidence with fixture hashes.

## Additional bounded call-shape evidence (2026-09-01)

- `confirmed`: `0x10074200` obtains the complete frame payload via
  `0x1000e0d0`, reads the declared payload length from `frame+2`, copies that
  bounded byte range into a zeroed local buffer, and invokes the formatter
  with the `&ack=` template at `0x10105d7c`. It does not expose a fixed-size
  byte encoder.
- `confirmed`: `0x10073f60` initializes a wide-character work buffer, calls
  `0x100742f0` (a varargs formatting wrapper around `0x100c213d` with format
  selector `0x1401`), then calls `0x100075f0` with conversion bound `0x3a8`
  and source bound `0x1400`. The converted object is dispatched through
  `0x1007b7f0`. This proves bounded text/object construction, not encryption,
  hashing, compression, or the 1379/1387 variant rule.
- `confirmed`: `0x10074380` appends subscription entries at
  `table + 0x12 + 6*count`: two little-endian bytes for market/code prefix
  and four bytes of opaque entry data, then updates table length to
  `0x0a + 6*count`. This matches the Rust `2a10` parser's `10 + 6*N` shape;
  the four-byte value remains unassigned.
- `needs-verification`: the indirect object method reached after
  `0x1007b7f0` and the exact source of the 1379/1387 ACK data. No new runtime
  default or decoder is justified by these call-shape observations.
