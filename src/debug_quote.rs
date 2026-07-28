use std::error::Error;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::debug_stream::parse_hex_like;
use crate::{Packet, parse_answer_buffer, to_wide_null};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum ProtoProbeEncoding {
    Utf8,
    Utf16Le,
    Hex,
}

pub type ProbeEncoding = ProtoProbeEncoding;

#[derive(Clone, Debug)]
pub struct ProtoProbeConfig {
    pub host: String,
    pub port: u16,
    pub payload: Option<String>,
    pub encoding: ProtoProbeEncoding,
    pub nul_terminate: bool,
    pub prefix_u32le: bool,
    pub read_secs: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProtoProbeResult {
    pub host: String,
    pub port: u16,
    pub target: String,
    pub payload_len: Option<usize>,
    pub sent_len: usize,
    pub request_bytes: Option<usize>,
    pub request_hex: Option<String>,
    pub request_hex_head: Option<String>,
    pub response_len: usize,
    pub response_hex_head: String,
    pub response_utf8_preview: Option<String>,
    pub response_utf16le_preview: Option<String>,
    pub response_packet_kind: Option<String>,
    pub response_packet_summary: Option<String>,
    pub reply_bytes: usize,
    pub reply_head_hex: String,
    pub parsed_kind: Option<String>,
    pub parsed_summary: Option<String>,
    pub transport_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct QuoteReplaySegment {
    pub offset: usize,
    pub len: usize,
    pub delay_ms: u64,
}

#[derive(Clone, Debug)]
pub struct QuoteReplayConfig {
    pub input: PathBuf,
    pub host: String,
    pub port: u16,
    pub segments: Vec<QuoteReplaySegment>,
    pub recv_ms: u64,
    pub connect_timeout_ms: u64,
    pub save_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize)]
pub struct QuoteReplayResult {
    pub input: String,
    pub host: String,
    pub port: u16,
    pub target: String,
    pub payload_size: usize,
    pub payload_len: usize,
    pub segment_count: usize,
    pub segments: Vec<QuoteReplaySegment>,
    pub sent_bytes: usize,
    pub response_len: usize,
    pub response_hex_head: String,
    pub response_utf8_preview: Option<String>,
    pub response_utf16le_preview: Option<String>,
    pub response_packet_kind: Option<String>,
    pub response_packet_summary: Option<String>,
    pub reply_bytes: usize,
    pub reply_head_hex: String,
    pub save_reply_path: Option<String>,
    pub saved_path: Option<String>,
    pub transport_error: Option<String>,
}

pub fn parse_probe_encoding(value: &str) -> Result<ProtoProbeEncoding, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "utf8" | "utf-8" => Ok(ProtoProbeEncoding::Utf8),
        "utf16le" | "utf-16le" | "utf16" => Ok(ProtoProbeEncoding::Utf16Le),
        "hex" | "binhex" => Ok(ProtoProbeEncoding::Hex),
        _ => Err(format!("unknown encoding: {value}")),
    }
}

pub fn parse_quote_segments(spec: &str) -> Result<Vec<QuoteReplaySegment>, Box<dyn Error>> {
    let mut out = Vec::new();
    for item in spec
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let parts = item.split(':').collect::<Vec<_>>();
        if parts.len() != 3 {
            return Err(format!("invalid segment spec: {item}").into());
        }
        out.push(QuoteReplaySegment {
            offset: parts[0].parse()?,
            len: parts[1].parse()?,
            delay_ms: parts[2].parse()?,
        });
    }
    Ok(out)
}

pub fn probe_proto(config: &ProtoProbeConfig) -> Result<ProtoProbeResult, Box<dyn Error>> {
    let timeout = Duration::from_secs(config.read_secs);
    let addr = resolve_addr(&config.host, config.port)?;
    let mut stream = TcpStream::connect_timeout(&addr, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;

    let mut transport_error = None;
    let request = if let Some(payload) = &config.payload {
        let request = build_probe_request(payload, config)?;
        if let Err(err) = stream.write_all(&request) {
            transport_error = Some(err.to_string());
            Vec::new()
        } else {
            let _ = stream.flush();
            request
        }
    } else {
        Vec::new()
    };

    let reply = read_reply(&mut stream, timeout, &mut transport_error)?;
    let _ = stream.shutdown(Shutdown::Both);

    Ok(ProtoProbeResult {
        host: config.host.clone(),
        port: config.port,
        target: format!("{}:{}", config.host, config.port),
        payload_len: (!request.is_empty()).then_some(request.len()),
        sent_len: request.len(),
        request_bytes: (!request.is_empty()).then_some(request.len()),
        request_hex: (!request.is_empty()).then(|| hex_dump_prefix(&request, 128)),
        request_hex_head: (!request.is_empty()).then(|| hex_dump_prefix(&request, 64)),
        response_len: reply.len(),
        response_hex_head: hex_dump_prefix(&reply, 128),
        response_utf8_preview: preview_utf8(&reply),
        response_utf16le_preview: preview_utf16le(&reply),
        response_packet_kind: unsafe { parse_answer_buffer(&reply) }.map(packet_kind_name),
        response_packet_summary: unsafe { parse_answer_buffer(&reply) }
            .map(|packet| packet.summary()),
        reply_bytes: reply.len(),
        reply_head_hex: hex_dump_prefix(&reply, 128),
        parsed_kind: unsafe { parse_answer_buffer(&reply) }.map(packet_kind_name),
        parsed_summary: unsafe { parse_answer_buffer(&reply) }.map(|packet| packet.summary()),
        transport_error,
    })
}

pub fn replay_quote_file(config: &QuoteReplayConfig) -> Result<QuoteReplayResult, Box<dyn Error>> {
    let payload = fs::read(&config.input)?;
    let segments = if config.segments.is_empty() {
        vec![QuoteReplaySegment {
            offset: 0,
            len: payload.len(),
            delay_ms: 0,
        }]
    } else {
        config.segments.clone()
    };

    for segment in &segments {
        if segment.offset + segment.len > payload.len() {
            return Err(format!(
                "segment out of range: offset={} len={} payload={}",
                segment.offset,
                segment.len,
                payload.len()
            )
            .into());
        }
    }

    let recv_timeout = Duration::from_millis(config.recv_ms);
    let connect_timeout = Duration::from_millis(config.connect_timeout_ms);
    let addr = resolve_addr(&config.host, config.port)?;
    let mut stream = TcpStream::connect_timeout(&addr, connect_timeout)?;
    stream.set_read_timeout(Some(recv_timeout))?;
    stream.set_write_timeout(Some(connect_timeout))?;

    let mut reply = Vec::new();
    let mut transport_error = None;
    let mut sent_bytes = 0usize;
    for segment in &segments {
        if segment.delay_ms > 0 {
            thread::sleep(Duration::from_millis(segment.delay_ms));
        }

        let part = &payload[segment.offset..segment.offset + segment.len];
        if let Err(err) = stream.write_all(part) {
            transport_error = Some(err.to_string());
            break;
        }
        sent_bytes += part.len();

        let part_reply = read_reply(&mut stream, recv_timeout, &mut transport_error)?;
        reply.extend_from_slice(&part_reply);
        if transport_error.is_some() {
            break;
        }
    }

    let _ = stream.shutdown(Shutdown::Both);

    let saved_path = if let Some(path) = &config.save_path {
        fs::write(path, &reply)?;
        Some(path.display().to_string())
    } else {
        None
    };

    Ok(QuoteReplayResult {
        input: config.input.display().to_string(),
        host: config.host.clone(),
        port: config.port,
        target: format!("{}:{}", config.host, config.port),
        payload_size: payload.len(),
        payload_len: payload.len(),
        segment_count: segments.len(),
        segments,
        sent_bytes,
        response_len: reply.len(),
        response_hex_head: hex_dump_prefix(&reply, 128),
        response_utf8_preview: preview_utf8(&reply),
        response_utf16le_preview: preview_utf16le(&reply),
        response_packet_kind: unsafe { parse_answer_buffer(&reply) }.map(packet_kind_name),
        response_packet_summary: unsafe { parse_answer_buffer(&reply) }
            .map(|packet| packet.summary()),
        reply_bytes: reply.len(),
        reply_head_hex: hex_dump_prefix(&reply, 128),
        save_reply_path: saved_path.clone(),
        saved_path,
        transport_error,
    })
}

fn resolve_addr(host: &str, port: u16) -> Result<std::net::SocketAddr, Box<dyn Error>> {
    let addr = format!("{host}:{port}");
    addr.to_socket_addrs()?
        .next()
        .ok_or_else(|| "failed to resolve target address".into())
}

fn build_probe_request(
    payload: &str,
    config: &ProtoProbeConfig,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut body = match config.encoding {
        ProtoProbeEncoding::Utf8 => {
            let mut bytes = payload.as_bytes().to_vec();
            if config.nul_terminate {
                bytes.push(0);
            }
            bytes
        }
        ProtoProbeEncoding::Utf16Le => {
            let words = if config.nul_terminate {
                to_wide_null(payload)
            } else {
                payload.encode_utf16().collect::<Vec<u16>>()
            };
            let mut bytes = Vec::with_capacity(words.len() * 2);
            for word in words {
                bytes.extend_from_slice(&word.to_le_bytes());
            }
            bytes
        }
        ProtoProbeEncoding::Hex => parse_hex_like(payload)?,
    };

    if config.prefix_u32le {
        let len = u32::try_from(body.len())?;
        let mut out = Vec::with_capacity(body.len() + 4);
        out.extend_from_slice(&len.to_le_bytes());
        out.append(&mut body);
        Ok(out)
    } else {
        Ok(body)
    }
}

fn read_reply(
    stream: &mut TcpStream,
    timeout: Duration,
    transport_error: &mut Option<String>,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut reply = Vec::new();
    let mut buf = [0u8; 65_536];
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => reply.extend_from_slice(&buf[..n]),
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) =>
            {
                break;
            }
            Err(err) => {
                *transport_error = Some(err.to_string());
                break;
            }
        }
    }

    Ok(reply)
}

fn packet_kind_name(packet: Packet) -> String {
    match packet {
        Packet::InvalidRequest { .. } => "InvalidRequest",
        Packet::CodeTable { .. } => "CodeTable",
        Packet::Realtime { .. } => "Realtime",
        Packet::Tick { .. } => "Tick",
        Packet::Trend { .. } => "Trend",
        Packet::Kline { .. } => "Kline",
        Packet::Split { .. } => "Split",
        Packet::Finance { .. } => "Finance",
        Packet::F10 { .. } => "F10",
        Packet::BuySell610 { .. } => "BuySell610",
        Packet::Unknown { .. } => "Unknown",
    }
    .to_string()
}

fn preview_utf8(bytes: &[u8]) -> Option<String> {
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

fn preview_utf16le(bytes: &[u8]) -> Option<String> {
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
        None
    } else {
        let text = String::from_utf16_lossy(&words);
        if text.trim().is_empty() {
            None
        } else {
            Some(text)
        }
    }
}

fn hex_dump_prefix(bytes: &[u8], limit: usize) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for (row, chunk) in bytes[..bytes.len().min(limit)].chunks(16).enumerate() {
        let offset = row * 16;
        out.push_str(&format!("{offset:08x}  "));
        for i in 0..16 {
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
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::{ProtoProbeEncoding, parse_probe_encoding, parse_quote_segments};

    #[test]
    fn parses_segment_spec() {
        let segments = parse_quote_segments("0:582:0,582:260:420").expect("segments");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[1].offset, 582);
        assert_eq!(segments[1].len, 260);
        assert_eq!(segments[1].delay_ms, 420);
    }

    #[test]
    fn parses_probe_encoding_aliases() {
        assert_eq!(parse_probe_encoding("utf8"), Ok(ProtoProbeEncoding::Utf8));
        assert_eq!(
            parse_probe_encoding("utf-16le"),
            Ok(ProtoProbeEncoding::Utf16Le)
        );
        assert_eq!(parse_probe_encoding("hex"), Ok(ProtoProbeEncoding::Hex));
        assert!(parse_probe_encoding("bad").is_err());
    }
}
