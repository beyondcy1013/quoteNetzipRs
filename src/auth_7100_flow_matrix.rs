use crate::auth_flow_sample::Auth7100ZstdFrameSummary;
use crate::capture_input::read_capture_file_as_pcap_bytes;
use crate::{Auth7100PrefixHints, summarize_auth_7100_prefix_hints};
use encoding_rs::GBK;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::io::{Cursor, Read};
use std::net::Ipv4Addr;
use std::path::Path;
use zstd::stream::decode_all as zstd_decode_all;

const NET_PACKET_PREFIX_LEN: usize = 74;
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const SAMPLE_PCAP_REL: &str = "tmp/netzip_full_tcp.pcap";
const STOCK_DICTIONARY_BYTES: &[u8] =
    include_bytes!("../docs/netzip_api_bin/NetzipAPI/StockC++/Stock.字典");

#[derive(Clone, Debug, Serialize)]
pub struct Auth7100FlowMatrix {
    pub pcap_path: String,
    pub server_endpoint: String,
    pub sessions: Vec<Auth7100FlowSession>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Auth7100FlowSession {
    pub local_endpoint: String,
    pub server_endpoint: String,
    pub started_with_handshake: bool,
    pub client_unique_packets: usize,
    pub server_unique_packets: usize,
    pub client_tcp_payload_bytes: usize,
    pub server_tcp_payload_bytes: usize,
    pub session_role: String,
    pub client_packets: Vec<Auth7100FlowPacket>,
    pub server_packets: Vec<Auth7100FlowPacket>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Auth7100FlowPacket {
    pub packet_offset: usize,
    pub packet_len: usize,
    pub outer_object_name: String,
    pub outer_tail: String,
    pub field_20: u32,
    pub field_44: u32,
    pub field_52: u32,
    pub field_56: u32,
    pub field_hints: Auth7100PrefixHints,
    pub packet_role: String,
    pub inflated_root_name: Option<String>,
    pub inflated_header_words: Option<[u32; 5]>,
    pub inflated_object_name: Option<String>,
    pub inflated_tail: Option<String>,
    pub inflated_field_hints: Option<Auth7100PrefixHints>,
    pub inflated_len: Option<usize>,
    pub inflated_fields: Vec<Auth7100DecodedField>,
    pub contains_penc_ascii: bool,
    pub penc_offsets: Vec<usize>,
    pub hypenc_offsets: Vec<usize>,
    pub utf16_strings: Vec<String>,
    pub ascii_strings: Vec<String>,
    pub zstd_frame: Option<Auth7100ZstdFrameSummary>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Auth7100DecodedField {
    pub offset: usize,
    pub type_id: u32,
    pub value_len: usize,
    pub value_crc32: Option<u32>,
    pub field_12: u32,
    pub span_len: usize,
    pub label: String,
    pub sensitive: bool,
    pub value_first_nul: Option<usize>,
    pub value_visible_ascii_bytes: usize,
    pub value_visible_ascii_runs: Vec<[usize; 2]>,
    pub value_gbk_decode_ok: bool,
    pub value_gbk_character_count: Option<usize>,
    pub value_crlf_count: usize,
    pub value_non_ascii_bytes: usize,
    pub value_other_control_bytes: usize,
    pub value_line_lengths: Vec<usize>,
    pub value_line_shapes: Vec<String>,
    pub value_schema_labels: Vec<String>,
    pub value_penc_offsets: Vec<usize>,
    pub value_hypenc_offsets: Vec<usize>,
    pub value_zstd_offsets: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Endpoint {
    ip: Ipv4Addr,
    port: u16,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PacketIdentity {
    src: Endpoint,
    dst: Endpoint,
    seq: u32,
    ack: u32,
    flags: u8,
    wire_payload_len: usize,
    captured_payload: Vec<u8>,
}

#[derive(Clone, Debug)]
struct TcpPacket {
    src: Endpoint,
    dst: Endpoint,
    seq: u32,
    ack: u32,
    flags: u8,
    wire_payload_len: usize,
    captured_payload: Vec<u8>,
}

#[derive(Default)]
struct SessionPackets {
    client_packets: Vec<TcpPacket>,
    server_packets: Vec<TcpPacket>,
    started_with_handshake: bool,
}

#[derive(Clone, Copy)]
enum Endian {
    Little,
    Big,
}

#[derive(Clone, Copy)]
enum TimestampScale {
    Micros,
    Nanos,
}

#[derive(Clone, Debug)]
struct NetPacketPrefix {
    object_name: String,
    packet_len: usize,
    field_20: u32,
    field_32: u32,
    field_36: u32,
    field_40: u32,
    field_44: u32,
    field_52: u32,
    field_56: u32,
    tail: String,
}

pub fn analyze_auth_7100_flow_matrix_sample() -> Result<Auth7100FlowMatrix, Box<dyn Error>> {
    let path = crate::repository_fixture_path(SAMPLE_PCAP_REL);
    analyze_auth_7100_flow_matrix(path)
}

pub fn analyze_auth_7100_flow_matrix(
    path: impl AsRef<Path>,
) -> Result<Auth7100FlowMatrix, Box<dyn Error>> {
    analyze_auth_flow_matrix_for_port(path, 7100)
}

/// Analyze the shared authenticated envelope on a selected TCP service port.
/// The historical 7100 API remains the compatibility wrapper above.
pub fn analyze_auth_flow_matrix_for_port(
    path: impl AsRef<Path>,
    service_port: u16,
) -> Result<Auth7100FlowMatrix, Box<dyn Error>> {
    let path = path.as_ref();
    let bytes = read_capture_file_as_pcap_bytes(path)?;
    let parsed = parse_pcap(&bytes)?;
    let (local_host, server) = discover_service_endpoints(&parsed, service_port)?;
    let mut seen = HashSet::new();
    let mut sessions = BTreeMap::<Endpoint, SessionPackets>::new();

    for packet in parsed {
        let identity = PacketIdentity {
            src: packet.src,
            dst: packet.dst,
            seq: packet.seq,
            ack: packet.ack,
            flags: packet.flags,
            wire_payload_len: packet.wire_payload_len,
            captured_payload: packet.captured_payload.clone(),
        };
        if !seen.insert(identity) {
            continue;
        }

        if packet.src.ip == local_host && packet.dst == server {
            let entry = sessions.entry(packet.src).or_default();
            if packet.flags & 0x02 != 0 {
                entry.started_with_handshake = true;
            }
            if !packet.captured_payload.is_empty() {
                entry.client_packets.push(packet);
            }
            continue;
        }

        if packet.dst.ip == local_host && packet.src == server {
            let local_endpoint = Endpoint {
                ip: packet.dst.ip,
                port: packet.dst.port,
            };
            let entry = sessions.entry(local_endpoint).or_default();
            if packet.flags & 0x12 == 0x12 {
                entry.started_with_handshake = true;
            }
            if !packet.captured_payload.is_empty() {
                entry.server_packets.push(packet);
            }
        }
    }

    let mut summarized = Vec::new();
    for (local_endpoint, packets) in sessions {
        let client_stream = reassemble_stream(&packets.client_packets);
        let server_stream = reassemble_stream(&packets.server_packets);
        let client_packets = analyze_netpackets(&client_stream);
        let server_packets = analyze_netpackets(&server_stream);
        let session_role = classify_session(&client_packets, &server_packets);

        summarized.push(Auth7100FlowSession {
            local_endpoint: display_ep(local_endpoint),
            server_endpoint: display_ep(server),
            started_with_handshake: packets.started_with_handshake,
            client_unique_packets: packets.client_packets.len(),
            server_unique_packets: packets.server_packets.len(),
            client_tcp_payload_bytes: client_stream.len(),
            server_tcp_payload_bytes: server_stream.len(),
            session_role,
            client_packets,
            server_packets,
        });
    }

    summarized.sort_by(|left, right| left.local_endpoint.cmp(&right.local_endpoint));

    Ok(Auth7100FlowMatrix {
        pcap_path: path.display().to_string(),
        server_endpoint: display_ep(server),
        sessions: summarized,
    })
}

fn discover_service_endpoints(
    packets: &[TcpPacket],
    service_port: u16,
) -> Result<(Ipv4Addr, Endpoint), Box<dyn Error>> {
    let mut candidates = BTreeMap::<(Ipv4Addr, Endpoint), usize>::new();
    for packet in packets {
        let candidate = if packet.src.port == service_port {
            Some((packet.dst.ip, packet.src))
        } else if packet.dst.port == service_port {
            Some((packet.src.ip, packet.dst))
        } else {
            None
        };
        if let Some(candidate) = candidate {
            *candidates.entry(candidate).or_default() += 1;
        }
    }
    candidates
        .into_iter()
        .max_by_key(|(_, packet_count)| *packet_count)
        .map(|(candidate, _)| candidate)
        .ok_or_else(|| format!("capture contains no TCP/{service_port} packets").into())
}

fn classify_session(
    client_packets: &[Auth7100FlowPacket],
    server_packets: &[Auth7100FlowPacket],
) -> String {
    if client_packets
        .iter()
        .chain(server_packets.iter())
        .any(|packet| packet.packet_role == "auth_login")
    {
        return "auth_login".to_string();
    }
    if client_packets
        .iter()
        .chain(server_packets.iter())
        .any(|packet| packet.packet_role == "auth_probe")
    {
        return "auth_probe".to_string();
    }
    "unknown".to_string()
}

fn analyze_netpackets(bytes: &[u8]) -> Vec<Auth7100FlowPacket> {
    let mut packets = Vec::new();
    let mut offset = 0usize;

    while offset + NET_PACKET_PREFIX_LEN <= bytes.len() {
        let Some(prefix) = parse_netpacket_prefix(&bytes[offset..offset + NET_PACKET_PREFIX_LEN])
        else {
            offset += 1;
            continue;
        };
        if prefix.packet_len < NET_PACKET_PREFIX_LEN || offset + prefix.packet_len > bytes.len() {
            offset += 1;
            continue;
        }
        let packet_bytes = &bytes[offset..offset + prefix.packet_len];
        packets.push(analyze_netpacket(offset, packet_bytes, &prefix));
        offset += prefix.packet_len;
    }

    packets
}

fn analyze_netpacket(
    packet_offset: usize,
    packet_bytes: &[u8],
    prefix: &NetPacketPrefix,
) -> Auth7100FlowPacket {
    let raw_utf16_strings = scan_utf16_strings(packet_bytes, 2);
    let raw_ascii_strings = scan_ascii_strings(packet_bytes, 4);
    let uses_stock_dictionary = prefix.field_44 == 12;
    let zstd_frame = find_zstd(packet_bytes).map(|zstd_magic_offset| {
        summarize_zstd_frame(
            &packet_bytes[zstd_magic_offset..],
            zstd_magic_offset,
            uses_stock_dictionary,
        )
    });
    let inflated = zstd_frame.as_ref().and_then(|frame| {
        if !frame.decode_ok {
            return None;
        }
        decode_exact_zstd(
            packet_bytes,
            frame.zstd_magic_offset,
            frame.declared_frame_len,
            uses_stock_dictionary,
        )
    });
    let inflated_prefix = inflated
        .as_deref()
        .and_then(|inflated| parse_netpacket_prefix(inflated.get(..NET_PACKET_PREFIX_LEN)?));
    let inflated_root_name = inflated
        .as_deref()
        .and_then(|bytes| decode_utf16_z(bytes.get(..20)?));
    let inflated_header_words = inflated.as_deref().and_then(|bytes| {
        Some([
            u32::from_le_bytes(bytes.get(20..24)?.try_into().ok()?),
            u32::from_le_bytes(bytes.get(24..28)?.try_into().ok()?),
            u32::from_le_bytes(bytes.get(28..32)?.try_into().ok()?),
            u32::from_le_bytes(bytes.get(32..36)?.try_into().ok()?),
            u32::from_le_bytes(bytes.get(36..40)?.try_into().ok()?),
        ])
    });
    let inflated_utf16_strings = inflated
        .as_deref()
        .map(|inflated| scan_utf16_strings(inflated, 2))
        .unwrap_or_default();
    let inflated_fields = inflated
        .as_deref()
        .map(scan_structured_fields)
        .unwrap_or_default();
    let packet_role = classify_packet(prefix, inflated_prefix.as_ref(), &inflated_utf16_strings);
    let penc_offsets = find_all_literals(packet_bytes, b"penc");
    let hypenc_offsets = find_all_literals(packet_bytes, b"hypenc");

    Auth7100FlowPacket {
        packet_offset,
        packet_len: packet_bytes.len(),
        outer_object_name: prefix.object_name.clone(),
        outer_tail: prefix.tail.clone(),
        field_20: prefix.field_20,
        field_44: prefix.field_44,
        field_52: prefix.field_52,
        field_56: prefix.field_56,
        field_hints: summarize_auth_7100_prefix_hints(
            &prefix.object_name,
            &prefix.tail,
            prefix.field_20,
            prefix.field_32,
            prefix.field_36,
            prefix.field_40,
            prefix.field_44,
            prefix.field_52,
            prefix.field_56,
        ),
        packet_role,
        inflated_root_name,
        inflated_header_words,
        inflated_object_name: inflated_prefix
            .as_ref()
            .map(|item| item.object_name.clone()),
        inflated_tail: inflated_prefix.as_ref().map(|item| item.tail.clone()),
        inflated_field_hints: inflated_prefix.as_ref().map(|item| {
            summarize_auth_7100_prefix_hints(
                &item.object_name,
                &item.tail,
                item.field_20,
                item.field_32,
                item.field_36,
                item.field_40,
                item.field_44,
                item.field_52,
                item.field_56,
            )
        }),
        inflated_len: inflated.as_ref().map(Vec::len),
        inflated_fields,
        contains_penc_ascii: !penc_offsets.is_empty(),
        penc_offsets,
        hypenc_offsets,
        utf16_strings: redact_sensitive_strings(if !inflated_utf16_strings.is_empty() {
            inflated_utf16_strings
        } else {
            raw_utf16_strings
        }),
        ascii_strings: redact_sensitive_strings(raw_ascii_strings),
        zstd_frame,
    }
}

fn classify_packet(
    prefix: &NetPacketPrefix,
    inflated_prefix: Option<&NetPacketPrefix>,
    inflated_utf16_strings: &[String],
) -> String {
    if prefix.tail.contains("下载文件") {
        return "download_file".to_string();
    }
    if let Some(inflated_prefix) = inflated_prefix {
        if inflated_prefix.tail.contains("测速")
            || inflated_utf16_strings
                .iter()
                .any(|text| text.contains("测速"))
        {
            return "auth_probe".to_string();
        }
        if inflated_prefix.tail.contains("登录")
            || inflated_utf16_strings
                .iter()
                .any(|text| text.contains("登录"))
        {
            return "auth_login".to_string();
        }
    }
    if prefix.tail.contains("ZSTD") {
        return "zstd_dict_envelope".to_string();
    }
    "unknown".to_string()
}

fn decode_exact_zstd(
    bytes: &[u8],
    zstd_magic_offset: usize,
    declared_frame_len: usize,
    uses_stock_dictionary: bool,
) -> Option<Vec<u8>> {
    let end = zstd_magic_offset.checked_add(declared_frame_len)?;
    let exact = bytes.get(zstd_magic_offset..end)?;
    decode_zstd(exact, uses_stock_dictionary).ok()
}

fn summarize_zstd_frame(
    bytes: &[u8],
    zstd_magic_offset: usize,
    uses_stock_dictionary: bool,
) -> Auth7100ZstdFrameSummary {
    let frame_descriptor = *bytes.get(4).unwrap_or(&0);
    let single_segment = frame_descriptor & 0x20 != 0;
    let content_checksum = frame_descriptor & 0x04 != 0;
    let dictionary_id_flag = frame_descriptor & 0x03;
    let frame_content_size_flag = frame_descriptor >> 6;
    let dictionary_id_size = match dictionary_id_flag {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 4,
        _ => 0,
    };
    let frame_content_size_size = if single_segment {
        match frame_content_size_flag {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 8,
            _ => 0,
        }
    } else {
        match frame_content_size_flag {
            0 => 0,
            1 => 2,
            2 => 4,
            3 => 8,
            _ => 0,
        }
    };

    let mut cursor = 5usize;
    if !single_segment && bytes.len() > cursor {
        cursor += 1;
    }
    cursor += dictionary_id_size;

    let frame_content_size = if bytes.len() >= cursor + frame_content_size_size {
        read_le_u64(&bytes[cursor..cursor + frame_content_size_size])
    } else {
        None
    };
    cursor += frame_content_size_size;

    let block_header = if bytes.len() >= cursor + 3 {
        Some(u32::from_le_bytes([
            bytes[cursor],
            bytes[cursor + 1],
            bytes[cursor + 2],
            0,
        ]))
    } else {
        None
    };
    let (block_last, block_type, block_size) = if let Some(block_header) = block_header {
        (
            block_header & 1 != 0,
            match (block_header >> 1) & 0x3 {
                0 => "raw",
                1 => "rle",
                2 => "compressed",
                _ => "reserved",
            }
            .to_string(),
            (block_header >> 3) as usize,
        )
    } else {
        (false, "unknown".to_string(), 0usize)
    };

    let declared_frame_len = cursor
        + usize::from(block_header.is_some()) * 3
        + block_size
        + usize::from(content_checksum) * 4;
    let trailing_bytes = bytes.len().saturating_sub(declared_frame_len);
    let exact = bytes.get(..declared_frame_len).unwrap_or(bytes);
    let decode = decode_zstd(exact, uses_stock_dictionary);

    Auth7100ZstdFrameSummary {
        zstd_magic_offset,
        frame_descriptor,
        single_segment,
        content_checksum,
        dictionary_id_flag,
        dictionary_id_size,
        frame_content_size_flag,
        frame_content_size_size,
        frame_content_size,
        block_last,
        block_type,
        block_size,
        declared_frame_len,
        trailing_bytes,
        decode_ok: decode.is_ok(),
        decode_error: decode.err().map(|err| err.to_string()),
    }
}

fn decode_zstd(bytes: &[u8], uses_stock_dictionary: bool) -> Result<Vec<u8>, std::io::Error> {
    if !uses_stock_dictionary {
        return zstd_decode_all(Cursor::new(bytes));
    }
    let mut decoder =
        zstd::stream::read::Decoder::with_dictionary(Cursor::new(bytes), STOCK_DICTIONARY_BYTES)?
            .single_frame();
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded)?;
    Ok(decoded)
}

fn redact_sensitive_strings(strings: Vec<String>) -> Vec<String> {
    let mut redact_next = false;
    strings
        .into_iter()
        .map(|value| {
            if redact_next {
                redact_next = false;
                return "<redacted>".to_string();
            }
            if is_sensitive_label(value.trim()) {
                redact_next = true;
            }
            value
        })
        .collect()
}

fn scan_structured_fields(bytes: &[u8]) -> Vec<Auth7100DecodedField> {
    let mut fields = Vec::new();
    let mut offset = 0usize;
    while offset + 22 <= bytes.len() {
        let type_id = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        let value_len =
            u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let field_12 = u32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().unwrap());
        let span_len =
            u32::from_le_bytes(bytes[offset + 16..offset + 20].try_into().unwrap()) as usize;
        if !(1..=16).contains(&type_id)
            || bytes[offset + 8..offset + 12] != [0; 4]
            || span_len < 22 + value_len
            || offset + span_len > bytes.len()
        {
            offset += 2;
            continue;
        }
        let label_bytes = &bytes[offset + 20..offset + span_len];
        let Some(label_end) = label_bytes
            .chunks_exact(2)
            .position(|chunk| chunk == [0, 0])
            .map(|word| word * 2)
        else {
            offset += 2;
            continue;
        };
        let label = decode_utf16_z(&label_bytes[..label_end + 2]).unwrap_or_default();
        let value_start = offset + 20 + label_end + 2;
        if label.is_empty()
            || !label.encode_utf16().all(is_visible_utf16)
            || value_start + value_len + 2 > offset + span_len
        {
            offset += 2;
            continue;
        }
        let sensitive = is_sensitive_label(&label);
        let value = &bytes[value_start..value_start + value_len];
        let value_crc32 = (!sensitive && value_len > 0).then(|| crc32fast::hash(value));
        fields.push(Auth7100DecodedField {
            offset,
            type_id,
            value_len,
            value_crc32,
            field_12,
            span_len,
            sensitive,
            label,
            value_first_nul: (!sensitive)
                .then(|| value.iter().position(|byte| *byte == 0))
                .flatten(),
            value_visible_ascii_bytes: if sensitive {
                0
            } else {
                value
                    .iter()
                    .filter(|byte| byte.is_ascii_graphic() || **byte == b' ')
                    .count()
            },
            value_visible_ascii_runs: if sensitive {
                Vec::new()
            } else {
                visible_ascii_runs(value)
            },
            value_gbk_decode_ok: if sensitive {
                false
            } else {
                !GBK.decode_without_bom_handling(value).1
            },
            value_gbk_character_count: if sensitive {
                None
            } else {
                let (decoded, had_errors) = GBK.decode_without_bom_handling(value);
                (!had_errors).then(|| decoded.chars().count())
            },
            value_crlf_count: if sensitive {
                0
            } else {
                value.windows(2).filter(|pair| *pair == b"\r\n").count()
            },
            value_non_ascii_bytes: if sensitive {
                0
            } else {
                value.iter().filter(|byte| !byte.is_ascii()).count()
            },
            value_other_control_bytes: if sensitive {
                0
            } else {
                value
                    .iter()
                    .filter(|byte| byte.is_ascii_control() && !matches!(**byte, b'\r' | b'\n'))
                    .count()
            },
            value_line_lengths: if sensitive {
                Vec::new()
            } else {
                value
                    .split(|byte| *byte == b'\n')
                    .map(|line| line.strip_suffix(b"\r").unwrap_or(line).len())
                    .collect()
            },
            value_line_shapes: if sensitive {
                Vec::new()
            } else {
                value
                    .split(|byte| *byte == b'\n')
                    .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
                    .map(ascii_line_shape)
                    .collect()
            },
            value_schema_labels: if sensitive {
                Vec::new()
            } else {
                ascii_schema_labels(value)
            },
            value_penc_offsets: if sensitive {
                Vec::new()
            } else {
                find_all_literals(value, b"penc")
            },
            value_hypenc_offsets: if sensitive {
                Vec::new()
            } else {
                find_all_literals(value, b"hypenc")
            },
            value_zstd_offsets: if sensitive {
                Vec::new()
            } else {
                find_all_literals(value, &ZSTD_MAGIC)
            },
        });
        offset += span_len;
    }
    fields
}

fn visible_ascii_runs(bytes: &[u8]) -> Vec<[usize; 2]> {
    let mut runs = Vec::new();
    let mut start = None;
    for (offset, byte) in bytes.iter().chain(std::iter::once(&0)).enumerate() {
        let visible = byte.is_ascii_graphic() || *byte == b' ';
        match (start, visible) {
            (None, true) => start = Some(offset),
            (Some(run_start), false) => {
                if offset - run_start >= 4 {
                    runs.push([run_start, offset]);
                }
                start = None;
            }
            _ => {}
        }
    }
    runs
}

fn ascii_line_shape(line: &[u8]) -> String {
    let mut shape = String::new();
    let mut offset = 0usize;
    while offset < line.len() {
        let byte = line[offset];
        let class = if byte.is_ascii_alphabetic() {
            Some('L')
        } else if byte.is_ascii_digit() {
            Some('D')
        } else {
            None
        };
        if let Some(class) = class {
            let start = offset;
            while offset < line.len()
                && if class == 'L' {
                    line[offset].is_ascii_alphabetic()
                } else {
                    line[offset].is_ascii_digit()
                }
            {
                offset += 1;
            }
            shape.push(class);
            shape.push_str(&(offset - start).to_string());
        } else {
            shape.push(char::from(byte));
            offset += 1;
        }
    }
    shape
}

fn ascii_schema_labels(bytes: &[u8]) -> Vec<String> {
    let mut labels = Vec::new();
    for raw_line in bytes.split(|byte| *byte == b'\n') {
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        let Ok(text) = std::str::from_utf8(line) else {
            continue;
        };
        if let Some((key, _)) = text.split_once('=') {
            if is_schema_identifier(key) {
                labels.push(format!("key:{key}"));
            }
            continue;
        }
        if text.starts_with('<') && text.ends_with('>') {
            let tag = text.trim_matches(['<', '>', '/']);
            if is_schema_identifier(tag) {
                labels.push(format!("tag:{tag}"));
            }
            continue;
        }
        if let Some((path, _)) = text.split_once('|') {
            if path.starts_with(".//") && path.bytes().all(is_schema_path_byte) {
                labels.push(format!("path:{path}"));
                continue;
            }
            if !text.bytes().any(|byte| byte.is_ascii_digit()) {
                for column in text.split('|').filter(|column| !column.is_empty()) {
                    if is_schema_identifier(column) {
                        labels.push(format!("column:{column}"));
                    }
                }
            }
        }
    }
    labels
}

fn is_schema_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || byte == b'_')
}

fn is_schema_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'.' | b'/')
}

fn is_sensitive_label(label: &str) -> bool {
    matches!(
        label,
        "账号" | "密码" | "用户ID" | "本地IP" | "网卡MAC" | "客户标识" | "账号到期"
    )
}

fn parse_netpacket_prefix(bytes: &[u8]) -> Option<NetPacketPrefix> {
    if bytes.len() < NET_PACKET_PREFIX_LEN {
        return None;
    }
    let object_name = decode_utf16_z(&bytes[..20])?;
    if object_name != "网络包" && object_name != "认证" && object_name != "数据" {
        return None;
    }
    Some(NetPacketPrefix {
        object_name,
        packet_len: u32::from_le_bytes(bytes[32..36].try_into().ok()?) as usize,
        field_20: u32::from_le_bytes(bytes[20..24].try_into().ok()?),
        field_32: u32::from_le_bytes(bytes[32..36].try_into().ok()?),
        field_36: u32::from_le_bytes(bytes[36..40].try_into().ok()?),
        field_40: u32::from_le_bytes(bytes[40..44].try_into().ok()?),
        field_44: u32::from_le_bytes(bytes[44..48].try_into().ok()?),
        field_52: u32::from_le_bytes(bytes[52..56].try_into().ok()?),
        field_56: u32::from_le_bytes(bytes[56..60].try_into().ok()?),
        tail: decode_utf16_tail(&bytes[60..74]),
    })
}

fn find_zstd(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(ZSTD_MAGIC.len())
        .position(|window| window == ZSTD_MAGIC)
}

fn scan_utf16_strings(bytes: &[u8], min_chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut words = Vec::<u16>::new();
    for chunk in bytes.chunks_exact(2) {
        let word = u16::from_le_bytes([chunk[0], chunk[1]]);
        if is_visible_utf16(word) {
            words.push(word);
            continue;
        }
        if words.len() >= min_chars {
            let text = String::from_utf16_lossy(&words).trim().to_string();
            if !text.is_empty() {
                out.push(text);
            }
        }
        words.clear();
    }
    if words.len() >= min_chars {
        let text = String::from_utf16_lossy(&words).trim().to_string();
        if !text.is_empty() {
            out.push(text);
        }
    }
    out
}

fn scan_ascii_strings(bytes: &[u8], min_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = Vec::<u8>::new();
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

fn is_visible_utf16(word: u16) -> bool {
    matches!(word, 0x0009 | 0x000a | 0x000d | 0x0020..=0x007e) || word >= 0x4e00
}

fn read_le_u64(bytes: &[u8]) -> Option<u64> {
    match bytes.len() {
        0 => None,
        1 => Some(u64::from(bytes[0])),
        2 => Some(u64::from(u16::from_le_bytes([bytes[0], bytes[1]]))),
        4 => Some(u64::from(u32::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ]))),
        8 => Some(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])),
        _ => None,
    }
}

fn decode_utf16_z(bytes: &[u8]) -> Option<String> {
    let mut words = Vec::new();
    for chunk in bytes.chunks_exact(2) {
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
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn decode_utf16_tail(bytes: &[u8]) -> String {
    let mut words = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        let word = u16::from_le_bytes([chunk[0], chunk[1]]);
        if word == 0 {
            continue;
        }
        if let Some(ch) = char::from_u32(word as u32) {
            if ch.is_control() {
                continue;
            }
        }
        words.push(word);
    }
    String::from_utf16_lossy(&words)
}

fn reassemble_stream(packets: &[TcpPacket]) -> Vec<u8> {
    if packets.is_empty() {
        return Vec::new();
    }

    let mut packets = packets.to_vec();
    packets.sort_by_key(|packet| packet.seq);
    let first_seq = packets[0].seq;
    let mut stream = Vec::new();
    let mut expected_seq = first_seq;

    for packet in &packets {
        let packet_start = packet.seq;
        let payload = &packet.captured_payload;
        let packet_end = packet.seq.wrapping_add(payload.len() as u32);

        if packet_start > expected_seq {
            expected_seq = packet_start;
        }

        let overlap = expected_seq.saturating_sub(packet_start) as usize;
        if overlap >= payload.len() {
            continue;
        }
        stream.extend_from_slice(&payload[overlap..]);
        expected_seq = packet_end;
    }

    stream
}

fn parse_pcap(bytes: &[u8]) -> Result<Vec<TcpPacket>, Box<dyn Error>> {
    if bytes.len() < 24 {
        return Err("pcap file too small".into());
    }

    let (endian, scale) = parse_global_header(&bytes[..24])?;
    let mut offset = 24usize;
    let mut packets = Vec::new();

    while offset + 16 <= bytes.len() {
        let ts_sec = read_u32(endian, &bytes[offset..offset + 4])?;
        let ts_frac = read_u32(endian, &bytes[offset + 4..offset + 8])?;
        let incl_len = read_u32(endian, &bytes[offset + 8..offset + 12])?;
        let orig_len = read_u32(endian, &bytes[offset + 12..offset + 16])?;
        offset += 16;

        let incl_len_usize = incl_len as usize;
        if offset + incl_len_usize > bytes.len() {
            break;
        }
        let frame = &bytes[offset..offset + incl_len_usize];
        offset += incl_len_usize;

        let Some(packet) = parse_tcp_packet(frame, incl_len, orig_len, ts_sec, ts_frac, scale)
        else {
            continue;
        };
        packets.push(packet);
    }

    Ok(packets)
}

fn parse_global_header(bytes: &[u8]) -> Result<(Endian, TimestampScale), Box<dyn Error>> {
    let magic: [u8; 4] = bytes[..4]
        .try_into()
        .map_err(|_| "pcap global header too short")?;
    match magic {
        [0xd4, 0xc3, 0xb2, 0xa1] => Ok((Endian::Little, TimestampScale::Micros)),
        [0xa1, 0xb2, 0xc3, 0xd4] => Ok((Endian::Big, TimestampScale::Micros)),
        [0x4d, 0x3c, 0xb2, 0xa1] => Ok((Endian::Little, TimestampScale::Nanos)),
        [0xa1, 0xb2, 0x3c, 0x4d] => Ok((Endian::Big, TimestampScale::Nanos)),
        other => Err(format!("unsupported pcap magic: {:02x?}", other).into()),
    }
}

fn parse_tcp_packet(
    frame: &[u8],
    _incl_len: u32,
    _orig_len: u32,
    _ts_sec: u32,
    _ts_frac: u32,
    _scale: TimestampScale,
) -> Option<TcpPacket> {
    let ip = if frame.first().is_some_and(|byte| byte >> 4 == 4) {
        frame
    } else if frame.len() >= 14 && u16::from_be_bytes([frame[12], frame[13]]) == 0x0800 {
        &frame[14..]
    } else {
        let offset = (0..=frame.len().saturating_sub(40).min(64)).find(|offset| {
            let candidate = &frame[*offset..];
            if candidate[0] >> 4 != 4 || candidate[9] != 6 {
                return false;
            }
            let ihl = usize::from(candidate[0] & 0x0f) * 4;
            let total_len = usize::from(u16::from_be_bytes([candidate[2], candidate[3]]));
            ihl >= 20 && total_len >= ihl + 20 && candidate.len() >= total_len
        })?;
        &frame[offset..]
    };
    if ip.len() < 20 {
        return None;
    }
    let version = ip[0] >> 4;
    let ihl = (ip[0] & 0x0f) as usize * 4;
    if version != 4 || ihl < 20 || ip.len() < ihl {
        return None;
    }
    if ip[9] != 6 {
        return None;
    }

    let total_len = u16::from_be_bytes([ip[2], ip[3]]) as usize;
    if total_len < ihl || ip.len() < total_len {
        return None;
    }
    let payload = &ip[ihl..total_len];
    if payload.len() < 20 {
        return None;
    }

    let src = Endpoint {
        ip: Ipv4Addr::new(ip[12], ip[13], ip[14], ip[15]),
        port: u16::from_be_bytes([payload[0], payload[1]]),
    };
    let dst = Endpoint {
        ip: Ipv4Addr::new(ip[16], ip[17], ip[18], ip[19]),
        port: u16::from_be_bytes([payload[2], payload[3]]),
    };

    let data_offset = ((payload[12] >> 4) as usize) * 4;
    if data_offset < 20 || payload.len() < data_offset {
        return None;
    }

    Some(TcpPacket {
        src,
        dst,
        seq: u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]),
        ack: u32::from_be_bytes([payload[8], payload[9], payload[10], payload[11]]),
        flags: payload[13],
        wire_payload_len: payload.len() - data_offset,
        captured_payload: payload[data_offset..].to_vec(),
    })
}

fn read_u32(endian: Endian, bytes: &[u8]) -> Result<u32, Box<dyn Error>> {
    let bytes: [u8; 4] = bytes.try_into().map_err(|_| "expected 4 bytes")?;
    Ok(match endian {
        Endian::Little => u32::from_le_bytes(bytes),
        Endian::Big => u32::from_be_bytes(bytes),
    })
}

fn display_ep(endpoint: Endpoint) -> String {
    format!("{}:{}", endpoint.ip, endpoint.port)
}

#[cfg(test)]
mod tests {
    use super::{
        TimestampScale, analyze_auth_7100_flow_matrix, analyze_auth_7100_flow_matrix_sample,
        analyze_netpackets, ascii_line_shape, parse_tcp_packet, redact_sensitive_strings,
    };
    use crate::auth_7100_client::{
        build_auth_7100_followup_download_packet, build_auth_7100_login_packet,
    };

    #[test]
    fn summarizes_probe_and_login_sessions_from_sample_pcap() {
        let matrix = analyze_auth_7100_flow_matrix_sample().expect("flow matrix");
        assert_eq!(matrix.sessions.len(), 2);

        let probe = matrix
            .sessions
            .iter()
            .find(|session| session.session_role == "auth_probe")
            .expect("probe session");
        assert_eq!(probe.client_packets.len(), 1);
        assert_eq!(probe.client_packets[0].packet_len, 419);
        assert_eq!(
            probe.client_packets[0].field_hints.layout_hint,
            "compressed_zstd"
        );
        assert_eq!(
            probe.client_packets[0]
                .inflated_field_hints
                .as_ref()
                .expect("probe client inflated hints")
                .layout_hint,
            "auth_request_record"
        );
        assert_eq!(probe.server_packets.len(), 1);
        assert_eq!(probe.server_packets[0].packet_len, 345);
        assert_eq!(
            probe.server_packets[0]
                .inflated_field_hints
                .as_ref()
                .expect("probe server inflated hints")
                .layout_hint,
            "auth_reply_record"
        );

        let login = matrix
            .sessions
            .iter()
            .find(|session| session.session_role == "auth_login")
            .expect("login session");
        assert_eq!(
            login
                .client_packets
                .iter()
                .map(|packet| packet.packet_len)
                .collect::<Vec<_>>(),
            vec![611, 271, 333]
        );
        assert_eq!(
            login
                .server_packets
                .iter()
                .map(|packet| packet.packet_len)
                .collect::<Vec<_>>(),
            vec![443, 1540, 395]
        );
        assert_eq!(
            login.client_packets[0].field_hints.fixed_overhead_len_hint,
            Some(28)
        );
        assert_eq!(
            login.client_packets[0]
                .inflated_field_hints
                .as_ref()
                .expect("login client inflated hints")
                .layout_hint,
            "auth_request_record"
        );
        assert_eq!(
            login.server_packets[1].field_hints.layout_hint,
            "download_file_record"
        );
        assert_eq!(login.server_packets[1].penc_offsets, vec![60, 416]);
        assert!(login.server_packets[1].hypenc_offsets.is_empty());
    }

    #[test]
    fn accepts_pcapng_capture_via_tcpdump_conversion() {
        let path =
            crate::repository_fixture_path("captured_windows_traffic/capture_netzip_full.pcapng");
        let matrix = analyze_auth_7100_flow_matrix(path).expect("flow matrix from pcapng");
        assert_eq!(matrix.sessions.len(), 2);

        let login = matrix
            .sessions
            .iter()
            .find(|session| session.session_role == "auth_login")
            .expect("login session");
        assert_eq!(
            login
                .client_packets
                .iter()
                .map(|packet| packet.packet_len)
                .collect::<Vec<_>>(),
            vec![611, 271, 333]
        );
        assert_eq!(
            login
                .server_packets
                .iter()
                .map(|packet| packet.packet_len)
                .collect::<Vec<_>>(),
            vec![443, 1540, 395]
        );
    }

    #[test]
    #[ignore = "reads the local credential-bearing vendor capture without logging values"]
    fn verifies_vendor_pm_initial_login_manifest_shape() {
        let path = crate::repository_fixture_path(
            "diagnostics/20260831-netzip-windows-vs-rust/captures/vendor_pm.pcapng",
        );
        let matrix =
            super::analyze_auth_flow_matrix_for_port(&path, 7100).expect("analyze vendor capture");
        let login = matrix
            .sessions
            .iter()
            .find(|session| session.local_endpoint == "198.18.0.1:3091")
            .expect("vendor login session");
        let first = login.client_packets.first().expect("initial manifest");
        assert_eq!(first.packet_len, 632);
        assert_eq!(first.inflated_len, Some(854));
        assert_eq!(first.inflated_header_words, Some([19, 0, 0, 854, 854]));
        assert!(
            first
                .inflated_fields
                .iter()
                .any(|field| field.label == "账号")
        );
        assert!(
            first
                .inflated_fields
                .iter()
                .any(|field| field.label == "密码")
        );
    }

    #[test]
    fn decodes_stock_dictionary_followup_in_flow_matrix() {
        let packet = build_auth_7100_followup_download_packet(1).expect("followup packet");
        let packets = analyze_netpackets(&packet);
        assert_eq!(packets.len(), 1);
        let summary = &packets[0];
        assert_eq!(summary.field_44, 12);
        assert!(summary.zstd_frame.as_ref().expect("zstd frame").decode_ok);
        assert_eq!(summary.inflated_len, Some(130));
        assert_eq!(summary.inflated_root_name.as_deref(), Some("下载文件"));
        assert_eq!(summary.inflated_header_words, Some([2, 0, 0, 130, 130]));
        assert!(summary.inflated_field_hints.is_none());
        assert!(
            summary
                .inflated_fields
                .iter()
                .any(|field| field.label == "编号" && field.value_len == 4 && field.field_12 == 3)
        );
        assert!(
            summary
                .utf16_strings
                .iter()
                .any(|value| value == "下载文件")
        );
    }

    #[test]
    fn parses_pktmon_bare_ipv4_tcp_packet() {
        let mut frame = vec![0u8; 40];
        frame[0] = 0x45;
        frame[2..4].copy_from_slice(&40u16.to_be_bytes());
        frame[9] = 6;
        frame[12..16].copy_from_slice(&[198, 18, 0, 1]);
        frame[16..20].copy_from_slice(&[121, 41, 70, 217]);
        frame[20..22].copy_from_slice(&3091u16.to_be_bytes());
        frame[22..24].copy_from_slice(&7100u16.to_be_bytes());
        frame[32] = 5 << 4;
        frame[33] = 0x18;

        let packet = parse_tcp_packet(&frame, 40, 40, 0, 0, TimestampScale::Micros)
            .expect("bare IPv4 packet");
        assert_eq!(packet.src.port, 3091);
        assert_eq!(packet.dst.port, 7100);
    }

    #[test]
    fn parses_prefixed_bare_ipv4_tcp_packet() {
        let mut frame = vec![0xa5u8; 24];
        frame.extend_from_slice(&[0x45, 0, 0, 40, 0, 0, 0, 0, 64, 6, 0, 0]);
        frame.extend_from_slice(&[198, 18, 0, 1, 121, 41, 70, 217]);
        frame.extend_from_slice(&3091u16.to_be_bytes());
        frame.extend_from_slice(&7100u16.to_be_bytes());
        frame.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 2, 0x50, 0x18, 0, 0, 0, 0, 0, 0]);
        let packet = parse_tcp_packet(
            &frame,
            frame.len() as u32,
            frame.len() as u32,
            0,
            0,
            TimestampScale::Micros,
        )
        .expect("prefixed bare IPv4 packet");
        assert_eq!(packet.src.port, 3091);
        assert_eq!(packet.dst.port, 7100);
    }

    #[test]
    fn redacts_values_after_credential_labels() {
        assert_eq!(
            redact_sensitive_strings(vec![
                "账号".to_string(),
                "ACCOUNT_VALUE".to_string(),
                "密码".to_string(),
                "PASSWORD_VALUE".to_string(),
                "登录".to_string(),
            ]),
            vec!["账号", "<redacted>", "密码", "<redacted>", "登录"]
        );
    }

    #[test]
    fn line_shape_never_retains_alphanumeric_content() {
        assert_eq!(ascii_line_shape(b"Server_12=AB9:80"), "L6_D2=L2D1:D2");
    }

    #[test]
    fn structured_field_catalog_never_contains_values() {
        let packet =
            build_auth_7100_login_packet("ACCOUNT_VALUE", "PASSWORD_VALUE").expect("login packet");
        let packets = analyze_netpackets(&packet);
        let fields = &packets[0].inflated_fields;
        assert!(
            fields
                .iter()
                .any(|field| field.label == "账号" && field.sensitive)
        );
        for field in fields.iter().filter(|field| field.sensitive) {
            assert_eq!(field.value_crc32, None);
            assert_eq!(field.value_first_nul, None);
            assert_eq!(field.value_visible_ascii_bytes, 0);
            assert!(field.value_visible_ascii_runs.is_empty());
            assert!(!field.value_gbk_decode_ok);
            assert_eq!(field.value_gbk_character_count, None);
            assert_eq!(field.value_crlf_count, 0);
            assert_eq!(field.value_non_ascii_bytes, 0);
            assert_eq!(field.value_other_control_bytes, 0);
            assert!(field.value_line_lengths.is_empty());
            assert!(field.value_line_shapes.is_empty());
            assert!(field.value_schema_labels.is_empty());
            assert!(field.value_penc_offsets.is_empty());
            assert!(field.value_hypenc_offsets.is_empty());
            assert!(field.value_zstd_offsets.is_empty());
        }
        assert!(
            fields
                .iter()
                .any(|field| field.label == "密码" && field.sensitive)
        );
        let serialized = serde_json::to_string(fields).expect("serialize fields");
        assert!(!serialized.contains("ACCOUNT_VALUE"));
        assert!(!serialized.contains("PASSWORD_VALUE"));
        assert!(
            fields
                .iter()
                .filter(|field| field.sensitive)
                .all(|field| field.value_crc32.is_none())
        );
    }
}
