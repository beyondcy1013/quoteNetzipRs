use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use netzip_fullpull::Official5188MapBaselineResolver;
use netzipapi_rust_demo::extract_official_5188_frames;
use serde::Serialize;

type InternalRecordBySymbol = HashMap<([u8; 2], u16), netzip_fullpull::Official5188InternalRecord>;

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
    decoded_value_trace_file: Option<String>,
    decoded_value_error: Option<String>,
    omitted_tail_records: Option<usize>,
    decoded_value_completed_prefix_bits: Option<usize>,
    decoded_value_stream_bits: Option<usize>,
    decoded_value_remaining_bits: Option<usize>,
    decoded_value_residue_bit_offset: Option<u8>,
    decoded_value_residue_bytes: Option<usize>,
    decoded_value_residue_crc32: Option<String>,
    decoded_value_residue_prefix_hex: Option<String>,
}

#[derive(Serialize)]
struct ResolvedDeltaIndex {
    market: String,
    symbol_index: u16,
    timestamp: u32,
    uses_baseline: bool,
    code: Option<String>,
    name: Option<String>,
    price_scale_hint: Option<f64>,
}

#[derive(Serialize)]
struct SeedProvenance {
    decode_policy: &'static str,
    destination_filter: Option<String>,
    capture_code_tables: usize,
    external_metadata_dir: Option<String>,
    external_metadata_files: Vec<String>,
    metadata_seed_records: usize,
    baseline_core: Option<String>,
    baseline_records: usize,
}

#[derive(Clone, Copy)]
enum DecodePolicy {
    Strict,
    WineTail,
}

impl DecodePolicy {
    fn parse(value: Option<&str>) -> Result<Self, String> {
        match value {
            None | Some("strict") => Ok(Self::Strict),
            Some("wine-tail") => Ok(Self::WineTail),
            Some(value) => Err(format!(
                "unknown decode policy {value:?}; expected strict or wine-tail"
            )),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::WineTail => "wine-tail",
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let usage = "usage: official_5188_extract CAPTURE OUTPUT_DIR [BASELINE_CORE|-] [METADATA_DIR] [DST_FILTER] [strict|wine-tail]";
    let capture = args.next().map(PathBuf::from).ok_or(usage)?;
    let output_dir = args.next().map(PathBuf::from).ok_or(usage)?;
    let baseline_core = args
        .next()
        .and_then(|value| (value != std::ffi::OsStr::new("-")).then(|| PathBuf::from(value)));
    let metadata_dir = args.next().map(PathBuf::from);
    let dst_filter = args
        .next()
        .map(|value| value.to_string_lossy().into_owned());
    let decode_policy_arg = args
        .next()
        .map(|value| value.to_string_lossy().into_owned());
    if args.next().is_some() {
        return Err(usage.into());
    }
    let decode_policy = DecodePolicy::parse(decode_policy_arg.as_deref())?;

    fs::create_dir_all(&output_dir)?;
    let frames = extract_official_5188_frames(capture)?
        .into_iter()
        .filter(|frame| {
            dst_filter
                .as_deref()
                .is_none_or(|dst| frame.src == dst || frame.dst == dst)
        })
        .collect::<Vec<_>>();
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
    let mut initial_baselines = Official5188MapBaselineResolver::new();
    let capture_code_tables = code_tables.len();
    let mut seed_tables = code_tables.values().cloned().collect::<Vec<_>>();
    let external_metadata_files = metadata_dir
        .as_deref()
        .map(load_code_table_directory)
        .transpose()?
        .unwrap_or_default();
    seed_tables.extend(
        external_metadata_files
            .iter()
            .map(|(_, table)| table.clone()),
    );
    let metadata_seed_records = initial_baselines.seed_code_tables(&seed_tables);
    let mut baseline_records = 0;
    if let Some(core_path) = baseline_core.as_deref() {
        let wanted = frames
            .iter()
            .filter_map(|frame| frame.delta_indexes.as_deref())
            .flatten()
            .map(|index| (index.market, index.symbol_index))
            .collect::<HashSet<_>>();
        let baselines = load_core_baselines(core_path, &wanted)?;
        for ((market, symbol_index), record) in &baselines {
            initial_baselines.insert(*market, *symbol_index, record.clone());
        }
        baseline_records = baselines.len();
        eprintln!("loaded {} Wine core baselines", baselines.len());
    }
    let provenance = SeedProvenance {
        decode_policy: decode_policy.name(),
        destination_filter: dst_filter.clone(),
        capture_code_tables,
        external_metadata_dir: metadata_dir
            .as_deref()
            .map(|path| path.display().to_string()),
        external_metadata_files: external_metadata_files
            .iter()
            .map(|(path, _)| path.display().to_string())
            .collect(),
        metadata_seed_records,
        baseline_core: baseline_core
            .as_deref()
            .map(|path| path.display().to_string()),
        baseline_records,
    };
    fs::write(
        output_dir.join("seed-provenance.json"),
        serde_json::to_vec_pretty(&provenance)?,
    )?;
    eprintln!(
        "seeded {} metadata records from {} capture and {} external code tables",
        metadata_seed_records,
        capture_code_tables,
        external_metadata_files.len()
    );
    let mut flow_resolvers: HashMap<(String, String), Official5188MapBaselineResolver> =
        HashMap::new();
    let mut manifest = Vec::with_capacity(frames.len());
    for (ordinal, frame) in frames.into_iter().enumerate() {
        let flow_key = (frame.src.clone(), frame.dst.clone());
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
                        let (code, name, price_scale_hint) = record
                            .map(|record| {
                                (
                                    Some(record.code.clone()),
                                    Some(record.name.clone()),
                                    record.price_scale_hint(),
                                )
                            })
                            .unwrap_or((None, None, None));
                        ResolvedDeltaIndex {
                            market: String::from_utf8_lossy(&index.market).into_owned(),
                            symbol_index: index.symbol_index,
                            timestamp: index.timestamp,
                            uses_baseline: index.uses_baseline,
                            code,
                            name,
                            price_scale_hint,
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
        let (
            decoded_values_file,
            decoded_value_trace_file,
            decoded_value_error,
            omitted_tail_records,
            decoded_value_completed_prefix_bits,
            decoded_value_stream_bits,
            decoded_value_remaining_bits,
            decoded_value_residue_bit_offset,
            decoded_value_residue_bytes,
            decoded_value_residue_crc32,
            decoded_value_residue_prefix_hex,
        ) = match (
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
                // Mirror the production shadow reader: a capture can begin
                // after the connection's absolute baseline frames, so a
                // missing baseline must take the vendor fresh-record path.
                // Keeping the extractor on the strict entry point made its
                // diagnostics incomparable with the live decoder.
                let resolver = flow_resolvers
                    .entry(flow_key)
                    .or_insert_with(|| initial_baselines.clone());
                let traced = match decode_policy {
                    DecodePolicy::Strict => {
                        netzip_fullpull::decode_official_5188_values_partial_with_trace(
                            streams, indexes, resolver,
                        )
                    }
                    DecodePolicy::WineTail => {
                        netzip_fullpull::decode_official_5188_values_partial_with_wine_tail_trace(
                            streams, indexes, resolver,
                        )
                    }
                };
                let outcome = traced.outcome;
                flow_resolvers
                    .get_mut(&(frame.src.clone(), frame.dst.clone()))
                    .expect("flow resolver was inserted before decode")
                    .update_from_decoded(&outcome.records);
                let decoded_values_file = if outcome.records.is_empty() {
                    None
                } else {
                    let name = format!("{stem}.decoded-values.json");
                    fs::write(
                        output_dir.join(&name),
                        serde_json::to_vec_pretty(&outcome.records)?,
                    )?;
                    Some(name)
                };
                let decoded_value_trace_file = if traced.traces.is_empty() {
                    None
                } else {
                    let name = format!("{stem}.value-trace.json");
                    fs::write(
                        output_dir.join(&name),
                        serde_json::to_vec_pretty(&traced.traces)?,
                    )?;
                    Some(name)
                };
                let consumption = value_stream_consumption(value_stream, &outcome.records);
                (
                    decoded_values_file,
                    decoded_value_trace_file,
                    outcome.error,
                    Some(outcome.omitted_tail_records),
                    Some(consumption.completed_prefix_bits),
                    Some(consumption.stream_bits),
                    Some(consumption.remaining_bits),
                    Some(consumption.residue_bit_offset),
                    Some(consumption.residue_bytes),
                    Some(consumption.residue_crc32),
                    Some(consumption.residue_prefix_hex),
                )
            }
            _ => (
                None, None, None, None, None, None, None, None, None, None, None,
            ),
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
            decoded_value_trace_file,
            decoded_value_error,
            omitted_tail_records,
            decoded_value_completed_prefix_bits,
            decoded_value_stream_bits,
            decoded_value_remaining_bits,
            decoded_value_residue_bit_offset,
            decoded_value_residue_bytes,
            decoded_value_residue_crc32,
            decoded_value_residue_prefix_hex,
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

struct ValueStreamConsumption {
    completed_prefix_bits: usize,
    stream_bits: usize,
    remaining_bits: usize,
    residue_bit_offset: u8,
    residue_bytes: usize,
    residue_crc32: String,
    residue_prefix_hex: String,
}

fn value_stream_consumption(
    value_stream: &[u8],
    records: &[netzip_fullpull::Official5188DecodedValueRecord],
) -> ValueStreamConsumption {
    let completed_prefix_bits = records.last().map_or(0, |record| {
        if record.mask & 0x38 == 0x18 {
            record.bit_end
        } else {
            record.bit_end.saturating_add(7) & !7
        }
    });
    let stream_bits = value_stream.len().saturating_mul(8);
    let completed_prefix_bits = completed_prefix_bits.min(stream_bits);
    let residue_start_byte = completed_prefix_bits / 8;
    let residue = &value_stream[residue_start_byte..];
    ValueStreamConsumption {
        completed_prefix_bits,
        stream_bits,
        remaining_bits: stream_bits - completed_prefix_bits,
        residue_bit_offset: u8::try_from(completed_prefix_bits % 8).expect("bit offset is 0..=7"),
        residue_bytes: residue.len(),
        residue_crc32: format!("{:08x}", crc32fast::hash(residue)),
        residue_prefix_hex: hex(&residue[..residue.len().min(32)]),
    }
}

fn load_code_table_directory(
    directory: &Path,
) -> Result<Vec<(PathBuf, netzip_fullpull::Official5188CodeTable)>, Box<dyn Error>> {
    let mut tables = Vec::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.ends_with("0104.code-table.json") {
            continue;
        }
        let bytes = fs::read(&path)?;
        let table = serde_json::from_slice(&bytes)?;
        tables.push((path, table));
    }
    tables.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(tables)
}

fn load_core_baselines(
    path: &Path,
    wanted: &HashSet<([u8; 2], u16)>,
) -> Result<InternalRecordBySymbol, Box<dyn Error>> {
    let core = fs::read(path)?;
    let segments = core_segments(path, core.len())?;
    let mut found: HashMap<_, (u32, usize, netzip_fullpull::Official5188InternalRecord)> =
        HashMap::new();
    for (segment_start, segment_len) in segments {
        let segment_end = segment_start + segment_len;
        let segment = &core[segment_start..segment_end];
        for position in 0xe1..segment.len().saturating_sub(2) {
            let market = [segment[position], segment[position + 1]];
            if !matches!(&market, b"SH" | b"SZ") {
                continue;
            }
            let symbol_index = u16::from_le_bytes([segment[position - 2], segment[position - 1]]);
            let key = (market, symbol_index);
            if !wanted.contains(&key) {
                continue;
            }
            let start = position - 0xe1;
            let end = start + netzip_fullpull::OFFICIAL_5188_INTERNAL_RECORD_LEN;
            if end > segment.len() {
                continue;
            }
            let record = netzip_fullpull::Official5188InternalRecord::decode(&segment[start..end])?;
            let timestamp = record.timestamp();
            if !is_core_baseline_candidate(&record) {
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
    }
    Ok(found
        .into_iter()
        .map(|(key, (_, _, record))| (key, record))
        .collect())
}

fn core_segments(path: &Path, core_len: usize) -> Result<Vec<(usize, usize)>, Box<dyn Error>> {
    let maps_path = path.with_extension("maps");
    if !maps_path.is_file() {
        return Ok(vec![(0, core_len)]);
    }
    let text = fs::read_to_string(&maps_path)?;
    let mut segments = Vec::new();
    let mut offset = 0usize;
    for line in text.lines() {
        let Some(range) = line.split_whitespace().next() else {
            continue;
        };
        let Some((start, end)) = range.split_once('-') else {
            continue;
        };
        let start = usize::from_str_radix(start, 16)?;
        let end = usize::from_str_radix(end, 16)?;
        let len = end.checked_sub(start).ok_or("invalid core map range")?;
        if len == 0 {
            continue;
        }
        let next = offset.checked_add(len).ok_or("core map offset overflow")?;
        if next > core_len {
            return Err(format!("core map exceeds dump length at offset {offset}").into());
        }
        segments.push((offset, len));
        offset = next;
    }
    if segments.is_empty() || offset != core_len {
        return Err(format!(
            "core map length mismatch: mapped {offset} bytes, dump has {core_len}"
        )
        .into());
    }
    Ok(segments)
}

fn is_core_baseline_candidate(record: &netzip_fullpull::Official5188InternalRecord) -> bool {
    if (1_700_000_000..=1_900_000_000).contains(&record.timestamp()) {
        return true;
    }
    if record.timestamp() != 0 {
        return false;
    }
    let bytes = record.as_bytes();
    let metadata_code = &bytes[0xe3..0xeb];
    let reference_price = i32::from_le_bytes(bytes[0x12b..0x12f].try_into().unwrap());
    metadata_code[..2] == record.market()
        && metadata_code[2..].iter().all(u8::is_ascii_digit)
        && reference_price > 0
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn decode_policy_is_strict_by_default_and_wine_tail_is_explicit() {
        assert_eq!(DecodePolicy::parse(None).unwrap().name(), "strict");
        assert_eq!(
            DecodePolicy::parse(Some("strict")).unwrap().name(),
            "strict"
        );
        assert_eq!(
            DecodePolicy::parse(Some("wine-tail")).unwrap().name(),
            "wine-tail"
        );
        assert!(DecodePolicy::parse(Some("permissive")).is_err());
    }

    fn record(
        bytes: &[u8; netzip_fullpull::OFFICIAL_5188_INTERNAL_RECORD_LEN],
    ) -> netzip_fullpull::Official5188InternalRecord {
        netzip_fullpull::Official5188InternalRecord::decode(bytes).unwrap()
    }

    #[test]
    fn core_baseline_candidate_accepts_live_record() {
        let mut bytes = [0u8; netzip_fullpull::OFFICIAL_5188_INTERNAL_RECORD_LEN];
        bytes[..4].copy_from_slice(&1_788_246_000u32.to_le_bytes());
        assert!(is_core_baseline_candidate(&record(&bytes)));
    }

    #[test]
    fn core_baseline_candidate_accepts_metadata_initialized_slot() {
        let mut bytes = [0u8; netzip_fullpull::OFFICIAL_5188_INTERNAL_RECORD_LEN];
        bytes[0xdf..0xe1].copy_from_slice(&3395u16.to_le_bytes());
        bytes[0xe1..0xe3].copy_from_slice(b"SZ");
        bytes[0xe3..0xeb].copy_from_slice(b"SZ300637");
        bytes[0x12b..0x12f].copy_from_slice(&992i32.to_le_bytes());
        assert!(is_core_baseline_candidate(&record(&bytes)));
    }

    #[test]
    fn core_baseline_candidate_rejects_weak_empty_slot() {
        let mut bytes = [0u8; netzip_fullpull::OFFICIAL_5188_INTERNAL_RECORD_LEN];
        bytes[0xdf..0xe1].copy_from_slice(&3395u16.to_le_bytes());
        bytes[0xe1..0xe3].copy_from_slice(b"SZ");
        assert!(!is_core_baseline_candidate(&record(&bytes)));
    }

    #[test]
    fn core_segments_falls_back_without_maps() {
        let path = std::env::temp_dir().join(format!(
            "official-5188-core-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let segments = core_segments(&path, 17).unwrap();
        assert_eq!(segments, vec![(0, 17)]);
    }

    #[test]
    fn core_segments_rejects_map_length_mismatch() {
        let path = std::env::temp_dir().join(format!(
            "official-5188-core-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(path.with_extension("maps"), "00001000-00001010\n").unwrap();
        let error = core_segments(&path, 15).unwrap_err().to_string();
        assert!(error.contains("core map exceeds dump length at offset 0"));
        let _ = fs::remove_file(path.with_extension("maps"));
    }

    #[test]
    fn core_segments_preserves_mapping_boundaries() {
        let path = std::env::temp_dir().join(format!(
            "official-5188-core-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(
            path.with_extension("maps"),
            "00001000-00001010\n00002000-0000200c\n",
        )
        .unwrap();
        assert_eq!(core_segments(&path, 28).unwrap(), vec![(0, 16), (16, 12)]);
        let _ = fs::remove_file(path.with_extension("maps"));
    }

    #[test]
    fn value_stream_consumption_reports_aligned_suffix() {
        let mut bytes = [0u8; netzip_fullpull::OFFICIAL_5188_INTERNAL_RECORD_LEN];
        bytes[..4].copy_from_slice(&1_788_246_000u32.to_le_bytes());
        let records = vec![netzip_fullpull::Official5188DecodedValueRecord {
            index: netzip_fullpull::Official5188DeltaIndexState {
                market: *b"SH",
                symbol_index: 1,
                timestamp: 1_788_246_000,
                uses_baseline: false,
            },
            mask: 0,
            header: netzip_fullpull::Official5188ValueRecordHeader::decode(0).unwrap(),
            bit_start: 0,
            bit_end: 13,
            record: record(&bytes),
        }];
        let consumption = value_stream_consumption(&[0xaa, 0xbb, 0xcc], &records);
        assert_eq!(consumption.completed_prefix_bits, 16);
        assert_eq!(consumption.remaining_bits, 8);
        assert_eq!(consumption.residue_bit_offset, 0);
        assert_eq!(consumption.residue_bytes, 1);
        assert_eq!(consumption.residue_prefix_hex, "cc");
    }

    #[test]
    fn value_stream_consumption_preserves_early_return_bit_offset() {
        let mut bytes = [0u8; netzip_fullpull::OFFICIAL_5188_INTERNAL_RECORD_LEN];
        bytes[..4].copy_from_slice(&1_788_246_000u32.to_le_bytes());
        let records = vec![netzip_fullpull::Official5188DecodedValueRecord {
            index: netzip_fullpull::Official5188DeltaIndexState {
                market: *b"SZ",
                symbol_index: 2,
                timestamp: 1_788_246_000,
                uses_baseline: false,
            },
            mask: 0x18,
            header: netzip_fullpull::Official5188ValueRecordHeader::decode(0).unwrap(),
            bit_start: 0,
            bit_end: 13,
            record: record(&bytes),
        }];
        let consumption = value_stream_consumption(&[0xaa, 0xbb, 0xcc], &records);
        assert_eq!(consumption.completed_prefix_bits, 13);
        assert_eq!(consumption.remaining_bits, 11);
        assert_eq!(consumption.residue_bit_offset, 5);
        assert_eq!(consumption.residue_bytes, 2);
        assert_eq!(consumption.residue_prefix_hex, "bbcc");
    }
}
