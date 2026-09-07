# Official 5188 Opening Readiness

This runbook keeps the official 5188 implementation in shadow mode until its
transport and trade-quote gates pass independently. It does not authorize a
publication-source switch.

## Before 09:15

- Keep exactly one formal-account session. Do not start a second login probe.
- Start the packet capture before login so the same-session `0104`, `2a10`,
  baselines, and first `2704` records are retained.
- Keep the existing production quote source active. Official 5188 remains a
  separately identified shadow source with no implicit 7709 fallback.
- Confirm enough disk space for the capture and record its path and start time.

## Transport gate

After initialization and after business frames begin, run:

```bash
python3 scripts/official-5188-open-readiness.py --level transport --interval 5
```

Exit `0` requires authentication, a selected `:5188` endpoint, initialization,
ten connections, code tables, a receive list, a running shadow, advancing
business frames, and no receive termination. Exit `2` is a failed readiness
gate. Exit `3` means the status endpoint or input was invalid.

## Trade-quote gate

Run the strict gate without changing publication routing:

```bash
python3 scripts/official-5188-open-readiness.py --level trade-quotes --interval 10
```

In addition to transport, this requires within the two-snapshot interval:

- `decoded_records == oem_state_updates + missing_previous_close_seeds +
  rejected_public_quotes`;
- zero missing same-session metadata rows;
- zero semantically rejected merged projections;
- zero failed or partial frames in the observation interval/state;
- zero non-ladder decoder errors.
- a non-empty shadow quote snapshot with the expected source/publication
  identity, timestamps inside the China trading date, and finite nonnegative
  scalar and ten-level array values. OHLC must contain nonzero open/last values;
  prices must remain within a deliberately broad ten-times-previous-close
  bound, and cumulative/book quantities must remain below documented broad
  corruption bounds.

Although ladder volume is not a required product field, a ladder decode error
can truncate later records in a frame. It remains blocking until the parser can
skip that field without losing later trade quotes.

After deploying a build that includes the diagnostic quote snapshot, inspect:

```bash
curl -sS http://127.0.0.1:16893/api/fullpull/official-5188/shadow/quotes
```

The response is deliberately labeled `source=netzipRustOfficial5188Shadow` and
`publication=disabled`. It contains only successfully projected records backed
by complete same-session `0104` metadata. This research endpoint is for field
parity and freshness checks; it is not a quoteGateway or stockScreener source.
The readiness script reads this endpoint automatically for `trade-quotes`.
For historical fixtures, pass `--quotes-json` and `--trade-date YYYY-MM-DD`;
omitting the quote fixture fails closed rather than accepting counter-only
evidence.

## Required evidence

Retain snapshots at 09:15, 09:25, 09:30, and after the first reconnect:

- connection count and selected endpoint;
- `0104` table count and receive-list size;
- frames received, `2704` frames, clean/partial/failed counts;
- decoded records, OEM updates, missing metadata, error kinds;
- receive terminations and freshness between successive snapshots;
- capture hash and sanitized Wine/OEM comparison fixture when available.

On reconnect or day change, the old index/code/metadata maps must be discarded.
The new session may publish only after its complete `0104` rows seed projection.
Counter conservation alone is not proof of identity correctness.

## Decision

- Transport pass, quote fail: continue shadow capture and offline replay.
- Quote pass once: keep shadow mode and verify sustained freshness/reconnect.
- Any stale metadata, frame failure, silent stop, or termination: do not switch
  `quoteGateway` or `stockScreener`; preserve evidence and diagnose offline.
- Product-source promotion requires an explicit, separately reviewed change
  with source identity, metrics, and rollback verification.
