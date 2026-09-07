# Official 5188 Open Live Evidence - 2026-09-07

## Runtime start

- The resident installed build was started through the webClx service deploy
  API at 09:36 local time. No source-tree build or deployment was performed.
- `quote-netzip-rs-supplement.service` and
  `quote-netzip-rs-full-push.service` were both active.
- `/health` returned `ok=true`, service `quoteNetzipRs`, version `0.1.0`.
- Authentication was already successful, the retained control session selected
  `222.85.139.177:5188`, and the full-push log reported ten slots ready at
  09:36:45. No parallel login was initiated.

## Live readiness

The two-snapshot transport gate passed: authenticated, selected 5188 endpoint,
initialized, ten connections, code tables and receive list present, shadow
running, frames advancing, and zero receive termination.

The strict trade-quote gate failed closed. At its second snapshot:

- attempted frames: 2,296
- decoded frames: 2,166
- partial frames: 103
- failed frames: 130
- decoded records: 42,976
- OEM state updates: 0
- missing metadata: 0

The installed 2026-09-04 binary predates the complete-row OEM projection and
accounting deployment, so zero OEM updates is not evidence that current source
regressed. It does prove that the installed build must not publish official
5188 as the canonical product source.

## Passive capture

- Path: `captures/20260907-open-live/official-5188-0938.pcap`
- Size: 6.4 MiB
- SHA-256: `021867759009b76efa9a59643d17c62db30601231707f6243ba5a679033131ff`
- tcpdump: 24,801 packets captured, zero kernel drops
- Capture format: classic pcap, Linux cooked v2
- Service scanner: 20 flow directions/instances, 8,604 server frames, 7,078
  `2704` frames, zero reassembly errors, zero trailing bytes

This capture was passive on the already authenticated session. It did not
restart Wine, create another login, or alter publication routing.

## Strict offline replay

`official_5188_extract` exported all 8,604 complete application frames to
`diagnostics/20260907-open-live/extract-no-baseline-v1`. The capture contains
7,078 `2704` frames: 6,317 clean and 761 with a strict decode error. Of the
failed frames, 664 retained a complete decoded-record prefix.

Failure stage counts are: ladder volumes 322, accumulators 109, ladder values
88, OHLC 61, record mask 45, trailing DA 35, record header 30, auxiliary 24,
timestamp delta 12, special book prices 12, special book volumes 12, offset
`0x2c` 9, and ladder move 2. Baseline modes are absolute 359, missing-fresh
231, relative 96, and unknown 75.

The passive capture began after ten-slot initialization and contains zero code
tables, so the 231 missing-fresh failures are a capture-boundary limitation,
not protocol-defect evidence. The remaining 530 failures are the active-market
strict regression population for decoder work.

## Decision

- Transport and official 5188 delivery are live.
- Keep official 5188 in shadow mode.
- Use this capture for strict offline 2704 failure classification and decoder
  regression.
- Do not promote to `quoteGateway` or `stockScreener` until a freshly verified
  build passes complete-row projection, record accounting, active-market
  decode, sustained freshness, and reconnect gates.

## Product-wiring progress

Current source now retains the latest successfully projected public quote per
market/index inside each shadow decoder and exposes a read-only research route,
`GET /api/fullpull/official-5188/shadow/quotes`. The response explicitly says
`publication=disabled`; the endpoint is absent from stable/public capabilities
and does not invoke quoteGateway. Missing or invalid same-session metadata never
enters this snapshot. This creates the field-parity surface required before an
explicit production publisher can be reviewed.
