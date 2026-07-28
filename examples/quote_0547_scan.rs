use std::collections::{BTreeMap, HashMap};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Config {
    inputs: Vec<PathBuf>,
}

#[derive(Debug)]
struct Stats {
    numeric_tokens: usize,
    count_9393: usize,
    max_same_byte_run: usize,
    long_93_runs: usize,
    first_long_93_runs: Vec<(usize, usize)>,
    long_93_diff_top: Vec<(usize, usize)>,
    stride_candidates: Vec<StrideCandidate>,
    varint_len_top: Vec<(usize, usize)>,
    varint_avg_len: f64,
    top_u16: Vec<(u16, usize)>,
    xor93: Xor93Stats,
}

#[derive(Clone, Debug)]
struct StrideCandidate {
    record_size: usize,
    best_prefix: usize,
    consensus_score: f64,
    best_tail_under_32: Option<usize>,
}

#[derive(Debug)]
struct Xor93Stats {
    count_le: Option<u16>,
    printable_ratio: f64,
    records: Vec<RecordHit>,
    gap_hist: Vec<(usize, usize)>,
}

#[derive(Debug)]
struct RecordHit {
    start: usize,
    market: u8,
    code: String,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;
    let files = collect_files(&config.inputs)?;
    if files.is_empty() {
        return Err("no files found".into());
    }

    let mut datasets = Vec::new();
    for path in files {
        let bytes = fs::read(&path)?;
        let stats = analyze_bytes(&bytes);
        println!(
            "{} size={} numeric_tokens={} count_9393={} max_run={} long93_runs={} varint_avg_len={:.3} top_u16={}",
            path.display(),
            bytes.len(),
            stats.numeric_tokens,
            stats.count_9393,
            stats.max_same_byte_run,
            stats.long_93_runs,
            stats.varint_avg_len,
            stats
                .top_u16
                .iter()
                .map(|(value, count)| format!("0x{value:04x}:{count}"))
                .collect::<Vec<_>>()
                .join(",")
        );

        if !stats.first_long_93_runs.is_empty() {
            println!(
                "  long93-first={}",
                stats
                    .first_long_93_runs
                    .iter()
                    .map(|(offset, len)| format!("{offset}:{len}"))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }

        if !stats.long_93_diff_top.is_empty() {
            println!(
                "  long93-diffs={}",
                stats
                    .long_93_diff_top
                    .iter()
                    .map(|(gap, count)| format!("{gap}:{count}"))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }

        if !stats.stride_candidates.is_empty() {
            println!(
                "  stride-candidates={}",
                stats
                    .stride_candidates
                    .iter()
                    .map(|candidate| {
                        let tail = candidate
                            .best_tail_under_32
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        format!(
                            "{}@{} score={:.4} tail<=32={}",
                            candidate.record_size,
                            candidate.best_prefix,
                            candidate.consensus_score,
                            tail
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        if !stats.varint_len_top.is_empty() {
            println!(
                "  tdx-varint-lens={}",
                stats
                    .varint_len_top
                    .iter()
                    .map(|(len, count)| format!("{len}:{count}"))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }

        println!(
            "  xor93=count={} printable={:.3} filtered-records={} first=[{}] gaps={}",
            stats
                .xor93
                .count_le
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
            stats.xor93.printable_ratio,
            stats.xor93.records.len(),
            stats
                .xor93
                .records
                .iter()
                .take(5)
                .map(|record| format!("{}{}", market_prefix(record.market), record.code))
                .collect::<Vec<_>>()
                .join(","),
            stats
                .xor93
                .gap_hist
                .iter()
                .map(|(gap, count)| format!("{gap}:{count}"))
                .collect::<Vec<_>>()
                .join(",")
        );

        datasets.push((path, bytes, stats));
    }

    if datasets.len() >= 2 {
        let common_prefix =
            common_prefix_len(datasets.iter().map(|(_, bytes, _)| bytes.as_slice()));
        let common_suffix =
            common_suffix_len(datasets.iter().map(|(_, bytes, _)| bytes.as_slice()));
        println!("common-prefix={common_prefix}");
        println!("common-suffix={common_suffix}");
    }

    Ok(())
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h") {
        print_help();
        std::process::exit(0);
    }
    Ok(Config {
        inputs: args.into_iter().map(PathBuf::from).collect(),
    })
}

fn print_help() {
    println!("quote_0547_scan <file-or-dir> [more-files-or-dirs...]");
    println!("  scans extracted 0x0547 bodies/inflated payloads");
    println!("  if a directory is passed, it auto-loads *_inflated.bin then *_body.bin");
    println!("  also reports candidate stride sizes and public-TDX-style varint statistics");
    println!("  and applies xor 0x93 to surface decoded count/code/gap signals");
}

fn collect_files(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut out = Vec::new();
    for input in inputs {
        if input.is_dir() {
            let mut inflated = glob_in_dir(input, "_inflated.bin")?;
            let mut body = glob_in_dir(input, "_body.bin")?;
            if !inflated.is_empty() {
                out.append(&mut inflated);
            } else {
                out.append(&mut body);
            }
        } else {
            out.push(input.clone());
        }
    }
    out.sort();
    Ok(out)
}

fn glob_in_dir(dir: &Path, suffix: &str) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_file()
            && path
                .file_name()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .ends_with(suffix)
        {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

fn analyze_bytes(bytes: &[u8]) -> Stats {
    let numeric_tokens = count_numeric_tokens(bytes);
    let count_9393 = count_subslice(bytes, &[0x93, 0x93]);
    let max_same_byte_run = max_same_byte_run(bytes);
    let long_93 = byte_runs(bytes, 0x93, 5);
    let long_93_runs = long_93.len();
    let first_long_93_runs = long_93.iter().take(20).copied().collect::<Vec<_>>();
    let long_93_diff_top = top_run_gaps(&long_93, 12);
    let stride_candidates = stride_candidates(bytes, 100, 100, 126, 5);
    let (varint_len_top, varint_avg_len) = tdx_varint_len_stats(bytes, 8);
    let top_u16 = top_u16_words(bytes, 8);
    let xor93 = analyze_xor93(bytes);
    Stats {
        numeric_tokens,
        count_9393,
        max_same_byte_run,
        long_93_runs,
        first_long_93_runs,
        long_93_diff_top,
        stride_candidates,
        varint_len_top,
        varint_avg_len,
        top_u16,
        xor93,
    }
}

fn analyze_xor93(bytes: &[u8]) -> Xor93Stats {
    let decoded = bytes.iter().map(|byte| byte ^ 0x93).collect::<Vec<_>>();
    let count_le = decoded
        .get(..2)
        .map(|prefix| u16::from_le_bytes([prefix[0], prefix[1]]));
    let printable_ratio = if decoded.is_empty() {
        0.0
    } else {
        let printable = decoded
            .iter()
            .filter(|byte| matches!(byte, 0x20..=0x7e))
            .count();
        printable as f64 / decoded.len() as f64
    };
    let records = find_xor93_records(&decoded);
    let gap_hist = top_gap_hist(&records, 8);
    Xor93Stats {
        count_le,
        printable_ratio,
        records,
        gap_hist,
    }
}

fn find_xor93_records(bytes: &[u8]) -> Vec<RecordHit> {
    let mut out = Vec::new();
    let mut index = 1usize;
    while index + 6 < bytes.len() {
        let market = bytes[index - 1];
        let code = &bytes[index..index + 6];
        if market <= 2 && code.iter().all(|byte| byte.is_ascii_digit()) {
            out.push(RecordHit {
                start: index - 1,
                market,
                code: String::from_utf8_lossy(code).into_owned(),
            });
            index += 6;
        } else {
            index += 1;
        }
    }
    out
}

fn top_gap_hist(records: &[RecordHit], limit: usize) -> Vec<(usize, usize)> {
    let mut counts = HashMap::<usize, usize>::new();
    for window in records.windows(2) {
        let gap = window[1].start.saturating_sub(window[0].start);
        *counts.entry(gap).or_default() += 1;
    }
    let mut items = counts.into_iter().collect::<Vec<_>>();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    items.truncate(limit);
    items
}

fn market_prefix(market: u8) -> &'static str {
    match market {
        0 => "SZ",
        1 => "SH",
        2 => "BJ",
        _ => "?",
    }
}

fn count_numeric_tokens(bytes: &[u8]) -> usize {
    let mut count = 0usize;
    let mut start = None;
    for (i, byte) in bytes.iter().copied().enumerate() {
        if byte.is_ascii_digit() {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            if i - s >= 6 {
                count += 1;
            }
        }
    }
    if let Some(s) = start {
        if bytes.len() - s >= 6 {
            count += 1;
        }
    }
    count
}

fn count_subslice(bytes: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || bytes.len() < needle.len() {
        return 0;
    }
    let mut count = 0usize;
    for i in 0..=bytes.len() - needle.len() {
        if &bytes[i..i + needle.len()] == needle {
            count += 1;
        }
    }
    count
}

fn max_same_byte_run(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    let mut max_run = 1usize;
    let mut run = 1usize;
    for i in 1..bytes.len() {
        if bytes[i] == bytes[i - 1] {
            run += 1;
            max_run = max_run.max(run);
        } else {
            run = 1;
        }
    }
    max_run
}

fn byte_runs(bytes: &[u8], needle: u8, min_len: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != needle {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && bytes[index] == needle {
            index += 1;
        }
        let len = index - start;
        if len >= min_len {
            out.push((start, len));
        }
    }
    out
}

fn top_run_gaps(runs: &[(usize, usize)], limit: usize) -> Vec<(usize, usize)> {
    let mut counts = BTreeMap::<usize, usize>::new();
    for window in runs.windows(2) {
        let gap = window[1].0.saturating_sub(window[0].0);
        *counts.entry(gap).or_default() += 1;
    }
    let mut items = counts.into_iter().collect::<Vec<_>>();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    items.truncate(limit);
    items
}

fn stride_candidates(
    bytes: &[u8],
    expected_items: usize,
    min_record: usize,
    max_record: usize,
    limit: usize,
) -> Vec<StrideCandidate> {
    let mut candidates = Vec::new();
    for record_size in min_record..=max_record {
        let mut best_prefix = None::<(usize, f64)>;
        for prefix in 0..record_size {
            let Some(score) = consensus_score(bytes, prefix, record_size, expected_items) else {
                continue;
            };
            if best_prefix
                .as_ref()
                .map(|(_, best)| score > *best)
                .unwrap_or(true)
            {
                best_prefix = Some((prefix, score));
            }
        }
        let Some((best_prefix, consensus_score)) = best_prefix else {
            continue;
        };
        let best_tail_under_32 = best_small_prefix_tail(bytes, expected_items, record_size, 32);
        candidates.push(StrideCandidate {
            record_size,
            best_prefix,
            consensus_score,
            best_tail_under_32,
        });
    }
    candidates.sort_by(|a, b| {
        b.consensus_score
            .total_cmp(&a.consensus_score)
            .then_with(|| match (a.best_tail_under_32, b.best_tail_under_32) {
                (Some(left), Some(right)) => left.cmp(&right),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
            .then_with(|| a.record_size.cmp(&b.record_size))
    });
    candidates.truncate(limit);
    candidates
}

fn consensus_score(
    bytes: &[u8],
    prefix: usize,
    record_size: usize,
    expected_items: usize,
) -> Option<f64> {
    if prefix >= bytes.len() {
        return None;
    }
    let available = bytes.len().saturating_sub(prefix) / record_size;
    let count = available.min(expected_items);
    if count < 16 {
        return None;
    }
    let mut same = 0usize;
    let mut total = 0usize;
    for offset in 0..record_size {
        let mut counts = [0u8; 256];
        for index in 0..count {
            let value = bytes[prefix + index * record_size + offset];
            counts[value as usize] = counts[value as usize].saturating_add(1);
        }
        same += counts.into_iter().max().unwrap_or(0) as usize;
        total += count;
    }
    Some(same as f64 / total as f64)
}

fn best_small_prefix_tail(
    bytes: &[u8],
    expected_items: usize,
    record_size: usize,
    max_prefix: usize,
) -> Option<usize> {
    let mut best = None::<usize>;
    for prefix in 0..=max_prefix.min(bytes.len()) {
        let payload = prefix + record_size.saturating_mul(expected_items);
        if payload > bytes.len() {
            continue;
        }
        let tail = bytes.len() - payload;
        if best.map(|old| tail < old).unwrap_or(true) {
            best = Some(tail);
        }
    }
    best
}

fn tdx_varint_len_stats(bytes: &[u8], limit: usize) -> (Vec<(usize, usize)>, f64) {
    if bytes.is_empty() {
        return (Vec::new(), 0.0);
    }
    let mut counts = BTreeMap::<usize, usize>::new();
    let mut index = 0usize;
    let mut total_len = 0usize;
    let mut total_count = 0usize;
    while index < bytes.len() {
        let start = index;
        let mut byte = bytes[index];
        index += 1;
        while byte & 0x80 != 0 {
            if index >= bytes.len() {
                break;
            }
            byte = bytes[index];
            index += 1;
        }
        let len = index - start;
        *counts.entry(len).or_default() += 1;
        total_len += len;
        total_count += 1;
    }
    let mut items = counts.into_iter().collect::<Vec<_>>();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    items.truncate(limit);
    let avg = if total_count == 0 {
        0.0
    } else {
        total_len as f64 / total_count as f64
    };
    (items, avg)
}

fn top_u16_words(bytes: &[u8], limit: usize) -> Vec<(u16, usize)> {
    let mut counts = HashMap::<u16, usize>::new();
    for i in (0..bytes.len().saturating_sub(1)).step_by(2) {
        let value = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        *counts.entry(value).or_default() += 1;
    }
    let mut items = counts.into_iter().collect::<Vec<_>>();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    items.truncate(limit);
    items
}

fn common_prefix_len<'a>(iter: impl Iterator<Item = &'a [u8]>) -> usize {
    let buffers = iter.collect::<Vec<_>>();
    if buffers.is_empty() {
        return 0;
    }
    let min_len = buffers.iter().map(|x| x.len()).min().unwrap_or(0);
    let mut index = 0usize;
    while index < min_len {
        let byte = buffers[0][index];
        if buffers.iter().all(|buf| buf[index] == byte) {
            index += 1;
        } else {
            break;
        }
    }
    index
}

fn common_suffix_len<'a>(iter: impl Iterator<Item = &'a [u8]>) -> usize {
    let buffers = iter.collect::<Vec<_>>();
    if buffers.is_empty() {
        return 0;
    }
    let min_len = buffers.iter().map(|x| x.len()).min().unwrap_or(0);
    let mut index = 0usize;
    while index < min_len {
        let byte = buffers[0][buffers[0].len() - 1 - index];
        if buffers.iter().all(|buf| buf[buf.len() - 1 - index] == byte) {
            index += 1;
        } else {
            break;
        }
    }
    index
}
