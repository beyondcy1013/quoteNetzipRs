# Wine formal-account full-start capture (2026-09-01)

## Scope

- Account class: formal.
- Capture window: approximately 14:37:17 to 15:06:52 Asia/Shanghai.
- Runtime configuration observed: authentication enabled, primary login
  disabled, backup login enabled.
- Purpose: preserve the late-session Wine startup, callback and supplement
  traffic before market close.

## Confirmed

- The connection timeline contains 7100 and many parallel 7709 connections but
  no 5188 connection. This is consistent with the primary-login switch being
  disabled and must not be labeled official full-push evidence.
- The resilient callback collector retained 80 JSONL rows representing 38
  unique sequence values. All raw payload lengths are zero; these rows preserve
  status/control timing, not OEM quote bytes.
- The Wine-host 7709 capture contains 1,213,326 packets over about 23 minutes.
  It is useful for supplementation cadence and connection-count analysis.

## Fixtures

| Fixture | Size | SHA-256 |
|---|---:|---|
| `diagnostics/20260901-wine-full-start/captures/wine-full-start-formal-20260901T1437.pcap` | 6,547,538 | `34eb92a4eaead1e61ec3b1fe09e5b83622d445710626c008899d2ace37775a68` |
| `diagnostics/20260901-wine-full-start/captures/wine-full-start-formal-7709-late-20260901T1442.pcap` | 28,274,088 | `89fcab0d3cf9457fcdc18faf77d924bc695cb29bebd668ce2b0969a608d88213` |
| `diagnostics/20260901-wine-full-start/captures/wine-full-start-formal-7709-winehost-20260901T1443.pcap` | 346,937,378 | `6ab06debb3b2eed33846554d34f0d57504374dbcc6b5a9d2e24fe48bc2dab2a6` |
| `diagnostics/20260901-wine-full-start/callbacks/wine-events-formal-resilient-20260901T1444.jsonl` | 17,225 | `f7374db4d87612ebaff9292c9607779907a70573268c66bd67df445b9c6d50ac` |
| `diagnostics/20260901-wine-full-start/connection-timeline-formal-20260901T1444.txt` | 1,842,194 | `494a308fa78d484e1b3f90aabe01c763d5187c643f5301455a4ddad4e4e05c60` |

## Disposition

- **Confirmed:** primary-login-disabled Wine uses backup/control plus 7709
  supplementation and did not establish the observed official 5188 chain.
- **Needs-verification:** whether the large Windows all-port ETL in
  `captures/2026-09-01_full-session/` contains a separate primary-enabled
  attempt; it requires correct ETL conversion first.
