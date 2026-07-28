use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fmt;
use std::fs;
use std::net::Ipv4Addr;
use std::path::Path;

use serde::Serialize;

const NET_PACKET_PREFIX_LEN: usize = 74;

#[derive(Clone, Debug, Serialize)]
pub struct PcapSummary {
    pub input: String,
    pub pcap_packets: usize,
    pub unique_packets: usize,
    pub duplicate_packets: usize,
    pub truncated_unique_packets: usize,
    pub flows: Vec<PcapFlowSummary>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PcapFlowSummary {
    pub src: String,
    pub dst: String,
    pub total_packets: usize,
    pub unique_packets: usize,
    pub duplicate_packets: usize,
    pub data_packets: usize,
    pub truncated_packets: usize,
    pub wire_payload_bytes: usize,
    pub captured_payload_bytes: usize,
    pub timespan_seconds: f64,
    pub samples: Vec<PcapPacketSample>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PcapPacketSample {
    pub seq: u32,
    pub ack: u32,
    pub flags: u8,
    pub wire_payload_len: usize,
    pub captured_payload_len: usize,
    pub incl_len: u32,
    pub orig_len: u32,
    pub head_hex: String,
    pub netpacket_prefix: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Endpoint {
    ip: Ipv4Addr,
    port: u16,
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.ip, self.port)
    }
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
    ts: f64,
    src: Endpoint,
    dst: Endpoint,
    seq: u32,
    ack: u32,
    flags: u8,
    incl_len: u32,
    orig_len: u32,
    wire_payload_len: usize,
    captured_payload: Vec<u8>,
}

#[derive(Clone, Debug)]
struct FlowStats {
    total_packets: usize,
    unique_packets: usize,
    data_packets: usize,
    truncated_packets: usize,
    wire_payload_bytes: usize,
    captured_payload_bytes: usize,
    first_ts: f64,
    last_ts: f64,
    samples: Vec<TcpPacket>,
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

pub fn summarize_pcap_file(
    path: impl AsRef<Path>,
    segment_limit: usize,
) -> Result<PcapSummary, Box<dyn Error>> {
    let path = path.as_ref();
    let bytes = fs::read(path)?;
    let packets = parse_pcap(&bytes)?;

    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for packet in &packets {
        let identity = PacketIdentity {
            src: packet.src,
            dst: packet.dst,
            seq: packet.seq,
            ack: packet.ack,
            flags: packet.flags,
            wire_payload_len: packet.wire_payload_len,
            captured_payload: packet.captured_payload.clone(),
        };
        if seen.insert(identity) {
            unique.push(packet.clone());
        }
    }

    let duplicate_packets = packets.len().saturating_sub(unique.len());
    let truncated_unique_packets = unique
        .iter()
        .filter(|packet| packet.incl_len < packet.orig_len)
        .count();

    let mut stats_by_flow = BTreeMap::<(Endpoint, Endpoint), FlowStats>::new();
    for packet in &packets {
        let entry = stats_by_flow
            .entry((packet.src, packet.dst))
            .or_insert_with(|| FlowStats {
                total_packets: 0,
                unique_packets: 0,
                data_packets: 0,
                truncated_packets: 0,
                wire_payload_bytes: 0,
                captured_payload_bytes: 0,
                first_ts: packet.ts,
                last_ts: packet.ts,
                samples: Vec::new(),
            });
        entry.total_packets += 1;
        entry.first_ts = entry.first_ts.min(packet.ts);
        entry.last_ts = entry.last_ts.max(packet.ts);
    }

    for packet in &unique {
        let entry = stats_by_flow
            .get_mut(&(packet.src, packet.dst))
            .expect("flow should exist");
        entry.unique_packets += 1;
        if packet.wire_payload_len > 0 {
            entry.data_packets += 1;
            entry.wire_payload_bytes += packet.wire_payload_len;
            entry.captured_payload_bytes += packet.captured_payload.len();
            if packet.incl_len < packet.orig_len {
                entry.truncated_packets += 1;
            }
            if entry.samples.len() < segment_limit {
                entry.samples.push(packet.clone());
            }
        }
    }

    let mut flows = Vec::new();
    for ((src, dst), stats) in stats_by_flow {
        flows.push(PcapFlowSummary {
            src: src.to_string(),
            dst: dst.to_string(),
            total_packets: stats.total_packets,
            unique_packets: stats.unique_packets,
            duplicate_packets: stats.total_packets.saturating_sub(stats.unique_packets),
            data_packets: stats.data_packets,
            truncated_packets: stats.truncated_packets,
            wire_payload_bytes: stats.wire_payload_bytes,
            captured_payload_bytes: stats.captured_payload_bytes,
            timespan_seconds: stats.last_ts - stats.first_ts,
            samples: stats
                .samples
                .into_iter()
                .map(|sample| PcapPacketSample {
                    seq: sample.seq,
                    ack: sample.ack,
                    flags: sample.flags,
                    wire_payload_len: sample.wire_payload_len,
                    captured_payload_len: sample.captured_payload.len(),
                    incl_len: sample.incl_len,
                    orig_len: sample.orig_len,
                    head_hex: hex_prefix(&sample.captured_payload, 16),
                    netpacket_prefix: decode_netpacket_prefix(&sample.captured_payload),
                })
                .collect(),
        });
    }

    Ok(PcapSummary {
        input: path.display().to_string(),
        pcap_packets: packets.len(),
        unique_packets: unique.len(),
        duplicate_packets,
        truncated_unique_packets,
        flows,
    })
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
    match bytes[..4] {
        [0xd4, 0xc3, 0xb2, 0xa1] => Ok((Endian::Little, TimestampScale::Micros)),
        [0xa1, 0xb2, 0xc3, 0xd4] => Ok((Endian::Big, TimestampScale::Micros)),
        [0x4d, 0x3c, 0xb2, 0xa1] => Ok((Endian::Little, TimestampScale::Nanos)),
        [0xa1, 0xb2, 0x3c, 0x4d] => Ok((Endian::Big, TimestampScale::Nanos)),
        _ => Err("unsupported pcap magic".into()),
    }
}

fn read_u32(endian: Endian, bytes: &[u8]) -> Result<u32, Box<dyn Error>> {
    let array: [u8; 4] = bytes.try_into()?;
    Ok(match endian {
        Endian::Little => u32::from_le_bytes(array),
        Endian::Big => u32::from_be_bytes(array),
    })
}

fn parse_tcp_packet(
    frame: &[u8],
    incl_len: u32,
    orig_len: u32,
    ts_sec: u32,
    ts_frac: u32,
    scale: TimestampScale,
) -> Option<TcpPacket> {
    if frame.len() < 14 + 20 + 20 {
        return None;
    }
    if frame[12..14] != [0x08, 0x00] {
        return None;
    }

    let ihl = ((frame[14] & 0x0f) as usize) * 4;
    if frame.len() < 14 + ihl + 20 || frame[23] != 6 {
        return None;
    }

    let ip_total_len = u16::from_be_bytes([frame[16], frame[17]]) as usize;
    let src = Endpoint {
        ip: Ipv4Addr::new(frame[26], frame[27], frame[28], frame[29]),
        port: u16::from_be_bytes([frame[34], frame[35]]),
    };
    let dst = Endpoint {
        ip: Ipv4Addr::new(frame[30], frame[31], frame[32], frame[33]),
        port: u16::from_be_bytes([frame[36], frame[37]]),
    };

    let tcp_start = 14 + ihl;
    let tcp_header_len = (((frame[tcp_start + 12] >> 4) & 0x0f) as usize) * 4;
    if frame.len() < tcp_start + tcp_header_len {
        return None;
    }

    let seq = u32::from_be_bytes([
        frame[tcp_start + 4],
        frame[tcp_start + 5],
        frame[tcp_start + 6],
        frame[tcp_start + 7],
    ]);
    let ack = u32::from_be_bytes([
        frame[tcp_start + 8],
        frame[tcp_start + 9],
        frame[tcp_start + 10],
        frame[tcp_start + 11],
    ]);
    let flags = frame[tcp_start + 13];

    let payload_start = tcp_start + tcp_header_len;
    let wire_payload_len = ip_total_len.saturating_sub(ihl + tcp_header_len);
    let payload_end = frame.len().min(14 + ip_total_len);
    let captured_payload = if payload_start < payload_end {
        frame[payload_start..payload_end].to_vec()
    } else {
        Vec::new()
    };

    let divisor = match scale {
        TimestampScale::Micros => 1_000_000.0,
        TimestampScale::Nanos => 1_000_000_000.0,
    };

    Some(TcpPacket {
        ts: ts_sec as f64 + ts_frac as f64 / divisor,
        src,
        dst,
        seq,
        ack,
        flags,
        incl_len,
        orig_len,
        wire_payload_len,
        captured_payload,
    })
}

fn decode_netpacket_prefix(bytes: &[u8]) -> Option<String> {
    if bytes.len() < NET_PACKET_PREFIX_LEN {
        return None;
    }
    let name = decode_utf16_z(&bytes[..20])?;
    if name != "网络包" {
        return None;
    }

    let field_20 = u32::from_le_bytes(bytes[20..24].try_into().ok()?);
    let field_32 = u32::from_le_bytes(bytes[32..36].try_into().ok()?);
    let field_36 = u32::from_le_bytes(bytes[36..40].try_into().ok()?);
    let field_40 = u32::from_le_bytes(bytes[40..44].try_into().ok()?);
    let field_44 = u32::from_le_bytes(bytes[44..48].try_into().ok()?);
    let field_52 = u32::from_le_bytes(bytes[52..56].try_into().ok()?);
    let field_56 = u32::from_le_bytes(bytes[56..60].try_into().ok()?);
    let tail = decode_utf16_tail(&bytes[60..74]);

    Some(format!(
        "name={name} field20={field_20} field32={field_32} field36={field_36} field40={field_40} field44={field_44} field52={field_52} field56={field_56} tail={tail}"
    ))
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
            out.push(ch);
        }
    }
    out.trim_matches('|').to_string()
}

fn hex_prefix(bytes: &[u8], len: usize) -> String {
    let mut out = String::new();
    for byte in bytes.iter().take(len) {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::summarize_pcap_file;

    #[test]
    fn summarizes_empty_classic_pcap() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0xd4, 0xc3, 0xb2, 0xa1]);
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&65535u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());

        let path = std::env::temp_dir().join("netzip_test_empty.pcap");
        fs::write(&path, bytes).expect("write pcap");

        let summary = summarize_pcap_file(&path, 4).expect("summarize pcap");
        assert_eq!(summary.pcap_packets, 0);
        assert_eq!(summary.unique_packets, 0);
        assert_eq!(summary.duplicate_packets, 0);
        assert!(summary.flows.is_empty());

        let _ = fs::remove_file(path);
    }
}
