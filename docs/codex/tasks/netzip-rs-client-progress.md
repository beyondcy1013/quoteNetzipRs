# NetzipRs Market Client Progress

Updated: 2026-08-31

## Objective

Deliver a production-capable native Rust Netzip market-data client that supplies correct, complete,
timely, and observable quotes to quoteGateway without requiring Wine for the supported Linux path.
Wine remains a comparison anchor until native authentication, initialization, and post-login
business behavior have equivalent evidence.

## Current Delivery State

- The native `7709` route supports code-table synchronization, live quotes, K-line, F10, FIN, and
  `0547` parsing.
- Historical supplementation is now separated into the shared `netzip-supplement` business crate.
  `POST /api/supplement/kline` exposes Wine-configured daily, five-minute, and one-minute defaults
  over paged `0x052d` requests with dedupe, throttling, and per-item partial-failure status.
- `netzip_linux` and the local HTTP service expose the Linux-first snapshot and focused query paths.
- Full-market Shanghai/Shenzhen push and the lightweight Beijing polling lane publish into
  quoteGateway through `netzip-rs-full-push.service`.
- Rust account `1522` authentication to `121.41.70.217:7100` completed successfully in three
  independent TCP sessions. Success requires the response sequence
  `zstd_dictionary -> download_file -> zstd_dictionary`.
- The vendor Wine chain also uses account `1522`. Short Rust authentication sessions succeeded while
  Wine remained connected, with no immediate kick or rejection observed.
- Commit `28ea978` replaced thread-keyed gateway TCP client slots with bounded lane/address slots.
  The deployment replaced the leaked process, and immediate checks remained at `fd=7` and
  `CLOSE-WAIT=0`.
- The 2026-08-07 morning session kept NetzipRs near 68 file descriptors with no persistent
  NetzipRs-owned `CLOSE-WAIT`, no `Too many open files`, and zero main/BJ TCP publish or ACK
  failures. Session-boundary evidence showed 53 shards reconnecting together: eight initial quote
  requests lacked a decodable 0547 frame at 09:42:11, and five bootstraps received only two of three
  frames at 09:46:21. Native shard startup and retries are now deterministically staggered to remove
  that reconnect burst. The first deployment also proved that cancelling the systemd-owned curl did
  not cancel its `spawn_blocking` readers: a rapid full-push service restart briefly created two
  independent 53-shard sessions. The resident endpoint now holds an atomic worker-owned lease and
  rejects overlapping full-push requests with HTTP 409 until the original worker actually exits.
  Deployment `095434-18c8f2f31ead11e3` proved the lease under the real systemd double-start: the
  second caller received 409 until the orphaned worker exited, then the managed script completed a
  257-second session with `fd=76`, NetzipRs-owned `CLOSE-WAIT=0`, no open-file or ACK errors, five
  reader failures and five recoveries, zero main/BJ publish failures, and zero TCP fallbacks.
- The final 2026-08-13 live deployment moved gateway ACK before downstream broadcast, applied the
  configured TCP ACK timeout to persistent clients, expanded the runner budget to 330 seconds, and
  stopped the current full-push unit before restarting the main service. Two consecutive sessions
  completed in 263.894s and 271.610s with zero ACK errors, sequence mismatches, HTTP fallbacks,
  HTTP 409 responses, publish failures, or reader failures. Main/BJ/manual TCP metrics were
  2,934/2,934, 321/321, and 673/673 cumulatively after the second session. One audit failure was
  counted without interrupting publication and still needs detailed cause instrumentation.
- The same live run separated the remaining bottleneck from Netzip transport: quoteGateway's
  5,543-code merged WebSocket client reached 116.4s p99 event age with 3ms socket-send p99, while a
  177-code native client stayed at 3.74s p99. The gateway report is
  `/home/codes/stock/quoteTdx/docs/history/2026-08-13-open-market-push-profile.md`.
- The 2026-08-31 native-push deployment raised the default worker ceiling and converged it to the
  53 subscription shards. A normal five-endpoint session completed in 278.780s with
  `endpoint_pool_size=5`, `endpoint_failovers=0`, `worker_count=53`, and `shard_count=53`.
  Reader, recovery, publish, audit, and Beijing-poll failure counters were all zero. Main, Beijing,
  and manual publication completed 622/622, 116/116, and 228/228 TCP sends with no HTTP fallback.
- A controlled two-endpoint run put an unreachable endpoint first. All 53 workers failed over once
  and recovered (`endpoint_failovers=53`, `reader_failures=53`, `reader_recoveries=53`) without a
  publish, audit, Beijing-poll, TCP, or ACK failure. Deployment `112317-18d09f02146d4728` then
  restored the built-in five-endpoint pool. The gateway's cumulative `ingest_failures=1` remained
  unchanged across consecutive post-restore samples while message sequence and receive timestamps
  continued advancing, so the single failure is bounded to the deployment transition window.
- The restored process held all 53 full-push worker connections on the default first endpoint
  `120.195.71.160:7709`. Replacement is still blocked: the quality report had
  `peer_comparable_count=0`, `evidence_gate_passed=false`, and `can_replace_all=false`. Rust covered
  5,545 symbols (99.89%) but event-to-arrival latency was p50/p95/p99 21.187/45.192/45.314s versus
  Wine's 2.002/4.970/5.443s. Wine must remain online as the comparison source.

## Open Problems

### Completed: Verify the TCP cache and ACK fixes under trading load

The previous process reached its 1024-descriptor soft limit with approximately 922 `CLOSE-WAIT`
sockets. The live 2026-08-13 acceptance retained three established gateway TCP client connections,
zero publisher-owned `CLOSE-WAIT`, bounded descriptors, and zero ACK/transport failures across two
complete sessions. The remaining gateway CPU, WebSocket lag, and allocator retention are downstream
gateway concerns rather than an unresolved Netzip TCP cache failure.

### P1: Integrate native 7100 authentication into the resident service

Authentication is currently exposed through `auth_7100_client` and `auth_7100_login`. The resident
`netzip-rs` production chain still uses `7709` directly and does not own a long-lived 7100 session,
credential-state reporting, reconnect policy, renewal behavior, or startup authentication gate.

### P1: Complete the post-login initialization and business chain

The native implementation does not yet fully replace the vendor `Tdx_Encrypt / penc / ZSTD`
post-login shell, local `2000` bridge, or the complete code-table, corporate-action, finance, file,
and real-time initialization object sequence.

Historical bar supplementation is no longer part of this gap: its daily/five-minute/one-minute
business layer is implemented separately. Corporate-action, finance, and file refresh remain in
this initialization gap.

### P1: Prove long-lived same-account behavior

Short overlap between Wine and Rust account `1522` sessions caused no immediate conflict. Long-lived
parallel sessions have not yet proved whether the vendor backend applies delayed session eviction,
resource limits, or account-level exclusivity.

### P1: Close the Wine replacement evidence and latency gap

The native route has stronger standalone coverage, a proven five-endpoint pool, and controlled
failover recovery, but the 2026-08-31 quality snapshot still had no peer-comparable samples and was
materially slower than Wine. Keep Wine enabled until `can_replace_all=true`, peer-comparison coverage
passes the configured evidence gate, no final codes are lost, and Rust latency is competitive over a
representative open-market window.

### P2: Complete legacy client compatibility

The Rust compatibility surface is not a drop-in replacement for the production FoxTrader/Stock.dll
contract. `GetTradeData`, daily and 1/5-minute bars, code-table behavior, and real pointer/callback
contracts still require controlled Windows validation.

## Verification Evidence

- Native 7100 authentication implementation: `src/auth_7100_client.rs`
- Authentication CLI: `examples/auth_7100_login.rs`
- Resident service and gateway transport: `src/bin/netzip_service.rs`
- Operational history and root causes: `docs/EXPERIENCE.md`
- Authentication completion commit: `0945fb9`
- TCP client cache fix commit: `28ea978`
- Workspace regression request: `151138-18c8f2f31ead11bc`
- Deployment request: `151328-18c8f2f31ead11be`
- Five-endpoint restore deployment: `112317-18d09f02146d4728`

## Acceptance Checklist

- [x] Native account `1522` completes the verified 7100 authentication response chain.
- [x] Workspace tests and the authentication CLI build pass through webClx.
- [x] Gateway TCP cache is bounded by lane and address, with a cross-thread regression test.
- [x] Fixed binary is deployed under `/home/bin/netzip/` and both Rust services are active.
- [x] A complete trading session keeps fd and `CLOSE-WAIT` counts bounded.
- [x] Shanghai/Shenzhen readers recover transient session-start failures without descriptor-related
  failures.
- [x] Beijing polling and gateway ACK publication remain healthy throughout the same session.
- [x] Default five-endpoint operation and one-failure-per-shard controlled failover are proven under
  a complete live session, followed by restoration to the default pool.
- [ ] Rust passes the quoteGateway replacement evidence gate and reaches competitive Wine-relative
  event-to-arrival latency over a representative open-market window.
- [ ] Resident 7100 authentication and reconnect state are visible through service status.
- [ ] Long-lived Wine/Rust account overlap is either proven safe or replaced by an explicit ownership
  policy.
