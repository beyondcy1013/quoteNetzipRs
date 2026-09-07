# Live-Market Capture Inventory

Captured traffic is retained for replay and comparison. Payloads may contain
sensitive account or market data; do not copy them into logs, tests or public
documentation. Hashes identify the exact local fixture.

| Fixture | Size | SHA-256 | Use / caveat |
|---|---:|---|---|
| `diagnostics/20260831-netzip-windows-vs-rust/captures/vendor_pm.pcapng` | 131,844,096 | `b9d3b2b4a3e766ee45fd453b6ee745611328d56bd665982ce5c37284ab0a2ad3` | Main afternoon Wine 5188 stream; long-lived server-driven delivery |
| `diagnostics/20260831-netzip-windows-vs-rust/captures/vendor_lunch.pcapng` | 4,924,648 | `1c71f68b6de87d29761348e12204418fda6dabce0f929681db22ffb3f84fcb48` | Independent 7100 cold-start chain with ten login/ABK/ACK objects; no captured client 5188 initialization frames, so useful for ACK-template stability but not ACK-to-`2d10` pairing |
| `docs/forensics/7100-auth-20260806/sanitized_login_success.pcapng` | 8,934,128 | `3afbe856b78a04919b47eef1e961a222d3bb97fe0741c0f0cab41533a656dbfb` | Redacted formal-login fixture |
| `docs/forensics/7100-auth-20260806/sanitized_login_reconnect.pcapng` | 35,744 | `98471162e66ee0c90c9623b62fdf9b7b60e8ec66c2497e0834e14fae160e82f8` | Redacted reconnect/control comparison |
| `captured_windows_traffic/capture_netzip_full.pcapng` | 16,405,420 | `2a033d8c4663bdab642cd0791c053873c306cd989f69b97a11a8def377f9c70d` | Historical broad Wine traffic; protocol context only |
| `diagnostics/20260728-0547/wine-noon-1154.pcap` | 202,854 | `50a72338787e500103013d53c9be504d3eb3272e6a8164ecf81644ae7960e4de` | 7709/0547 supplementation semantics; not 5188 evidence |
| `captures/2026-09-01_full-session/official_full_session.pcapng` | 1,362,636 | `0da9d5fe54b9cdfbfacd490f46004da0123e7e3266053cdbd0c2c46ab2ebec87` | Windows pktmon raw-IPv4 capture mislabeled as Ethernet; generated 7100 matrix has zero packets/payload bytes, so using it as an independent ACK/`2d10` session is rejected |
| `captures/2026-09-01_full-session/official_full_session.etl` | 3,093,551 | `a54319192210e096778a92348943cdffd537a2f3dab1be80730483da6b76f712` | Source ETL for the preceding pcapng; retain for conversion/parser checks |
| `captures/2026-09-01_full-session/official_all_ports_followup.etl` | 1,073,741,824 | `0c3c7c3cf940dca97e00306330f93b4914c3afe6127f7bc4d46159019fd97535` | Large all-port follow-up; needs conversion and bounded endpoint inventory before protocol conclusions |

Conversion requirement: this Windows pktmon ETL must be converted on a Windows
host with `pktmon etl2pcap` (using the original capture settings) before Rust
replay. The current Linux workspace has no pktmon/etl2pcap converter; until a
converted pcapng is hash-recorded here, endpoint coverage and any claimed 5188
primary-login flow remain `needs-verification`.

New live fixture `diagnostics/20260901-live-pair/wine-7100-5188-20260901T191241.pcap`
was captured while the local Wine host had an authenticated `7100` connection
and ten established `5188` connections. Capture duration was 30 seconds,
with 108 packets and no kernel drops; SHA-256 is
`49aa350c4e9eede1f2b02d3ae58c1b97b20f74674e80f9ddf02798a915e25ce6`.
This is a transport/cadence fixture. No synchronized callback JSONL was
attached during this short capture, so business-field parity remains
`needs-verification`.

The concurrent collector was exercised under
`diagnostics/20260901-live-pair/concurrent-1938/`: 952 callback JSONL events
and a synchronized pcap. Rust replay found 10 flows, three `3901` status
frames and zero complete business frames; the callback-window index produced
zero candidates. This is a valid negative cadence result: synchronization
works, but the short interval contained no reassembled 2704/3e04 market frame.

The synchronized attempt at 19:34 produced
`wine-primary-sync-20260901T193421.pcap` (SHA-256
`bcb685d624bb7e789eb1a6b97293651b1e2aa27040a53ef1f642f4b014ebe026`) and
`callbacks/wine-events-primary-sync-20260901T1934.json` (SHA-256
`0c90ce659b7c654260649ed82a65d27757e74378e7f08695b464baffa2aa2c10`). The
pcap began around 19:34:21, while the event ring's latest callback was
19:33:53, leaving roughly 28 seconds without overlap. This demonstrates that
post-capture polling of the bounded event ring is insufficient; future pairing
must stream `/api/v1/events` concurrently during packet capture.

Rust release replay of the primary-window pcap produced
`diagnostics/20260901-live-pair/primary-5188-summary.json`: 20 flows, 38
complete server frames, and observed wire kinds `3901`, `3a01`, `5104`, and
`3e04`. The callback-window tool now accepts the Wine API `{"data":[...]}`
wrapper and `timestamp_ms`; the generated index
`diagnostics/20260901-live-pair/callback-window-index.json` has
`candidate_count=0` for a 30-second window. This proves the captured callback
ring and pcap still do not overlap in time; it does not invalidate either
fixture.

The recovered Wine API event ring was persisted at
`diagnostics/20260901-live-pair/callbacks/wine-events-primary-20260901T1930.json`.
It contains 62 events, including 32 `股票数据` callbacks totaling 26,545,458
raw bytes, covering timestamp milliseconds `1788262229770..1788262255560`.
The callback objects identify `source_protocol=wjf.oem_report.v5` and include
market/code/quote fields. The API status still reports
`effective.login_primary=false`, so this is a real Wine callback fixture but
its primary-server provenance is unresolved.

At 2026-09-01 19:30 the recovered Wine host API reported 27 received batches,
18,314 received quotes, 23 successful gateway posts, zero dropped batches and
HTTP 200 status. The event stream contained multiple `股票数据` callback
records with nonzero raw lengths. The same status response still reported
`effective.login_primary=false`, so these callbacks prove live Wine data
delivery but not primary-server provenance; source classification remains
`needs-verification`.

After directly writing the primary-enabled UTF-16 setting and starting the
vendor process with config preparation skipped, a new formal-account capture
was collected while `网际风.exe` held ten established 5188 sockets:
`diagnostics/20260901-live-pair/wine-primary-5188-20260901T192820.pcap`.
The 25-second fixture contains 688 packets, zero kernel drops, and SHA-256
`e7504fddc3ba28dc9579aa12b7fbfd2b525fdbfc11940d6d7b93217e8dac83fa`.
This is the first post-primary-switch transport fixture. The host API was not
available during capture, so callback parity and gateway quote counts remain
unverified.

At inspection time the running Wine host API `/api/v1/status` reported
`login_primary=false`, `gateway_forwarder.received_quotes=0`, and
`successful_posts=0`. Therefore the observed ten 5188 sockets in the live
fixtures cannot be classified as a verified primary official full-push market
session. They remain transport evidence only; socket presence alone is not
authentication or business-data success.

An independent 60-second live capture was collected from the same running
formal Wine host:
`diagnostics/20260901-live-pair/wine-7100-5188-20260901T191511-60s.pcap`.
Its SHA-256 is
`bccda4d517ea3c63db6ef39b66566e4187ff5ff0cd2e5d8260b23f1f70e80d97`.
It is retained for Rust replay and cadence comparison; no callback stream was
captured in the same process window, so field parity remains unresolved.

Bounded inspection of the live fixture shows recurring 10-byte server payloads
with wire prefix `39 01` (`0x0139` decoded kind) across multiple 5188 sockets.
This is an observed control/status cadence sample only; it is not promoted to
the 5188 business-kind table without complete-frame reassembly and callback
correlation.

The formal Wine callback JSONL under
`diagnostics/20260901-wine-full-start/callbacks/` timestamps its startup around
14:39 (Asia/Shanghai), while `captures/2026-09-01_full-session/connection_timeline.csv`
starts the corresponding localhost timeline around 14:56. These capture
windows do not overlap, so they cannot support same-session `OEM_REPORT` to
5188 payload pairing. This is a timing exclusion, not evidence about the wire
decoder.

Runtime check with the current `target/release/quoteNetzipRs` (request
`184516-18d115af25cb040f` artifact) reached `/api/debug/pcap-5188-summary`, but
returned `flows=0` for this pcapng. The result is classified as a parser/link-
type failure, not evidence of absent 5188 traffic: tcpdump shows the file is
labelled Ethernet while its records contain bare IPv4 bytes.

Follow-up conversion to `/tmp/allports-converted.pcap` produced a 340,044,456-
byte classic pcap (tcpdump reported only a truncated terminal pcapng block).
The converted records are readable and begin with bare IPv4 packets, but
`tcpdump -r ... 'tcp port 5188'` returns no records; sampled traffic is SMB and
other non-5188 traffic. Thus this specific all-port fixture is now classified
as `no-observed-5188-in-file`, while the separate `vendor_pm.pcapng` remains
the authoritative 5188 full-push capture.
| `captures/2026-09-01_full-session/connection_timeline.csv` | 13,929 | `dd1f4422b992affb106577f10870bb73e68af9ca07a1038d9dba2f60dbed432b` | Companion timeline; sampled rows currently contain local control sockets only, so it cannot establish remote 5188 by itself |
| `captures/2026-09-01_full-session/official_full_session_7100_flow_matrix.json` | 512 | `d249be60b08897224bc1d67d01a41cab2536c2dbe8fd2a4bc24f0c3319eee5ba` | Existing analysis output records a placeholder 7100 flow with zero unique packets and zero payload bytes; not protocol evidence |
| `diagnostics/20260901-wine-full-start/captures/wine-full-start-formal-20260901T1437.pcap` | 6,547,538 | `34eb92a4eaead1e61ec3b1fe09e5b83622d445710626c008899d2ace37775a68` | Formal-account Wine late-session startup; primary login was disabled, so this is control/supplement boundary evidence rather than 5188 evidence |
| `diagnostics/20260901-wine-full-start/captures/wine-full-start-formal-7709-late-20260901T1442.pcap` | 28,274,088 | `89fcab0d3cf9457fcdc18faf77d924bc695cb29bebd668ce2b0969a608d88213` | Short synchronized 7709 supplement capture |
| `diagnostics/20260901-wine-full-start/captures/wine-full-start-formal-7709-winehost-20260901T1443.pcap` | 346,937,378 | `6ab06debb3b2eed33846554d34f0d57504374dbcc6b5a9d2e24fe48bc2dab2a6` | About 23 minutes and 1,213,326 packets of Wine-host 7709 traffic; no 5188 claim |
| `diagnostics/20260831-netzip-windows-vs-rust/analysis/official-5188-control-correlation.json` | 203,802 | `cc07d6dc74abb9351dd3960db093ffa00573d1672a496e8a6b963d7b33edf47f` | Schema v7 metadata-only 7100 request/response-to-5188 correlation; no control bodies |
| `diagnostics/20260831-netzip-windows-vs-rust/analysis/vendor_pm_7100_flow_matrix.json` | 917,476 | `6eb08e919834076b07341108bb5bed2d24294fcf23f84f89b248bfbe6fa96ee5` | Regenerated 7100 structural catalog; sensitive values and fingerprints are omitted |

The detailed 5188 analysis reports a representative connection lasting about
3355 seconds, with 6–8 client initialization frames and thousands of server
frames. These figures are capture-specific, not protocol constants.

Replay verification: webClx request `132432-18d115af25cb03d0` rebuilt the
scanner after correcting little-endian kind constants and excluding empty TCP
segments from application reassembly. The ignored Rust replay then classified
the long-lived server flows (`server_frames > 0`) and preserved raw wire kinds;
inner payloads remain opaque pending same-symbol Wine callback matching.
