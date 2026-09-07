# Official 5188 control-frame correlation (2026-09-01)

## Scope

- Account class: formal.
- Input: `diagnostics/20260831-netzip-windows-vs-rust/captures/vendor_pm.pcapng`.
- SHA-256: `b9d3b2b4a3e766ee45fd453b6ee745611328d56bd665982ce5c37284ab0a2ad3`.
- Method: reassemble client-direction 5188 streams, then search complete frames in
  non-5188 TCP payloads from the same capture.
- Generated report: `diagnostics/20260831-netzip-windows-vs-rust/analysis/official-5188-control-correlation.json`.

## Confirmed

- Ten complete 5188 flows each carry three `3610` frames. The 95-byte-payload
  variant contains a per-connection 64-byte region; the 94-byte and 67-byte
  variants are stable across these flows.
- For every one of the ten dynamic `3610` variants, the same 64-byte region is
  present in a preceding server-to-client 7100 TCP payload.
- Across those ten 103-byte frames, every varying byte is in the single
  contiguous frame range 24..88. Nine control responses contain the complete
  frame; the remaining response contains that exact 64-byte dynamic region.
  The generated report labels these separately as `full_frame` and
  `dynamic_region`; a region match is not handshake acceptance.
- Representative 7100 payloads contain the complete 103-byte `3610` frame at
  offset 241, followed by additional control bytes. Wine forwards that frame to
  the assigned 5188 connection roughly 100 microseconds later.
- This rejects static replay and guessed key material as the runtime model. The
  current authenticated control session supplies at least the dynamic `3610`
  initialization frame.
- The selected 7100 flow first sends a login-sized request, receives a control
  response, sends a dictionary/download request and receives the server object.
  It then exchanges a burst of per-connection requests/responses; responses
  embed the `3610` frames and are followed almost immediately by matching 5188
  sends. The same 7100 socket later carries a stable request and variable
  response at roughly 100-second intervals.
- This does not conflict with the controlled 2026-08-06 run that used 7100 only
  for probe and 6100 for login. It rejects the later over-generalization of that
  run: authentication/control port roles are session/config dependent.

## Needs verification

- The control-to-data connection assignment rule and whether it is strictly
  creation order, response order, or an opaque connection role.
- One of 30 complete `3610` frames was not found byte-for-byte in an individual
  non-5188 payload, although its dynamic 64-byte region was found. TCP control
  reassembly may explain the discrepancy.
- Complete `2d10`, `2a10`, `0710` and observed `2e10` client frames were not
  found byte-for-byte in individual control payloads. Their derivation remains
  open and must not be guessed.
- The per-connection control request fields and their mapping to selected 5188
  endpoints remain opaque. Existing 6100 three-response login code must not be
  reused as the persistent 7100 lifecycle without a role-aware state machine.

## Control request structure

- `confirmed`: ten client requests whose decoded `请求` value identifies the
  Wine login-packet class precede the ten dynamic `3610` responses. Their wire
  length distribution is 369 B x 5, 371 B x 1, 372 B x 1, and 376 B x 3,
  exactly matching the correlation report; all ten request hashes are distinct.
- `confirmed`: the paired control response reaches the matching 5188 send in
  74–111 microseconds in this capture. This supports synchronous forwarding on
  the live control lifecycle; it does not establish a portable timeout.
- `confirmed`: the ten login-packet `编号` values are consecutive `2..11`.
  The following ABK values are `12,13,16,17,...,28,29`; ACK values are
  `14,15,18,19,...,30,31`. This is an observed request-number schedule, not yet
  a rule for server/connection assignment.
- Stable 94-byte and 67-byte `3610` frames occur on every connection, so a raw
  byte search has ten valid control matches per frame. Report schema v4 retains
  those matches and separately identifies the nearest non-negative-time match;
  only that field may be used for request/response transaction pairing.
- Schema v5 also emits `assigned_control_match`, a capture-order one-to-one
  candidate within each identical-frame hash group. It prevents one control
  response from being assigned twice. This is an analysis aid and remains
  `needs-verification` until connection roles independently predict the same
  assignment.
- Schema v6 reconstructs both directions of each 6100/7100 TCP stream before
  splitting complete `网络包` objects by their declared length. Correlation no
  longer treats an individual TCP segment as a complete control response; this
  is required for the previously missing request-20 response.
- `confirmed`: even after application-level reassembly, only nine distinct
  control objects contain the stable 67-byte-payload `3610`, while Wine sends
  it on ten 5188 connections. The tenth send cannot currently be attributed to
  request 20; caching, local construction, or another unidentified source is
  `needs-verification`.
- Schema v7 includes a metadata-only control transaction ledger: request index,
  endpoint flow, timestamps, lengths, and SHA-256 values for both sides. It
  never serializes request or response bodies and is intended for ordering
  comparisons with unmatched `2d10`/`2a10` frames.

## ACK data and 2d10 partition

- `confirmed`: ACK request `数据` has exactly two complete-session variants in
  `vendor_pm`: 1379 bytes on six connections and 1387 bytes on four. Each
  length has one stable CRC32 across its occurrences.
- `confirmed`: the independent `vendor_lunch.pcapng` cold-start control flow
  also has ten login, ten ABK and ten ACK objects. Its 1379- and 1387-byte ACK
  `数据` CRC32 values exactly equal the corresponding `vendor_pm` values, but
  the counts are reversed: lunch has four 1379-byte and six 1387-byte objects.
  The hypothesis that the protocol always assigns a fixed six/four count is
  therefore `rejected`.
- `needs-verification`: the cross-session equality supports two stable
  role/partition templates rather than random per-login material, but does not
  prove that the bytes are universal across account, version or server. The
  Rust runtime must still source or derive them from the current lifecycle.
- `confirmed`: all six 1379-byte connections send the same first `2d10 x3`
  hash set; all four 1387-byte connections send the other `2d10 x3` hash set.
  There are no exceptions in the ten complete connections.
- `needs-verification`: whether ACK data generates the `2d10` bytes or merely
  identifies a shared market/permission partition. No complete `2d10` frame is
  present in a control object, so captured `2d10` frames remain fixtures rather
  than runtime defaults.
- `confirmed`: every `2d10` payload is 32 bytes: four little-endian u32 words
  followed by sixteen zero bytes. Across the two three-frame sets, per-position
  `word0` and `word3` are stable; `word1` distinguishes the two sets, while
  `word2` differs only in the first position. None of the four complete word
  values appears byte-for-byte in the corresponding ACK data.
- `vendor_lunch` has no captured client 5188 initialization frames
  (`client_frame_count=0`), so it cannot independently pair its ACK templates
  with `2d10`. It strengthens ACK-template stability only, not causality.
- `rejected`: a complete 1379/1387-byte ACK `数据` value is not returned as one
  decoded server field immediately before the ACK request. Across both cold
  starts, server `数据` fields are 75, 102 or 103 bytes. Whether those shorter
  values are embedded or transformed into the ACK blob remains
  `needs-verification`.

## Vendor implementation ownership clues

- `confirmed`: the staged official `Stock.dll` and `Stock.dat` are PE32 DLLs
  exporting the same `Start`, `Ask`, and `Stop` ABI. Their PDB records identify
  the `ThsStock/Stock` implementation family. `Stock.dat` contains protocol
  strings including `SFLogInPack=ABK`, `SSLogInRePack`, `&ack=`, `penc`, and
  `hypenc`. Login/ABK/ACK construction and unpacking therefore belong to the
  official Stock implementation, not the Rust Wine adapter.
- Exact staged hashes are `524f3d11...9084df` for `Stock.dll` and
  `9f6ea44f...3947c9` for `Stock.dat`. These identify local evidence only; they
  are not distributable fixtures or runtime defaults.
- `confirmed`: staged `大智慧/C8_Login_1036.dat` (SHA-256
  `4ab51664...5c26b`) is a complete historical `3610` frame with payload length
  93, metadata `[0, 0, 3, 0]`, and total length 101. Current `vendor_pm`
  per-connection frames use payload length 95. Reusing this historical file as
  the current handshake is `rejected`; it confirms framing and historical
  template ownership only.
- Rust owner: `Official5188ClientSessionEnvelope` retains the four words without
  semantic names and rejects a non-zero reserved tail. Constructing current
  values remains outside the parser.
- `confirmed`: all ten decode to the same 448-byte, 12-field shape. The labels
  are `请求`, `本地IP`, `网卡MAC`, `版本`, `账号权限`, `券商`, `加密版本`,
  `账号`, `密码`, `用户ID`, `接口版本`, and `编号`. The evidence catalog stores
  only label/type/length/metadata/offset and an optional CRC32 equality
  fingerprint for non-sensitive values; it does not store field values.
  Credential/identity labels never receive a fingerprint.
- `needs-verification`: which values are copied from login response, downloaded
  configuration, host identity, or connection assignment. Packet hashes and
  lengths prove transaction identity but are not constructor rules.

## Server initialization control lifecycle

- `confirmed`: every complete `vendor_pm` 5188 flow contains two server-to-client
  wire-`3110` frames (numeric kind `0x1031`) and one wire-`3210` frame (numeric
  kind `0x1032`). Rust classifies both as server initialization control while
  retaining their payloads as opaque.
- `confirmed`: the 20 `3110` payloads split into 48 bytes x10 and a second
  per-connection frame of 631 bytes x4 / 636 bytes x6. The ten `3210` payloads
  are 466 bytes x8 / 467 bytes x2. These counts are capture facts, not a
  portable connection-role rule.
- `confirmed`: staged `Stock.dat` handler `0x10038120` accepts numeric `0x1031`;
  its ACK branch copies the supplied payload into per-session buffer
  `+0x6c261c`. The only direct caller of `ack_string_pack_helper` (`0x10074200`)
  later reads that same buffer.
- `rejected`: the 75-byte ACK-stage `3610` control response is directly passed
  to `ack_string_pack_helper`. The helper receives already-stored session data.
- `needs-verification`: exact transformations and whether `3110`, `3210`, or
  another inner object populates the 1379/1387-byte ACK request data. Length
  correlation alone is insufficient, so the Rust runtime still requires
  caller-supplied current-lifecycle opaque data and contains no captured blob.
- `confirmed`: frame timestamps establish an interleaved connection state
  machine, not a batched triplet. Each complete flow follows
  `3610(95) -> 3110(48) -> 3610(94) -> 3110(631|636) -> 3610(67) ->
  3210(466|467) -> 2d10 x3` before subscription traffic.
- `confirmed`: the second `3110` length predicts the observed `2d10` set with
  no exceptions in `vendor_pm`: all four 631-byte responses precede one set,
  and all six 636-byte responses precede the other. `3210` length does not
  partition the sets: both 466 and 467 occur in the four-connection group.
- `rejected`: collect all three `3610` frames on 7100 before interacting with
  the assigned 5188 socket. The second server `3110` is received before the
  ACK-stage `3610`, matching the staged ACK-data lifecycle in `Stock.dat`.
- `needs-verification`: whether the 631/636 response generates the 1379/1387
  ACK data or both merely encode the same connection role. Exact transformation
  evidence is still required before Rust may construct ACK data or `2d10`.
- `confirmed`: field-local inspection in webClx request
  `182245-18d115af25cb0406` found no NUL, `penc`, `hypenc`, or ZSTD marker in
  either ACK `数据` variant. The 1379-byte value has 1293 visible ASCII bytes
  and the 1387-byte value has 1301; both therefore contain exactly 86
  non-visible bytes. WebClx request `182450-18d115af25cb0407` further shows 43
  visible runs separated exclusively by 43 CRLF pairs. Request
  `182905-18d115af25cb0409` confirms zero non-ASCII bytes, zero other control
  bytes, 44 lines and a trailing empty line. This rejects both a fixed binary
  skeleton and mixed-GBK interpretation: the value is pure CRLF-delimited
  ASCII text. The eight-byte variant difference is concentrated in lines
  4-12 plus one later line; exact text grammar remains `needs-verification`.
- `rejected`: apply the `0x10072e70` `2d10` field-copy offsets directly to the
  whole `3210` payload, or to an arbitrary base within it. No actual `2d10`
  frame in any of the ten complete flows satisfies that relation. The function
  consumes a separately parsed/session record whose construction remains to be
  traced.

## Rust decision

`Auth7100ControlSession::exchange_official_5188_init_triplet()` binds each
response to its request at field level: exact stage name, source `认证服务器`,
echoed request number, and the single complete `3610` held by `数据`. Generic
embedded-frame scanning remains available for research, but it no longer
defines stage acceptance. The triplet helper is offline compatibility only;
live orchestration uses the separate login/ABK/ACK methods interleaved with
`Official5188Session::exchange_initialization_stage()`. WebClx request
`171957-18d115af25cb03fd` confirms the field bindings and mismatched
answer-number rejection tests.

`netzip-fullpull::official_5188::embedded_client_frames()` may extract complete
known client frames from authenticated control payloads while preserving their
metadata and byte offsets. Extraction is a candidate stage only. The caller
must bind candidates to the current authentication lifecycle and validate the
ordered sequence with `Official5188Handshake` before sending.

## 7100 decompressed field boundary correction

- `confirmed`: the dictionary-compressed `下载文件` follow-up request expands to
  130 bytes. Its second field starts at offset 98 and has label `编号`,
  `type=2`, `value_len=4`, `field_12=3`, and `span=32`; its value starts at
  offset 124. The complete object fits exactly inside the decoded boundary.
- `rejected`: both 32-bit words at field-header offsets 8..16 must be zero.
  Current formal-session evidence only supports requiring offsets 8..12 to be
  zero. Offsets 12..16 are metadata and cannot be used to reject a candidate.
- `needs-verification`: the business meaning of `field_12=3`. The Rust
  evidence catalog preserves the number but does not use it to generate a
  request or infer authentication state.
