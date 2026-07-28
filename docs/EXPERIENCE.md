# NetzipRs Experience

## 2026-07-28 - Separate raw 0547 time from OEM public quote time

### Phenomenon

Direct `0x0547` replies ended at `15:30:50` for Shanghai and `15:30:00` for Shenzhen,
while the Wine `Ask` public reports for the same instruments ended at `15:00:00`. Prices and
volume fields remained aligned, so treating the difference as another parser cursor problem would
have changed unrelated fields.

### Root Cause

The production `网际风.exe` local bridge caps its internal source time at `150000` before
converting `HHMMSS` to seconds and writing the public OEM object. The branch is at `0x4839a3` and
the conversion routine is at `0x438800` in SHA-256
`de712a8dde6d990e1c586f8afd4194575e35dffa2d0f81245fe29f6f8509bd29`. Direct Rust access to
the same `7709` server retained the later raw time, proving that the behavior belongs to the local
public-object boundary rather than the upstream protocol.

### Change

- Keep `time_hhmmss_raw` and `extra0_time_hhmmss` unchanged for protocol diagnostics.
- Format public `datetime / quote_datetime` with the verified `15:00:00` upper bound.
- Apply the same public bound when `extra0` is the only available time hint.
- Mark corrected quoteGateway rows as `netzip-rust-7709-0547.v2`; quoteGateway accepts only the
  explicit v1-to-v2 cache migration and keeps ordinary stale rejection unchanged.
- Leave price, cumulative volume, current volume, and amount decoding untouched.

### Verification

The TDD boundary covers `153050`, `153000`, `150000`, `113200`, and an invalid value. A service
test verifies that `153050` becomes `2026-07-24 15:00:00` in the quoteGateway payload while the
record still contains raw `153050`; a separate regression preserves a diagnostic
`extra0_time_hhmmss=15:30:12` while exposing public `15:00:00`.

The final workspace regression, webClx request `194041-18c6694faaae503f`, passed 121 tests across
the parser, service, compatibility, and protocol crates. Deployment request
`194509-18c6694faaae5043` installed SHA-256
`bcfe12ef569a8ff97a24f9811b49ba85a8d1b69e149e5883de7549e0cd300da3` after quoteGateway's
v2 consumer had already been deployed. A closed-session worklist publication delivered 5,523
current quotes in 57 v2 TCP/MsgPack batches; direct and gateway samples agreed on
`SH600000=15:00:00/1020605`, `SZ000001=15:00:00/1061012`, and
`BJ920001=15:00:00/16888`. Both resident services remained active and the timer units remained
absent. Repeating the full worklist in the same resident process produced
`published_count=0` and `unchanged_count=5523`, so closed-session residency did not create
duplicate downstream batches.

## 2026-07-28 - Keep the Rust full-market publisher online without a timer

### Phenomenon

`netzip-rs-full-push.service` exited after each trading session and depended on a timer at
`09:14:50` and `12:59:50` to return. Removing only the timer would therefore leave the publisher
offline. Repeated snapshot polling could also send identical quote batches when upstream source
times had not advanced.

### Root Cause

The shell runner treated an out-of-session start as successful completion. The service had no
install target and used `Restart=on-failure`, so a normal exit was not restarted. The publisher
also had no source-side last-published timestamp cache; equal timestamps were filtered only after
reaching quoteGateway.

### Change

- Keep the runner alive outside trading sessions and sleep without calling quote or publish APIs.
- Install and enable `netzip-rs-full-push.service` directly with `Restart=always`.
- Remove the timer unit and clean any installed legacy timer during deployment.
- Publish only quotes whose `trade_date + quote_datetime` is newer than the last successful send.
- Report unchanged rows as `unchanged_count` and back off when a round has no new quotes.

### Verification

Verification evidence is recorded in the webClx build and install logs for the deployment that
introduced this change. Runtime acceptance requires the publisher service to remain active during
an idle window, the timer unit to be absent, and quoteGateway counters to stay unchanged when all
upstream quote times are unchanged.
