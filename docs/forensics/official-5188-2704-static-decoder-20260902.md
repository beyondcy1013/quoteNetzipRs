# Official 5188 `2704` static decoder evidence

- Status: partially confirmed
- Last verified: 2026-09-02
- Project: `quoteNetzipRs`, shared crate `netzip-fullpull`
- Binary SHA-256: `222ee644a99edb82da46d0bee3d858a935076aecf085d6cd8fde354313452418`
- Paired PCAP SHA-256: `fa79bc77b81fb83a8a5b8eca9f3d8e00a08b8f241628cf9d4dcbed2b8f79a8a3`

## Confirmed

Wine function `0x44aa30` accepts wire opcode `0x0427` and reads the payload as:

```text
u16 record_count
u32 value_end_offset
value_bits = payload[6..value_end_offset]
index_bits = payload[value_end_offset..payload_len]
```

`value_end_offset` is measured from the payload start. At `0x44aa99` Wine uses
`payload_len - value_end_offset` for the index reader; at `0x44abce` it uses
`value_end_offset - 6` bytes beginning at `payload + 6` for the value reader.

The index pass initializes one 311-byte (`0x137`) internal record per count and
restores four state values: market, symbol index, timestamp and whether the
value pass uses a baseline record. `0x448a10` reads bits MSB first. `0x448b90` and
`0x448d10` use 13-byte prefix-token entries. The tables used by the index pass
are at `0x5b2318` (market), `0x5b22fc` (market slot), and `0x5b22b8` (code-table
index). Rust reproduces these tables without reading or executing the vendor
binary at runtime.

The second pass switches the same reader to the value bitstream, reads an
8-bit record mask, then calls `0x449770` per record. The callback-facing array
is still based on 311-byte internal records; it is not a plain 500-byte
`OEM_REPORT` wire array.

Replaying the paired PCAP through the corrected exporter decoded all 15
complete `2704` frames and all 2,102 declared records without an index-stream
overrun. Markets resolved to `SH` and `SZ`. The restored 32-bit values include
`1788246000`, which is exactly `2026-09-01 15:00:00 +08:00`; this confirms the
field is a timestamp rather than a code-table index. In an SH frame the
16-bit value advances from 24661 through 24826 and is passed with market to
Wine's lookup at `0x4157b0`, supporting the `symbol_index` name.

After correcting the `0104` boundary to a 98-byte header plus 68-byte records,
the final replay parsed 36 code tables and resolved all 2,102 delta indexes
(`unresolved=0`). Examples include `SH603059` / `倍加洁`, `SH688403` /
`汇成股份`, `SZ300313` / `天山生物`, and `SZ301516` / `中远通`. Export:
`/tmp/official-5188-formal-0006-resolved-030940`; build log:
`/home/bin/webclx/logs/quoteNetzipRs/3050_build.log`.

## Implemented evidence API

- `Official5188DeltaEnvelope` exposes `record_count` and `value_end_offset`.
- `Official5188BitReader` performs bounded MSB-first reads.
- `decode_official_5188_delta_indexes` restores market, symbol index,
  timestamp and the one-bit baseline flag for every internal record.
- `official_5188_extract` preserves payload/value/index bytes and emits the
  restored index states as JSON.
- `Official5188CodeTable` validates the `0104` shape
  `98 + record_count * 68` and maps each embedded zero-based symbol index to its ASCII
  code and GBK name. The exporter writes both the complete code tables and
  resolved delta-index rows.

## Not yet confirmed

- The field table and all delta dependencies inside `0x449770`.
- The exact baseline-record lookup and lifetime rules behind `uses_baseline`.
- Whether every less common market table uses the same zero-based index rule.
- Aggregation rules from one or more `2704` frames into a Wine callback batch.
- Field parity for time, OHLC, price levels, volume, amount and status flags.

## 2026-09-02 paired internal-record mapping

The synchronized formal fixture and the retained Wine core provide one
same-symbol, same-timestamp internal-record match.  The 311-byte record at
the baseline slot for `SH` symbol index `24661` (`603059`) contains the
following values:

| Internal offset | Meaning | Observed value | Wine callback field |
|---:|---|---:|---|
| `+0x00` | Unix timestamp seconds | `1788246001` | `datetime=2026-09-01 15:00:00` |
| `+0x04` | open price integer | `2509` | `open=25.09` |
| `+0x08` | high price integer | `2556` | `high=25.56` |
| `+0x0c` | low price integer | `2502` | `low=25.02` |
| `+0x10` | last price integer | `2517` | `price=25.17` |
| `+0x14` | cumulative volume | `10261` | `volume=10261` |
| `+0x1c` | cumulative amount integer | `25934317` | `amount=25934334` after OEM float conversion |
| `+0x58..0x7c` | ten price levels, integer ticks | `2505..2530` | internal order is bid levels reversed, then ask levels |
| `+0xa8..0xcc` | ten level volumes | `19,14,8,22,2,16,12,2,44,25` | internal order is bid volumes reversed, then ask volumes |

Price integers use the security decimal scale (for this SH equity, cents).
The internal amount differs from the callback's IEEE-754-derived value by a
small rounding/derivation delta, so amount parity must be tested with the
Wine conversion rule rather than integer equality.  This evidence confirms
the target field offsets, but it does not yet prove that the Rust value-bit
decoder reconstructs every record or that all market classes use the same
price scale.

Do not route this evidence into the production lane until the value-bit
decoder reconstructs the matched records and the mappings pass same-symbol,
same-timestamp parity across an independent Wine fixture.

## Verification

```bash
objdump -d -M intel --start-address=0x44aa30 --stop-address=0x44adae \
  netzip_api_bin/NetzipAPI/StockC++/网际风.exe
objdump -d -M intel --start-address=0x448990 --stop-address=0x448f8c \
  netzip_api_bin/NetzipAPI/StockC++/网际风.exe
cargo +nightly test --manifest-path ../crates/netzip-fullpull/Cargo.toml official_5188
cargo +nightly test --lib debug_pcap::tests
cargo +nightly build --example official_5188_extract
```

The Rust commands are submitted through the webClx compile API. This evidence
expires if the binary hash changes or an independent Wine fixture contradicts
the stream boundary, token tables, record size or callback aggregation model.
