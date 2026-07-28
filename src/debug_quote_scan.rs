use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::Path;

use encoding_rs::GBK;
use flate2::read::ZlibDecoder;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct QuoteFrameScanResult {
    pub input: String,
    pub size: usize,
    pub mode: String,
    pub server_frames: Vec<Server16FrameSummary>,
    pub client_frames: Vec<Client10FrameSummary>,
    pub head_hex: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Server16FrameSummary {
    pub index: usize,
    pub offset: usize,
    pub label: Option<String>,
    pub op: u16,
    pub sub: u16,
    pub flags: u16,
    pub tag: u16,
    pub compressed_len: usize,
    pub original_len: usize,
    pub zlib_ok: bool,
    pub inflated_len: Option<usize>,
    pub inflated_prefix: Option<String>,
    pub body_head_hex: String,
    pub block8: Option<BlockPatternSummary>,
    pub utf16_strings: Vec<String>,
    pub ascii_strings: Vec<String>,
    pub code_table: Option<CodeTableSummary>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Client10FrameSummary {
    pub index: usize,
    pub offset: usize,
    pub label: Option<String>,
    pub op: u16,
    pub sub: u16,
    pub flags: u16,
    pub payload_len: usize,
    pub inner_tag: Option<u16>,
    pub inner_kind: Option<u16>,
    pub code_table_request: Option<CodeTableRequestSummary>,
    pub body_head_hex: String,
    pub block8: Option<BlockPatternSummary>,
    pub body_after_tag_block8: Option<BlockPatternSummary>,
    pub utf16_strings: Vec<String>,
    pub ascii_strings: Vec<String>,
    pub numeric_tokens: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlockPatternSummary {
    pub block_size: usize,
    pub total_blocks: usize,
    pub unique_blocks: usize,
    pub most_common_hex: String,
    pub most_common_count: usize,
    pub most_common_positions: Vec<usize>,
    pub remainder_len: usize,
    pub remainder_hex: String,
    pub top_patterns: Vec<BlockPatternHit>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlockPatternHit {
    pub block_hex: String,
    pub count: usize,
    pub first_positions: Vec<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CodeTableRequestSummary {
    pub bucket: u16,
    pub offset: u16,
}

#[derive(Clone, Debug, Serialize)]
pub struct CodeTableRowSummary {
    pub code: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CodeTableSummary {
    pub count: usize,
    pub first: CodeTableRowSummary,
    pub last: CodeTableRowSummary,
}

#[derive(Clone, Copy, Debug)]
struct Server16Frame {
    offset: usize,
    op: u16,
    sub: u16,
    flags: u16,
    tag: u16,
    compressed_len: usize,
    original_len: usize,
}

#[derive(Clone, Copy, Debug)]
struct Client10Frame {
    offset: usize,
    op: u16,
    sub: u16,
    flags: u16,
    payload_len: usize,
}

pub fn scan_quote_frame_file(
    path: impl AsRef<Path>,
) -> Result<QuoteFrameScanResult, Box<dyn Error>> {
    let path = path.as_ref();
    let bytes = fs::read(path)?;
    Ok(scan_quote_frame_bytes(path.display().to_string(), &bytes))
}

pub fn scan_quote_frame_bytes(input: String, bytes: &[u8]) -> QuoteFrameScanResult {
    if let Some(frames) = parse_server16_frames(bytes) {
        let server_frames = frames
            .iter()
            .enumerate()
            .map(|(index, frame)| summarize_server_frame(index, *frame, bytes))
            .collect::<Vec<_>>();
        return QuoteFrameScanResult {
            input,
            size: bytes.len(),
            mode: "server16".to_string(),
            server_frames,
            client_frames: Vec::new(),
            head_hex: hex_dump(bytes, 128),
        };
    }

    if let Some(frames) = parse_client10_frames(bytes) {
        let client_frames = frames
            .iter()
            .enumerate()
            .map(|(index, frame)| summarize_client_frame(index, *frame, bytes))
            .collect::<Vec<_>>();
        return QuoteFrameScanResult {
            input,
            size: bytes.len(),
            mode: "client10".to_string(),
            server_frames: Vec::new(),
            client_frames,
            head_hex: hex_dump(bytes, 128),
        };
    }

    QuoteFrameScanResult {
        input,
        size: bytes.len(),
        mode: "unknown".to_string(),
        server_frames: Vec::new(),
        client_frames: Vec::new(),
        head_hex: hex_dump(bytes, bytes.len().min(128)),
    }
}

fn parse_server16_frames(bytes: &[u8]) -> Option<Vec<Server16Frame>> {
    if bytes.len() < 16 || !bytes.starts_with(&[0xb1, 0xcb, 0x74, 0x00]) {
        return None;
    }

    let mut out = Vec::new();
    let mut offset = 0usize;
    while offset < bytes.len() {
        if offset + 16 > bytes.len() || bytes[offset..].len() < 16 {
            return None;
        }
        if bytes[offset..offset + 4] != [0xb1, 0xcb, 0x74, 0x00] {
            return None;
        }
        let compressed_len = le_u16(&bytes[offset + 12..offset + 14]) as usize;
        let total = 16 + compressed_len;
        if offset + total > bytes.len() {
            return None;
        }
        out.push(Server16Frame {
            offset,
            op: le_u16(&bytes[offset + 4..offset + 6]),
            sub: le_u16(&bytes[offset + 6..offset + 8]),
            flags: le_u16(&bytes[offset + 8..offset + 10]),
            tag: le_u16(&bytes[offset + 10..offset + 12]),
            compressed_len,
            original_len: le_u16(&bytes[offset + 14..offset + 16]) as usize,
        });
        offset += total;
    }
    Some(out)
}

fn parse_client10_frames(bytes: &[u8]) -> Option<Vec<Client10Frame>> {
    if bytes.len() < 10 {
        return None;
    }

    let mut out = Vec::new();
    let mut offset = 0usize;
    while offset < bytes.len() {
        if offset + 10 > bytes.len() {
            return None;
        }
        let len1 = le_u16(&bytes[offset + 6..offset + 8]) as usize;
        let len2 = le_u16(&bytes[offset + 8..offset + 10]) as usize;
        if len1 == 0 || len1 != len2 {
            return None;
        }
        let total = 10 + len1;
        if offset + total > bytes.len() {
            return None;
        }
        out.push(Client10Frame {
            offset,
            op: le_u16(&bytes[offset..offset + 2]),
            sub: le_u16(&bytes[offset + 2..offset + 4]),
            flags: le_u16(&bytes[offset + 4..offset + 6]),
            payload_len: len1,
        });
        offset += total;
    }
    Some(out)
}

fn summarize_server_frame(
    index: usize,
    frame: Server16Frame,
    bytes: &[u8],
) -> Server16FrameSummary {
    let body_start = frame.offset + 16;
    let body_end = body_start + frame.compressed_len;
    let body = &bytes[body_start..body_end];
    let mut summary = Server16FrameSummary {
        index,
        offset: frame.offset,
        label: None,
        op: frame.op,
        sub: frame.sub,
        flags: frame.flags,
        tag: frame.tag,
        compressed_len: frame.compressed_len,
        original_len: frame.original_len,
        zlib_ok: false,
        inflated_len: None,
        inflated_prefix: None,
        body_head_hex: hex_line(&body[..body.len().min(32)]),
        block8: summarize_blocks(body, 8),
        utf16_strings: Vec::new(),
        ascii_strings: Vec::new(),
        code_table: None,
    };

    if let Some(inflated) = zlib_decode(body) {
        summary.zlib_ok = true;
        summary.inflated_len = Some(inflated.len());
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
        summary.code_table = parse_code_table_records(&inflated);
    }

    summary.label = detect_server_label(frame, body, &summary);

    summary
}

fn summarize_client_frame(
    index: usize,
    frame: Client10Frame,
    bytes: &[u8],
) -> Client10FrameSummary {
    let body_start = frame.offset + 10;
    let body_end = body_start + frame.payload_len;
    let body = &bytes[body_start..body_end];
    let code_table_request = parse_code_table_request(body);
    let summary = Client10FrameSummary {
        index,
        offset: frame.offset,
        label: None,
        op: frame.op,
        sub: frame.sub,
        flags: frame.flags,
        payload_len: frame.payload_len,
        inner_tag: if body.len() >= 2 {
            Some(le_u16(&body[0..2]))
        } else {
            None
        },
        inner_kind: if body.len() >= 4 {
            Some(le_u16(&body[2..4]))
        } else {
            None
        },
        code_table_request,
        body_head_hex: hex_line(&body[..body.len().min(32)]),
        block8: summarize_blocks(body, 8),
        body_after_tag_block8: body.get(2..).and_then(|rest| summarize_blocks(rest, 8)),
        utf16_strings: scan_utf16_strings(body, 2)
            .into_iter()
            .take(12)
            .map(|(_, text)| text)
            .collect(),
        ascii_strings: scan_ascii_strings(body, 4)
            .into_iter()
            .take(12)
            .map(|(_, text)| text)
            .collect(),
        numeric_tokens: extract_numeric_tokens(body),
    };

    Client10FrameSummary {
        label: detect_client_label(frame, body, &summary),
        ..summary
    }
}

fn parse_code_table_request(bytes: &[u8]) -> Option<CodeTableRequestSummary> {
    if bytes.len() != 6 || le_u16(&bytes[0..2]) != 0x0450 {
        return None;
    }
    Some(CodeTableRequestSummary {
        bucket: le_u16(&bytes[2..4]),
        offset: le_u16(&bytes[4..6]),
    })
}

fn summarize_blocks(bytes: &[u8], block_size: usize) -> Option<BlockPatternSummary> {
    if block_size == 0 || bytes.len() < block_size * 2 {
        return None;
    }

    let full_chunks = bytes
        .chunks(block_size)
        .take_while(|chunk| chunk.len() == block_size)
        .collect::<Vec<_>>();
    if full_chunks.len() < 2 {
        return None;
    }

    let mut positions = std::collections::BTreeMap::<Vec<u8>, Vec<usize>>::new();
    let mut total = 0usize;
    for (index, chunk) in full_chunks.iter().enumerate() {
        total += 1;
        positions.entry((*chunk).to_vec()).or_default().push(index);
    }

    let (most_common, most_common_positions) = positions
        .iter()
        .max_by_key(|(_, indexes)| indexes.len())
        .expect("non-empty");
    let most_common_count = most_common_positions.len();
    let unique_blocks = positions.len();
    let top_patterns = positions
        .iter()
        .map(|(block, indexes)| BlockPatternHit {
            block_hex: hex_line(block),
            count: indexes.len(),
            first_positions: indexes.iter().copied().take(12).collect(),
        })
        .collect::<Vec<_>>();
    let mut top_patterns = top_patterns;
    top_patterns.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.first_positions.cmp(&right.first_positions))
            .then_with(|| left.block_hex.cmp(&right.block_hex))
    });
    top_patterns.truncate(6);

    Some(BlockPatternSummary {
        block_size,
        total_blocks: total,
        unique_blocks,
        most_common_hex: hex_line(most_common),
        most_common_count,
        most_common_positions: most_common_positions.iter().copied().take(16).collect(),
        remainder_len: bytes.len() % block_size,
        remainder_hex: hex_line(&bytes[total * block_size..]),
        top_patterns,
    })
}

fn parse_code_table_records(bytes: &[u8]) -> Option<CodeTableSummary> {
    if bytes.len() < 2 {
        return None;
    }
    let count = le_u16(&bytes[0..2]) as usize;
    if count == 0 || bytes.len() != 2 + count * 29 {
        return None;
    }
    let first = parse_code_table_row(&bytes[2..31])?;
    let last_start = 2 + (count - 1) * 29;
    let last = parse_code_table_row(&bytes[last_start..last_start + 29])?;
    Some(CodeTableSummary { count, first, last })
}

fn parse_code_table_row(bytes: &[u8]) -> Option<CodeTableRowSummary> {
    if bytes.len() != 29 {
        return None;
    }
    let digits = std::str::from_utf8(&bytes[0..6]).ok()?;
    if !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let suffix = bytes[6];
    let code = if suffix.is_ascii_alphanumeric() {
        format!("{digits}{}", suffix as char)
    } else {
        digits.to_string()
    };
    Some(CodeTableRowSummary {
        code,
        name: decode_gbk_name_slot(&bytes[8..16])?,
    })
}

fn decode_gbk_name_slot(bytes: &[u8]) -> Option<String> {
    let field = bytes
        .iter()
        .position(|byte| *byte == 0)
        .map(|end| &bytes[..end])
        .unwrap_or(bytes);

    for end in (0..=field.len()).rev() {
        let (decoded, _, had_errors) = GBK.decode(&field[..end]);
        if !had_errors && !decoded.is_empty() {
            return Some(decoded.into_owned());
        }
    }

    None
}

fn extract_numeric_tokens(bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut token = Vec::new();
    for &byte in bytes {
        match byte {
            0 | 1 => {
                if !token.is_empty() {
                    if let Ok(text) = std::str::from_utf8(&token) {
                        if text.chars().all(|ch| ch.is_ascii_digit()) {
                            out.push(text.to_string());
                        }
                    }
                    token.clear();
                }
            }
            b if b.is_ascii_digit() => token.push(b),
            _ => {
                if !token.is_empty() {
                    if let Ok(text) = std::str::from_utf8(&token) {
                        if text.chars().all(|ch| ch.is_ascii_digit()) {
                            out.push(text.to_string());
                        }
                    }
                    token.clear();
                }
            }
        }
    }
    if !token.is_empty() {
        if let Ok(text) = std::str::from_utf8(&token) {
            if text.chars().all(|ch| ch.is_ascii_digit()) {
                out.push(text.to_string());
            }
        }
    }
    out
}

#[allow(dead_code)]
fn extract_ascii_strings(bytes: &[u8], min_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = Vec::new();
    for &byte in bytes {
        if byte.is_ascii_graphic() || byte == b' ' {
            current.push(byte);
            continue;
        }

        if current.len() >= min_len {
            out.push(String::from_utf8_lossy(&current).to_string());
        }
        current.clear();
    }
    if current.len() >= min_len {
        out.push(String::from_utf8_lossy(&current).to_string());
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

fn zlib_decode(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

fn detect_client_label(
    frame: Client10Frame,
    body: &[u8],
    summary: &Client10FrameSummary,
) -> Option<String> {
    match (frame.op, frame.sub, frame.payload_len) {
        (0x010c, 0x0000, 2) if body == [0x15, 0x00] => Some("probe.hello".to_string()),
        (0x010c, 0x7b00, 0x011a) if body.starts_with(&[0x0b, 0x00]) => {
            Some("bootstrap.main-site-validate".to_string())
        }
        (0x020c, 0x9400, 3) if body.starts_with(&[0x0d, 0x00]) => {
            Some("bootstrap.server-info".to_string())
        }
        (0x030c, 0x9900, 0x20)
            if body.starts_with(&[0xdb, 0x0f])
                && summary.ascii_strings.iter().any(|s| s == "tdxlevel2") =>
        {
            Some("bootstrap.ticket-register".to_string())
        }
        (_, 0x6d00 | 0x6e00, 6) if summary.code_table_request.is_some() => {
            let bucket = summary
                .code_table_request
                .as_ref()
                .map(|item| item.bucket)
                .unwrap_or_default();
            Some(format!("bootstrap.code-table-chunk.bucket{bucket}"))
        }
        _ => detect_post_login_client_label(frame, body, summary),
    }
}

fn detect_server_label(
    frame: Server16Frame,
    body: &[u8],
    summary: &Server16FrameSummary,
) -> Option<String> {
    match (frame.op, frame.sub, frame.tag) {
        (0x011c, 0x0000, 0x0015) if summary.zlib_ok && body.starts_with(&[0x78, 0x9c]) => {
            Some("probe.hello-reply".to_string())
        }
        (0x010c, 0x7b00, 0x000b) => Some("bootstrap.main-site-validate-reply".to_string()),
        (0x021c, 0x9400, 0x000d) => Some("bootstrap.server-info-reply".to_string()),
        (0x030c, 0x9900, 0x0fdb) => Some("bootstrap.ticket-register-reply".to_string()),
        (_, 0x6d00 | 0x6e00, 0x0450) if summary.code_table.is_some() => {
            let bucket = if frame.sub == 0x6d00 { 0 } else { 1 };
            Some(format!("bootstrap.code-table-chunk-reply.bucket{bucket}"))
        }
        _ => detect_post_login_server_label(frame, body, summary),
    }
}

fn detect_post_login_client_label(
    frame: Client10Frame,
    body: &[u8],
    summary: &Client10FrameSummary,
) -> Option<String> {
    match (
        frame.sub,
        summary.inner_tag,
        summary.inner_kind,
        frame.flags,
    ) {
        (0x7600 | 0x7601, Some(0x000f), Some(0x0032 | 0x0002), 0x0100) => {
            Some("post-login.bulk-record-29b-request".to_string())
        }
        (0x7501 | 0x7502, Some(0x0010), Some(0x0014 | 0x0009), 0x0100) => {
            Some("post-login.fin-143b-request".to_string())
        }
        (0x2900, Some(0x0547), Some(0x0064), 0x0100) => {
            Some("post-login.quote-0547-batch-request.initial".to_string())
        }
        (0x2a00, Some(0x0547), Some(0x0064), 0x0100) => {
            Some("post-login.quote-0547-batch-request.continue".to_string())
        }
        (0x2902, Some(0x0547), Some(0x0064), 0x0100) => {
            Some("post-login.quote-0547-refresh-request.initial".to_string())
        }
        (0x2a02, Some(0x0547), Some(0x0064), 0x0100) => {
            Some("post-login.quote-0547-refresh-request.continue".to_string())
        }
        (0x2a02, Some(0x0547), Some(0x0004), 0x0100) if frame.payload_len <= 64 => {
            Some("post-login.quote-0547-index-request.2a02".to_string())
        }
        (0x2800 | 0x2801 | 0x2802, Some(0x0004), None, 0x0200) if body == [0x04, 0x00] => {
            Some("post-login.quote-0547-status-request".to_string())
        }
        (0x2b02, Some(0x054c), Some(0x0005), 0x0100) => {
            Some("post-login.quote-054c-list-request.2b02".to_string())
        }
        (0x0a07, Some(0x054c), Some(0x0005), 0x0200) => {
            Some("post-login.quote-054c-list-request.0a07".to_string())
        }
        _ => None,
    }
}

fn detect_post_login_server_label(
    frame: Server16Frame,
    body: &[u8],
    summary: &Server16FrameSummary,
) -> Option<String> {
    match (frame.sub, frame.tag) {
        (0x7600 | 0x7601, 0x000f) => Some("post-login.bulk-record-29b-reply".to_string()),
        (0x7501 | 0x7502, 0x0010) => Some("post-login.fin-143b-reply".to_string()),
        (0x2900, 0x0547) if summary.zlib_ok => {
            Some("post-login.quote-0547-batch-reply.initial".to_string())
        }
        (0x2a00, 0x0547) if summary.zlib_ok => {
            Some("post-login.quote-0547-batch-reply.continue".to_string())
        }
        (0x2902, 0x0547) if frame.original_len == 2 || body == [0x93, 0x93] => {
            Some("post-login.quote-0547-ack.initial".to_string())
        }
        (0x2a02, 0x0547) if frame.original_len == 2 || body == [0x93, 0x93] => {
            Some("post-login.quote-0547-ack.continue".to_string())
        }
        (0x2800 | 0x2801 | 0x2802, 0x0004) => {
            Some("post-login.quote-0547-status-reply".to_string())
        }
        (0x2b02, 0x054c) if summary.zlib_ok => {
            Some("post-login.quote-054c-list-reply.2b02".to_string())
        }
        (0x0a07, 0x054c) if summary.zlib_ok => {
            Some("post-login.quote-054c-list-reply.0a07".to_string())
        }
        _ => None,
    }
}

fn parse_object_prefix(bytes: &[u8]) -> Option<ObjectPrefix> {
    if bytes.len() < 74 {
        return None;
    }
    let object_name = decode_utf16_z(&bytes[..20])?;
    if object_name.is_empty() {
        return None;
    }
    Some(ObjectPrefix {
        object_name,
        field_20: le_u32(&bytes[20..24]),
        field_24: le_u32(&bytes[24..28]),
        field_28: le_u32(&bytes[28..32]),
        field_32: le_u32(&bytes[32..36]),
        field_36: le_u32(&bytes[36..40]),
        field_40: le_u32(&bytes[40..44]),
        field_44: le_u32(&bytes[44..48]),
        field_48: le_u32(&bytes[48..52]),
        field_52: le_u32(&bytes[52..56]),
        field_56: le_u32(&bytes[56..60]),
        tail: decode_utf16_tail(&bytes[60..74]),
    })
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct ObjectPrefix {
    object_name: String,
    field_20: u32,
    field_24: u32,
    field_28: u32,
    field_32: u32,
    field_36: u32,
    field_40: u32,
    field_44: u32,
    field_48: u32,
    field_52: u32,
    field_56: u32,
    tail: String,
}

impl ObjectPrefix {
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

fn hex_line(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn hex_dump(bytes: &[u8], limit: usize) -> String {
    let mut out = String::new();
    for (row, chunk) in bytes[..bytes.len().min(limit)].chunks(16).enumerate() {
        out.push_str(&format!("{:08x}: {}\n", row * 16, hex_line(chunk)));
    }
    out.trim_end().to_string()
}

fn le_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn le_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[cfg(test)]
mod tests {
    use super::{
        Client10Frame, Client10FrameSummary, CodeTableRequestSummary, CodeTableRowSummary,
        CodeTableSummary, Server16Frame, Server16FrameSummary, detect_client_label,
        detect_server_label, parse_client10_frames, parse_server16_frames, summarize_blocks,
        summarize_client_frame,
    };

    #[test]
    fn rejects_unknown_blob() {
        assert!(parse_server16_frames(b"abc").is_none());
        assert!(parse_client10_frames(b"abc").is_none());
    }

    #[test]
    fn labels_bootstrap_client_frames() {
        let frame = Client10Frame {
            offset: 0,
            op: 0x030c,
            sub: 0x9900,
            flags: 0x0100,
            payload_len: 0x20,
        };
        let summary = Client10FrameSummary {
            index: 0,
            offset: 0,
            label: None,
            op: frame.op,
            sub: frame.sub,
            flags: frame.flags,
            payload_len: frame.payload_len,
            inner_tag: Some(0x0fdb),
            inner_kind: Some(0x6474),
            code_table_request: None,
            body_head_hex: String::new(),
            block8: None,
            body_after_tag_block8: None,
            utf16_strings: Vec::new(),
            ascii_strings: vec!["tdxlevel2".to_string()],
            numeric_tokens: Vec::new(),
        };
        assert_eq!(
            detect_client_label(frame, &[0xdb, 0x0f, 0x74, 0x64], &summary).as_deref(),
            Some("bootstrap.ticket-register")
        );
    }

    #[test]
    fn labels_code_table_buckets() {
        let client = Client10Frame {
            offset: 0,
            op: 0x2d0c,
            sub: 0x6e00,
            flags: 0x0100,
            payload_len: 6,
        };
        let client_summary = Client10FrameSummary {
            index: 0,
            offset: 0,
            label: None,
            op: client.op,
            sub: client.sub,
            flags: client.flags,
            payload_len: client.payload_len,
            inner_tag: Some(0x0450),
            inner_kind: Some(1),
            code_table_request: Some(CodeTableRequestSummary {
                bucket: 1,
                offset: 18_000,
            }),
            body_head_hex: String::new(),
            block8: None,
            body_after_tag_block8: None,
            utf16_strings: Vec::new(),
            ascii_strings: Vec::new(),
            numeric_tokens: Vec::new(),
        };
        assert_eq!(
            detect_client_label(
                client,
                &[0x50, 0x04, 0x01, 0x00, 0x50, 0x46],
                &client_summary
            )
            .as_deref(),
            Some("bootstrap.code-table-chunk.bucket1")
        );

        let server = Server16Frame {
            offset: 0,
            op: 0x041c,
            sub: 0x6e00,
            flags: 0,
            tag: 0x0450,
            compressed_len: 10,
            original_len: 10,
        };
        let server_summary = Server16FrameSummary {
            index: 0,
            offset: 0,
            label: None,
            op: server.op,
            sub: server.sub,
            flags: server.flags,
            tag: server.tag,
            compressed_len: server.compressed_len,
            original_len: server.original_len,
            zlib_ok: true,
            inflated_len: Some(29),
            inflated_prefix: None,
            body_head_hex: String::new(),
            block8: None,
            utf16_strings: Vec::new(),
            ascii_strings: Vec::new(),
            code_table: Some(CodeTableSummary {
                count: 1,
                first: CodeTableRowSummary {
                    code: "000001d".to_string(),
                    name: "foo".to_string(),
                },
                last: CodeTableRowSummary {
                    code: "000001d".to_string(),
                    name: "foo".to_string(),
                },
            }),
        };
        assert_eq!(
            detect_server_label(server, &[], &server_summary).as_deref(),
            Some("bootstrap.code-table-chunk-reply.bucket1")
        );
    }

    #[test]
    fn labels_post_login_client_frames() {
        let batch_0547 = Client10Frame {
            offset: 0,
            op: 0x400c,
            sub: 0x2900,
            flags: 0x0100,
            payload_len: 1104,
        };
        let batch_0547_summary = Client10FrameSummary {
            index: 0,
            offset: 0,
            label: None,
            op: batch_0547.op,
            sub: batch_0547.sub,
            flags: batch_0547.flags,
            payload_len: batch_0547.payload_len,
            inner_tag: Some(0x0547),
            inner_kind: Some(0x0064),
            code_table_request: None,
            body_head_hex: String::new(),
            block8: None,
            body_after_tag_block8: None,
            utf16_strings: Vec::new(),
            ascii_strings: Vec::new(),
            numeric_tokens: Vec::new(),
        };
        assert_eq!(
            detect_client_label(batch_0547, &[0x47, 0x05, 0x64, 0x00], &batch_0547_summary)
                .as_deref(),
            Some("post-login.quote-0547-batch-request.initial")
        );

        let refresh_0547 = Client10Frame {
            offset: 0,
            op: 0x5d0c,
            sub: 0x2902,
            flags: 0x0100,
            payload_len: 1104,
        };
        let refresh_0547_summary = Client10FrameSummary {
            index: 0,
            offset: 0,
            label: None,
            op: refresh_0547.op,
            sub: refresh_0547.sub,
            flags: refresh_0547.flags,
            payload_len: refresh_0547.payload_len,
            inner_tag: Some(0x0547),
            inner_kind: Some(0x0064),
            code_table_request: None,
            body_head_hex: String::new(),
            block8: None,
            body_after_tag_block8: None,
            utf16_strings: Vec::new(),
            ascii_strings: Vec::new(),
            numeric_tokens: Vec::new(),
        };
        assert_eq!(
            detect_client_label(
                refresh_0547,
                &[0x47, 0x05, 0x64, 0x00],
                &refresh_0547_summary
            )
            .as_deref(),
            Some("post-login.quote-0547-refresh-request.initial")
        );

        let status = Client10Frame {
            offset: 0,
            op: 0x550c,
            sub: 0x2802,
            flags: 0x0200,
            payload_len: 2,
        };
        let status_summary = Client10FrameSummary {
            index: 0,
            offset: 0,
            label: None,
            op: status.op,
            sub: status.sub,
            flags: status.flags,
            payload_len: status.payload_len,
            inner_tag: Some(0x0004),
            inner_kind: None,
            code_table_request: None,
            body_head_hex: String::new(),
            block8: None,
            body_after_tag_block8: None,
            utf16_strings: Vec::new(),
            ascii_strings: Vec::new(),
            numeric_tokens: Vec::new(),
        };
        assert_eq!(
            detect_client_label(status, &[0x04, 0x00], &status_summary).as_deref(),
            Some("post-login.quote-0547-status-request")
        );

        let quote_054c = Client10Frame {
            offset: 0,
            op: 0x440c,
            sub: 0x2b02,
            flags: 0x0100,
            payload_len: 572,
        };
        let quote_054c_summary = Client10FrameSummary {
            index: 0,
            offset: 0,
            label: None,
            op: quote_054c.op,
            sub: quote_054c.sub,
            flags: quote_054c.flags,
            payload_len: quote_054c.payload_len,
            inner_tag: Some(0x054c),
            inner_kind: Some(0x0005),
            code_table_request: None,
            body_head_hex: String::new(),
            block8: None,
            body_after_tag_block8: None,
            utf16_strings: Vec::new(),
            ascii_strings: Vec::new(),
            numeric_tokens: Vec::new(),
        };
        assert_eq!(
            detect_client_label(quote_054c, &[0x4c, 0x05, 0x05, 0x00], &quote_054c_summary)
                .as_deref(),
            Some("post-login.quote-054c-list-request.2b02")
        );
    }

    #[test]
    fn labels_post_login_server_frames() {
        let quote_0547 = Server16Frame {
            offset: 0,
            op: 0x401c,
            sub: 0x2900,
            flags: 0,
            tag: 0x0547,
            compressed_len: 7412,
            original_len: 11541,
        };
        let quote_0547_summary = Server16FrameSummary {
            index: 0,
            offset: 0,
            label: None,
            op: quote_0547.op,
            sub: quote_0547.sub,
            flags: quote_0547.flags,
            tag: quote_0547.tag,
            compressed_len: quote_0547.compressed_len,
            original_len: quote_0547.original_len,
            zlib_ok: true,
            inflated_len: Some(11541),
            inflated_prefix: None,
            body_head_hex: String::new(),
            block8: None,
            utf16_strings: Vec::new(),
            ascii_strings: Vec::new(),
            code_table: None,
        };
        assert_eq!(
            detect_server_label(quote_0547, &[], &quote_0547_summary).as_deref(),
            Some("post-login.quote-0547-batch-reply.initial")
        );

        let bulk_29b = Server16Frame {
            offset: 0,
            op: 0x361c,
            sub: 0x7600,
            flags: 0,
            tag: 0x000f,
            compressed_len: 8328,
            original_len: 27132,
        };
        let bulk_29b_summary = Server16FrameSummary {
            index: 0,
            offset: 0,
            label: None,
            op: bulk_29b.op,
            sub: bulk_29b.sub,
            flags: bulk_29b.flags,
            tag: bulk_29b.tag,
            compressed_len: bulk_29b.compressed_len,
            original_len: bulk_29b.original_len,
            zlib_ok: true,
            inflated_len: Some(27132),
            inflated_prefix: None,
            body_head_hex: String::new(),
            block8: None,
            utf16_strings: Vec::new(),
            ascii_strings: Vec::new(),
            code_table: None,
        };
        assert_eq!(
            detect_server_label(bulk_29b, &[], &bulk_29b_summary).as_deref(),
            Some("post-login.bulk-record-29b-reply")
        );

        let status = Server16Frame {
            offset: 0,
            op: 0x550c,
            sub: 0x2802,
            flags: 0,
            tag: 0x0004,
            compressed_len: 10,
            original_len: 10,
        };
        let status_summary = Server16FrameSummary {
            index: 0,
            offset: 0,
            label: None,
            op: status.op,
            sub: status.sub,
            flags: status.flags,
            tag: status.tag,
            compressed_len: status.compressed_len,
            original_len: status.original_len,
            zlib_ok: false,
            inflated_len: None,
            inflated_prefix: None,
            body_head_hex: String::new(),
            block8: None,
            utf16_strings: Vec::new(),
            ascii_strings: Vec::new(),
            code_table: None,
        };
        assert_eq!(
            detect_server_label(
                status,
                &[0, 0, 0, 0, 0, 0, 0xe7, 0x25, 0x35, 0x01],
                &status_summary
            )
            .as_deref(),
            Some("post-login.quote-0547-status-reply")
        );

        let quote_054c = Server16Frame {
            offset: 0,
            op: 0x441c,
            sub: 0x2b02,
            flags: 0,
            tag: 0x054c,
            compressed_len: 7093,
            original_len: 8101,
        };
        let quote_054c_summary = Server16FrameSummary {
            index: 0,
            offset: 0,
            label: None,
            op: quote_054c.op,
            sub: quote_054c.sub,
            flags: quote_054c.flags,
            tag: quote_054c.tag,
            compressed_len: quote_054c.compressed_len,
            original_len: quote_054c.original_len,
            zlib_ok: true,
            inflated_len: Some(8101),
            inflated_prefix: None,
            body_head_hex: String::new(),
            block8: None,
            utf16_strings: Vec::new(),
            ascii_strings: Vec::new(),
            code_table: None,
        };
        assert_eq!(
            detect_server_label(quote_054c, &[], &quote_054c_summary).as_deref(),
            Some("post-login.quote-054c-list-reply.2b02")
        );
    }

    #[test]
    fn summarizes_repeated_blocks() {
        let bytes = b"ABCDEFGHABCDEFGH12345678";
        let summary = summarize_blocks(bytes, 8).expect("block summary");
        assert_eq!(summary.total_blocks, 3);
        assert_eq!(summary.unique_blocks, 2);
        assert_eq!(summary.most_common_hex, "41 42 43 44 45 46 47 48");
        assert_eq!(summary.most_common_count, 2);
        assert_eq!(summary.most_common_positions, vec![0, 1]);
        assert_eq!(summary.top_patterns[0].count, 2);
        assert_eq!(summary.top_patterns[0].first_positions, vec![0, 1]);
    }

    #[test]
    fn summarizes_blocks_after_client_tag() {
        let frame = Client10Frame {
            offset: 0,
            op: 0x010c,
            sub: 0x7b00,
            flags: 0x0100,
            payload_len: 26,
        };
        let bytes = [
            0x0c, 0x01, 0x00, 0x7b, 0x00, 0x01, 0x1a, 0x00, 0x1a, 0x00, 0x0b, 0x00, b'A', b'B',
            b'C', b'D', b'E', b'F', b'G', b'H', b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H',
            b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8',
        ];
        let summary = summarize_client_frame(0, frame, &bytes);
        let aligned = summary
            .body_after_tag_block8
            .as_ref()
            .expect("aligned block summary");
        assert_eq!(aligned.total_blocks, 3);
        assert_eq!(aligned.unique_blocks, 2);
        assert_eq!(aligned.most_common_hex, "41 42 43 44 45 46 47 48");
        assert_eq!(aligned.most_common_count, 2);
        assert_eq!(aligned.most_common_positions, vec![0, 1]);
        assert_eq!(summary.inner_tag, Some(0x000b));
    }
}
