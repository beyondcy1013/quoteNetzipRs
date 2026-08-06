use crate::auth_flow_sample::Auth7100ZstdFrameSummary;
use crate::capture_input::read_capture_file_as_pcap_bytes;
use crate::{Auth7100PrefixHints, summarize_auth_7100_prefix_hints};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::io::Cursor;
use std::net::Ipv4Addr;
use std::path::Path;
use zstd::stream::decode_all as zstd_decode_all;

const NET_PACKET_PREFIX_LEN: usize = 74;
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const SAMPLE_PCAP_REL: &str = "tmp/netzip_full_tcp.pcap";

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
    pub inflated_object_name: Option<String>,
    pub inflated_tail: Option<String>,
    pub inflated_field_hints: Option<Auth7100PrefixHints>,
    pub contains_penc_ascii: bool,
    pub penc_offsets: Vec<usize>,
    pub hypenc_offsets: Vec<usize>,
    pub utf16_strings: Vec<String>,
    pub ascii_strings: Vec<String>,
    pub zstd_frame: Option<Auth7100ZstdFrameSummary>,
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
    let path = path.as_ref();
    let bytes = read_capture_file_as_pcap_bytes(path)?;
    let parsed = parse_pcap(&bytes)?;
    let (local_host, server) = discover_7100_endpoints(&parsed)?;
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

fn discover_7100_endpoints(packets: &[TcpPacket]) -> Result<(Ipv4Addr, Endpoint), Box<dyn Error>> {
    let mut candidates = BTreeMap::<(Ipv4Addr, Endpoint), usize>::new();
    for packet in packets {
        let candidate = if packet.src.port == 7100 {
            Some((packet.dst.ip, packet.src))
        } else if packet.dst.port == 7100 {
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
        .ok_or_else(|| "capture contains no TCP/7100 packets".into())
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
    let zstd_frame = find_zstd(packet_bytes).map(|zstd_magic_offset| {
        summarize_zstd_frame(&packet_bytes[zstd_magic_offset..], zstd_magic_offset)
    });
    let inflated = zstd_frame.as_ref().and_then(|frame| {
        if !frame.decode_ok {
            return None;
        }
        decode_exact_zstd(
            packet_bytes,
            frame.zstd_magic_offset,
            frame.declared_frame_len,
        )
    });
    let inflated_prefix = inflated
        .as_deref()
        .and_then(|inflated| parse_netpacket_prefix(inflated.get(..NET_PACKET_PREFIX_LEN)?));
    let inflated_utf16_strings = inflated
        .as_deref()
        .map(|inflated| scan_utf16_strings(inflated, 2))
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
        contains_penc_ascii: !penc_offsets.is_empty(),
        penc_offsets,
        hypenc_offsets,
        utf16_strings: if !inflated_utf16_strings.is_empty() {
            inflated_utf16_strings
        } else {
            raw_utf16_strings
        },
        ascii_strings: raw_ascii_strings,
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
) -> Option<Vec<u8>> {
    let end = zstd_magic_offset.checked_add(declared_frame_len)?;
    let exact = bytes.get(zstd_magic_offset..end)?;
    zstd_decode_all(Cursor::new(exact)).ok()
}

fn summarize_zstd_frame(bytes: &[u8], zstd_magic_offset: usize) -> Auth7100ZstdFrameSummary {
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
    let decode = zstd_decode_all(Cursor::new(exact));

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
    if frame.len() < 14 {
        return None;
    }
    let ethertype = u16::from_be_bytes([frame[12], frame[13]]);
    if ethertype != 0x0800 {
        return None;
    }

    let ip = &frame[14..];
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
    use super::{analyze_auth_7100_flow_matrix, analyze_auth_7100_flow_matrix_sample};

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
}
