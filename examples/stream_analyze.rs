use std::env;
use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

use flate2::read::{GzDecoder, ZlibDecoder};
use netzipapi_rust_demo::{Packet, parse_answer_buffer};
use zstd::stream::decode_all as zstd_decode_all;

const OEM_HEAD_LEN: usize = 200;
const NET_PACKET_PREFIX_LEN: usize = 74;
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const MAX_UTF16_STRINGS: usize = 24;
const MAX_ASCII_STRINGS: usize = 24;

#[derive(Debug)]
struct Config {
    input: PathBuf,
    is_hex: bool,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;
    let bytes = if config.is_hex {
        parse_hex_like(&fs::read_to_string(&config.input)?)?
    } else {
        fs::read(&config.input)?
    };

    println!("input: {}", config.input.display());
    println!("size: {} bytes", bytes.len());
    println!("{}", hex_dump(&bytes[..bytes.len().min(128)], 16));
    print_byte_profile(&bytes);

    if let Some(text) = try_utf8_preview(&bytes) {
        println!("utf8-preview: {text}");
    }
    if let Some(text) = try_utf16le_preview(&bytes) {
        println!("utf16le-preview: {text}");
    }
    if let Some(http) = detect_http_headers(&bytes) {
        println!("http-headers:\n{http}");
    }
    scan_netpacket_prefixes(&bytes);
    analyze_packet_slices(&bytes);
    if let Some(offset) = find_gzip(&bytes) {
        println!("gzip-offset: {offset}");
        if let Some(inflated) = try_gzip(&bytes[offset..]) {
            println!("gzip-inflated-size: {}", inflated.len());
            println!("{}", hex_dump(&inflated[..inflated.len().min(128)], 16));
            if let Some(packet) = try_parse_packet(&inflated) {
                println!("inflated-oem: {}", packet.summary());
            }
        }
    }
    scan_zlib_streams(&bytes);

    if let Some(packet) = try_parse_packet(&bytes) {
        println!("oem@0: {}", packet.summary());
    } else {
        println!("oem@0: no packet");
    }

    if bytes.len() > 4 {
        if let Some(packet) = try_parse_packet(&bytes[4..]) {
            println!("oem@4: {}", packet.summary());
        } else {
            println!("oem@4: no packet");
        }
    }

    scan_candidates(&bytes);
    Ok(())
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let mut input = None;
    let mut is_hex = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--hex" => is_hex = true,
            "--bin" => is_hex = false,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            value if input.is_none() => input = Some(PathBuf::from(value)),
            other => return Err(format!("unknown arg: {other}").into()),
        }
    }

    let input = input.ok_or("missing input file path")?;
    Ok(Config { input, is_hex })
}

fn print_help() {
    println!("stream_analyze <file> [--hex|--bin]");
    println!("  --hex    parse Wireshark/xxd/hexdump style text");
    println!("  --bin    parse raw binary stream (default)");
}

fn parse_hex_like(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut compact = String::new();
    for line in input.lines() {
        let mut part = line;
        if let Some((left, _)) = line.split_once("  ") {
            if left
                .chars()
                .all(|ch| ch.is_ascii_hexdigit() || ch.is_ascii_whitespace())
            {
                part = left;
            }
        }
        for token in part.split_whitespace() {
            if token.ends_with(':') {
                continue;
            }
            if token.len() == 8 && token.chars().all(|ch| ch.is_ascii_hexdigit()) {
                continue;
            }
            if token.len() == 2 && token.chars().all(|ch| ch.is_ascii_hexdigit()) {
                compact.push_str(token);
            }
        }
    }

    if compact.is_empty() {
        for ch in input.chars() {
            if ch.is_ascii_hexdigit() {
                compact.push(ch);
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

fn print_byte_profile(bytes: &[u8]) {
    if bytes.is_empty() {
        return;
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

    println!(
        "byte-profile: entropy={entropy:.3} ascii-ish={:.1}% zeroes={:.1}%",
        ascii_like as f64 * 100.0 / len,
        zeroes as f64 * 100.0 / len
    );
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

fn find_gzip(bytes: &[u8]) -> Option<usize> {
    bytes.windows(2).position(|w| w == [0x1f, 0x8b])
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

fn try_gzip(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = GzDecoder::new(bytes);
    let mut out = Vec::new();
    std::io::Read::read_to_end(&mut decoder, &mut out).ok()?;
    Some(out)
}

fn try_zlib(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(bytes);
    let mut out = Vec::new();
    std::io::Read::read_to_end(&mut decoder, &mut out).ok()?;
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

fn scan_candidates(bytes: &[u8]) {
    if bytes.len() < OEM_HEAD_LEN {
        return;
    }

    let mut hits = 0usize;
    for offset in 0..=bytes.len() - OEM_HEAD_LEN {
        let slice = &bytes[offset..];
        if let Some(packet) = try_parse_packet(slice) {
            if is_useful_packet(&packet) {
                println!("candidate-oem@{offset}: {}", packet.summary());
                hits += 1;
                if hits >= 8 {
                    break;
                }
            }
        }
    }

    if hits == 0 {
        println!("candidate-oem: none");
    }
}

fn scan_zlib_streams(bytes: &[u8]) {
    let offsets = find_zlib_offsets(bytes);
    if offsets.is_empty() {
        return;
    }

    println!("zlib-offsets: {}", offsets.len());
    for offset in offsets.into_iter().take(8) {
        let Some(inflated) = try_zlib(&bytes[offset..]) else {
            continue;
        };

        println!("zlib@{offset}: decompressed={}", inflated.len());
        println!(
            "{}",
            hex_dump(&bytes[offset..(offset + 32).min(bytes.len())], 16)
        );
        println!(
            "zlib@{offset}-inflated-head:\n{}",
            hex_dump(&inflated[..inflated.len().min(128)], 16)
        );

        if let Some(text) = try_utf8_preview(&inflated) {
            println!("zlib@{offset}-utf8-preview: {text}");
        }
        if let Some(text) = try_utf16le_preview(&inflated) {
            println!("zlib@{offset}-utf16-preview: {text}");
        }

        let ascii_strings = scan_ascii_strings(&inflated, 4);
        if !ascii_strings.is_empty() {
            println!("zlib@{offset}-ascii-strings:");
            for (inner, text) in ascii_strings.into_iter().take(MAX_ASCII_STRINGS) {
                println!("  {inner:04x}: {text}");
            }
        }

        let utf16_strings = scan_utf16_strings(&inflated, 2);
        if !utf16_strings.is_empty() {
            println!("zlib@{offset}-utf16-strings:");
            for (inner, text) in utf16_strings.into_iter().take(MAX_UTF16_STRINGS) {
                println!("  {inner:04x}: {text}");
            }
        }
    }
}

fn is_useful_packet(packet: &Packet) -> bool {
    match packet {
        Packet::Unknown { head } => is_known_type(&head.packet_type),
        _ => true,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NetPacketPrefix {
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

fn scan_netpacket_prefixes(bytes: &[u8]) {
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

    if records.is_empty() {
        return;
    }

    println!(
        "netpacket-prefix-records: {} ({} bytes)",
        records.len(),
        records.len() * NET_PACKET_PREFIX_LEN
    );

    let mut run_start = 0usize;
    while run_start < records.len() {
        let mut run_end = run_start + 1;
        while run_end < records.len() && records[run_end] == records[run_start] {
            run_end += 1;
        }
        println!(
            "netpacket-run@{} count={} {}",
            run_start * NET_PACKET_PREFIX_LEN,
            run_end - run_start,
            records[run_start].summary()
        );
        run_start = run_end;
    }

    if offset < bytes.len() {
        let remain = &bytes[offset..];
        println!(
            "netpacket-remainder: {} bytes after {} prefix records",
            remain.len(),
            records.len()
        );
        println!("{}", hex_dump(&remain[..remain.len().min(128)], 16));
    }
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

fn analyze_packet_slices(bytes: &[u8]) {
    let packets = collect_netpacket_packets(bytes);
    if packets.is_empty() {
        return;
    }

    println!("netpacket-packets: {}", packets.len());
    for packet in packets.iter().take(8) {
        println!(
            "packet@{} len={} {}",
            packet.offset,
            packet.prefix.field_32,
            packet.prefix.summary()
        );
    }

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

    println!("packet-unique: {}", unique_packets.len());

    for (index, (packet, slice)) in unique_packets.iter().take(4).enumerate() {
        analyze_zstd_payload(
            &format!("packet{index}@{}", packet.offset),
            slice,
            &packet.prefix,
        );
    }
}

fn analyze_zstd_payload(label: &str, bytes: &[u8], prefix: &NetPacketPrefix) {
    let Some(start) = find_zstd(bytes) else {
        maybe_print_raw_strings(label, prefix, bytes);
        return;
    };
    let Some((frame_len, inflated)) = extract_zstd_frame(&bytes[start..]) else {
        println!("{label}-zstd: found magic at {start}, but decode failed");
        let end = (start + 32).min(bytes.len());
        println!("{label}-zstd-head:\n{}", hex_dump(&bytes[start..end], 16));
        maybe_print_raw_strings(label, prefix, bytes);
        return;
    };

    println!(
        "{label}-zstd: offset={} compressed={} decompressed={}",
        start,
        frame_len,
        inflated.len()
    );
    println!(
        "{label}-inflated-head:\n{}",
        hex_dump(&inflated[..inflated.len().min(128)], 16)
    );

    if let Some(prefix) = parse_object_prefix(&inflated) {
        println!("{label}-inflated-prefix: {}", prefix.summary());
    }

    let strings = scan_utf16_strings(&inflated, 2);
    if !strings.is_empty() {
        println!("{label}-utf16-strings:");
        for (offset, text) in strings.into_iter().take(MAX_UTF16_STRINGS) {
            println!("  {offset:04x}: {text}");
        }
    }
}

#[derive(Debug, Clone)]
struct NetPacketSlice {
    offset: usize,
    prefix: NetPacketPrefix,
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

fn maybe_print_raw_strings(label: &str, prefix: &NetPacketPrefix, bytes: &[u8]) {
    let should_print = prefix.tail.contains("下载文件")
        || prefix.tail.contains("ZSTD")
        || find_ascii(bytes, b"penc").is_some()
        || find_ascii(bytes, b"Tdx_Encrypt").is_some();

    if !should_print {
        return;
    }

    let utf16_strings = scan_utf16_strings(bytes, 2);
    if !utf16_strings.is_empty() {
        println!("{label}-raw-utf16-strings:");
        for (offset, text) in utf16_strings.into_iter().take(MAX_UTF16_STRINGS) {
            println!("  {offset:04x}: {text}");
        }
    }

    let ascii_strings = scan_ascii_strings(bytes, 4);
    if !ascii_strings.is_empty() {
        println!("{label}-raw-ascii-strings:");
        for (offset, text) in ascii_strings.into_iter().take(MAX_ASCII_STRINGS) {
            println!("  {offset:04x}: {text}");
        }
    }
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

fn find_ascii(bytes: &[u8], needle: &[u8]) -> Option<usize> {
    bytes
        .windows(needle.len())
        .position(|window| window == needle)
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

fn hex_dump(bytes: &[u8], width: usize) -> String {
    let mut out = String::new();
    for (row, chunk) in bytes.chunks(width).enumerate() {
        let offset = row * width;
        out.push_str(&format!("{offset:08x}  "));
        for i in 0..width {
            if let Some(byte) = chunk.get(i) {
                out.push_str(&format!("{byte:02x} "));
            } else {
                out.push_str("   ");
            }
        }
        out.push(' ');
        for byte in chunk {
            let ch = if byte.is_ascii_graphic() || *byte == b' ' {
                *byte as char
            } else {
                '.'
            };
            out.push(ch);
        }
        out.push('\n');
    }
    out
}
