use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use netzipapi_rust_demo::extract_official_5188_frames;
use netzip_fullpull::Official5188MapBaselineResolver;
use serde::Serialize;

#[derive(Serialize)]
struct ManifestEntry {
    src: String,
    dst: String,
    frame_index: usize,
    completed_at_micros: Option<u64>,
    kind: String,
    wire_kind: String,
    metadata_hex: String,
    payload_len: usize,
    payload_file: String,
    decoded_zlib_file: Option<String>,
    code_table_file: Option<String>,
    delta_value_file: Option<String>,
    delta_index_file: Option<String>,
    delta_indexes_file: Option<String>,
    delta_index_count: Option<usize>,
    decoded_values_file: Option<String>,
    decoded_value_error: Option<String>,
}

#[derive(Serialize)]
struct ResolvedDeltaIndex {
    market: String,
    symbol_index: u16,
    timestamp: u32,
    uses_baseline: bool,
    code: Option<String>,
    name: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let capture = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: official_5188_extract CAPTURE OUTPUT_DIR [BASELINE_CORE]")?;
    let output_dir = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: official_5188_extract CAPTURE OUTPUT_DIR [BASELINE_CORE]")?;
    let baseline_core = args.next().map(PathBuf::from);
    if args.next().is_some() {
        return Err("usage: official_5188_extract CAPTURE OUTPUT_DIR [BASELINE_CORE]".into());
    }

    fs::create_dir_all(&output_dir)?;
    let frames = extract_official_5188_frames(capture)?;
    let mut code_tables = HashMap::new();
    for table in frames.iter().filter_map(|frame| frame.code_table.as_ref()) {
        let replace = code_tables.get(&table.market).is_none_or(
            |current: &netzip_fullpull::Official5188CodeTable| {
                current.records.len() < table.records.len()
            },
        );
        if replace {
            code_tables.insert(table.market, table.clone());
        }
    }
    let mut baseline_resolver = Official5188MapBaselineResolver::new();
    if let Some(core_path) = baseline_core.as_deref() {
        let wanted = frames
            .iter()
            .filter_map(|frame| frame.delta_indexes.as_deref())
            .flatten()
            .map(|index| (index.market, index.symbol_index))
            .collect::<HashSet<_>>();
        let baselines = load_core_baselines(core_path, &wanted)?;
        for ((market, symbol_index), record) in &baselines {
            baseline_resolver.insert(*market, *symbol_index, record.clone());
        }
        eprintln!("loaded {} Wine core baselines", baselines.len());
    }
    let mut manifest = Vec::with_capacity(frames.len());
    for (ordinal, frame) in frames.into_iter().enumerate() {
        let stem = format!(
            "{ordinal:04}-{}-{}-{}",
            frame.completed_at_micros.unwrap_or_default(),
            endpoint_slug(&frame.dst),
            frame.wire_kind
        );
        let payload_file = format!("{stem}.payload.bin");
        fs::write(output_dir.join(&payload_file), &frame.payload)?;
        let decoded_zlib_file = write_optional(
            &output_dir,
            &stem,
            "zlib.bin",
            frame.decoded_zlib_object.as_deref(),
        )?;
        let code_table_file = match frame.code_table.as_ref() {
            Some(table) => {
                let name = format!("{stem}.code-table.json");
                fs::write(output_dir.join(&name), serde_json::to_vec_pretty(table)?)?;
                Some(name)
            }
            None => None,
        };
        let delta_value_file = write_optional(
            &output_dir,
            &stem,
            "delta-values.bin",
            frame.delta_value_stream.as_deref(),
        )?;
        let delta_index_file = write_optional(
            &output_dir,
            &stem,
            "delta-index.bin",
            frame.delta_index_stream.as_deref(),
        )?;
        let delta_index_count = frame.delta_indexes.as_ref().map(Vec::len);
        let delta_indexes_file = match frame.delta_indexes.as_deref() {
            Some(indexes) => {
                let name = format!("{stem}.delta-indexes.json");
                let resolved = indexes
                    .iter()
                    .map(|index| {
                        let record = code_tables
                            .get(&index.market)
                            .and_then(|table| table.record(index.symbol_index))
                            .filter(|record| !record.code.is_empty());
                        ResolvedDeltaIndex {
                            market: String::from_utf8_lossy(&index.market).into_owned(),
                            symbol_index: index.symbol_index,
                            timestamp: index.timestamp,
                            uses_baseline: index.uses_baseline,
                            code: record.map(|record| record.code.clone()),
                            name: record.map(|record| record.name.clone()),
                        }
                    })
                    .collect::<Vec<_>>();
                fs::write(
                    output_dir.join(&name),
                    serde_json::to_vec_pretty(&resolved)?,
                )?;
                Some(name)
            }
            None => None,
        };
        let (decoded_values_file, decoded_value_error) = match (
            frame.delta_value_stream.as_deref(),
            frame.delta_index_stream.as_deref(),
            frame.delta_indexes.as_deref(),
        ) {
            (Some(value_stream), Some(index_stream), Some(indexes)) => {
                let streams = netzip_fullpull::Official5188DeltaStreams {
                    record_count: indexes.len(),
                    value_stream,
                    index_stream,
                };
                match netzip_fullpull::decode_official_5188_values(
                    streams,
                    indexes,
                    &mut baseline_resolver,
                ) {
                    Ok(decoded) => {
                        baseline_resolver.update_from_decoded(&decoded);
                        let name = format!("{stem}.decoded-values.json");
                        fs::write(output_dir.join(&name), serde_json::to_vec_pretty(&decoded)?)?;
                        (Some(name), None)
                    }
                    Err(error) => (None, Some(error)),
                }
            }
            _ => (None, None),
        };
        manifest.push(ManifestEntry {
            src: frame.src,
            dst: frame.dst,
            frame_index: frame.frame_index,
            completed_at_micros: frame.completed_at_micros,
            kind: frame.kind,
            wire_kind: frame.wire_kind,
            metadata_hex: hex(&frame.metadata),
            payload_len: frame.payload.len(),
            payload_file,
            decoded_zlib_file,
            code_table_file,
            delta_value_file,
            delta_index_file,
            delta_indexes_file,
            delta_index_count,
            decoded_values_file,
            decoded_value_error,
        });
    }
    fs::write(
        output_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!(
        "exported {} complete frames to {}",
        manifest.len(),
        output_dir.display()
    );
    Ok(())
}

fn load_core_baselines(
    path: &Path,
    wanted: &HashSet<([u8; 2], u16)>,
) -> Result<HashMap<([u8; 2], u16), netzip_fullpull::Official5188InternalRecord>, Box<dyn Error>> {
    let core = fs::read(path)?;
    let mut found: HashMap<_, (u32, usize, netzip_fullpull::Official5188InternalRecord)> =
        HashMap::new();
    for position in 0xe1..core.len().saturating_sub(2) {
        let market = [core[position], core[position + 1]];
        if !matches!(&market, b"SH" | b"SZ") {
            continue;
        }
        let symbol_index = u16::from_le_bytes([core[position - 2], core[position - 1]]);
        let key = (market, symbol_index);
        if !wanted.contains(&key) {
            continue;
        }
        let start = position - 0xe1;
        let end = start + netzip_fullpull::OFFICIAL_5188_INTERNAL_RECORD_LEN;
        if end > core.len() {
            continue;
        }
        let record = netzip_fullpull::Official5188InternalRecord::decode(&core[start..end])?;
        let timestamp = record.timestamp();
        if !(1_700_000_000..=1_900_000_000).contains(&timestamp) {
            continue;
        }
        let nonzero = record.as_bytes().iter().filter(|byte| **byte != 0).count();
        let replace = found
            .get(&key)
            .is_none_or(|(old_timestamp, old_nonzero, _)| {
                (timestamp, nonzero) > (*old_timestamp, *old_nonzero)
            });
        if replace {
            found.insert(key, (timestamp, nonzero, record));
        }
    }
    Ok(found
        .into_iter()
        .map(|(key, (_, _, record))| (key, record))
        .collect())
}

fn write_optional(
    output_dir: &Path,
    stem: &str,
    suffix: &str,
    bytes: Option<&[u8]>,
) -> Result<Option<String>, Box<dyn Error>> {
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    let name = format!("{stem}.{suffix}");
    fs::write(output_dir.join(&name), bytes)?;
    Ok(Some(name))
}

fn endpoint_slug(endpoint: &str) -> String {
    endpoint.replace(['.', ':'], "-")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
