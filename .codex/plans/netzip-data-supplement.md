# Task: Reproduce Netzip Data Supplementation

## Highest Goal

Expose evidence-backed historical market-data supplementation from the native Rust service without
conflating it with the separate realtime push or startup reference-data synchronization paths.

## Acceptance Criteria

- [x] Keep `netzip-fullpull` as the shared native protocol/transport crate used by quoteNetzipRs and
  tdxRs.
- [x] Add a sibling `netzip-supplement` crate under `/home/codes/stock/crates`.
- [x] Support the Wine-confirmed daily, five-minute, and one-minute supplementation periods.
- [x] Preserve the Wine-configured default counts: 1000 daily, 3000 five-minute, 2000 one-minute.
- [x] Paginate 7709 `0x052d` requests, deduplicate/sort bars, throttle requests, and report partial
  failures without hiding them.
- [x] Add a quoteNetzipRs HTTP endpoint that exposes the crate without duplicating protocol logic.
- [x] Record the evidence matrix and distinguish confirmed behavior from remaining live-session
  acceptance work.

## Evidence Matrix

| Behavior | Official/Windows evidence | Capture/runtime evidence | Rust counterpart |
|---|---|---|---|
| Realtime push | `OEM_REPORT`, `type=实时数据` | init object #9 and open-market callbacks | `netzip-fullpull` 0547 modules |
| Daily supplement | `股票数据?请求=日线...` | `probe_run_20260329_v3.txt`, exact 200 + 3*32 reply | 7709 K-line category 4 |
| Five-minute supplement | Wine config `补5分钟根数=3000` and install notes | Windows behavior notes identify the extension toggle | 7709 K-line category 0 |
| One-minute supplement | official `1分钟线` request and Wine config count 2000 | exact 200 + 3*32 probe reply | 7709 K-line category 7 |
| Startup reference data | init callback sequence | complete object catalog: code tables, split, finance, file, realtime | existing parsers; not historical-bar supplementation |

## Tasks

1. Build supplementation domain and executor crate - L
2. Add focused offline pagination/validation tests - M
3. Integrate HTTP request/response route - M
4. Update evidence and API documentation - M
5. Run webClx compile/test verification and inspect the final diff - M

## Dependencies

- `netzip-supplement` depends on `netzip-fullpull`; the reverse dependency is prohibited.
- The HTTP integration depends on the supplementation crate API.

## Risks

- The current time is outside an open-market session. Historical 052d requests can be exercised,
  but open-market coexistence and load acceptance remain separate runtime gates.
- The worktree already contains substantial user changes, including shared-crate extraction. Edits
  must be additive and must not revert those changes.
- Large bulk requests can overload the upstream or produce oversized JSON. The public endpoint must
  enforce explicit symbol and bar-count bounds.
