use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use netzip_fullpull::Official5188InternalRecord;
use serde::{Deserialize, Serialize};

const DEFAULT_WINDOW_MS: i64 = 250;
const SAMPLE_LIMIT: usize = 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JoinPolicy {
    Nearest,
    BusinessTimestamp,
}

impl JoinPolicy {
    fn parse(value: Option<&std::ffi::OsStr>) -> Result<Self, Box<dyn Error>> {
        match value.map(|value| value.to_string_lossy()) {
            None => Ok(Self::Nearest),
            Some(value) if value == "nearest" => Ok(Self::Nearest),
            Some(value) if value == "business-ts" => Ok(Self::BusinessTimestamp),
            Some(value) => Err(format!(
                "unknown join policy {value:?}; expected nearest or business-ts"
            )
            .into()),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Nearest => "nearest",
            Self::BusinessTimestamp => "business-ts",
        }
    }
}

type MetadataBySymbol = HashMap<([u8; 2], u16), CodeTableRecord>;

#[derive(Deserialize)]
struct ManifestEntry {
    completed_at_micros: Option<u64>,
    wire_kind: String,
    code_table_file: Option<String>,
    decoded_values_file: Option<String>,
}

#[derive(Deserialize)]
struct CodeTable {
    market: [u8; 2],
    records: Vec<CodeTableRecord>,
}

#[derive(Clone, Deserialize)]
struct CodeTableRecord {
    symbol_index: u16,
    code: String,
    name: String,
    opaque_tail: [u8; 23],
}

impl CodeTableRecord {
    fn price_scale(&self) -> Option<f32> {
        let decimal_places = self.opaque_tail[0];
        (decimal_places <= 6).then(|| 10_f32.powi(i32::from(decimal_places)))
    }
}

#[derive(Deserialize)]
struct DecodedValue {
    index: DeltaIndex,
    record: Vec<u8>,
}

#[derive(Deserialize)]
struct DeltaIndex {
    market: [u8; 2],
    symbol_index: u16,
    timestamp: u32,
}

#[derive(Deserialize)]
struct CallbackEvent {
    sequence: u64,
    timestamp_ms: u64,
    quote_batch: Option<CallbackBatch>,
}

#[derive(Deserialize)]
struct CallbackBatch {
    schema: String,
    quotes: Vec<WineQuote>,
}

#[derive(Clone, Deserialize, Serialize)]
struct WineQuote {
    market: String,
    code: String,
    name: String,
    datetime: String,
    price: f64,
    last_close: f64,
    open: f64,
    high: f64,
    low: f64,
    volume: f64,
    amount: f64,
    ask_prices: [f64; 10],
    ask_volumes: [f64; 10],
    bid_prices: [f64; 10],
    bid_volumes: [f64; 10],
}

#[derive(Clone)]
struct IndexedQuote {
    batch_sequence: u64,
    batch_timestamp_ms: u64,
    quote: WineQuote,
}

#[derive(Clone)]
struct IndexedBatch {
    sequence: u64,
    timestamp_ms: u64,
    quotes: Vec<IndexedQuote>,
}

#[derive(Default, Serialize)]
struct FieldHits {
    name: usize,
    price: usize,
    last_close: usize,
    open: usize,
    high: usize,
    low: usize,
    volume: usize,
    amount: usize,
    ask_prices: usize,
    ask_volumes: usize,
    bid_prices: usize,
    bid_volumes: usize,
    timestamp: usize,
}

#[derive(Default, Serialize)]
struct BatchStats {
    callback_quotes: usize,
    matched_records: usize,
    matched_symbols: BTreeSet<String>,
    frame_delta_ms_min: Option<f64>,
    frame_delta_ms_max: Option<f64>,
    fields: FieldHits,
}

#[derive(Serialize)]
struct MismatchSample {
    sequence: u64,
    market: String,
    code: String,
    symbol_index: u16,
    frame_completed_at_micros: u64,
    frame_to_callback_ms: f64,
    internal_timestamp: u32,
    callback_datetime: String,
    price_scale: f32,
    internal_price: i32,
    projected_price_f32: f32,
    callback_price_f32: f32,
    internal_last_close: i32,
    projected_last_close_f32: f32,
    callback_last_close_f32: f32,
    internal_open: i32,
    internal_high: i32,
    internal_low: i32,
    internal_volume: i64,
    internal_amount: i64,
    projected_amount_f32: f32,
    callback_amount_f32: f32,
    mismatched_fields: Vec<&'static str>,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    extracted_dir: String,
    callback_jsonl: String,
    window_ms: i64,
    join_policy: &'static str,
    decoded_records: usize,
    metadata_resolved: usize,
    matched_records: usize,
    carried_forward_records: usize,
    unmatched_records: usize,
    ambiguous_business_timestamp_records: usize,
    equivalent_duplicate_business_timestamp_records: usize,
    conflicting_business_timestamp_records: usize,
    missing_metadata: usize,
    invalid_records: usize,
    callback_batches: usize,
    compared_sequences: Vec<u64>,
    fields: FieldHits,
    batches: BTreeMap<u64, BatchStats>,
    mismatch_samples: Vec<MismatchSample>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let extracted_dir = args.next().map(PathBuf::from).ok_or(
        "usage: official_5188_callback_parity EXTRACTED_DIR CALLBACK_JSONL [OUTPUT_JSON] [WINDOW_MS] [METADATA_DIR]",
    )?;
    let callback_jsonl = args.next().map(PathBuf::from).ok_or(
        "usage: official_5188_callback_parity EXTRACTED_DIR CALLBACK_JSONL [OUTPUT_JSON] [WINDOW_MS] [METADATA_DIR]",
    )?;
    let output_json = args.next().map(PathBuf::from);
    let window_ms = args
        .next()
        .map(|value| value.to_string_lossy().parse::<i64>())
        .transpose()?
        .unwrap_or(DEFAULT_WINDOW_MS);
    let metadata_dir = args.next().map(PathBuf::from);
    let join_policy = JoinPolicy::parse(args.next().as_deref())?;
    if args.next().is_some() || window_ms < 0 {
        return Err("WINDOW_MS must be a non-negative integer".into());
    }

    let report = build_report(
        &extracted_dir,
        &callback_jsonl,
        window_ms,
        metadata_dir.as_deref(),
        join_policy,
    )?;
    let json = serde_json::to_vec_pretty(&report)?;
    if let Some(path) = output_json {
        fs::write(&path, &json)?;
        println!("wrote callback parity report to {}", path.display());
    } else {
        println!("{}", String::from_utf8(json)?);
    }
    eprintln!(
        "decoded={} metadata={} matched={} unmatched={} sequences={:?}",
        report.decoded_records,
        report.metadata_resolved,
        report.matched_records,
        report.unmatched_records,
        report.compared_sequences
    );
    Ok(())
}

fn build_report(
    extracted_dir: &Path,
    callback_jsonl: &Path,
    window_ms: i64,
    metadata_dir: Option<&Path>,
    join_policy: JoinPolicy,
) -> Result<Report, Box<dyn Error>> {
    let manifest: Vec<ManifestEntry> =
        serde_json::from_slice(&fs::read(extracted_dir.join("manifest.json"))?)?;
    let mut metadata = load_metadata(extracted_dir, &manifest)?;
    if let Some(metadata_dir) = metadata_dir {
        load_metadata_directory(metadata_dir, &mut metadata)?;
    }
    let batches = load_batches(callback_jsonl)?;
    let mut report = Report {
        schema: "quoteNetzipRs.official_5188_callback_parity.v1",
        extracted_dir: extracted_dir.display().to_string(),
        callback_jsonl: callback_jsonl.display().to_string(),
        window_ms,
        join_policy: join_policy.name(),
        decoded_records: 0,
        metadata_resolved: 0,
        matched_records: 0,
        carried_forward_records: 0,
        unmatched_records: 0,
        ambiguous_business_timestamp_records: 0,
        equivalent_duplicate_business_timestamp_records: 0,
        conflicting_business_timestamp_records: 0,
        missing_metadata: 0,
        invalid_records: 0,
        callback_batches: batches.len(),
        compared_sequences: Vec::new(),
        fields: FieldHits::default(),
        batches: BTreeMap::new(),
        mismatch_samples: Vec::new(),
    };

    for batch in &batches {
        report.batches.insert(
            batch.sequence,
            BatchStats {
                callback_quotes: batch.quotes.len(),
                ..BatchStats::default()
            },
        );
    }

    for entry in manifest.iter().filter(|entry| entry.wire_kind == "2704") {
        let (Some(completed_at_micros), Some(decoded_file)) = (
            entry.completed_at_micros,
            entry.decoded_values_file.as_deref(),
        ) else {
            continue;
        };
        let decoded: Vec<DecodedValue> =
            serde_json::from_slice(&fs::read(extracted_dir.join(decoded_file))?)?;
        for value in decoded {
            report.decoded_records += 1;
            let key = (value.index.market, value.index.symbol_index);
            let Some(meta) = metadata.get(&key) else {
                report.missing_metadata += 1;
                continue;
            };
            let Some(scale) = meta.price_scale() else {
                report.missing_metadata += 1;
                continue;
            };
            report.metadata_resolved += 1;
            let Ok(record) = Official5188InternalRecord::decode(&value.record) else {
                report.invalid_records += 1;
                continue;
            };
            let market = String::from_utf8_lossy(&value.index.market).into_owned();
            let quote_key = (market.clone(), meta.code.clone());
            let matched = match join_policy {
                JoinPolicy::Nearest => nearest_batch(
                    &batches,
                    &quote_key,
                    value.index.timestamp,
                    completed_at_micros,
                    window_ms,
                ),
                JoinPolicy::BusinessTimestamp => nearest_business_timestamp_batch(
                    &batches,
                    &quote_key,
                    value.index.timestamp,
                    completed_at_micros,
                    window_ms,
                ),
            };
            if join_policy == JoinPolicy::BusinessTimestamp {
                let candidate_stats = business_timestamp_candidate_stats(
                    &batches,
                    &quote_key,
                    value.index.timestamp,
                    completed_at_micros,
                    window_ms,
                );
                if candidate_stats.count > 1 {
                    report.ambiguous_business_timestamp_records += 1;
                    if candidate_stats.distinct_states == 1 {
                        report.equivalent_duplicate_business_timestamp_records += 1;
                    } else if !business_timestamp_live_leftover_shape(
                        &batches,
                        &quote_key,
                        value.index.timestamp,
                    ) {
                        report.conflicting_business_timestamp_records += 1;
                        report.unmatched_records += 1;
                        continue;
                    }
                }
            }
            let Some((batch, quote, delta_micros)) = matched else {
                report.unmatched_records += 1;
                continue;
            };

            report.matched_records += 1;
            let frame_to_callback_ms = delta_micros as f64 / 1_000.0;
            let matches = compare(&record, &meta.name, scale, quote);
            if matches.is_zero_amount_delta_skip(&record) {
                report.carried_forward_records += 1;
            }
            add_hits(&mut report.fields, &matches);
            let stats = report.batches.get_mut(&batch.sequence).unwrap();
            stats.matched_records += 1;
            stats
                .matched_symbols
                .insert(format!("{market}{}", meta.code));
            stats.frame_delta_ms_min = Some(
                stats
                    .frame_delta_ms_min
                    .map_or(frame_to_callback_ms, |old| old.min(frame_to_callback_ms)),
            );
            stats.frame_delta_ms_max = Some(
                stats
                    .frame_delta_ms_max
                    .map_or(frame_to_callback_ms, |old| old.max(frame_to_callback_ms)),
            );
            add_hits(&mut stats.fields, &matches);

            let mismatched_fields = matches.mismatched_fields();
            if !mismatched_fields.is_empty()
                && (report.mismatch_samples.len() < SAMPLE_LIMIT || meta.code == "603059")
            {
                let sample = MismatchSample {
                    sequence: batch.sequence,
                    market,
                    code: meta.code.clone(),
                    symbol_index: value.index.symbol_index,
                    frame_completed_at_micros: completed_at_micros,
                    frame_to_callback_ms,
                    internal_timestamp: value.index.timestamp,
                    callback_datetime: quote.datetime.clone(),
                    price_scale: scale,
                    internal_price: record.last_price_integer(),
                    projected_price_f32: record.last_price_integer() as f32 / scale,
                    callback_price_f32: quote.price as f32,
                    internal_last_close: last_close_integer(&record),
                    projected_last_close_f32: last_close_integer(&record) as f32 / scale,
                    callback_last_close_f32: quote.last_close as f32,
                    internal_open: record.open_price_integer(),
                    internal_high: record.high_price_integer(),
                    internal_low: record.low_price_integer(),
                    internal_volume: record.volume_integer(),
                    internal_amount: record.amount_integer(),
                    projected_amount_f32: record.amount_integer() as f32,
                    callback_amount_f32: quote.amount as f32,
                    mismatched_fields,
                };
                if meta.code == "603059" && report.mismatch_samples.len() >= SAMPLE_LIMIT {
                    report.mismatch_samples[SAMPLE_LIMIT - 1] = sample;
                } else {
                    report.mismatch_samples.push(sample);
                }
            }
        }
    }

    report.unmatched_records += report.missing_metadata + report.invalid_records;
    report.compared_sequences = report
        .batches
        .iter()
        .filter_map(|(sequence, stats)| (stats.matched_records > 0).then_some(*sequence))
        .collect();
    Ok(report)
}

fn load_metadata_directory(
    directory: &Path,
    metadata: &mut MetadataBySymbol,
) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.ends_with("0104.code-table.json") {
            continue;
        }
        let table: CodeTable = serde_json::from_slice(&fs::read(path)?)?;
        for record in table.records {
            if !record.code.is_empty() {
                metadata.insert((table.market, record.symbol_index), record);
            }
        }
    }
    Ok(())
}

fn load_metadata(
    extracted_dir: &Path,
    manifest: &[ManifestEntry],
) -> Result<MetadataBySymbol, Box<dyn Error>> {
    let mut metadata = HashMap::new();
    for file in manifest
        .iter()
        .filter_map(|entry| entry.code_table_file.as_deref())
    {
        let table: CodeTable = serde_json::from_slice(&fs::read(extracted_dir.join(file))?)?;
        for record in table.records {
            if !record.code.is_empty() {
                metadata.insert((table.market, record.symbol_index), record);
            }
        }
    }
    Ok(metadata)
}

fn load_batches(path: &Path) -> Result<Vec<IndexedBatch>, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let mut batches = Vec::new();
    let mut seen_sequences = BTreeSet::new();
    for (line_index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event: CallbackEvent = serde_json::from_str(line)
            .map_err(|error| format!("invalid callback JSONL line {}: {error}", line_index + 1))?;
        let Some(batch) = event.quote_batch else {
            continue;
        };
        // The capture endpoint returns a rolling window, so consecutive polls
        // contain the same event sequences. Compare each vendor callback once.
        if !seen_sequences.insert(event.sequence) {
            continue;
        }
        if batch.schema != "quoteNetzipWine.quote_batch.v1" {
            return Err(format!("unsupported callback schema {:?}", batch.schema).into());
        }
        let quotes = batch
            .quotes
            .into_iter()
            .map(|quote| IndexedQuote {
                batch_sequence: event.sequence,
                batch_timestamp_ms: event.timestamp_ms,
                quote,
            })
            .collect();
        batches.push(IndexedBatch {
            sequence: event.sequence,
            timestamp_ms: event.timestamp_ms,
            quotes,
        });
    }
    batches.sort_by_key(|batch| (batch.timestamp_ms, batch.sequence));
    Ok(batches)
}

fn nearest_batch<'a>(
    batches: &'a [IndexedBatch],
    key: &(String, String),
    internal_timestamp: u32,
    completed_at_micros: u64,
    window_ms: i64,
) -> Option<(&'a IndexedBatch, &'a WineQuote, i64)> {
    let frame_micros = i128::from(completed_at_micros);
    let window_micros = i128::from(window_ms) * 1_000;
    let lower_ms = ((frame_micros - window_micros).max(0) / 1_000) as u64;
    let upper_ms = ((frame_micros + window_micros).max(0) / 1_000) as u64;
    let start = batches.partition_point(|batch| batch.timestamp_ms < lower_ms);
    let end = batches.partition_point(|batch| batch.timestamp_ms <= upper_ms);
    batches[start..end]
        .iter()
        .flat_map(|batch| {
            batch
                .quotes
                .iter()
                .filter(|indexed| indexed.quote.market == key.0 && indexed.quote.code == key.1)
                .map(move |indexed| (batch, indexed))
        })
        .filter_map(|(batch, indexed)| {
            let delta = i128::from(batch.timestamp_ms) * 1_000 - frame_micros;
            (delta.abs() <= window_micros).then_some((batch, indexed, delta as i64))
        })
        .min_by_key(|(batch, indexed, delta)| {
            (
                parse_callback_timestamp(&indexed.quote.datetime).map_or(u64::MAX, |timestamp| {
                    u64::from(timestamp.abs_diff(internal_timestamp))
                }),
                delta.abs(),
                indexed.batch_timestamp_ms,
                indexed.batch_sequence,
                batch.sequence,
            )
        })
        .map(|(batch, indexed, delta)| (batch, &indexed.quote, delta))
}

fn nearest_business_timestamp_batch<'a>(
    batches: &'a [IndexedBatch],
    key: &(String, String),
    internal_timestamp: u32,
    completed_at_micros: u64,
    _window_ms: i64,
) -> Option<(&'a IndexedBatch, &'a WineQuote, i64)> {
    let frame_micros = i128::from(completed_at_micros);
    // Business timestamp is the primary key. Unlike nearest-window mode,
    // candidates are global across the callback capture; wall-clock distance
    // only breaks ties among the same code and business second.
    let candidates = batches
        .iter()
        .flat_map(|batch| {
            batch
                .quotes
                .iter()
                .filter(|indexed| {
                    indexed.quote.market == key.0
                        && indexed.quote.code == key.1
                        && parse_callback_timestamp(&indexed.quote.datetime)
                            == Some(internal_timestamp)
                })
                .map(move |indexed| (batch, indexed))
        })
        .map(|(batch, indexed)| {
            let delta = i128::from(batch.timestamp_ms) * 1_000 - frame_micros;
            (batch, indexed, delta as i64)
        })
        .collect::<Vec<_>>();
    if candidates.len() == 2
        && candidates
            .iter()
            .any(|(_, indexed, _)| indexed.quote.price == 0.0)
        && candidates
            .iter()
            .any(|(_, indexed, _)| indexed.quote.price != 0.0)
        && candidates
            .iter()
            .map(|(_, indexed, _)| indexed.quote.last_close.to_bits())
            .collect::<BTreeSet<_>>()
            .len()
            == 1
    {
        return candidates
            .into_iter()
            .filter(|(_, indexed, _)| indexed.quote.price != 0.0)
            .max_by_key(|(batch, indexed, _)| (batch.sequence, indexed.batch_sequence))
            .map(|(batch, indexed, delta)| (batch, &indexed.quote, delta));
    }
    candidates
        .into_iter()
        .min_by_key(|(batch, indexed, delta)| {
            (
                delta.abs(),
                indexed.batch_timestamp_ms,
                indexed.batch_sequence,
                batch.sequence,
            )
        })
        .map(|(batch, indexed, delta)| (batch, &indexed.quote, delta))
}

fn business_timestamp_live_leftover_shape(
    batches: &[IndexedBatch],
    key: &(String, String),
    internal_timestamp: u32,
) -> bool {
    let candidates = batches
        .iter()
        .flat_map(|batch| batch.quotes.iter())
        .filter(|indexed| {
            indexed.quote.market == key.0
                && indexed.quote.code == key.1
                && parse_callback_timestamp(&indexed.quote.datetime) == Some(internal_timestamp)
        })
        .collect::<Vec<_>>();
    candidates.len() == 2
        && candidates.iter().any(|indexed| indexed.quote.price == 0.0)
        && candidates.iter().any(|indexed| indexed.quote.price != 0.0)
        && candidates
            .iter()
            .map(|indexed| indexed.quote.last_close.to_bits())
            .collect::<BTreeSet<_>>()
            .len()
            == 1
}

#[derive(Debug, Eq, PartialEq)]
struct BusinessTimestampCandidateStats {
    count: usize,
    distinct_states: usize,
}

fn business_timestamp_candidate_stats(
    batches: &[IndexedBatch],
    key: &(String, String),
    internal_timestamp: u32,
    _completed_at_micros: u64,
    _window_ms: i64,
) -> BusinessTimestampCandidateStats {
    let candidates = batches
        .iter()
        .flat_map(|batch| batch.quotes.iter())
        .filter(|indexed| {
            indexed.quote.market == key.0
                && indexed.quote.code == key.1
                && parse_callback_timestamp(&indexed.quote.datetime) == Some(internal_timestamp)
        })
        .collect::<Vec<_>>();
    let distinct_states = candidates
        .iter()
        .map(|indexed| wine_quote_state_fingerprint(&indexed.quote))
        .collect::<BTreeSet<_>>()
        .len();
    BusinessTimestampCandidateStats {
        count: candidates.len(),
        distinct_states,
    }
}

fn wine_quote_state_fingerprint(quote: &WineQuote) -> Vec<u8> {
    serde_json::to_vec(quote).expect("WineQuote serialization is infallible")
}

struct Matches {
    name: bool,
    price: bool,
    last_close: bool,
    open: bool,
    high: bool,
    low: bool,
    volume: bool,
    amount: bool,
    ask_prices: bool,
    ask_volumes: bool,
    bid_prices: bool,
    bid_volumes: bool,
    timestamp: bool,
}

impl Matches {
    /// Amount equal to zero on the 5188 side means the delta stream never
    /// carried this field for the symbol, so the Wine public state kept its
    /// previous value (`0x44aa30` merge semantics).  It is not a decoder
    /// error and must not count as a field mismatch.
    fn is_zero_amount_delta_skip(&self, record: &Official5188InternalRecord) -> bool {
        !self.amount && record.amount_integer() == 0
    }

    fn mismatched_fields(&self) -> Vec<&'static str> {
        [
            ("name", self.name),
            ("price", self.price),
            ("last_close", self.last_close),
            ("open", self.open),
            ("high", self.high),
            ("low", self.low),
            ("volume", self.volume),
            ("amount", self.amount),
            ("ask_prices", self.ask_prices),
            ("ask_volumes", self.ask_volumes),
            ("bid_prices", self.bid_prices),
            ("bid_volumes", self.bid_volumes),
            ("timestamp", self.timestamp),
        ]
        .into_iter()
        .filter_map(|(name, matches)| (!matches).then_some(name))
        .collect()
    }
}

fn compare(
    record: &Official5188InternalRecord,
    metadata_name: &str,
    scale: f32,
    quote: &WineQuote,
) -> Matches {
    let prices = record.ladder_price_integers();
    let volumes = record.ladder_volume_integers();
    let scaled = |value: i32| value as f32 / scale;
    let bid_prices = std::array::from_fn(|index| {
        if index < 5 {
            scaled(prices[4 - index])
        } else {
            0.0
        }
    });
    let ask_prices = std::array::from_fn(|index| {
        if index < 5 {
            scaled(prices[5 + index])
        } else {
            0.0
        }
    });
    let bid_volumes = std::array::from_fn(|index| {
        if index < 5 {
            volumes[4 - index] as f32
        } else {
            0.0
        }
    });
    let ask_volumes = std::array::from_fn(|index| {
        if index < 5 {
            volumes[5 + index] as f32
        } else {
            0.0
        }
    });
    Matches {
        name: quote.name == metadata_name,
        price: same_f32(scaled(record.last_price_integer()), quote.price),
        last_close: same_f32(scaled(last_close_integer(record)), quote.last_close),
        open: same_f32(scaled(record.open_price_integer()), quote.open),
        high: same_f32(scaled(record.high_price_integer()), quote.high),
        low: same_f32(scaled(record.low_price_integer()), quote.low),
        volume: same_f32(record.volume_integer() as f32, quote.volume),
        amount: same_f32(record.amount_integer() as f32, quote.amount),
        ask_prices: same_f32_array(&ask_prices, &quote.ask_prices),
        ask_volumes: same_f32_array(&ask_volumes, &quote.ask_volumes),
        bid_prices: same_f32_array(&bid_prices, &quote.bid_prices),
        bid_volumes: same_f32_array(&bid_volumes, &quote.bid_volumes),
        timestamp: parse_callback_timestamp(&quote.datetime) == Some(record.timestamp()),
    }
}

fn last_close_integer(record: &Official5188InternalRecord) -> i32 {
    i32::from_le_bytes(record.as_bytes()[0x12b..0x12f].try_into().unwrap())
}

fn same_f32(projected: f32, callback: f64) -> bool {
    projected.to_bits() == (callback as f32).to_bits()
}

fn same_f32_array(projected: &[f32; 10], callback: &[f64; 10]) -> bool {
    projected
        .iter()
        .zip(callback)
        .all(|(left, right)| left.to_bits() == (*right as f32).to_bits())
}

fn add_hits(target: &mut FieldHits, matches: &Matches) {
    target.name += usize::from(matches.name);
    target.price += usize::from(matches.price);
    target.last_close += usize::from(matches.last_close);
    target.open += usize::from(matches.open);
    target.high += usize::from(matches.high);
    target.low += usize::from(matches.low);
    target.volume += usize::from(matches.volume);
    target.amount += usize::from(matches.amount);
    target.ask_prices += usize::from(matches.ask_prices);
    target.ask_volumes += usize::from(matches.ask_volumes);
    target.bid_prices += usize::from(matches.bid_prices);
    target.bid_volumes += usize::from(matches.bid_volumes);
    target.timestamp += usize::from(matches.timestamp);
}

fn parse_callback_timestamp(value: &str) -> Option<u32> {
    let bytes = value.as_bytes();
    if bytes.len() != 19
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b' '
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    let year = decimal(&bytes[0..4])? as i64;
    let month = decimal(&bytes[5..7])? as i64;
    let day = decimal(&bytes[8..10])? as i64;
    let hour = decimal(&bytes[11..13])? as i64;
    let minute = decimal(&bytes[14..16])? as i64;
    let second = decimal(&bytes[17..19])? as i64;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    // Vendor callback datetimes are Asia/Shanghai local time (UTC+8).
    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second
        - 8 * 3_600;
    u32::try_from(seconds).ok()
}

fn decimal(bytes: &[u8]) -> Option<u32> {
    bytes.iter().try_fold(0, |value, byte| {
        byte.is_ascii_digit()
            .then_some(value * 10 + u32::from(*byte - b'0'))
    })
}

// Howard Hinnant's civil-date conversion, returning days since 1970-01-01.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_quote(datetime: &str) -> WineQuote {
        WineQuote {
            market: "SH".to_string(),
            code: "600000".to_string(),
            name: "fixture".to_string(),
            datetime: datetime.to_string(),
            price: 0.0,
            last_close: 0.0,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            volume: 0.0,
            amount: 0.0,
            ask_prices: [0.0; 10],
            ask_volumes: [0.0; 10],
            bid_prices: [0.0; 10],
            bid_volumes: [0.0; 10],
        }
    }

    fn fixture_batch(sequence: u64, timestamp_ms: u64, datetime: &str) -> IndexedBatch {
        IndexedBatch {
            sequence,
            timestamp_ms,
            quotes: vec![IndexedQuote {
                batch_sequence: sequence,
                batch_timestamp_ms: timestamp_ms,
                quote: fixture_quote(datetime),
            }],
        }
    }

    #[test]
    fn business_timestamp_join_is_explicit() {
        assert_eq!(JoinPolicy::parse(None).unwrap(), JoinPolicy::Nearest);
        assert_eq!(
            JoinPolicy::parse(Some(std::ffi::OsStr::new("business-ts"))).unwrap(),
            JoinPolicy::BusinessTimestamp
        );
        assert!(JoinPolicy::parse(Some(std::ffi::OsStr::new("loose"))).is_err());
    }

    #[test]
    fn business_timestamp_join_ignores_wall_clock_window() {
        let business_timestamp = parse_callback_timestamp("2026-09-01 15:00:00").unwrap();
        let frame_micros = 1_000_000;
        let batches = [fixture_batch(7, 3_000, "2026-09-01 15:00:00")];
        let matched = nearest_business_timestamp_batch(
            &batches,
            &("SH".to_string(), "600000".to_string()),
            business_timestamp,
            frame_micros,
            250,
        )
        .expect("same business second must match beyond the wall-clock window");
        assert_eq!(matched.0.sequence, 7);
        assert_eq!(matched.2, 2_000_000);
    }

    #[test]
    fn business_timestamp_join_rejects_nearby_wrong_state() {
        let business_timestamp = parse_callback_timestamp("2026-09-01 15:00:00").unwrap();
        let batches = [fixture_batch(8, 1_100, "2026-09-01 15:00:01")];
        assert!(
            nearest_business_timestamp_batch(
                &batches,
                &("SH".to_string(), "600000".to_string()),
                business_timestamp,
                1_000_000,
                250,
            )
            .is_none()
        );
    }

    #[test]
    fn business_timestamp_join_reports_ambiguity_and_uses_wall_clock_tie_break() {
        let datetime = "2026-09-01 15:00:00";
        let business_timestamp = parse_callback_timestamp(datetime).unwrap();
        let batches = [
            fixture_batch(9, 1_900, datetime),
            fixture_batch(10, 1_100, datetime),
        ];
        let key = ("SH".to_string(), "600000".to_string());
        assert_eq!(
            business_timestamp_candidate_stats(&batches, &key, business_timestamp, 1_000_000, 250,),
            BusinessTimestampCandidateStats {
                count: 2,
                distinct_states: 1,
            }
        );
        let matched =
            nearest_business_timestamp_batch(&batches, &key, business_timestamp, 1_000_000, 250)
                .unwrap();
        assert_eq!(matched.0.sequence, 10);
    }

    #[test]
    fn business_timestamp_join_prefers_live_shape_by_sequence() {
        let datetime = "2026-09-01 15:00:00";
        let business_timestamp = parse_callback_timestamp(datetime).unwrap();
        let leftover = fixture_batch(9, 1_900, datetime);
        let mut live = fixture_batch(10, 1_100, datetime);
        live.quotes[0].quote.price = 12.34;
        let batches = [leftover, live];
        let key = ("SH".to_string(), "600000".to_string());
        assert!(business_timestamp_live_leftover_shape(
            &batches,
            &key,
            business_timestamp
        ));
        let matched =
            nearest_business_timestamp_batch(&batches, &key, business_timestamp, 1_000_000, 250)
                .unwrap();
        assert_eq!(matched.0.sequence, 10);
        assert_eq!(matched.1.price, 12.34);
    }

    #[test]
    fn code_table_decimal_places_define_price_scale() {
        let record = CodeTableRecord {
            symbol_index: 1,
            code: "TARGET".to_string(),
            name: "fixture".to_string(),
            opaque_tail: [
                2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        };
        assert_eq!(record.price_scale(), Some(100.0));
    }

    #[test]
    fn parses_vendor_local_datetime_as_unix_seconds() {
        assert_eq!(
            parse_callback_timestamp("2026-09-01 15:00:00"),
            Some(1_788_246_000)
        );
        assert_eq!(parse_callback_timestamp("bad"), None);
    }

    #[test]
    fn compares_callback_numbers_with_float32_semantics() {
        assert!(same_f32(25.17_f32, 25.17));
        assert!(same_f32(25_934_317_i64 as f32, 25_934_316.0));
        assert!(!same_f32(25_934_317_i64 as f32, 25_934_334.0));
    }
}
