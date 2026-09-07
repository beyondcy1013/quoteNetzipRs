use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use netzipapi_rust_demo::tdx_0547_delivery::Tdx0547DeliveryKind;
use netzipapi_rust_demo::{Tdx7709Config, Tdx7709QuoteRequestItem, Tdx7709Session};
use serde_json::json;

const SHANGHAI_UTC_OFFSET_SECONDS: i64 = 8 * 60 * 60;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let options = Options::parse(std::env::args().skip(1))?;
    let (items, symbol_source) = if let Some(path) = &options.worklist_json {
        (
            load_worklist(path, options.max_symbols)?,
            "gateway_worklist",
        )
    } else {
        let code_table = Tdx7709Session::open(&Tdx7709Config::default())?
            .sync_result()
            .records;
        (
            select_symbols(&code_table, options.max_symbols),
            "tdx_code_table",
        )
    };
    if items.is_empty() {
        return Err("code table contained no supported six-digit symbols".into());
    }

    if items.len() > options.connections * options.batch_size {
        return Err(format!(
            "{} symbols require at least {} connections at batch-size {}",
            items.len(),
            items.len().div_ceil(options.batch_size),
            options.batch_size
        )
        .into());
    }

    let session_started = SystemTime::now();
    let mut initial_records = 0usize;
    let mut observations = Vec::new();
    let mut workers = Vec::new();
    for batch in items.chunks(options.batch_size) {
        let batch = batch.to_vec();
        let duration = options.duration;
        workers.push(std::thread::spawn(move || -> Result<_, String> {
            let mut session = Tdx7709Session::open_quote_only(&Tdx7709Config::default())
                .map_err(|error| error.to_string())?;
            let initial = session
                .request_live_quotes(&batch)
                .map_err(|error| error.to_string())?;
            let initial_records = initial
                .quote_bodies
                .iter()
                .map(|body| body.records.len())
                .sum::<usize>();
            let observation = session
                .collect_quote_delivery_observation(duration)
                .map_err(|error| error.to_string())?;
            Ok((initial_records, observation))
        }));
    }
    for worker in workers {
        let (worker_initial_records, observation) = worker
            .join()
            .map_err(|_| "shadow connection worker panicked")??;
        initial_records += worker_initial_records;
        observations.push(observation);
    }
    let total_session_ms = session_started.elapsed()?.as_millis();

    let subscribed = items
        .iter()
        .map(|item| (item.market, item.code.clone()))
        .collect::<BTreeSet<_>>();
    let subscribed_batch = items
        .iter()
        .enumerate()
        .map(|(index, item)| ((item.market, item.code.clone()), index / options.batch_size))
        .collect::<BTreeMap<_, _>>();
    let mut pushed_symbols = BTreeSet::new();
    let mut last_received_ms = BTreeMap::<(u8, String), u128>::new();
    let mut intervals_ms = Vec::new();
    let mut latencies_ms = Vec::new();
    let mut push_events = Vec::new();
    let mut unsolicited_deliveries = 0usize;
    let mut solicited_deliveries = 0usize;
    let mut unsolicited_records = 0usize;
    let mut solicited_records = 0usize;

    for timed in observations
        .iter()
        .flat_map(|observation| &observation.deliveries)
    {
        let received_ms = unix_millis(timed.received_at)?;
        let unsolicited = timed.delivery.kind == Tdx0547DeliveryKind::UnsolicitedUpdate;
        if unsolicited {
            unsolicited_deliveries += 1;
        } else {
            solicited_deliveries += 1;
        }
        for record in &timed.delivery.body.records {
            if unsolicited {
                unsolicited_records += 1;
            } else {
                solicited_records += 1;
            }
            let key = (record.market, record.code.clone());
            if !subscribed.contains(&key) {
                continue;
            }
            pushed_symbols.insert(key.clone());
            push_events.push((received_ms, key.clone()));
            if let Some(previous) = last_received_ms.insert(key, received_ms) {
                intervals_ms.push(received_ms.saturating_sub(previous));
            }
            if let Some(source_time) = record.time_hhmmss_raw
                && let Some(latency) = source_latency_ms(timed.received_at, source_time)
            {
                latencies_ms.push(latency);
            }
        }
    }

    let coverage = pushed_symbols.len() as f64 / subscribed.len() as f64;
    let mut pushed_by_batch = vec![0usize; items.len().div_ceil(options.batch_size)];
    for symbol in &pushed_symbols {
        if let Some(batch) = subscribed_batch.get(symbol) {
            pushed_by_batch[*batch] += 1;
        }
    }
    let pushed_symbol_sample = pushed_symbols
        .iter()
        .take(30)
        .map(|(market, code)| format!("{}{}", if *market == 0 { "SZ" } else { "SH" }, code))
        .collect::<Vec<_>>();
    let coalescing = coalescing_summary(&push_events, options.coalesce_ms, 100);
    let coalescing_candidates = [50u128, 100, 200, 500, 1_000]
        .into_iter()
        .map(|window_ms| coalescing_summary(&push_events, window_ms, 100))
        .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "mode": "isolated_shadow_no_publication",
            "symbol_source": symbol_source,
            "duration_ms": options.duration.as_millis(),
            "subscribed_symbols": subscribed.len(),
            "connections": observations.len(),
            "subscription_batches": items.len().div_ceil(options.batch_size),
            "total_session_ms": total_session_ms,
            "initial_records": initial_records,
            "unsolicited_deliveries": unsolicited_deliveries,
            "unsolicited_records": unsolicited_records,
            "solicited_deliveries": solicited_deliveries,
            "solicited_records": solicited_records,
            "pushed_symbols": pushed_symbols.len(),
            "pushed_symbols_by_subscription_batch": pushed_by_batch,
            "pushed_symbol_sample": pushed_symbol_sample,
            "push_coverage_ratio": coverage,
            "source_latency_ms": distribution(&mut latencies_ms),
            "per_symbol_update_interval_ms": distribution_unsigned(&mut intervals_ms),
            "coalescing": coalescing,
            "coalescing_candidates": coalescing_candidates,
            "socket": {
                "bytes_read": observations.iter().map(|item| item.bytes_read).sum::<usize>(),
                "read_count": observations.iter().map(|item| item.read_count).sum::<usize>(),
                "timeout_count": observations.iter().map(|item| item.timeout_count).sum::<usize>(),
                "pending_bytes": observations.iter().map(|item| item.pending_bytes).sum::<usize>(),
                "reconnect_count": 0,
            }
        }))?
    );
    Ok(())
}

struct Options {
    duration: Duration,
    max_symbols: usize,
    batch_size: usize,
    connections: usize,
    worklist_json: Option<String>,
    coalesce_ms: u128,
}

impl Options {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, Box<dyn Error>> {
        let mut duration_secs = 30u64;
        let mut max_symbols = 500usize;
        let mut batch_size = 100usize;
        let mut connections = 5usize;
        let mut worklist_json = None;
        let mut coalesce_ms = 100u128;
        let args = args.collect::<Vec<_>>();
        let mut index = 0;
        while index < args.len() {
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("missing value after {}", args[index]))?;
            match args[index].as_str() {
                "--duration-secs" => duration_secs = value.parse()?,
                "--max-symbols" => max_symbols = value.parse()?,
                "--batch-size" => batch_size = value.parse()?,
                "--connections" => connections = value.parse()?,
                "--worklist-json" => worklist_json = Some(value.clone()),
                "--coalesce-ms" => coalesce_ms = value.parse()?,
                flag => return Err(format!("unknown option: {flag}").into()),
            }
            index += 2;
        }
        if duration_secs == 0
            || max_symbols == 0
            || connections == 0
            || coalesce_ms == 0
            || !(1..=100).contains(&batch_size)
        {
            return Err(
                "duration/max-symbols must be positive and batch-size must be 1..=100".into(),
            );
        }
        Ok(Self {
            duration: Duration::from_secs(duration_secs),
            max_symbols,
            batch_size,
            connections,
            worklist_json,
            coalesce_ms,
        })
    }
}

fn coalescing_summary(
    events: &[(u128, (u8, String))],
    window_ms: u128,
    batch_size: usize,
) -> serde_json::Value {
    let Some(origin_ms) = events.iter().map(|(received_ms, _)| *received_ms).min() else {
        return serde_json::Value::Null;
    };
    let mut windows = BTreeMap::<u128, BTreeSet<(u8, String)>>::new();
    for (received_ms, symbol) in events {
        let window = received_ms.saturating_sub(origin_ms) / window_ms;
        windows.entry(window).or_default().insert(symbol.clone());
    }
    let output_records = windows.values().map(BTreeSet::len).sum::<usize>();
    let output_batches = windows
        .values()
        .map(|symbols| symbols.len().div_ceil(batch_size))
        .sum::<usize>();
    json!({
        "window_ms": window_ms,
        "input_records": events.len(),
        "output_records": output_records,
        "output_batches": output_batches,
        "record_reduction_ratio": 1.0 - output_records as f64 / events.len() as f64,
        "maximum_added_hold_ms": window_ms,
    })
}

fn load_worklist(path: &str, limit: usize) -> Result<Vec<Tdx7709QuoteRequestItem>, Box<dyn Error>> {
    let document: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    let rows = document
        .pointer("/payload/data")
        .and_then(serde_json::Value::as_array)
        .ok_or("worklist JSON is missing payload.data[]")?;
    let mut result = Vec::with_capacity(limit.min(rows.len()));
    for row in rows {
        let Some(market) = row.get("market").and_then(serde_json::Value::as_u64) else {
            continue;
        };
        let Some(code) = row.get("code").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if market > 1 || code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        result.push(Tdx7709QuoteRequestItem {
            market: market as u8,
            code: code.to_string(),
            token: 0,
        });
        if result.len() == limit {
            break;
        }
    }
    Ok(result)
}

fn select_symbols(
    records: &[netzipapi_rust_demo::Tdx7709CodeTableRecord],
    limit: usize,
) -> Vec<Tdx7709QuoteRequestItem> {
    let mut by_market = [Vec::new(), Vec::new()];
    for record in records {
        if record.market > 1
            || record.code.len() != 6
            || !record.code.bytes().all(|byte| byte.is_ascii_digit())
        {
            continue;
        }
        by_market[usize::from(record.market)].push(record.code.clone());
    }
    let mut result = Vec::with_capacity(limit);
    let mut offsets = [0usize; 2];
    while result.len() < limit {
        let mut added = false;
        for market in [0usize, 1] {
            if let Some(code) = by_market[market].get(offsets[market]) {
                result.push(Tdx7709QuoteRequestItem {
                    market: market as u8,
                    code: code.clone(),
                    token: 0,
                });
                offsets[market] += 1;
                added = true;
                if result.len() == limit {
                    break;
                }
            }
        }
        if !added {
            break;
        }
    }
    result
}

fn unix_millis(time: SystemTime) -> Result<u128, Box<dyn Error>> {
    Ok(time.duration_since(UNIX_EPOCH)?.as_millis())
}

fn source_latency_ms(received_at: SystemTime, hhmmss: u32) -> Option<i64> {
    let second = i64::from(hhmmss % 100);
    let minute = i64::from((hhmmss / 100) % 100);
    let hour = i64::from(hhmmss / 10_000);
    if hour >= 24 || minute >= 60 || second >= 60 {
        return None;
    }
    let received_ms = received_at.duration_since(UNIX_EPOCH).ok()?.as_millis() as i64;
    let local_ms = received_ms + SHANGHAI_UTC_OFFSET_SECONDS * 1_000;
    let source_ms = (hour * 3_600 + minute * 60 + second) * 1_000;
    Some(local_ms.rem_euclid(86_400_000) - source_ms)
}

fn distribution(values: &mut [i64]) -> serde_json::Value {
    values.sort_unstable();
    if values.is_empty() {
        return serde_json::Value::Null;
    }
    json!({
        "samples": values.len(),
        "min": values[0],
        "p50": percentile(values, 50),
        "p95": percentile(values, 95),
        "max": values[values.len() - 1],
        "negative_samples": values.iter().filter(|value| **value < 0).count(),
    })
}

fn distribution_unsigned(values: &mut [u128]) -> serde_json::Value {
    values.sort_unstable();
    if values.is_empty() {
        return serde_json::Value::Null;
    }
    json!({
        "samples": values.len(),
        "min": values[0],
        "p50": percentile(values, 50),
        "p95": percentile(values, 95),
        "max": values[values.len() - 1],
    })
}

fn percentile<T: Copy>(values: &[T], percentile: usize) -> T {
    values[(values.len().saturating_sub(1) * percentile) / 100]
}
