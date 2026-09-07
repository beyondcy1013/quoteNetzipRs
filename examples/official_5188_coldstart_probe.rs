//! Cold-start replay probe for official 5188 `2704` frames.
//!
//! ```text
//! official_5188_coldstart_probe <pcap> [dst-filter|-] [mode]
//!   mode 0  0104 seeds as metadata only + fresh fallback (Wine-equivalent)
//!   mode 1  0104 seeds inserted as baselines, strict decoder (negative control)
//! env PROBE_DUMP=<out.json>   dump every decoded record (idx, raw record hex, ...)
//! env PROBE_SCAN=<truth.json> byte-alignment scan of each frame tail
//! ```
//!
//! The dump can be compared field-by-field / byte-by-byte with a memory
//! snapshot of the vendor client taken after the same cold start, see
//! `docs/codex/tasks/cold-start-parity-plan-2026-09-04.md`.
use std::collections::HashMap;

use netzip_fullpull::{
    Official5188DeltaStreams, Official5188InternalRecord, Official5188MapBaselineResolver,
    decode_official_5188_values, decode_official_5188_values_with_fresh_fallback,
};
use netzipapi_rust_demo::extract_official_5188_frames;

fn main() {
    let mut args = std::env::args().skip(1);
    let capture = args.next().expect("capture path");
    let dst_filter = args.next().filter(|v| !v.is_empty() && v != "-"); // e.g. 192.168.3.2:60860
    // mode: 1 = 0104 seed inserted as baseline (strict decoder), 0 = seed is
    // metadata only + fresh fallback (Wine-equivalent cold start)
    let mode: u8 = args.next().and_then(|v| v.parse().ok()).unwrap_or(1);
    let seed_as_baseline = mode == 1;
    let frames = extract_official_5188_frames(&capture).expect("extract frames");
    // pick the largest code table per market
    let mut tables: HashMap<[u8; 2], netzip_fullpull::Official5188CodeTable> = HashMap::new();
    for t in frames.iter().filter_map(|f| f.code_table.as_ref()) {
        let replace = tables
            .get(&t.market)
            .is_none_or(|cur| cur.records.len() < t.records.len());
        if replace {
            tables.insert(t.market, t.clone());
        }
    }
    let mut names: HashMap<([u8; 2], u16), (String, String, u8)> = HashMap::new();
    let mut resolver = Official5188MapBaselineResolver::new();
    for t in tables.values() {
        for r in &t.records {
            names.insert(
                (t.market, r.symbol_index),
                (r.code.clone(), r.name.clone(), r.opaque_tail[1]),
            );
            let rec =
                Official5188InternalRecord::from_code_table_metadata(t.market, r.symbol_index, r);
            if seed_as_baseline {
                resolver.insert(t.market, r.symbol_index, rec);
            }
        }
    }
    if !seed_as_baseline {
        let v: Vec<_> = tables.values().cloned().collect();
        resolver.seed_code_tables(&v);
    }
    eprintln!(
        "frames={} tables={} seeded={} seed_as_baseline={}",
        frames.len(),
        tables.len(),
        resolver.len(),
        seed_as_baseline
    );
    let watch = [
        "000001", "600000", "600259", "000002", "300750", "399001", "159915", "688981",
    ];
    let dump_path = std::env::var("PROBE_DUMP").ok();
    // PROBE_SCAN=<truth.json>: {"SH:600000": {"last_close": 9.28, ...}, ...}
    let scan_truth: Option<HashMap<String, f64>> = std::env::var("PROBE_SCAN").ok().map(|p| {
        let v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&p).expect("truth file")).expect("truth json");
        v.as_object()
            .unwrap()
            .iter()
            .filter_map(|(k, v)| {
                v.get("last_close")
                    .and_then(|x| x.as_f64())
                    .map(|x| (k.clone(), x))
            })
            .collect()
    });
    let mut scanned = 0usize;
    let mut dump: Vec<serde_json::Value> = Vec::new();
    let mut ok = 0usize;
    let mut fail = 0usize;
    let mut first_err: Option<String> = None;
    let mut printed = 0;
    for f in frames.iter().filter(|f| f.wire_kind == "2704") {
        if let Some(filter) = dst_filter.as_deref()
            && f.src != filter
            && f.dst != filter
        {
            continue;
        }
        let (Some(vs), Some(is), Some(idx)) = (
            f.delta_value_stream.as_deref(),
            f.delta_index_stream.as_deref(),
            f.delta_indexes.as_deref(),
        ) else {
            continue;
        };
        let streams = Official5188DeltaStreams {
            record_count: idx.len(),
            value_stream: vs,
            index_stream: is,
        };
        if let Some(truth) = scan_truth.as_ref() {
            // Alignment scan: decode the frame tail starting at byte k with
            // the fresh-fallback decoder and score close-price matches.
            if idx.len() < 2 {
                continue;
            }
            let first_record = decode_official_5188_values_with_fresh_fallback(
                Official5188DeltaStreams {
                    record_count: idx.len(),
                    value_stream: vs,
                    index_stream: is,
                },
                idx,
                &mut resolver,
            )
            .ok()
            .and_then(|r| r.first().cloned());
            let first_bytes = first_record
                .as_ref()
                .map(|d| d.bit_end.div_ceil(8))
                .unwrap_or(0);
            let mut scores: Vec<(usize, usize, usize)> = Vec::new();
            let max_k = vs.len().saturating_sub(1).min(160);
            for k in 1..=max_k {
                let sub = Official5188DeltaStreams {
                    record_count: idx.len() - 1,
                    value_stream: &vs[k..],
                    index_stream: is,
                };
                let attempt = match mode {
                    1 => decode_official_5188_values(sub, &idx[1..], &mut resolver),
                    _ => decode_official_5188_values_with_fresh_fallback(
                        sub,
                        &idx[1..],
                        &mut resolver,
                    ),
                };
                let Ok(records) = attempt else {
                    scores.push((k, 0, 0));
                    continue;
                };
                let mut n = 0usize;
                let mut good = 0usize;
                for d in &records {
                    let key = (d.index.market, d.index.symbol_index);
                    let Some((code, _name, scale)) = names.get(&key) else {
                        continue;
                    };
                    let mk = String::from_utf8_lossy(&d.index.market).to_string();
                    let Some(t) = truth.get(&format!("{mk}:{code}")) else {
                        continue;
                    };
                    n += 1;
                    let raw = d.record.last_price_integer() as f64;
                    let _ = scale;
                    if raw > 0.0
                        && [1.0, 10.0, 100.0, 1000.0, 10000.0]
                            .iter()
                            .any(|s| ((raw / s) - t).abs() < 0.0051)
                    {
                        good += 1;
                    }
                }
                scores.push((k, good, n));
            }
            scores.sort_by_key(|entry| std::cmp::Reverse(entry.1));
            let top: Vec<String> = scores
                .iter()
                .take(4)
                .map(|(k, g, n)| format!("k={k}:{g}/{n}"))
                .collect();
            println!(
                "frame records={} value_bytes={} first_record_bytes={first_bytes} top={}",
                idx.len(),
                vs.len(),
                top.join(" ")
            );
            scanned += 1;
            if scanned >= 12 {
                break;
            }
            continue;
        }
        let decoded = match mode {
            1 => decode_official_5188_values(streams, idx, &mut resolver),
            _ => decode_official_5188_values_with_fresh_fallback(streams, idx, &mut resolver),
        };
        match decoded {
            Ok(records) => {
                ok += 1;
                resolver.update_from_decoded(&records);
                for d in &records {
                    let key = (d.index.market, d.index.symbol_index);
                    let Some((code, name, scale)) = names.get(&key) else {
                        continue;
                    };
                    let mk = String::from_utf8_lossy(&d.index.market).to_string();
                    if dump_path.is_some() {
                        let r = &d.record;
                        let b = r.as_bytes();
                        let last_close = i32::from_le_bytes(b[0x12b..0x12f].try_into().unwrap());
                        dump.push(serde_json::json!({
                            "market": mk, "code": code, "name": name, "scale": scale,
                            "mask": d.mask, "header": format!("{:?}", d.header),
                            "uses_baseline": d.index.uses_baseline, "ts": r.timestamp(),
                            "open": r.open_price_integer(), "high": r.high_price_integer(),
                            "low": r.low_price_integer(), "close": r.last_price_integer(),
                            "volume": r.volume_integer(), "amount": r.amount_integer(),
                            "last_close": last_close,
                            "bid1": r.ladder_price_integers()[4], "ask1": r.ladder_price_integers()[5],
                            "bits": d.bit_end - d.bit_start, "bit_start": d.bit_start,
                            "idx": d.index.symbol_index, "frame_us": f.completed_at_micros,
                            "frame_index": f.frame_index, "src": f.src, "dst": f.dst,
                            "record_hex": b.iter().map(|x| format!("{x:02x}")).collect::<String>(),
                        }));
                    }
                    if watch.contains(&code.as_str()) && printed < 40 {
                        printed += 1;
                        let r = &d.record;
                        let b = r.as_bytes();
                        let last_close = i32::from_le_bytes(b[0x12b..0x12f].try_into().unwrap());
                        println!(
                            "{mk} {code} {name:<8} scale={scale:<3} mask={:#04x} uses_baseline={} ts={} O={} H={} L={} C={} V={} A={} 昨收={} bid1={} ask1={}",
                            d.mask,
                            d.index.uses_baseline,
                            r.timestamp(),
                            r.open_price_integer(),
                            r.high_price_integer(),
                            r.low_price_integer(),
                            r.last_price_integer(),
                            r.volume_integer(),
                            r.amount_integer(),
                            last_close,
                            r.ladder_price_integers()[4],
                            r.ladder_price_integers()[5],
                        );
                    }
                }
            }
            Err(e) => {
                fail += 1;
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
    }
    eprintln!("decoded_frames_ok={ok} failed={fail} first_err={first_err:?}");
    if let Some(path) = dump_path {
        std::fs::write(&path, serde_json::to_vec(&dump).unwrap()).unwrap();
        eprintln!("dumped {} records to {path}", dump.len());
    }
}
