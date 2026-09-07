//! Offline replay: decoded official 5188 (2704) records -> Stockdrv push packets.
//!
//! Consumes the JSON artifacts produced by `official_5188_extract` (one
//! `*-2704.decoded-values.json` stream plus the `*-0104.code-table.json`
//! metadata seeds), merges every record through the authoritative
//! `Official5188OemState` projection, converts the resulting public quotes to
//! the Stockdrv `PushQuote` shape, and writes one `RCV_REPORTV3` push packet
//! per batch of 2000 symbols. No network access, no credentials, evidence-only.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use Stockdrv::gateway::{PushQuote, encode_push_packet};
use netzip_fullpull::official_5188::{
    Official5188CodeTable, Official5188DecodedValueRecord, Official5188DeltaIndexState,
    Official5188InternalRecord, Official5188OemState, Official5188ValueRecordHeader,
};

#[derive(Deserialize)]
struct DecodedIndex {
    market: [u8; 2],
    symbol_index: u16,
    #[allow(dead_code)]
    timestamp: u32,
    #[allow(dead_code)]
    uses_baseline: bool,
}

#[derive(Deserialize)]
struct DecodedHeader {
    #[allow(dead_code)]
    clear_ladder: bool,
    raw_level_count: u8,
    level_count: u8,
    has_book: bool,
    special_path: bool,
}

#[derive(Deserialize)]
struct DecodedEntry {
    index: DecodedIndex,
    mask: u8,
    header: DecodedHeader,
    #[allow(dead_code)]
    bit_start: usize,
    #[allow(dead_code)]
    bit_end: usize,
    record: Vec<u8>,
}

fn load_code_tables(dir: &Path) -> Result<BTreeMap<[u8; 2], Official5188CodeTable>, String> {
    let mut tables = BTreeMap::new();
    for entry in fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if !name.ends_with("-0104.code-table.json") {
            continue;
        }
        let bytes = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let table: Official5188CodeTable =
            serde_json::from_slice(&bytes).map_err(|e| format!("parse {}: {e}", path.display()))?;
        tables.insert(table.market, table);
    }
    Ok(tables)
}

fn decode_entries(path: &Path) -> Result<Vec<DecodedEntry>, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("parse {}: {e}", path.display()))
}

fn to_decoded_value_record(entry: &DecodedEntry) -> Result<Official5188DecodedValueRecord, String> {
    Ok(Official5188DecodedValueRecord {
        index: Official5188DeltaIndexState {
            market: entry.index.market,
            symbol_index: entry.index.symbol_index,
            timestamp: entry.index.timestamp,
            uses_baseline: entry.index.uses_baseline,
        },
        mask: entry.mask,
        header: Official5188ValueRecordHeader {
            clear_ladder: entry.header.clear_ladder,
            raw_level_count: entry.header.raw_level_count,
            level_count: entry.header.level_count,
            has_book: entry.header.has_book,
            special_path: entry.header.special_path,
        },
        bit_start: entry.bit_start,
        bit_end: entry.bit_end,
        record: Official5188InternalRecord::decode(&entry.record)?,
    })
}

fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let usage = "usage: replay_2704_push EXTRACT_DIR OUTPUT_DIR [STALE_EXTRACT_DIR]";
    let extract_dir = PathBuf::from(args.next().ok_or(usage)?);
    let output_dir = PathBuf::from(args.next().ok_or(usage)?);
    let stale_dir = args.next().map(PathBuf::from);
    if args.next().is_some() {
        return Err(usage.to_string());
    }

    let code_tables = load_code_tables(&extract_dir)?;
    let table_count = code_tables.len();
    println!("S\tcode-tables\t{table_count}");

    let mut states: BTreeMap<([u8; 2], u16), Official5188OemState> = BTreeMap::new();
    let mut seeded = 0usize;
    let mut unseeded = 0usize;
    let mut merged_records = 0usize;
    let mut processed_frames = 0usize;

    let mut streams: Vec<PathBuf> = fs::read_dir(&extract_dir)
        .map_err(|e| format!("read {}: {e}", extract_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .ends_with("-2704.decoded-values.json")
        })
        .collect();
    streams.sort();
    for path in &streams {
        let entries = decode_entries(path)?;
        for entry in &entries {
            let key = (entry.index.market, entry.index.symbol_index);
            if let std::collections::btree_map::Entry::Vacant(slot) = states.entry(key) {
                let Some(table) = code_tables.get(&entry.index.market) else {
                    unseeded += 1;
                    continue;
                };
                let Some(metadata) = table
                    .records
                    .iter()
                    .find(|record| record.symbol_index == entry.index.symbol_index)
                else {
                    unseeded += 1;
                    continue;
                };
                slot.insert(
                    Official5188OemState::from_code_table_metadata(entry.index.market, metadata)
                        .map_err(|e| format!("seed {key:?}: {e}"))?,
                );
                seeded += 1;
            }
            let decoded = to_decoded_value_record(entry)?;
            states
                .get_mut(&key)
                .expect("state was just inserted")
                .merge(&decoded);
            merged_records += 1;
        }
        processed_frames += 1;
        println!("P\tframe\t{processed_frames}\trecords_merged\t{merged_records}");
    }
    println!("S\tstates\tseeded={seeded}\tunseeded={unseeded}");
    if unseeded != 0 {
        return Err(format!(
            "refusing partial push: {unseeded} decoded records had no same-session 0104 seed"
        ));
    }

    let mut quotes: Vec<PushQuote> = Vec::new();
    let mut projected = 0usize;
    let mut price_zero = 0usize;
    let mut projection_errors = 0usize;
    for ((market, symbol_index), state) in &states {
        let Some(table) = code_tables.get(market) else {
            continue;
        };
        let Some(metadata) = table
            .records
            .iter()
            .find(|record| record.symbol_index == *symbol_index)
        else {
            continue;
        };
        let quote = match state.project_from_code_table_metadata(metadata) {
            Ok(quote) => quote,
            Err(error) => {
                projection_errors += 1;
                eprintln!(
                    "E\tprojection_failed\tmarket={:?}\tsymbol_index={symbol_index}\t{error}",
                    market
                );
                continue;
            }
        };
        projected += 1;
        if quote.price <= 0.0 {
            price_zero += 1;
            continue;
        }
        quotes.push(PushQuote {
            instrument: format!("{}{}", quote.market, quote.code),
            name: quote.name,
            timestamp: quote.timestamp,
            price: quote.price as f32,
            last_close: quote.last_close as f32,
            volume: quote.volume as f32,
            amount: quote.amount as f32,
        });
    }
    println!(
        "S\tprojected\t{projected}\tprojection_errors\t{projection_errors}\tprice_zero_skipped\t{price_zero}\tpushable\t{}",
        quotes.len()
    );
    if projection_errors != 0 {
        return Err(format!(
            "refusing partial push: {projection_errors} states failed metadata projection"
        ));
    }

    // Optional stale-seed evidence: quantify exactly what breaks when a
    // previous session's 0104 tables (older table version) are reused for the
    // current stream — index->code drift on streamed symbols, and stale
    // previous-close/decimals per code. This is the executable form of the
    // seed-lifecycle gate (reset on day-cut/reconnect/restart).
    if let Some(stale_dir) = &stale_dir {
        let stale_tables = load_code_tables(stale_dir)?;
        let mut streamed = 0usize;
        let mut stale_missing = 0usize;
        let mut index_code_drift = 0usize;
        let mut code_shared = 0usize;
        let mut prev_close_mismatch = 0usize;
        let mut decimals_mismatch = 0usize;
        let mut drift_examples: Vec<serde_json::Value> = Vec::new();
        for ((market, symbol_index), state) in &states {
            streamed += 1;
            let Some(stale_table) = stale_tables.get(market) else {
                stale_missing += 1;
                continue;
            };
            let stale_row = stale_table
                .records
                .iter()
                .find(|record| record.symbol_index == *symbol_index);
            let Some(current_table) = code_tables.get(market) else {
                continue;
            };
            let Some(current_row) = current_table
                .records
                .iter()
                .find(|record| record.symbol_index == *symbol_index)
            else {
                continue;
            };
            match stale_row {
                None => stale_missing += 1,
                Some(stale_row) => {
                    if stale_row.code != current_row.code {
                        index_code_drift += 1;
                        if drift_examples.len() < 5 {
                            drift_examples.push(serde_json::json!({
                                "market": String::from_utf8_lossy(market),
                                "symbol_index": symbol_index,
                                "stale_code": stale_row.code,
                                "current_code": current_row.code,
                            }));
                        }
                    }
                    if let Some(stale_by_code) = stale_tables
                        .get(market)
                        .map(|table| {
                            table
                                .records
                                .iter()
                                .find(|record| record.code == current_row.code)
                        })
                        .unwrap_or(None)
                    {
                        code_shared += 1;
                        let stale_prev = stale_by_code
                            .opaque_tail
                            .get(11..15)
                            .and_then(|bytes| bytes.try_into().ok())
                            .map(i32::from_le_bytes);
                        let current_prev = current_row
                            .opaque_tail
                            .get(11..15)
                            .and_then(|bytes| bytes.try_into().ok())
                            .map(i32::from_le_bytes);
                        if stale_prev != current_prev {
                            prev_close_mismatch += 1;
                        }
                        if stale_by_code.price_scale_hint() != current_row.price_scale_hint() {
                            decimals_mismatch += 1;
                        }
                    }
                }
            }
            let _ = state; // states drive the streamed symbol set
        }
        let evidence = serde_json::json!({
            "schema": "netzip.stale-seed-evidence.v1",
            "current_extract": extract_dir,
            "stale_extract": stale_dir,
            "streamed_symbols": streamed,
            "stale_index_missing": stale_missing,
            "index_to_code_drift": index_code_drift,
            "code_shared_with_stale": code_shared,
            "prev_close_mismatch_on_shared": prev_close_mismatch,
            "decimals_mismatch_on_shared": decimals_mismatch,
            "index_drift_examples": drift_examples,
            "note": "index_to_code_drift and prev_close_mismatch quantify why stale 0104 seeds must be dropped at day-cut/reconnect/restart",
        });
        let path = output_dir.join("stale-seed-evidence.json");
        fs::create_dir_all(&output_dir).map_err(|e| format!("create output: {e}"))?;
        fs::write(&path, serde_json::to_string_pretty(&evidence).unwrap())
            .map_err(|e| format!("write {}: {e}", path.display()))?;
        println!(
            "S\tstale-seed\tstreamed={streamed}\tindex_drift={index_code_drift}\t\
             prev_close_mismatch={prev_close_mismatch}\tdecimals_mismatch={decimals_mismatch}\t{}",
            path.display()
        );
    }

    fs::create_dir_all(&output_dir).map_err(|e| format!("create output: {e}"))?;
    for (batch_index, batch) in quotes.chunks(2_000).enumerate() {
        let packet = encode_push_packet(batch)?;
        let path = output_dir.join(format!("replay-push-batch-{batch_index:03}.bin"));
        fs::write(&path, &packet).map_err(|e| format!("write {}: {e}", path.display()))?;
        println!(
            "S\tpacket\t{}\tcount={}\tbytes={}",
            path.display(),
            batch.len(),
            packet.len()
        );
    }

    println!(
        "Z\treplay-complete\tframes={processed_frames}\tmerged={merged_records}\tpushable={}",
        quotes.len()
    );
    Ok(())
}
