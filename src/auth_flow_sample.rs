use crate::{
    Auth7100PrefixHints, DownloadedServerConfig, parse_downloaded_server_config_from_path,
    summarize_auth_7100_prefix_hints,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use zstd::stream::decode_all as zstd_decode_all;

const NET_PACKET_PREFIX_LEN: usize = 74;
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const SAMPLE_SERVER_FLOW_REL: &str = "tmp/flow_7100_2697_server.bin";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Auth7100ServerSampleAnalysis {
    pub sample_path: String,
    pub size_bytes: usize,
    pub downloaded_server_config: Option<DownloadedServerConfig>,
    pub packets: Vec<Auth7100ServerPacketAnalysis>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Auth7100ServerPacketAnalysis {
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
    pub contains_penc_ascii: bool,
    pub contains_tdx_encrypt_utf16: bool,
    pub utf16_strings: Vec<String>,
    pub ascii_strings: Vec<String>,
    pub zstd_frame: Option<Auth7100ZstdFrameSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Auth7100ZstdFrameSummary {
    pub zstd_magic_offset: usize,
    pub frame_descriptor: u8,
    pub single_segment: bool,
    pub content_checksum: bool,
    pub dictionary_id_flag: u8,
    pub dictionary_id_size: usize,
    pub frame_content_size_flag: u8,
    pub frame_content_size_size: usize,
    pub frame_content_size: Option<u64>,
    pub block_last: bool,
    pub block_type: String,
    pub block_size: usize,
    pub declared_frame_len: usize,
    pub trailing_bytes: usize,
    pub decode_ok: bool,
    pub decode_error: Option<String>,
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

pub fn analyze_auth_7100_server_sample() -> Result<Auth7100ServerSampleAnalysis, Box<dyn Error>> {
    let path = crate::repository_fixture_path(SAMPLE_SERVER_FLOW_REL);
    analyze_auth_7100_server_flow(path)
}

pub fn analyze_auth_7100_server_flow(
    path: impl AsRef<Path>,
) -> Result<Auth7100ServerSampleAnalysis, Box<dyn Error>> {
    let path = path.as_ref();
    let bytes = fs::read(path)?;
    let downloaded_server_config = parse_downloaded_server_config_from_path(path).ok();
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
        packets.push(analyze_packet(offset, packet_bytes, &prefix));
        offset += prefix.packet_len;
    }

    Ok(Auth7100ServerSampleAnalysis {
        sample_path: path.display().to_string(),
        size_bytes: bytes.len(),
        downloaded_server_config,
        packets,
    })
}

fn analyze_packet(
    packet_offset: usize,
    packet_bytes: &[u8],
    prefix: &NetPacketPrefix,
) -> Auth7100ServerPacketAnalysis {
    let utf16_strings = scan_utf16_strings(packet_bytes, 2);
    let ascii_strings = scan_ascii_strings(packet_bytes, 4);
    let zstd_frame = find_zstd(packet_bytes).map(|zstd_magic_offset| {
        summarize_zstd_frame(&packet_bytes[zstd_magic_offset..], zstd_magic_offset)
    });

    Auth7100ServerPacketAnalysis {
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
        packet_role: classify_packet(prefix),
        contains_penc_ascii: packet_bytes.windows(4).any(|window| window == b"penc"),
        contains_tdx_encrypt_utf16: packet_bytes
            .windows(22)
            .any(|window| window == utf16le_literal("Tdx_Encrypt").as_slice()),
        utf16_strings,
        ascii_strings,
        zstd_frame,
    }
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

fn classify_packet(prefix: &NetPacketPrefix) -> String {
    if prefix.tail.contains("下载文件") {
        return "download_file".to_string();
    }
    if prefix.tail.contains("ZSTD") {
        return "zstd_dict_envelope".to_string();
    }
    "unknown".to_string()
}

fn parse_netpacket_prefix(bytes: &[u8]) -> Option<NetPacketPrefix> {
    if bytes.len() < NET_PACKET_PREFIX_LEN {
        return None;
    }
    let object_name = decode_utf16_z(&bytes[..20])?;
    if object_name != "网络包" {
        return None;
    }
    Some(NetPacketPrefix {
        object_name,
        field_20: u32::from_le_bytes(bytes[20..24].try_into().ok()?),
        field_32: u32::from_le_bytes(bytes[32..36].try_into().ok()?),
        field_36: u32::from_le_bytes(bytes[36..40].try_into().ok()?),
        field_40: u32::from_le_bytes(bytes[40..44].try_into().ok()?),
        field_44: u32::from_le_bytes(bytes[44..48].try_into().ok()?),
        field_52: u32::from_le_bytes(bytes[52..56].try_into().ok()?),
        field_56: u32::from_le_bytes(bytes[56..60].try_into().ok()?),
        packet_len: u32::from_le_bytes(bytes[32..36].try_into().ok()?) as usize,
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

fn is_visible_utf16(word: u16) -> bool {
    matches!(word, 0x0009 | 0x000a | 0x000d | 0x0020..=0x007e) || word >= 0x4e00
}

fn utf16le_literal(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 2);
    for word in text.encode_utf16() {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out
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
        if word != 0 {
            words.push(word);
        }
    }
    String::from_utf16_lossy(&words)
}

#[cfg(test)]
mod tests {
    use super::analyze_auth_7100_server_sample;

    #[test]
    fn parses_sample_flow_and_detects_corrupted_zstd_packets() {
        let analysis = analyze_auth_7100_server_sample().expect("sample analysis");
        assert_eq!(analysis.packets.len(), 3);
        assert_eq!(analysis.packets[1].packet_role, "download_file");
        assert!(analysis.downloaded_server_config.is_some());
        let zstd_packets = analysis
            .packets
            .iter()
            .filter(|packet| packet.packet_role == "zstd_dict_envelope")
            .collect::<Vec<_>>();
        assert_eq!(zstd_packets.len(), 2);
        assert!(zstd_packets.iter().all(|packet| {
            packet
                .zstd_frame
                .as_ref()
                .is_some_and(|frame| !frame.decode_ok && frame.decode_error.is_some())
        }));
    }
}
