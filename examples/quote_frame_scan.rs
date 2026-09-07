use std::env;
use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

use encoding_rs::GBK;
use flate2::read::ZlibDecoder;

#[derive(Debug)]
struct Config {
    input: PathBuf,
}

#[derive(Clone, Copy, Debug)]
struct Client10Frame {
    offset: usize,
    op: u16,
    sub: u16,
    flags: u16,
    payload_len: usize,
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

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;
    let bytes = fs::read(&config.input)?;

    println!("input: {}", config.input.display());
    println!("size: {}", bytes.len());

    if let Some(frames) = parse_server16_frames(&bytes) {
        println!("mode: server16");
        for (index, frame) in frames.iter().enumerate() {
            describe_server_frame(index, *frame, &bytes);
        }
        return Ok(());
    }

    if let Some(frames) = parse_client10_frames(&bytes) {
        println!("mode: client10");
        for (index, frame) in frames.iter().enumerate() {
            describe_client10_frame(index, *frame, &bytes);
        }
        return Ok(());
    }

    println!("mode: unknown");
    println!("{}", hex_dump(&bytes[..bytes.len().min(128)]));
    Ok(())
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let Some(first) = args.next() else {
        return Err("missing input file path".into());
    };
    if matches!(first.as_str(), "--help" | "-h") {
        print_help();
        std::process::exit(0);
    }
    if let Some(other) = args.next() {
        return Err(format!("unknown arg: {other}").into());
    }
    Ok(Config {
        input: PathBuf::from(first),
    })
}

fn print_help() {
    println!("quote_frame_scan <raw-flow.bin>");
    println!("  auto-detects:");
    println!("  - server-side 16-byte quote frames with b1 cb 74 00");
    println!("  - client-side 10-byte quote frames (used by 7719 code-list requests)");
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

fn describe_server_frame(index: usize, frame: Server16Frame, bytes: &[u8]) {
    let body_start = frame.offset + 16;
    let body_end = body_start + frame.compressed_len;
    let body = &bytes[body_start..body_end];

    println!(
        "frame[{index}] @{} op=0x{:04x} sub=0x{:04x} flags=0x{:04x} tag=0x{:04x} body={} original={}",
        frame.offset,
        frame.op,
        frame.sub,
        frame.flags,
        frame.tag,
        frame.compressed_len,
        frame.original_len
    );

    if body.starts_with(&[0x78, 0x01])
        || body.starts_with(&[0x78, 0x5e])
        || body.starts_with(&[0x78, 0x9c])
        || body.starts_with(&[0x78, 0xda])
    {
        match zlib_decode(body) {
            Some(inflated) => {
                println!(
                    "  zlib: ok inflated={} ratio={:.3}",
                    inflated.len(),
                    inflated.len() as f64 / body.len() as f64
                );
                describe_inflated_body(&inflated);
            }
            None => {
                println!("  zlib: decode-failed");
                println!("  body-head: {}", hex_line(&body[..body.len().min(32)]));
            }
        }
    } else {
        println!("  body-head: {}", hex_line(&body[..body.len().min(32)]));
    }
}

fn describe_client10_frame(index: usize, frame: Client10Frame, bytes: &[u8]) {
    let body_start = frame.offset + 10;
    let body_end = body_start + frame.payload_len;
    let body = &bytes[body_start..body_end];

    println!(
        "frame[{index}] @{} op=0x{:04x} sub=0x{:04x} flags=0x{:04x} payload={}",
        frame.offset, frame.op, frame.sub, frame.flags, frame.payload_len
    );
    if body.len() >= 10 {
        println!(
            "  inner-head: tag=0x{:04x} kind=0x{:04x} raw={}",
            le_u16(&body[0..2]),
            le_u16(&body[2..4]),
            hex_line(&body[..body.len().min(16)])
        );
    } else {
        println!("  body-head: {}", hex_line(body));
    }

    if let Some(request) = parse_code_table_request(body) {
        println!(
            "  code-table-chunk: bucket={} offset={} count-step=1000",
            request.bucket, request.offset
        );
    }

    let ascii = extract_ascii_strings(body, 6);
    if !ascii.is_empty() {
        println!(
            "  ascii-strings: {}",
            ascii.iter().take(4).cloned().collect::<Vec<_>>().join(", ")
        );
    }

    let tokens = extract_numeric_tokens(body);
    if !tokens.is_empty() {
        let first = tokens
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let last = tokens
            .iter()
            .rev()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(", ");
        println!("  numeric-tokens: {}", tokens.len());
        println!("  first: {first}");
        println!("  last:  {last}");
    }
}

fn describe_inflated_body(bytes: &[u8]) {
    println!(
        "  inflated-head: {}",
        hex_line(&bytes[..bytes.len().min(32)])
    );

    if bytes.len() >= 5 {
        let status = le_u16(&bytes[0..2]);
        let count = le_u16(&bytes[2..4]);
        let tail = bytes[4];
        println!("  inflated-prefix: status={status} count={count} tail={tail}");
    }

    if let Some(table) = parse_code_table_records(bytes) {
        println!(
            "  code-table29: count={} first={} {} last={} {}",
            table.count, table.first.code, table.first.name, table.last.code, table.last.name
        );
        println!("  code-table29-records: code[7]+nul + gbk-name[8] + meta[13]");
        return;
    }

    let ascii = extract_ascii_strings(bytes, 6);
    if !ascii.is_empty() {
        println!(
            "  ascii-strings: {}",
            ascii.iter().take(4).cloned().collect::<Vec<_>>().join(", ")
        );
    }

    let positions = six_digit_positions(bytes);
    if !positions.is_empty() {
        let codes = positions
            .iter()
            .map(|(_, code)| code.as_str())
            .collect::<Vec<_>>();
        let first = codes.iter().take(5).copied().collect::<Vec<_>>().join(", ");
        let last = codes
            .iter()
            .rev()
            .take(5)
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(", ");

        let mut lens = Vec::new();
        for pair in positions.windows(2) {
            lens.push(pair[1].0 - pair[0].0);
        }
        if let Some((last_pos, _)) = positions.last() {
            lens.push(bytes.len() - *last_pos);
        }
        let min_len = lens.iter().copied().min().unwrap_or(0);
        let max_len = lens.iter().copied().max().unwrap_or(0);
        let avg_len = lens.iter().sum::<usize>() as f64 / lens.len() as f64;

        println!("  six-digit-codes: {}", codes.len());
        println!("  first: {first}");
        println!("  last:  {last}");
        println!("  record-len: min={min_len} max={max_len} avg={avg_len:.1}");
    }
}

#[derive(Debug)]
struct CodeTableRequest {
    bucket: u16,
    offset: u16,
}

#[derive(Debug)]
struct CodeTableRow {
    code: String,
    name: String,
}

#[derive(Debug)]
struct CodeTableSummary {
    count: usize,
    first: CodeTableRow,
    last: CodeTableRow,
}

fn parse_code_table_request(bytes: &[u8]) -> Option<CodeTableRequest> {
    if bytes.len() != 6 || le_u16(&bytes[0..2]) != 0x0450 {
        return None;
    }
    Some(CodeTableRequest {
        bucket: le_u16(&bytes[2..4]),
        offset: le_u16(&bytes[4..6]),
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

fn parse_code_table_row(bytes: &[u8]) -> Option<CodeTableRow> {
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

    Some(CodeTableRow {
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
                    if let Ok(text) = std::str::from_utf8(&token)
                        && text.chars().all(|ch| ch.is_ascii_digit())
                    {
                        out.push(text.to_string());
                    }
                    token.clear();
                }
            }
            b if b.is_ascii_digit() => token.push(b),
            _ => {
                if !token.is_empty() {
                    if let Ok(text) = std::str::from_utf8(&token)
                        && text.chars().all(|ch| ch.is_ascii_digit())
                    {
                        out.push(text.to_string());
                    }
                    token.clear();
                }
            }
        }
    }
    if !token.is_empty()
        && let Ok(text) = std::str::from_utf8(&token)
        && text.chars().all(|ch| ch.is_ascii_digit())
    {
        out.push(text.to_string());
    }
    out
}

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

fn six_digit_positions(bytes: &[u8]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for index in 0..bytes.len().saturating_sub(5) {
        let chunk = &bytes[index..index + 6];
        if !chunk.iter().all(u8::is_ascii_digit) {
            continue;
        }
        if index > 0 && bytes[index - 1].is_ascii_digit() {
            continue;
        }
        out.push((index, String::from_utf8_lossy(chunk).to_string()));
    }
    out
}

fn zlib_decode(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

fn le_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn hex_line(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn hex_dump(bytes: &[u8]) -> String {
    let mut out = String::new();
    for (row, chunk) in bytes.chunks(16).enumerate() {
        out.push_str(&format!("{:08x}: {}\n", row * 16, hex_line(chunk)));
    }
    out
}
