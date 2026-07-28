use std::error::Error;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use flate2::read::{GzDecoder, ZlibDecoder};
use serde::Serialize;
use zstd::stream::decode_all as zstd_decode_all;

use crate::{Packet, parse_answer_buffer};

const OEM_HEAD_LEN: usize = 200;
const NET_PACKET_PREFIX_LEN: usize = 74;
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

#[derive(Clone, Debug, Serialize)]
pub struct StreamAnalyzeResult {
    pub input: String,
    pub is_hex: bool,
    pub size: usize,
    pub byte_profile: ByteProfile,
    pub utf8_preview: Option<String>,
    pub utf16le_preview: Option<String>,
    pub http_headers: Option<String>,
    pub netpacket_runs: Vec<NetPacketRun>,
    pub packet_summaries: Vec<NetPacketSliceSummary>,
    pub zlib_hits: Vec<ZlibHit>,
    pub oem_at_0: Option<String>,
    pub oem_at_4: Option<String>,
    pub candidate_packets: Vec<CandidatePacket>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ByteProfile {
    pub entropy: f64,
    pub ascii_ish_percent: f64,
    pub zero_percent: f64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct NetPacketPrefix {
    pub object_name: String,
    pub field_20: u32,
    pub field_24: u32,
    pub field_28: u32,
    pub field_32: u32,
    pub field_36: u32,
    pub field_40: u32,
    pub field_44: u32,
    pub field_48: u32,
    pub field_52: u32,
    pub field_56: u32,
    pub tail: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NetPacketRun {
    pub offset: usize,
    pub count: usize,
    pub summary: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NetPacketSliceSummary {
    pub offset: usize,
    pub packet_len: u32,
    pub summary: String,
    pub zstd_offset: Option<usize>,
    pub zstd_compressed_len: Option<usize>,
    pub zstd_decompressed_len: Option<usize>,
    pub inflated_prefix: Option<String>,
    pub utf16_strings: Vec<String>,
    pub ascii_strings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ZlibHit {
    pub offset: usize,
    pub decompressed_len: usize,
    pub utf8_preview: Option<String>,
    pub utf16_preview: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CandidatePacket {
    pub offset: usize,
    pub summary: String,
}

#[derive(Debug, Clone)]
struct NetPacketSlice {
    offset: usize,
    prefix: NetPacketPrefix,
}

pub fn analyze_stream_file(
    path: impl AsRef<Path>,
    is_hex: bool,
) -> Result<StreamAnalyzeResult, Box<dyn Error>> {
    let path = path.as_ref();
    let bytes = if is_hex {
        parse_hex_like(&fs::read_to_string(path)?)?
    } else {
        fs::read(path)?
    };

    Ok(StreamAnalyzeResult {
        input: path.display().to_string(),
        is_hex,
        size: bytes.len(),
        byte_profile: compute_byte_profile(&bytes),
        utf8_preview: try_utf8_preview(&bytes),
        utf16le_preview: try_utf16le_preview(&bytes),
        http_headers: detect_http_headers(&bytes),
        netpacket_runs: collect_netpacket_runs(&bytes),
        packet_summaries: collect_packet_summaries(&bytes),
        zlib_hits: collect_zlib_hits(&bytes),
        oem_at_0: try_parse_packet(&bytes).map(|packet| packet.summary()),
        oem_at_4: if bytes.len() > 4 {
            try_parse_packet(&bytes[4..]).map(|packet| packet.summary())
        } else {
            None
        },
        candidate_packets: collect_candidate_packets(&bytes),
    })
}

pub fn parse_hex_like(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut compact = String::new();
    for line in input.lines() {
        let mut part = line;
        if let Some((left, right)) = line.split_once("  ") {
            if !right.trim().is_empty()
                && left
                    .chars()
                    .all(|ch| ch.is_ascii_hexdigit() || ch.is_ascii_whitespace())
            {
                part = right;
            }
        }
        for token in part.split_whitespace() {
            let token =
                token.trim_matches(|ch: char| matches!(ch, ',' | ';' | '[' | ']' | '(' | ')'));
            let token = token.strip_suffix(':').unwrap_or(token);
            let token = token
                .strip_prefix("0x")
                .or_else(|| token.strip_prefix("0X"))
                .unwrap_or(token);
            if token.len() > 2 && token.chars().all(|ch| ch.is_ascii_hexdigit()) {
                continue;
            }
            if token.len() == 2 && token.chars().all(|ch| ch.is_ascii_hexdigit()) {
                compact.push_str(token);
            }
        }
    }

    if compact.is_empty() {
        let mut tokens = Vec::<String>::new();
        let mut current = String::new();
        for ch in input.chars() {
            if ch.is_ascii_hexdigit() || matches!(ch, 'x' | 'X') {
                current.push(ch);
            } else if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            tokens.push(current);
        }

        for token in &tokens {
            let token = token
                .strip_prefix("0x")
                .or_else(|| token.strip_prefix("0X"))
                .unwrap_or(token.as_str());
            if token.len() == 2 && token.chars().all(|ch| ch.is_ascii_hexdigit()) {
                compact.push_str(token);
            }
        }

        if compact.is_empty() && tokens.len() == 1 {
            let token = tokens[0]
                .strip_prefix("0x")
                .or_else(|| tokens[0].strip_prefix("0X"))
                .unwrap_or(tokens[0].as_str());
            if token.len() % 2 == 0 && token.chars().all(|ch| ch.is_ascii_hexdigit()) {
                compact.push_str(token);
            }
        }
    }

    if compact.len() % 2 != 0 {
        return Err("hex text has odd number of hex digits".into());
    }

    let mut out = Vec::with_capacity(compact.len() / 2);
    for i in (0..compact.len()).step_by(2) {
        out.push(u8::from_str_radix(&compact[i..i + 2], 16)?);
    }
    Ok(out)
}

fn compute_byte_profile(bytes: &[u8]) -> ByteProfile {
    if bytes.is_empty() {
        return ByteProfile {
            entropy: 0.0,
            ascii_ish_percent: 0.0,
            zero_percent: 0.0,
        };
    }

    let mut counts = [0usize; 256];
    let mut zeroes = 0usize;
    let mut ascii_like = 0usize;
    for &byte in bytes {
        counts[byte as usize] += 1;
        if byte == 0 {
            zeroes += 1;
        }
        if byte.is_ascii_graphic() || matches!(byte, b' ' | b'\n' | b'\r' | b'\t') {
            ascii_like += 1;
        }
    }

    let len = bytes.len() as f64;
    let mut entropy = 0.0f64;
    for &count in &counts {
        if count == 0 {
            continue;
        }
        let p = count as f64 / len;
        entropy -= p * p.log2();
    }

    ByteProfile {
        entropy,
        ascii_ish_percent: ascii_like as f64 * 100.0 / len,
        zero_percent: zeroes as f64 * 100.0 / len,
    }
}

fn try_utf8_preview(bytes: &[u8]) -> Option<String> {
    let len = bytes
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(bytes.len())
        .min(256);
    let text = std::str::from_utf8(&bytes[..len]).ok()?.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn try_utf16le_preview(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 4 {
        return None;
    }

    let limit = bytes.len().min(256) / 2;
    let mut words = Vec::with_capacity(limit);
    for chunk in bytes[..limit * 2].chunks_exact(2) {
        let word = u16::from_le_bytes([chunk[0], chunk[1]]);
        if word == 0 {
            break;
        }
        words.push(word);
    }

    if words.is_empty() {
        return None;
    }

    let text = String::from_utf16_lossy(&words);
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn detect_http_headers(bytes: &[u8]) -> Option<String> {
    let marker = b"\r\n\r\n";
    let end = bytes.windows(marker.len()).position(|win| win == marker)? + marker.len();
    let text = std::str::from_utf8(&bytes[..end]).ok()?;
    if text.starts_with("HTTP/") || text.starts_with("GET ") || text.starts_with("POST ") {
        Some(text.to_string())
    } else {
        None
    }
}

fn collect_netpacket_runs(bytes: &[u8]) -> Vec<NetPacketRun> {
    let mut records = Vec::new();
    let mut offset = 0usize;

    while offset + NET_PACKET_PREFIX_LEN <= bytes.len() {
        let Some(record) = parse_netpacket_prefix(&bytes[offset..offset + NET_PACKET_PREFIX_LEN])
        else {
            break;
        };
        records.push(record);
        offset += NET_PACKET_PREFIX_LEN;
    }

    let mut runs = Vec::new();
    let mut run_start = 0usize;
    while run_start < records.len() {
        let mut run_end = run_start + 1;
        while run_end < records.len() && records[run_end] == records[run_start] {
            run_end += 1;
        }
        runs.push(NetPacketRun {
            offset: run_start * NET_PACKET_PREFIX_LEN,
            count: run_end - run_start,
            summary: records[run_start].summary(),
        });
        run_start = run_end;
    }
    runs
}

fn collect_packet_summaries(bytes: &[u8]) -> Vec<NetPacketSliceSummary> {
    let packets = collect_netpacket_packets(bytes);
    let mut unique_packets = Vec::<(NetPacketSlice, &[u8])>::new();

    for packet in packets {
        let slice = &bytes[packet.offset..packet.offset + packet.prefix.field_32 as usize];
        if unique_packets
            .iter()
            .all(|(_, candidate)| *candidate != slice)
        {
            unique_packets.push((packet, slice));
        }
    }

    unique_packets
        .into_iter()
        .take(6)
        .map(|(packet, slice)| summarize_packet_slice(packet, slice))
        .collect()
}

fn summarize_packet_slice(packet: NetPacketSlice, bytes: &[u8]) -> NetPacketSliceSummary {
    let mut summary = NetPacketSliceSummary {
        offset: packet.offset,
        packet_len: packet.prefix.field_32,
        summary: packet.prefix.summary(),
        zstd_offset: None,
        zstd_compressed_len: None,
        zstd_decompressed_len: None,
        inflated_prefix: None,
        utf16_strings: Vec::new(),
        ascii_strings: Vec::new(),
    };

    if let Some(start) = find_zstd(bytes) {
        summary.zstd_offset = Some(start);
        if let Some((compressed_len, inflated)) = extract_zstd_frame(&bytes[start..]) {
            summary.zstd_compressed_len = Some(compressed_len);
            summary.zstd_decompressed_len = Some(inflated.len());
            summary.inflated_prefix = parse_object_prefix(&inflated).map(|prefix| prefix.summary());
            summary.utf16_strings = scan_utf16_strings(&inflated, 2)
                .into_iter()
                .take(12)
                .map(|(_, text)| text)
                .collect();
            summary.ascii_strings = scan_ascii_strings(&inflated, 4)
                .into_iter()
                .take(12)
                .map(|(_, text)| text)
                .collect();
        }
    }

    summary
}

fn collect_zlib_hits(bytes: &[u8]) -> Vec<ZlibHit> {
    find_zlib_offsets(bytes)
        .into_iter()
        .filter_map(|offset| {
            let inflated = try_zlib(&bytes[offset..])?;
            Some(ZlibHit {
                offset,
                decompressed_len: inflated.len(),
                utf8_preview: try_utf8_preview(&inflated),
                utf16_preview: try_utf16le_preview(&inflated),
            })
        })
        .take(8)
        .collect()
}

fn collect_candidate_packets(bytes: &[u8]) -> Vec<CandidatePacket> {
    if bytes.len() < OEM_HEAD_LEN {
        return Vec::new();
    }

    let mut hits = Vec::new();
    for offset in 0..=bytes.len() - OEM_HEAD_LEN {
        let slice = &bytes[offset..];
        if let Some(packet) = try_parse_packet(slice) {
            if is_useful_packet(&packet) {
                hits.push(CandidatePacket {
                    offset,
                    summary: packet.summary(),
                });
                if hits.len() >= 8 {
                    break;
                }
            }
        }
    }
    hits
}

fn collect_netpacket_packets(bytes: &[u8]) -> Vec<NetPacketSlice> {
    let mut packets = Vec::new();
    let mut offset = 0usize;

    while offset + NET_PACKET_PREFIX_LEN <= bytes.len() {
        let Some(prefix) = parse_netpacket_prefix(&bytes[offset..offset + NET_PACKET_PREFIX_LEN])
        else {
            offset += 1;
            continue;
        };

        let packet_len = prefix.field_32 as usize;
        if packet_len < NET_PACKET_PREFIX_LEN || offset + packet_len > bytes.len() {
            offset += 1;
            continue;
        }

        packets.push(NetPacketSlice { offset, prefix });
        offset += packet_len;
    }

    packets
}

fn parse_netpacket_prefix(bytes: &[u8]) -> Option<NetPacketPrefix> {
    let prefix = parse_object_prefix(bytes)?;
    if prefix.object_name != "网络包" {
        return None;
    }
    Some(prefix)
}

fn parse_object_prefix(bytes: &[u8]) -> Option<NetPacketPrefix> {
    if bytes.len() < NET_PACKET_PREFIX_LEN {
        return None;
    }

    let object_name = decode_utf16_z(&bytes[..20])?;
    if object_name.is_empty() {
        return None;
    }

    Some(NetPacketPrefix {
        object_name,
        field_20: u32::from_le_bytes(bytes[20..24].try_into().ok()?),
        field_24: u32::from_le_bytes(bytes[24..28].try_into().ok()?),
        field_28: u32::from_le_bytes(bytes[28..32].try_into().ok()?),
        field_32: u32::from_le_bytes(bytes[32..36].try_into().ok()?),
        field_36: u32::from_le_bytes(bytes[36..40].try_into().ok()?),
        field_40: u32::from_le_bytes(bytes[40..44].try_into().ok()?),
        field_44: u32::from_le_bytes(bytes[44..48].try_into().ok()?),
        field_48: u32::from_le_bytes(bytes[48..52].try_into().ok()?),
        field_52: u32::from_le_bytes(bytes[52..56].try_into().ok()?),
        field_56: u32::from_le_bytes(bytes[56..60].try_into().ok()?),
        tail: decode_utf16_tail(&bytes[60..74]),
    })
}

impl NetPacketPrefix {
    fn summary(&self) -> String {
        format!(
            "name={} field20={} field32={} field36={} field40={} field44={} field52={} field56={} tail={}",
            self.object_name,
            self.field_20,
            self.field_32,
            self.field_36,
            self.field_40,
            self.field_44,
            self.field_52,
            self.field_56,
            self.tail
        )
    }
}

fn find_zlib_offsets(bytes: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    for offset in 0..bytes.len().saturating_sub(1) {
        if looks_like_zlib_header(bytes[offset], bytes[offset + 1]) {
            out.push(offset);
        }
    }
    out
}

fn looks_like_zlib_header(cmf: u8, flg: u8) -> bool {
    if cmf & 0x0f != 8 {
        return false;
    }
    (((cmf as u16) << 8) | flg as u16) % 31 == 0
}

fn try_zlib(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

#[allow(dead_code)]
fn try_gzip(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = GzDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

fn try_parse_packet(bytes: &[u8]) -> Option<Packet> {
    if bytes.len() < OEM_HEAD_LEN {
        return None;
    }
    let packet = unsafe { parse_answer_buffer(bytes) }?;
    if is_useful_packet(&packet) {
        Some(packet)
    } else {
        None
    }
}

fn is_useful_packet(packet: &Packet) -> bool {
    match packet {
        Packet::Unknown { head } => is_known_type(&head.packet_type),
        _ => true,
    }
}

fn is_known_type(value: &str) -> bool {
    matches!(
        value,
        "无效请求"
            | "代码表"
            | "实时数据"
            | "分笔"
            | "分时"
            | "1分钟线"
            | "5分钟线"
            | "15分钟线"
            | "30分钟线"
            | "60分钟线"
            | "日线"
            | "周线"
            | "月线"
            | "季线"
            | "年线"
            | "多日线"
            | "除权"
            | "财务"
            | "F10资料"
            | "6到10档挂单"
    )
}

fn decode_utf16_z(bytes: &[u8]) -> Option<String> {
    if bytes.len() % 2 != 0 {
        return None;
    }

    let mut words = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        let word = u16::from_le_bytes([chunk[0], chunk[1]]);
        if word == 0 {
            break;
        }
        words.push(word);
    }

    if words.is_empty() {
        None
    } else {
        Some(String::from_utf16_lossy(&words))
    }
}

fn decode_utf16_tail(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks_exact(2) {
        let word = u16::from_le_bytes([chunk[0], chunk[1]]);
        if word == 0 {
            if !out.ends_with('|') {
                out.push('|');
            }
            continue;
        }
        if let Some(ch) = char::from_u32(word as u32) {
            if ch.is_control() {
                continue;
            }
            out.push(ch);
        }
    }
    out.trim_matches('|').to_string()
}

fn find_zstd(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(ZSTD_MAGIC.len())
        .position(|w| w == ZSTD_MAGIC)
}

fn extract_zstd_frame(bytes: &[u8]) -> Option<(usize, Vec<u8>)> {
    for end in (ZSTD_MAGIC.len()..=bytes.len()).rev() {
        let slice = &bytes[..end];
        let Ok(inflated) = zstd_decode_all(Cursor::new(slice)) else {
            continue;
        };
        return Some((end, inflated));
    }
    None
}

fn scan_ascii_strings(bytes: &[u8], min_chars: usize) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut start = 0usize;

    while start < bytes.len() {
        while start < bytes.len() && !bytes[start].is_ascii_graphic() {
            start += 1;
        }
        if start >= bytes.len() {
            break;
        }

        let mut end = start;
        while end < bytes.len() && bytes[end].is_ascii_graphic() {
            end += 1;
        }

        if end - start >= min_chars {
            out.push((
                start,
                String::from_utf8_lossy(&bytes[start..end]).to_string(),
            ));
        }
        start = end + 1;
    }

    out
}

fn scan_utf16_strings(bytes: &[u8], min_chars: usize) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut offset = 0usize;

    while offset + 1 < bytes.len() {
        let mut words = Vec::<u16>::new();
        let mut cursor = offset;
        let mut terminated = false;

        while cursor + 1 < bytes.len() {
            let word = u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
            if word == 0 {
                terminated = true;
                break;
            }

            let Some(ch) = char::from_u32(word as u32) else {
                words.clear();
                break;
            };
            if ch.is_control() {
                words.clear();
                break;
            }

            words.push(word);
            cursor += 2;
        }

        if terminated && words.len() >= min_chars {
            out.push((offset, String::from_utf16_lossy(&words)));
            offset = cursor + 2;
        } else {
            offset += 2;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{analyze_stream_file, parse_hex_like};

    #[test]
    fn parses_hex_text() {
        let bytes = parse_hex_like("0000  48 54 54 50 2f 31 2e 31").expect("parse hex");
        assert_eq!(bytes, b"HTTP/1.1");
    }

    #[test]
    fn analyzes_small_http_stream() {
        let sample = b"HTTP/1.1 200 OK\r\nServer: test\r\n\r\n";
        let path = std::env::temp_dir().join("netzip_test_http.bin");
        fs::write(&path, sample).expect("write stream");

        let result = analyze_stream_file(&path, false).expect("analyze stream");
        assert_eq!(result.size, sample.len());
        assert!(
            result
                .http_headers
                .as_deref()
                .unwrap_or("")
                .starts_with("HTTP/1.1")
        );
        assert!(result.zlib_hits.is_empty());

        let _ = fs::remove_file(path);
    }
}
