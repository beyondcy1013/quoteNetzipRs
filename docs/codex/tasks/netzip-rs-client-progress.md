# NetzipRs Market Client Progress

Updated: 2026-08-06

## Objective

Deliver a production-capable native Rust Netzip market-data client that supplies correct, complete,
timely, and observable quotes to quoteGateway without requiring Wine for the supported Linux path.
Wine remains a comparison anchor until native authentication, initialization, and post-login
business behavior have equivalent evidence.

## Current Delivery State

- The native `7709` route supports code-table synchronization, live quotes, K-line, F10, FIN, and
  `0547` parsing.
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

## Open Problems

### P0: Verify the TCP cache fix under trading load

The previous process reached its 1024-descriptor soft limit with approximately 922 `CLOSE-WAIT`
sockets, causing push shards, Beijing polling, quoteGateway publication, and ACK handling to fail.
The code-level cause is fixed and covered by a cross-thread regression test, but the new process was
deployed after the trading window. Acceptance still requires a live session showing bounded fd and
socket counts with no new `Too many open files` errors.

### P1: Integrate native 7100 authentication into the resident service

Authentication is currently exposed through `auth_7100_client` and `auth_7100_login`. The resident
`netzip-rs` production chain still uses `7709` directly and does not own a long-lived 7100 session,
credential-state reporting, reconnect policy, renewal behavior, or startup authentication gate.

### P1: Complete the post-login initialization and business chain

The native implementation does not yet fully replace the vendor `Tdx_Encrypt / penc / ZSTD`
post-login shell, local `2000` bridge, or the complete code-table, corporate-action, finance, file,
and real-time initialization object sequence.

### P1: Prove long-lived same-account behavior

Short overlap between Wine and Rust account `1522` sessions caused no immediate conflict. Long-lived
parallel sessions have not yet proved whether the vendor backend applies delayed session eviction,
resource limits, or account-level exclusivity.

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

## Acceptance Checklist

- [x] Native account `1522` completes the verified 7100 authentication response chain.
- [x] Workspace tests and the authentication CLI build pass through webClx.
- [x] Gateway TCP cache is bounded by lane and address, with a cross-thread regression test.
- [x] Fixed binary is deployed under `/home/bin/netzip/` and both Rust services are active.
- [ ] A complete trading session keeps fd and `CLOSE-WAIT` counts bounded.
- [ ] Shanghai/Shenzhen readers show stable recovery and no descriptor-related failures.
- [ ] Beijing polling and gateway ACK publication remain healthy throughout the same session.
- [ ] Resident 7100 authentication and reconnect state are visible through service status.
- [ ] Long-lived Wine/Rust account overlap is either proven safe or replaced by an explicit ownership
  policy.
