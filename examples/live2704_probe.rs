//! Live 2704 delta-index probe (2026-09-03).
//! Reads NETZIP_NATIVE_DUMP_DIR payloads (driver dump switch) and verifies
//! each 2704 frame parses via decode_official_5188_delta_indexes.
//! Usage: live2704_probe <dump_dir> [code_table_dir] [baseline_core]
//!
//! NOTE: the earlier guessed exponent/multiplier-only seed was removed after
//! replay disproved it. Code-table seeding now belongs to the shared resolver.

// 一次性: 读 live dump 的 2704 payload, 解析 delta envelope + indexes
use netzip_fullpull::{
    Official5188DeltaEnvelope, Official5188Frame, Official5188Kind,
    Official5188MapBaselineResolver, decode_official_5188_delta_indexes,
    decode_official_5188_values,
};
use std::fs;

fn main() {
    let mut args = std::env::args().skip(1);
    let dump_dir = args.next().expect("dump dir");
    let code_table_dir = args.next();
    let baseline_core = args.next();
    // optional code-table JSON dir for symbol->code mapping
    let mut code_map: std::collections::HashMap<(String, u16), (String, String)> =
        std::collections::HashMap::new();
    if let Some(dir) = &code_table_dir {
        for entry in fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if !name.ends_with("0104.code-table.json") {
                continue;
            }
            let v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            let market = v["market"].as_array().unwrap();
            let market = format!(
                "{}{}",
                market[0].as_u64().unwrap() as u8 as char,
                market[1].as_u64().unwrap() as u8 as char
            );
            for rec in v["records"].as_array().unwrap() {
                let idx = rec["symbol_index"].as_u64().unwrap() as u16;
                let code = rec["code"].as_str().unwrap().to_string();
                let name = rec["name"].as_str().unwrap().to_string();
                if !code.is_empty() {
                    code_map.insert((market.clone(), idx), (code, name));
                }
            }
        }
        eprintln!("loaded {} code-table entries", code_map.len());
    }
    let mut paths: Vec<_> = fs::read_dir(&dump_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| {
                    (n.contains("-2704-") || n.contains("-2704.")) && n.ends_with(".payload.bin")
                })
                .unwrap_or(false)
        })
        .collect();
    paths.sort();
    println!("2704 payload files: {}", paths.len());
    let mut resolver = Official5188MapBaselineResolver::new();
    let mut decoded_symbols: Vec<(String, u16, Vec<u8>)> = Vec::new();
    if let Some(core_path) = &baseline_core {
        let core = fs::read(core_path).expect("read core");
        let mut found = 0usize;
        for position in 0xe1..core.len().saturating_sub(2) {
            let market = [core[position], core[position + 1]];
            if !matches!(&market, b"SH" | b"SZ") {
                continue;
            }
            let start = position - 0xe1;
            let end = start + netzip_fullpull::OFFICIAL_5188_INTERNAL_RECORD_LEN;
            if end > core.len() {
                continue;
            }
            if let Ok(rec) = netzip_fullpull::Official5188InternalRecord::decode(&core[start..end])
            {
                resolver.insert(
                    market,
                    u16::from_le_bytes([core[position - 2], core[position - 1]]),
                    rec,
                );
                found += 1;
            }
        }
        eprintln!("loaded {found} baselines from core");
    }

    let mut frame_ok = 0usize;
    let mut frame_fail = 0usize;
    let mut first_err: Option<String> = None;
    for p in paths.iter() {
        let payload = fs::read(p).unwrap();
        let frame = Official5188Frame {
            kind: Official5188Kind::SERVER_DELTA,
            metadata: [0; 4],
            payload,
        };
        let envelope = match Official5188DeltaEnvelope::decode(&frame) {
            Ok(e) => e,
            Err(e) => {
                if first_err.is_none() {
                    first_err = Some(format!("envelope: {e}"));
                }
                frame_fail += 1;
                continue;
            }
        };
        let streams = match envelope.split_streams() {
            Ok(s) => s,
            Err(e) => {
                if first_err.is_none() {
                    first_err = Some(format!("split: {e}"));
                }
                frame_fail += 1;
                continue;
            }
        };
        let indexes = match decode_official_5188_delta_indexes(streams) {
            Ok(v) => v,
            Err(e) => {
                if first_err.is_none() {
                    first_err = Some(format!("indexes: {e}"));
                }
                frame_fail += 1;
                continue;
            }
        };
        match decode_official_5188_values(streams, &indexes, &mut resolver) {
            Ok(decoded) => {
                resolver.update_from_decoded(&decoded);
                for d in decoded {
                    decoded_symbols.push((
                        String::from_utf8_lossy(&d.index.market).into_owned(),
                        d.index.symbol_index,
                        d.record.as_bytes().to_vec(),
                    ));
                }
                frame_ok += 1;
            }
            Err(e) => {
                if first_err.is_none() {
                    first_err = Some(format!("values: {e}"));
                }
                frame_fail += 1;
            }
        }
    }
    if let Some(e) = &first_err {
        eprintln!("first error: {e}");
    }
    eprintln!(
        "frames ok={frame_ok} fail={frame_fail} decoded_symbol_records={}",
        decoded_symbols.len()
    );
    // unique symbols decoded and their last price int (offset 0x10)
    let mut latest: std::collections::HashMap<(String, u16), Vec<u8>> =
        std::collections::HashMap::new();
    for (m, idx, rec) in decoded_symbols {
        latest.insert((m, idx), rec);
    }
    eprintln!("unique symbols decoded: {}", latest.len());
    let mut i = 0;
    for ((m, idx), rec) in &latest {
        if i >= 20 {
            break;
        }
        let code_name = code_map
            .get(&(m.clone(), *idx))
            .cloned()
            .unwrap_or_default();
        if !code_map.is_empty() && code_name.0.is_empty() {
            continue;
        }
        let last = i32::from_le_bytes([rec[0x10], rec[0x11], rec[0x12], rec[0x13]]);
        let open = i32::from_le_bytes([rec[0x04], rec[0x05], rec[0x06], rec[0x07]]);
        let ts = u32::from_le_bytes([rec[0], rec[1], rec[2], rec[3]]);
        println!(
            "{m} idx={idx} code={} name={} last_int={last} open_int={open} ts={ts}",
            code_name.0, code_name.1
        );
        i += 1;
    }
}
