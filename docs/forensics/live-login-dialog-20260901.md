# Live login dialog validated on test-168 (2026-09-01)

> Superseded for initial-login selection by the 2026-09-02 byte-identical
> vendor-manifest validation recorded in `docs/fullpull-replication-authority.md`.
> The 12-field dialog remains evidence for per-5188-connection initialization,
> not the initial account login.

Status: **confirmed** (live round trip, single account class)
Date: 2026-09-01, approximately 22:40–24:00 Asia/Shanghai
Account class: `test-168` (credentials were process-memory only; none are
stored or printed anywhere)
Working Rust implementation:
`Z:\stock\netzip_win\crates\netzip-fullpull\src\auth_7100.rs`
(a port of this repository's `auth_7100_client.rs` with the new dialog).

## Hypothesis

The 2026-08-06-era `认证|登录` request (ordinary-ZSTD inner, 850-byte-class
object) is no longer the accepted initial login; the current dialog is the
12-field `加密包` shape (root request name `大智慧C_登录包`, credentials filled,
dictionary-compressed) already seen as the per-connection control packet in
`vendor_pm`.

## Observed (all against `121.41.70.217`, primary auth node)

- Probe `认证|测速` (ordinary ZSTD, dynamic credentials) still works
  everywhere tested: 419B request → 345B response, classified
  `auth_probe_response`, on both 6100 and 7100.
- Legacy `认证|登录` with ordinary ZSTD is **silently dropped**: the server
  holds the socket roughly 30 s and then closes it with zero payload bytes
  (8 s reads time out with WSAETIMEDOUT; a 40 s read saw EOF at ~30 s).
  Reproduced on 6100 and 7100, multiple attempts.
- The same legacy inner object compressed with the vendor dictionary gets an
  explicit rejection: a four-field `加密包` response whose `错误` value reads
  `认证服务器不支持该请求` (with an echoed `应答编号`).
- **Current initial login (confirmed)**: 12-field `加密包`, request name
  `大智慧C_登录包`, credentials filled into the `账号`/`密码` fields, other
  fields accepted as zeros, dictionary-compressed. Live exchange with
  test-168: 363B request (decodes to 460B, 12 fields in the confirmed label
  order) → 398B response (decodes to 275B) with the verified four-field shape
  `请求/来源/应答编号/数据`, `请求` echo `大智慧C_登录包`, `来源`
  `认证服务器`, `应答编号` echo of the request number, and `数据` holding one
  complete current-session `3610` frame with an **81-byte payload**.
- Follow-ups on the same socket, numbers 1 and 2, templates byte-equal to the
  captured 271B/333B fixtures: `download_file` response of 1540B with 5
  active server entries; then a `Tdx_Encrypt` response of 404B (decodes to
  472B) whose root is **`加密解密`** (not `加密包`) with opaque binary `数据`.
- The `登录成功` decoded-text marker does **not** appear anywhere in the new
  dialog. Dialog success is defined by the three validated stages
  (`login_control → download_file → tdx_encrypt`), each with field-level echo
  checks.
- test-168 provisioning: the download response's five active entries are all
  `7709/7709`; no 5188 row. The official 5188 full-push endpoint requires the
  formal account, consistent with the authority ledger's account policy and
  the `wine-full-start-formal` negative control.
- Length variance of the current-session `3610`: 81B (initial login with
  credentials), 94B (per-connection login-stage exchange sent with zero-valued
  fields on the authenticated socket), 95B (formal `vendor_pm` flows), 93B
  (historical `大智慧/C8_Login_1036.dat`). The payload length is therefore
  value/session-dependent; strict per-stage length gates (95/94/67) match the
  formal full-push chain but rejected a real 94B zero-field response.

## Decode traps (both hit live)

- Decoding a **plain-ZSTD** frame with a dictionary-configured decoder
  "succeeds" but silently primes the window with dictionary content and
  produces corrupt output. Control decoders must accept a dictionary decode
  only when the result carries the expected root (`加密包`) and otherwise
  fall back to plain ZSTD.
- `Tdx_Encrypt` responses use the `加密解密` root; a `加密包`-only parser
  rejects a valid stage response.

## Disposition

- `confirmed`: the 12-field `加密包` credentials-bearing initial login is the
  current accepted dialog on 6100/7100; the legacy `认证|登录` object is dead
  (silent drop when ordinary-ZSTD, `认证服务器不支持该请求` when
  dictionary-wrapped).
- `confirmed`: this resolves the `align-boot-2210` contradiction — the 6100
  582B client / 326B server pair (field44=12, ZSTD) is the **login dialog**,
  not market data. Sizes differ from the test-168 exchange because the formal
  account's credential/field bytes are longer; the shape is the same family
  (`加密包` root with a `3610` in `数据`). The "inflated root named 认证"
  classification in the 7100 flow matrix was a classifier artifact of the
  outer envelope, not a protocol root.
- `confirmed`: test-168 is supplement-only (7709 route set); it cannot be
  used to advance the 5188 initialization-field work.
- `needs-verification`: `3610` length derivation, ACK `数据` grammar, `2d10`
  word sources, and login-control field values — unchanged open items, now
  testable through a working live dialog on the formal account.

## Comparison

Not comparable to Wine callbacks; this experiment covers the authentication
control chain only.

## Follow up (smallest discriminating test)

Port the new dialog into `auth_7100_client.rs` (the reference copy in
netzip_win already passes 75 offline tests plus this live acceptance),
then repeat the two-login experiment on the **formal** account: per-connection
`3610` length/hash versus request field values, to close the length-derivation
and field-source questions before the ACK/`2d10` work resumes.
