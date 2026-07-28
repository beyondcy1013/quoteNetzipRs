use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::mem::size_of;
use std::path::Path;

use serde::Serialize;

use crate::debug_stream::parse_hex_like;
use crate::packet::{RawOemDataHead, read_unaligned};
use crate::parse_answer_buffer;

const LOCAL_2000_HEADER: [u8; 8] = [0x51, 0x7f, 0xdc, 0x7e, 0x05, 0x53, 0x00, 0x00];
const OEM_HEAD_LEN: usize = 200;

#[derive(Clone, Debug, Serialize)]
pub struct Local2000LogAnalysis {
    pub input: String,
    pub port: u16,
    pub small_max: usize,
    pub recv_entries: usize,
    pub recv_entries_with_headers: usize,
    pub recv_entries_with_multiple_headers: usize,
    pub complete_small_packets: usize,
    pub complete_large_packets: usize,
    pub aligned_next_header_links: usize,
    pub packet_kind_counts: BTreeMap<String, usize>,
    pub decl_a_counts: BTreeMap<u32, usize>,
    pub complete_small_field_tuples: Vec<Local2000FieldTupleCount>,
    pub complete_small_field56_minus_field44_counts: BTreeMap<i64, usize>,
    pub complete_small_field44_plus_68_matches: usize,
    pub complete_small_field44_plus_68_mismatches: usize,
    pub complete_small_penc_offset_counts: BTreeMap<i64, usize>,
    pub complete_small_hypenc_offset_counts: BTreeMap<i64, usize>,
    pub entries: Vec<Local2000RecvEntry>,
    pub large_objects: Vec<Local2000LargeObject>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Local2000FieldTupleCount {
    pub field40: u32,
    pub field44: u32,
    pub field48: u32,
    pub field52: u32,
    pub field56: u32,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Local2000RecvEntry {
    pub index: usize,
    pub line_no: usize,
    pub socket: String,
    pub port: u16,
    pub recv_len: usize,
    pub captured_len: usize,
    pub header_offsets: Vec<usize>,
    pub utf16_code_count: usize,
    pub first_codes: Vec<Local2000CodeHit>,
    pub code_step_counts: BTreeMap<usize, usize>,
    pub packet_hits: Vec<Local2000PacketHit>,
    #[serde(skip_serializing)]
    captured: Vec<u8>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Local2000PacketHit {
    pub offset: usize,
    pub decl_a: Option<u32>,
    pub decl_b: Option<u32>,
    pub decl_a_matches_decl_b: Option<bool>,
    pub field40: Option<u32>,
    pub field44: Option<u32>,
    pub field48: Option<u32>,
    pub field52: Option<u32>,
    pub field56: Option<u32>,
    pub complete: bool,
    pub next_header_aligned: bool,
    pub contains_penc: bool,
    pub contains_hypenc: bool,
    pub penc_offset: Option<usize>,
    pub hypenc_offset: Option<usize>,
    pub kind: String,
    pub preview_hex: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Local2000CodeHit {
    pub offset: usize,
    pub code: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Local2000LargeObject {
    pub start_recv_index: usize,
    pub start_line_no: usize,
    pub socket: String,
    pub port: u16,
    pub start_offset: usize,
    pub declared_len: u32,
    pub declared_len_duplicate: Option<u32>,
    pub field40: Option<u32>,
    pub field44: Option<u32>,
    pub field48: Option<u32>,
    pub field52: Option<u32>,
    pub field56: Option<u32>,
    pub accumulated_recv_len: usize,
    pub accumulated_captured_len: usize,
    pub recv_complete: bool,
    pub captured_complete: bool,
    pub segment_count: usize,
    pub stop_reason: String,
    pub penc_offsets: Vec<usize>,
    pub hypenc_offsets: Vec<usize>,
    pub utf16_code_count: usize,
    pub first_codes: Vec<Local2000CodeHit>,
    pub code_step_counts: BTreeMap<usize, usize>,
    pub oem_answer_offset: Option<usize>,
    pub oem_answer_summary: Option<String>,
    pub stream_head_hex: String,
    pub segments: Vec<Local2000LargeSegment>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Local2000LargeSegment {
    pub recv_index: usize,
    pub line_no: usize,
    pub recv_len: usize,
    pub captured_len: usize,
    pub source_offset: usize,
    pub stream_offset: usize,
    pub header_offsets: Vec<usize>,
    pub utf16_code_count: usize,
    pub first_codes: Vec<Local2000CodeHit>,
    pub code_step_counts: BTreeMap<usize, usize>,
    pub preview_hex: String,
}

pub fn analyze_local_2000_log_file(
    path: impl AsRef<Path>,
    port: u16,
    small_max: usize,
    code_preview_limit: usize,
) -> Result<Local2000LogAnalysis, Box<dyn Error>> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)?;
    analyze_local_2000_log_text(
        path.display().to_string(),
        &text,
        port,
        small_max,
        code_preview_limit,
    )
}

pub fn analyze_local_2000_log_text(
    input: String,
    text: &str,
    port: u16,
    small_max: usize,
    code_preview_limit: usize,
) -> Result<Local2000LogAnalysis, Box<dyn Error>> {
    let entries = parse_recv_entries(text, port, small_max, code_preview_limit)?;
    let large_objects = collect_large_objects(&entries, small_max, code_preview_limit);

    let mut recv_entries_with_headers = 0usize;
    let mut recv_entries_with_multiple_headers = 0usize;
    let mut complete_small_packets = 0usize;
    let mut complete_large_packets = 0usize;
    let mut aligned_next_header_links = 0usize;
    let mut packet_kind_counts = BTreeMap::<String, usize>::new();
    let mut decl_a_counts = BTreeMap::<u32, usize>::new();
    let mut complete_small_field_tuple_counts = BTreeMap::<(u32, u32, u32, u32, u32), usize>::new();
    let mut complete_small_field56_minus_field44_counts = BTreeMap::<i64, usize>::new();
    let mut complete_small_field44_plus_68_matches = 0usize;
    let mut complete_small_field44_plus_68_mismatches = 0usize;
    let mut complete_small_penc_offset_counts = BTreeMap::<i64, usize>::new();
    let mut complete_small_hypenc_offset_counts = BTreeMap::<i64, usize>::new();

    for entry in &entries {
        if !entry.packet_hits.is_empty() {
            recv_entries_with_headers += 1;
        }
        if entry.packet_hits.len() > 1 {
            recv_entries_with_multiple_headers += 1;
        }

        for hit in &entry.packet_hits {
            *packet_kind_counts.entry(hit.kind.clone()).or_default() += 1;
            if let Some(decl_a) = hit.decl_a {
                *decl_a_counts.entry(decl_a).or_default() += 1;
            }
            if hit.next_header_aligned {
                aligned_next_header_links += 1;
            }
            match hit.kind.as_str() {
                "complete-small" => complete_small_packets += 1,
                "complete-large" => complete_large_packets += 1,
                _ => {}
            }

            if hit.kind != "complete-small" {
                continue;
            }

            if let (Some(field40), Some(field44), Some(field48), Some(field52), Some(field56)) = (
                hit.field40,
                hit.field44,
                hit.field48,
                hit.field52,
                hit.field56,
            ) {
                *complete_small_field_tuple_counts
                    .entry((field40, field44, field48, field52, field56))
                    .or_default() += 1;
                *complete_small_field56_minus_field44_counts
                    .entry(i64::from(field56) - i64::from(field44))
                    .or_default() += 1;
            }

            if let (Some(field44), Some(decl_a)) = (hit.field44, hit.decl_a) {
                if field44 + 68 == decl_a {
                    complete_small_field44_plus_68_matches += 1;
                } else {
                    complete_small_field44_plus_68_mismatches += 1;
                }
            }

            *complete_small_penc_offset_counts
                .entry(hit.penc_offset.map(|value| value as i64).unwrap_or(-1))
                .or_default() += 1;
            *complete_small_hypenc_offset_counts
                .entry(hit.hypenc_offset.map(|value| value as i64).unwrap_or(-1))
                .or_default() += 1;
        }
    }

    let complete_small_field_tuples = complete_small_field_tuple_counts
        .into_iter()
        .map(
            |((field40, field44, field48, field52, field56), count)| Local2000FieldTupleCount {
                field40,
                field44,
                field48,
                field52,
                field56,
                count,
            },
        )
        .collect::<Vec<_>>();

    Ok(Local2000LogAnalysis {
        input,
        port,
        small_max,
        recv_entries: entries.len(),
        recv_entries_with_headers,
        recv_entries_with_multiple_headers,
        complete_small_packets,
        complete_large_packets,
        aligned_next_header_links,
        packet_kind_counts,
        decl_a_counts,
        complete_small_field_tuples,
        complete_small_field56_minus_field44_counts,
        complete_small_field44_plus_68_matches,
        complete_small_field44_plus_68_mismatches,
        complete_small_penc_offset_counts,
        complete_small_hypenc_offset_counts,
        entries,
        large_objects,
    })
}

fn parse_recv_entries(
    text: &str,
    port: u16,
    small_max: usize,
    code_preview_limit: usize,
) -> Result<Vec<Local2000RecvEntry>, Box<dyn Error>> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut entries = Vec::new();
    let mut line_index = 0usize;

    while line_index < lines.len() {
        let normalized_line = normalize_log_line(lines[line_index]);
        let Some((socket, recv_port, recv_len)) = parse_recv_line(&normalized_line) else {
            line_index += 1;
            continue;
        };
        let recv_line_no = line_index + 1;
        line_index += 1;

        let mut dump_lines = Vec::<&str>::new();
        while line_index < lines.len() {
            let normalized = normalize_log_line(lines[line_index]);
            if normalized.starts_with('[') {
                break;
            }
            if !lines[line_index].trim().is_empty() {
                dump_lines.push(lines[line_index]);
            }
            line_index += 1;
        }

        if recv_port != port {
            continue;
        }

        let captured = if dump_lines.is_empty() {
            Vec::new()
        } else {
            let normalized_dump = dump_lines
                .into_iter()
                .map(normalize_log_line)
                .collect::<Vec<_>>()
                .join("\n");
            parse_hex_like(&normalized_dump)?
        };
        let header_offsets = find_header_offsets(&captured);
        let code_hits = scan_utf16_market_codes(&captured);
        let code_step_counts = summarize_code_steps(&code_hits);
        let mut packet_hits = Vec::new();

        for (hit_index, &offset) in header_offsets.iter().enumerate() {
            let decl_a = read_u32_le(&captured, offset + 32);
            let decl_b = read_u32_le(&captured, offset + 36);
            let field40 = read_u32_le(&captured, offset + 40);
            let field44 = read_u32_le(&captured, offset + 44);
            let field48 = read_u32_le(&captured, offset + 48);
            let field52 = read_u32_le(&captured, offset + 52);
            let field56 = read_u32_le(&captured, offset + 56);
            let complete = decl_a
                .map(|declared| offset + declared as usize <= captured.len())
                .unwrap_or(false);
            let next_header_aligned = match (decl_a, header_offsets.get(hit_index + 1)) {
                (Some(declared), Some(next_offset)) => *next_offset == offset + declared as usize,
                _ => false,
            };
            let segment = if complete {
                let declared = decl_a.expect("complete packet has decl_a") as usize;
                &captured[offset..offset + declared]
            } else {
                &captured[offset..]
            };
            let penc_offset = find_literal(segment, b"penc");
            let hypenc_offset = find_literal(segment, b"hypenc");

            packet_hits.push(Local2000PacketHit {
                offset,
                decl_a,
                decl_b,
                decl_a_matches_decl_b: decl_a.zip(decl_b).map(|(left, right)| left == right),
                field40,
                field44,
                field48,
                field52,
                field56,
                complete,
                next_header_aligned,
                contains_penc: penc_offset.is_some(),
                contains_hypenc: hypenc_offset.is_some(),
                penc_offset,
                hypenc_offset,
                kind: classify_kind(decl_a, complete, small_max).to_string(),
                preview_hex: preview_hex(segment, 32),
            });
        }

        entries.push(Local2000RecvEntry {
            index: entries.len() + 1,
            line_no: recv_line_no,
            socket,
            port: recv_port,
            recv_len,
            captured_len: captured.len(),
            header_offsets,
            utf16_code_count: code_hits.len(),
            first_codes: code_hits.into_iter().take(code_preview_limit).collect(),
            code_step_counts,
            packet_hits,
            captured,
        });
    }

    Ok(entries)
}

fn collect_large_objects(
    entries: &[Local2000RecvEntry],
    small_max: usize,
    code_preview_limit: usize,
) -> Vec<Local2000LargeObject> {
    let mut out = Vec::new();
    let mut seen_starts = BTreeSet::<(usize, usize)>::new();

    for (entry_offset, entry) in entries.iter().enumerate() {
        for hit in &entry.packet_hits {
            let Some(declared_len) = hit.decl_a else {
                continue;
            };
            if declared_len as usize <= small_max {
                continue;
            }
            if !seen_starts.insert((entry_offset, hit.offset)) {
                continue;
            }
            out.push(reassemble_large_object(
                entries,
                entry_offset,
                hit.offset,
                hit,
                code_preview_limit,
            ));
        }
    }

    out
}

fn reassemble_large_object(
    entries: &[Local2000RecvEntry],
    start_entry_offset: usize,
    start_offset: usize,
    start_hit: &Local2000PacketHit,
    code_preview_limit: usize,
) -> Local2000LargeObject {
    let start_entry = &entries[start_entry_offset];
    let mut assembled = Vec::<u8>::new();
    let mut segments = Vec::<Local2000LargeSegment>::new();
    let mut accumulated_recv_len = 0usize;
    let mut accumulated_captured_len = 0usize;
    let mut stop_reason = "end-of-log".to_string();

    let start_slice = if start_offset <= start_entry.captured.len() {
        &start_entry.captured[start_offset..]
    } else {
        &[]
    };
    assembled.extend_from_slice(start_slice);
    accumulated_recv_len += start_entry.recv_len.saturating_sub(start_offset);
    accumulated_captured_len += start_slice.len();
    segments.push(summarize_large_segment(
        start_entry,
        start_offset,
        0,
        start_slice,
        code_preview_limit,
    ));

    let declared_len = start_hit.decl_a.expect("large object start has decl_a") as usize;
    for entry in entries.iter().skip(start_entry_offset + 1) {
        if entry.socket != start_entry.socket || entry.port != start_entry.port {
            continue;
        }
        if !entry.packet_hits.is_empty() {
            let first_header_offset = entry
                .packet_hits
                .iter()
                .map(|hit| hit.offset)
                .min()
                .unwrap_or(0);
            if first_header_offset > 0 {
                let continuation = &entry.captured[..first_header_offset.min(entry.captured.len())];
                let stream_offset = assembled.len();
                assembled.extend_from_slice(continuation);
                accumulated_recv_len += first_header_offset.min(entry.recv_len);
                accumulated_captured_len += continuation.len();
                segments.push(summarize_large_segment(
                    entry,
                    0,
                    stream_offset,
                    continuation,
                    code_preview_limit,
                ));
            }
            stop_reason = format!(
                "next-header-recv#{}@offset={first_header_offset}",
                entry.index
            );
            break;
        }

        let stream_offset = assembled.len();
        assembled.extend_from_slice(&entry.captured);
        accumulated_recv_len += entry.recv_len;
        accumulated_captured_len += entry.captured.len();
        segments.push(summarize_large_segment(
            entry,
            0,
            stream_offset,
            &entry.captured,
            code_preview_limit,
        ));

        if accumulated_recv_len >= declared_len {
            stop_reason = "declared-len-covered-by-recv".to_string();
            break;
        }
    }

    let recv_complete = accumulated_recv_len >= declared_len;
    let captured_complete = accumulated_captured_len >= declared_len;
    if recv_complete && captured_complete {
        stop_reason = "declared-len-captured".to_string();
    }

    let code_hits = scan_utf16_market_codes(&assembled);
    let code_step_counts = summarize_code_steps(&code_hits);
    let oem_answer_offset = find_possible_oem_offset(&assembled);
    let oem_answer_summary =
        oem_answer_offset.and_then(|offset| try_summarize_oem_packet(&assembled, offset));

    Local2000LargeObject {
        start_recv_index: start_entry.index,
        start_line_no: start_entry.line_no,
        socket: start_entry.socket.clone(),
        port: start_entry.port,
        start_offset,
        declared_len: start_hit.decl_a.expect("large object start has decl_a"),
        declared_len_duplicate: start_hit.decl_b,
        field40: start_hit.field40,
        field44: start_hit.field44,
        field48: start_hit.field48,
        field52: start_hit.field52,
        field56: start_hit.field56,
        accumulated_recv_len,
        accumulated_captured_len,
        recv_complete,
        captured_complete,
        segment_count: segments.len(),
        stop_reason,
        penc_offsets: find_all_literals(&assembled, b"penc"),
        hypenc_offsets: find_all_literals(&assembled, b"hypenc"),
        utf16_code_count: code_hits.len(),
        first_codes: code_hits.into_iter().take(code_preview_limit).collect(),
        code_step_counts,
        oem_answer_offset,
        oem_answer_summary,
        stream_head_hex: preview_hex(&assembled, 64),
        segments,
    }
}

fn summarize_large_segment(
    entry: &Local2000RecvEntry,
    source_offset: usize,
    stream_offset: usize,
    segment: &[u8],
    code_preview_limit: usize,
) -> Local2000LargeSegment {
    let code_hits = scan_utf16_market_codes(segment);
    Local2000LargeSegment {
        recv_index: entry.index,
        line_no: entry.line_no,
        recv_len: entry.recv_len,
        captured_len: entry.captured_len,
        source_offset,
        stream_offset,
        header_offsets: find_header_offsets(segment),
        utf16_code_count: code_hits.len(),
        first_codes: code_hits.into_iter().take(code_preview_limit).collect(),
        code_step_counts: summarize_code_steps(&scan_utf16_market_codes(segment)),
        preview_hex: preview_hex(segment, 32),
    }
}

fn parse_recv_line(line: &str) -> Option<(String, u16, usize)> {
    if !line.starts_with("[recv] ") && !line.starts_with("[recv-large] ") {
        return None;
    }
    let mut socket = None::<String>;
    let mut peer_port = None::<u16>;
    let mut recv_len = None::<usize>;

    for part in line.split_whitespace().skip(1) {
        if let Some(value) = part.strip_prefix("socket=") {
            socket = Some(value.to_string());
            continue;
        }
        if let Some(value) = part.strip_prefix("peer=") {
            let port = value.rsplit_once(':')?.1.parse::<u16>().ok()?;
            peer_port = Some(port);
            continue;
        }
        if let Some(value) = part.strip_prefix("len=") {
            recv_len = value.parse::<usize>().ok();
        }
    }

    Some((socket?, peer_port?, recv_len?))
}

fn normalize_log_line(line: &str) -> String {
    let stripped = strip_line_number_prefix(line);
    strip_ansi_codes(stripped)
}

fn strip_line_number_prefix(line: &str) -> &str {
    let Some((left, right)) = line.split_once(':') else {
        return line;
    };
    if !left.is_empty() && left.chars().all(|ch| ch.is_ascii_digit()) {
        right.trim_start()
    } else {
        line
    }
}

fn strip_ansi_codes(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == 0x1b {
            index += 1;
            if index < bytes.len() && bytes[index] == b'[' {
                index += 1;
                while index < bytes.len() && !(0x40..=0x7e).contains(&bytes[index]) {
                    index += 1;
                }
                if index < bytes.len() {
                    index += 1;
                }
            }
            continue;
        }
        out.push(bytes[index] as char);
        index += 1;
    }
    out
}

fn find_header_offsets(bytes: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while let Some(found) = bytes[offset..]
        .windows(LOCAL_2000_HEADER.len())
        .position(|window| window == LOCAL_2000_HEADER)
    {
        let absolute = offset + found;
        out.push(absolute);
        offset = absolute + 1;
        if offset >= bytes.len() {
            break;
        }
    }
    out
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes(slice.try_into().ok()?))
}

fn classify_kind(decl_a: Option<u32>, complete: bool, small_max: usize) -> &'static str {
    match decl_a {
        None => "invalid",
        Some(declared) if complete && declared as usize <= small_max => "complete-small",
        Some(declared) if complete && declared as usize > small_max => "complete-large",
        Some(declared) if declared as usize <= small_max => "truncated-small",
        Some(_) => "truncated-large",
    }
}

fn preview_hex(bytes: &[u8], limit: usize) -> String {
    bytes
        .iter()
        .take(limit)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn find_literal(bytes: &[u8], literal: &[u8]) -> Option<usize> {
    bytes
        .windows(literal.len())
        .position(|window| window == literal)
}

fn find_all_literals(bytes: &[u8], literal: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while let Some(found) = bytes[offset..]
        .windows(literal.len())
        .position(|window| window == literal)
    {
        let absolute = offset + found;
        out.push(absolute);
        offset = absolute + 1;
        if offset >= bytes.len() {
            break;
        }
    }
    out
}

fn scan_utf16_market_codes(bytes: &[u8]) -> Vec<Local2000CodeHit> {
    let mut out = Vec::new();
    if bytes.len() < 16 {
        return out;
    }

    for offset in (0..=bytes.len() - 16).step_by(2) {
        let slice = &bytes[offset..offset + 16];
        if slice.iter().skip(1).step_by(2).any(|&byte| byte != 0) {
            continue;
        }
        let chars = slice.iter().step_by(2).copied().collect::<Vec<_>>();
        let prefix_ok = matches!(
            chars.as_slice(),
            [b'S', b'H' | b'Z', _, _, _, _, _, _] | [b'B', b'J', _, _, _, _, _, _]
        );
        if !prefix_ok || chars[2..].iter().any(|byte| !byte.is_ascii_digit()) {
            continue;
        }
        out.push(Local2000CodeHit {
            offset,
            code: String::from_utf8_lossy(&chars).into_owned(),
        });
    }

    out
}

fn summarize_code_steps(codes: &[Local2000CodeHit]) -> BTreeMap<usize, usize> {
    let mut out = BTreeMap::<usize, usize>::new();
    for window in codes.windows(2) {
        let step = window[1].offset.saturating_sub(window[0].offset);
        *out.entry(step).or_default() += 1;
    }
    out
}

fn find_possible_oem_offset(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < OEM_HEAD_LEN {
        return None;
    }

    for marker in ["代码表", "除权"] {
        let utf16 = utf16le_literal(marker);
        for offset in (0..=bytes.len().saturating_sub(utf16.len())).step_by(2) {
            if bytes[offset..].starts_with(&utf16) {
                return Some(offset);
            }
        }
    }

    None
}

fn try_summarize_oem_packet(bytes: &[u8], offset: usize) -> Option<String> {
    let remaining = bytes.get(offset..)?;
    if remaining.len() < size_of::<RawOemDataHead>() {
        return None;
    }

    let head = unsafe { read_unaligned::<RawOemDataHead>(remaining.as_ptr()) }.decode();
    if head.len < 0 || head.count < 0 {
        return None;
    }

    match head.packet_type.as_str() {
        "代码表" | "除权" => {}
        _ => return None,
    }

    let declared_total = size_of::<RawOemDataHead>() + head.len as usize;
    if declared_total > remaining.len() {
        return None;
    }

    unsafe { parse_answer_buffer(&remaining[..declared_total]) }.map(|packet| packet.summary())
}

fn utf16le_literal(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 2);
    for word in text.encode_utf16() {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{analyze_local_2000_log_file, analyze_local_2000_log_text};

    fn hexdump(bytes: &[u8]) -> String {
        let mut out = String::new();
        for (offset, chunk) in bytes.chunks(16).enumerate() {
            out.push_str(&format!("{:08x}  ", offset * 16));
            for byte in chunk {
                out.push_str(&format!("{byte:02X} "));
            }
            out.push('\n');
        }
        out
    }

    fn make_packet(packet_len: usize, field44: u32, include_hypenc: bool) -> Vec<u8> {
        let mut out = vec![0u8; packet_len];
        out[..8].copy_from_slice(&super::LOCAL_2000_HEADER);
        out[32..36].copy_from_slice(&(packet_len as u32).to_le_bytes());
        out[36..40].copy_from_slice(&(packet_len as u32).to_le_bytes());
        out[40..44].copy_from_slice(&2u32.to_le_bytes());
        out[44..48].copy_from_slice(&field44.to_le_bytes());
        out[48..52].copy_from_slice(&0u32.to_le_bytes());
        out[52..56].copy_from_slice(&9u32.to_le_bytes());
        out[56..60].copy_from_slice(&(field44 + 28).to_le_bytes());
        out[60..64].copy_from_slice(b"penc");
        if include_hypenc && out.len() >= 74 {
            out[68..74].copy_from_slice(b"hypenc");
        }
        out
    }

    fn write_utf16_code(bytes: &mut [u8], offset: usize, code: &str) {
        let mut cursor = offset;
        for word in code.encode_utf16() {
            bytes[cursor..cursor + 2].copy_from_slice(&word.to_le_bytes());
            cursor += 2;
        }
    }

    #[test]
    fn parses_multi_header_small_packets() {
        let pkt1 = make_packet(288, 220, false);
        let pkt2 = make_packet(312, 244, false);
        let mut log = String::new();
        log.push_str("[recv] socket=0x111 peer=127.0.0.1:2000 len=600\n");
        log.push_str(&hexdump(&[pkt1, pkt2].concat()));

        let analysis = analyze_local_2000_log_text("synthetic".to_string(), &log, 2000, 4096, 8)
            .expect("scan");
        assert_eq!(analysis.recv_entries, 1);
        assert_eq!(analysis.recv_entries_with_headers, 1);
        assert_eq!(analysis.recv_entries_with_multiple_headers, 1);
        assert_eq!(analysis.complete_small_packets, 2);
        assert_eq!(analysis.aligned_next_header_links, 1);
        assert_eq!(analysis.decl_a_counts.get(&288), Some(&1));
        assert_eq!(analysis.decl_a_counts.get(&312), Some(&1));
        assert_eq!(analysis.complete_small_field44_plus_68_matches, 2);
        assert_eq!(
            analysis.complete_small_penc_offset_counts.get(&60),
            Some(&2)
        );
        assert_eq!(
            analysis
                .complete_small_field56_minus_field44_counts
                .get(&28),
            Some(&2)
        );
    }

    #[test]
    fn reassembles_large_object_until_next_header() {
        let mut start = make_packet(741_918, 741_850, true);
        start.resize(1536, 0);
        write_utf16_code(&mut start, 0x298, "SH000001");
        write_utf16_code(&mut start, 0x392, "SH000002");
        write_utf16_code(&mut start, 0x48c, "SH000003");

        let mut follow = vec![0u8; 1024];
        write_utf16_code(&mut follow, 0x0186, "SH000048");
        write_utf16_code(&mut follow, 0x0280, "SH000050");

        let next_small = make_packet(292, 224, false);
        let mut log = String::new();
        log.push_str("[recv] socket=0x1048 peer=127.0.0.1:2000 len=10240\n");
        log.push_str(&hexdump(&start));
        log.push_str("[recv] socket=0x1048 peer=127.0.0.1:2000 len=10240\n");
        log.push_str(&hexdump(&follow));
        log.push_str("[recv] socket=0x1048 peer=127.0.0.1:2000 len=292\n");
        log.push_str(&hexdump(&next_small));

        let analysis = analyze_local_2000_log_text("synthetic".to_string(), &log, 2000, 4096, 8)
            .expect("scan");
        assert_eq!(analysis.large_objects.len(), 1);
        let object = &analysis.large_objects[0];
        assert_eq!(object.declared_len, 741_918);
        assert_eq!(object.segment_count, 2);
        assert_eq!(object.accumulated_recv_len, 20_480);
        assert_eq!(object.accumulated_captured_len, 2_560);
        assert!(!object.recv_complete);
        assert_eq!(object.stop_reason, "next-header-recv#3@offset=0");
        assert_eq!(object.penc_offsets.first().copied(), Some(60));
        assert_eq!(object.hypenc_offsets.first().copied(), Some(68));
        assert_eq!(
            object.first_codes.first().map(|item| item.code.as_str()),
            Some("SH000001")
        );
        assert_eq!(object.code_step_counts.get(&250), Some(&3));
    }

    #[test]
    fn accepts_recv_large_with_ansi_and_line_numbers() {
        let packet = make_packet(288, 220, false);
        let mut log = String::new();
        log.push_str("234:\u{1b}[0;32m[recv-large]\u{1b}[0m index=7 socket=0x1168 peer=127.0.0.1:2000 len=288 preview=288\n");
        log.push_str(
            &hexdump(&packet)
                .lines()
                .enumerate()
                .map(|(index, line)| format!("{}:\u{1b}[0;32m{line}\u{1b}[0m", 235 + index))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        log.push('\n');

        let analysis = analyze_local_2000_log_text("synthetic".to_string(), &log, 2000, 4096, 8)
            .expect("scan");
        assert_eq!(analysis.recv_entries, 1);
        assert_eq!(analysis.complete_small_packets, 1);
        assert_eq!(analysis.entries[0].line_no, 1);
        assert_eq!(analysis.entries[0].packet_hits[0].decl_a, Some(288));
    }

    #[test]
    fn parses_known_v11_windows_debug_sample() {
        let path = crate::repository_fixture_path(
            "windows_debug/tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v11.log",
        );
        let analysis = analyze_local_2000_log_file(path, 2000, 4096, 20).expect("v11 sample");
        assert_eq!(analysis.recv_entries, 1940);
        assert_eq!(analysis.recv_entries_with_multiple_headers, 5);
        assert_eq!(analysis.complete_small_packets, 19);
        assert_eq!(analysis.aligned_next_header_links, 12);
        assert_eq!(analysis.decl_a_counts.get(&741_918), Some(&1));
        assert_eq!(
            analysis.complete_small_penc_offset_counts.get(&60),
            Some(&19)
        );
        assert_eq!(
            analysis.complete_small_hypenc_offset_counts.get(&68),
            Some(&1)
        );
        let first_large = analysis.large_objects.first().expect("large object");
        assert_eq!(first_large.declared_len, 741_918);
        assert_eq!(first_large.field40, Some(2));
        assert_eq!(first_large.field52, Some(9));
    }
}
