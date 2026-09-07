# External Phase S report (2026-09-02)

## Provenance

- Source: external reference-engineering report, supplied during the current
  replication session.
- Account class: formal account; credentials and raw payloads intentionally
  omitted.
- Status: `needs-verification` for this repository until reproduced locally.

## Reported observations

- 7100 login completed with the 19-field manifest and a 15-field success
  response.
- The downloaded L1 configuration contained four active 5188 routes and five
  7709 routes; route rows use seven columns and are filtered by broker and
   entitlement.
- A selected 5188 endpoint accepted the connection.
- Login-stage control exchange reportedly produced a 95-byte 3610 payload.
- ABK control exchange reportedly produced a 94-byte 3610 payload and a 632-byte
  3110 response.
- The driver-level acceptance still reported that the peer closed the session
  before the initialization session was established.

## Interpretation

The report confirms useful protocol hypotheses and matches the builders and
route parser now present in Rust. It does not prove continuous official
full-push delivery, callback parity, ACK/3210/2d10 completion, or reconnect
recovery.

## Discriminating next test

Replay one formal-account cold start while recording the complete 7100 and
5188 direction-ordered frame timeline. Require login 3110, ABK 3110, ACK,
3210, and 2d10 in sequence, followed by bounded 2704 delivery and a matching
Wine callback window. Keep payloads opaque and publish only lengths, kinds,
offsets, hashes, and field-level comparison results.
