# OEM Bridge Tools

## Latest Verification

webClx request `112336-18d31111735c6f5a` completed with status 0 on
2026-09-08 at 11:24:42 after fmt, example tests, Clippy and release verification.
The retained report is
`../../diagnostics/20260908-oem-bridge-scalar-v1/publish-vectors-v4.json`,
SHA256 `77dd51ce8b675ee7699b250db7535bab36a88711064e9888ac2cbd559037d482`.
All 50 conditional native/Rust vectors agree: 32 mode2 scale cases,
6 mode1/category cases and 12 direct-amount cases. Current source, verifier
and native-test hashes match the report. `native_session_parity=false` remains;
this result is not a full projection, live-state or callback acceptance result.
After the portable fixture-path change, 23 Python tests and all 50 vectors
passed again using the same built Rust binary. The retained follow-up report is
`../../diagnostics/20260908-oem-bridge-scalar-v1/portable-fixture-v5.json`.
The successful build alone does not establish a pushed commit; GitHub publication
is audited separately through the remote commit ID.

## Scope

These are conditional scalar experiments, not a default projection or a
publisher. Category, units and scale inputs need independent symbol identity
evidence; `0104.amount_mode` is not the bridge category selector.

The Rust example `examples/official_5188_bridge_scalars.rs` depends only on
the project's existing serde_json dependency. Build and verify through the
project's webClx workflow. Its CLI takes timestamp, volume, optional amount,
delta, category, amount mode, close and scale bits; consult the source for
defaults. Output marks experimental/noncanonical provenance.

Native test prerequisites are Python3, pefile, Unicorn and the exact executable
with SHA256 `de712a8dde6d990e1c586f8afd4194575e35dffa2d0f81245fe29f6f8509bd29`.
The executable and captured data are not distributed. The native harness reads
`NETZIP_NATIVE_EXE`, defaulting to `samples/native/netzip.exe` relative to the
working directory. Point this variable at the retained binary when it lives
elsewhere; the pinned SHA256 check still applies. Run commands from the project
root. The portable path change postdates request 112336 and does not change
the Rust implementation or native instruction slices.

Native controls:
```bash
python3 -m unittest discover -s scripts -p test_oem_bridge_scalars.py -v
python3 -m unittest discover -s scripts -p test_verify_oem_bridge_vectors.py -v
python3 scripts/verify_oem_bridge_mode2.py target/release/examples/official_5188_bridge_scalars REPORT.json
```

The verifier creates REPORT.json exclusively and preserves every vector,
packed amount and float32 bytes. An existing report is never overwritten.
Its hashes must be associated with the actual build log; they do not independently
prove build provenance. Native tests require explicit instruction endpoints and
do not treat instruction-budget exhaustion as success.

Verified conversion scope: low32 volume and unsigned-f32 getter, category9
wrapping preprocessing, timestamp normalization, amount modes0/1/2 and mode2
scale bits. Conditional controls also cover entry, selected copy/time gates
and final256B overwrite. They do not establish full acceptance, live metadata,
atomic native pre-state, reconnect/day-cut state or same-session callback parity.

Detailed history is in [the scalar evidence report](oem-bridge-scalar-implementation-20260908.md).
Private diagnostics referenced there are not part of the GitHub distribution.
