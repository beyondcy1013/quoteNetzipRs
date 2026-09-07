---
name: quoteNetzipRs-ops
description: Route quoteNetzipRs work between official authenticated full-push replication and 7709 supplementation without conflating them.
---

# quoteNetzipRs Maintainer Entry

## 1. Project identity and operating constraints

- Repository: `/home/codes/stock/quoteNetzipRs`.
- Dedicated official full-push replication workflow: `.agents/skills/quote-netzip-rs-fullpull-replication/SKILL.md`.
- Any Wine-vs-Rust replication, protocol reconstruction, or diagnostic-capture
  work must be routed through that dedicated skill. Its canonical results ledger
  is `docs/fullpull-replication-authority.md`; raw capture inventory is
  `docs/forensics/live-market-capture-inventory.md`.
- For real 5188 pcap/pcapng replay, use `POST /api/debug/pcap-5188-summary`.
  It returns deduplicated TCP/frame statistics and evidence-only payload hints;
  it is not a business decoder or proof of completed official full-push.
- Product goal: reproduce both data capabilities of `quoteNetzipWine`: authenticated official
  full-push and 7709 query-based supplementation.
- Canonical terminology: `docs/capability-boundaries.md`.
- Official full-push owner: `/home/codes/stock/crates/netzip-fullpull` (formerly the shared crate).
  Full-push means the formal-account, post-login, non-7709 vendor data path; current evidence points
  to 5188.
- Supplement owner: `/home/codes/stock/crates/netzip-supplement`. Code tables, snapshot queries,
  K-line, F10, finance, and any other 7709 operation are supplementation.
- Current-state warning: the resident `POST /api/hqw/push-worklist` path still scans 7709 and the
  renamed crate still contains inherited 7709 modules. Treat these as transition debt, not as proof
  of official full-push parity.
- Authority order: this file routes work; `AGENTS.MD` and `docs/capability-boundaries.md` own durable
  product semantics; `README.md` and `docs/` own detailed contracts and evidence. Current source and
  tests prove implementation state but legacy symbol/service names do not redefine product terms.
- Compile/deploy through the webClx queue. Do not manually deploy or restart unless the API is
  unavailable or the user explicitly requests manual execution.
- Account policy: use the formal account by default for replication and acceptance. Use the test
  account identified as `168` only for repeated short-interval structural diagnosis of the same
  issue; record account class and time window, never credentials. Test-168 has been observed
  receiving only `7709` routes and is not an equivalent data source: its market data may be absent,
  partial, stale, or inconsistent with formal-account entitlements. Never use test-168 routes,
  values, coverage, failure rates, or callback counts as production/full-push data evidence. A
  formal account has received `5188` during an active market window and `7709` after close; account
  class is therefore necessary but not the sole route selector. Record time window, server policy,
  login outcome, selected endpoints, active connection count, and capture window separately. Do not
  repeatedly or concurrently log in the same formal account during capture.
- Context-compaction policy: `/compact` is lossy session-context management, not evidence
  persistence. Before using it, persist exact observations, fixture paths/hashes, commands, account
  class, time window, failed hypotheses, and next tests to the authoritative docs/skill. After
  compaction, re-read those sources; never treat a compacted summary as protocol evidence or a
  reason to bypass the evidence gate.

Agent-first contract: classify the request, route to one owner below, prove the route with a
symbol/route/test, change the smallest owner, run a focused check plus the user-visible check, then
report files, commands, evidence, uncertainty, and recovery. 无法路由时：先运行项目自检/健康命令，
搜索用户词的注册点和测试名，输出证据缺口，不猜测文件并不修改。

## 2. Fast routing table

| User wording | Capability | Authoritative owner | First inspection | Verification |
|---|---|---|---|---|
| 全推 / 官方全推 / true push / Wine callback | Formal-account vendor full-push | `../crates/netzip-fullpull` + auth/post-login integration | `src/auth_7100_client.rs`, 5188 capture evidence, Wine callback contract | auth + post-login connection + callback parity + live freshness |
| 补数 / 补数据 / 7709 / K线 / F10 / 0547 / OEM | Query supplementation | `../crates/netzip-supplement` + service handlers | `tdx7709_supplement*`, `netzip_supplement::`; inherited modules currently in fullpull | supplement tests + targeted HTTP contract test |
| 7709 全推 / native push / push-worklist | Legacy transition publisher, semantically supplementation | `src/bin/netzip_service.rs` + `scripts/run-full-push.sh` | `execute_hqw_push_worklist`, `/api/hqw/push-worklist` | label as legacy; never accept as official full-push evidence |
| auth / 7100 / 5188 / endpoint discovery | Official full-push control/data chain | `src/auth_7100_client.rs` + `netzip-fullpull` | login state, discovered endpoints, post-login data frames | formal-account login without secret disclosure; long-lived data delivery |
| Web GUI / panel / debug API | Internal diagnostics | `src/bin/netzip_service.rs`, `webgui/` | Router registrations and `webgui/index.html` | user-visible GUI route |
| service / systemd / install | Deployment runtime | `deploy/*.service`, `scripts/install-service.sh`, `scripts/run-full-push.sh` | service names and environment file | install script test + service health |
| Wine/capture/protocol evidence | Research and regression | `docs/forensics/`, `PROTOCOL_NOTES.md` | referenced fixture hashes | do not treat history as runtime truth |

## 3. Feature implementation map

Capability: official full-market push
User terms: 全推, 官方全推, true push, Wine callback
Authoritative owner: `netzip-fullpull` plus quoteNetzipRs product integration
Target flow: formal account -> authentication/control route -> non-7709 vendor data connections ->
server-driven updates -> Wine-equivalent decoding/callback -> quoteGateway
Current implementation: incomplete. Authentication exists and `netzip-fullpull::official_5188`
now provides the evidence-backed TCP frame boundary, reassembler, and direction classifier, but the
authenticated 5188 initialization bytes, inner object decoder, and callback-equivalent resident
integration are not complete.
Non-evidence: 7709 0547 polling, unsolicited 7709 frames, worker sharding, and
`quote-netzip-rs-full-push.service` naming do not prove this capability.
Focused verification: authentication status, post-login socket/protocol evidence, decoded callback
parity, and live quoteGateway freshness/coverage.
Confidence: target confirmed; Rust completion pending.

Capability: internal supplementation
User terms: 补数, K线, OEM, split, finance, realtime
Authoritative owner: `netzip-supplement` crate plus service boundary handlers
Entry points: `/api/supplement/*` (internal/diagnostic)
Implementation symbols/files: `netzip_supplement::run_supplement`,
`tdx7709_supplement`, OEM handlers and tests
Data flow: explicit 7709 endpoint -> query/052d/0547/parsers -> JSON or packed OEM response
Non-owners: it does not own formal-account vendor full-push
Focused verification: `cargo test --manifest-path ../crates/netzip-supplement/Cargo.toml`
Confidence: confirmed
Evidence: Cargo dependency and route registrations/tests.

Capability: inherited 7709 compatibility modules
User terms: 0547, 7709 framing, code table, FIN, `NativeSession`
Target owner: `/home/codes/stock/crates/netzip-supplement`
Current physical location: `/home/codes/stock/crates/netzip-fullpull/src/tdx*.rs` and compatibility
re-exports. Do not infer ownership from this temporary layout.
Migration rule: avoid adding new 7709 responsibilities to `netzip-fullpull`; move them toward the
supplement crate in reviewable steps while preserving consumers.
Focused verification: both shared crate test suites and quoteNetzipRs integration tests.
Confidence: semantic owner confirmed; physical migration pending.

## 4. Decision tree

1. Does the request require the formal-account, server-driven Wine realtime chain? Use
   `netzip-fullpull`; require non-7709 post-login evidence.
2. Does it contact 7709 or actively request a symbol/range? Use supplementation, even when the
   output is realtime-shaped or a legacy service calls it push.
3. Is inherited 7709 code still physically in fullpull? Record both current location and target
   supplement owner; do not redefine fullpull around the legacy layout.
4. Is it login/auth/5188 endpoint selection? Change auth and fullpull integration, not supplement.
5. Is it service lifecycle? Change service files/scripts and queue deployment through webClx.
6. Is it only historical evidence? Update the owning evidence document; do not infer runtime behavior.

## 5. Change procedure

1. Reproduce or cite the failing contract; inspect the routing owner above.
2. State the smallest ownership boundary before editing.
3. Preserve dirty user work and avoid generated/reference directories.
4. Implement; update current documentation, not immutable historical evidence.
5. Run focused tests, then format/check/build through webClx.
6. Inspect diff and verify the user-visible path. Runtime changes require health/capability evidence.

Minimal path:

```bash
rg -n 'auth_7100|5188|post.login|callback' src docs
rg -n 'execute_hqw_push_worklist|/api/hqw/push-worklist' src/bin/netzip_service.rs
rg -n '7709|netzip_supplement' src ../crates/netzip-supplement
cargo test --manifest-path ../crates/netzip-fullpull/Cargo.toml
# Queue project checks through webclx-compile-and-deploy; do not bypass it.
```

## 6. Verification matrix

- Official full-push: formal login, non-7709 post-login data connection, continuous callback parity,
  quoteGateway freshness and coverage.
- Supplementation: netzip-supplement tests, inherited 7709 compatibility tests, and focused service
  handler tests.
- Legacy publisher: tests containing `full_push` / `native_push` verify only transition behavior.
- Transition capability report: the existing
  `capabilities_mark_only_full_market_push_as_public_service` test verifies the historical API
  declaration only; it is not official full-push acceptance.
- Runtime: `GET /health`, `GET /api/capabilities`, service active state, quoteGateway source metrics.
- Static boundary: no runtime the former crate name references and no built-in unauthenticated fixed 7709
  endpoint in current source/scripts.

## 7. Build/deploy/runbook

- Pure compile/check: use `webclx-compile-and-deploy` Mode 1 from this repository.
- Deployment: use the project script through webClx Mode 2/3; installation owns backup and restart.
- Runtime environment: `/etc/default/netzip-rs`; credentials must be referenced by protected runtime
  configuration and never copied into the skill, source, documentation, logs, or responses.
- Rollback: project install script keeps `/home/bin/netzip/quoteNetzipRs.bak`; service files can be
  restored from git while re-running `scripts/install-service.sh`.

## 8. Known traps

- `stable_endpoints` lists internal diagnostics; `public_service_endpoints` is the external contract.
- Historical captures may legitimately contain old server addresses; they are not runtime defaults.
- `netzipRust7709` explicitly identifies the legacy 7709 source; it is not official full-push.
- `netzip-fullpull` is the renamed crate for official full-push, despite its current inherited 7709
  modules.
- Never call a 7709 endpoint “authenticated full-push” merely because it was discovered after login.
- Do not silently make 7709 the fallback for official full-push; degradation must be explicit.
- Cargo target and `.agent-project-rename-backups` hits are generated/history, not source references.

## 9. Maintenance protocol

Refresh this skill when routes, service units, shared crate ownership, endpoint policy, or public
contract change. Every new route must name one owner and one acceptance check. Mark unproven routes
`needs-verification`; do not replace evidence with plausible paths.

## 10. History-derived regression routes

Capability: source identity collision
Symptom: Wine and Rust data are compared as one source.
Current owner: quoteGateway publisher in `src/bin/netzip_service.rs`.
Diagnosis: search `netzipRust7709` and `quoteNetzipWine`.
Acceptance: Rust publishes only `netzipRust7709`; tests reject Wine fallback.
Confidence: confirmed.

Capability: full-push/supplement collision
Symptom: a 7709 endpoint pool or `0547` worker is reported as official full-push.
Current owner: documentation/capability reporting plus the legacy resident publisher.
Diagnosis: inspect upstream port, authentication lifetime, request cadence, and whether delivery is
server-driven after initialization.
Acceptance: 7709 is labeled supplementation/legacy publication; official full-push requires the
formal-account non-7709 chain.
Confidence: confirmed.

Capability: public/internal contract drift
Symptom: consumers begin relying on supplement/debug APIs.
Current owner: `public_service_endpoints` and README service boundary.
Diagnosis: compare Router registrations with capability lists.
Acceptance: capability output labels the 7709 resident path as transition/supplementation and does
not claim official full-push until the authenticated non-7709 chain passes its evidence gate.
Confidence: confirmed.
