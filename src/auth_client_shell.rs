use crate::auth_flow_sample::Auth7100ZstdFrameSummary;
use crate::capture_input::read_capture_file_as_pcap_bytes;
use crate::debug_blob_compare::{DiffRun, compare_blob_bytes};
use crate::{Auth7100PrefixHints, summarize_auth_7100_prefix_hints};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use zstd::stream::decode_all as zstd_decode_all;

const NET_PACKET_PREFIX_LEN: usize = 74;
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const SAMPLE_PCAP_REL: &str = "tmp/netzip_full_tcp.pcap";
const MAX_MATCHES_PER_PACKET: usize = 8;
const FALLBACK_MAX_SIZE_DELTA: usize = 96;
const FALLBACK_MAX_CANDIDATES: usize = 64;

#[derive(Clone, Debug, Serialize)]
pub struct Auth7100ClientShellAnalysis {
    pub pcap_path: String,
    pub source_endpoint: String,
    pub destination_endpoint: String,
    pub tcp_payload_bytes: usize,
    pub packets: Vec<Auth7100ClientPacketAnalysis>,
    pub packet_matches: Vec<Auth7100ClientPacketMatches>,
    pub findings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Auth7100ShellCorrelationAnalysis {
    pub pcap_path: String,
    pub server_endpoint: String,
    pub sessions: Vec<Auth7100ShellSessionAnalysis>,
    pub findings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Auth7100ShellSessionAnalysis {
    pub local_endpoint: String,
    pub session_role: String,
    pub started_with_handshake: bool,
    pub directions: Vec<Auth7100ShellDirectionAnalysis>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Auth7100ShellDirectionAnalysis {
    pub source_endpoint: String,
    pub destination_endpoint: String,
    pub tcp_payload_bytes: usize,
    pub packets: Vec<Auth7100ClientPacketAnalysis>,
    pub packet_matches: Vec<Auth7100ClientPacketMatches>,
    pub findings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Auth7100ClientPacketAnalysis {
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
    pub utf16_strings: Vec<String>,
    pub ascii_strings: Vec<String>,
    pub zstd_frame: Option<Auth7100ZstdFrameSummary>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Auth7100ClientPacketMatches {
    pub packet_offset: usize,
    pub packet_len: usize,
    pub candidate_count: usize,
    pub exact_match_count: usize,
    pub exact_matches: Vec<Auth7100DumpCandidateMatch>,
    pub top_matches: Vec<Auth7100DumpCandidateMatch>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Auth7100DumpCandidateMatch {
    pub candidate_path: String,
    pub candidate_size: usize,
    pub size_delta: isize,
    pub same_size: bool,
    pub exact_match: bool,
    pub equal_prefix_len: usize,
    pub equal_suffix_len: usize,
    pub diff_bytes: usize,
    pub first_diff_offset: Option<usize>,
    pub first_diff_zone: Option<String>,
    pub outer_header_diff_bytes: usize,
    pub pre_zstd_metadata_diff_bytes: usize,
    pub zstd_frame_header_diff_bytes: usize,
    pub zstd_block_body_diff_bytes: usize,
    pub zstd_block_body_compare: Option<Auth7100ZoneCompareSummary>,
    pub diff_runs: Vec<DiffRun>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Auth7100ZoneCompareSummary {
    pub compare_len: usize,
    pub equal_prefix_len: usize,
    pub equal_suffix_len: usize,
    pub diff_bytes: usize,
    pub first_diff_offset: Option<usize>,
    pub diff_runs: Vec<DiffRun>,
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

#[derive(Clone, Copy)]
struct ZstdLayout {
    zstd_magic_offset: usize,
    block_data_offset: usize,
}

pub fn analyze_auth_7100_client_shell_sample() -> Result<Auth7100ClientShellAnalysis, Box<dyn Error>>
{
    let path = crate::repository_fixture_path(SAMPLE_PCAP_REL);
    analyze_auth_7100_client_shell_from_pcap(path)
}

pub fn analyze_auth_7100_shell_correlation_sample()
-> Result<Auth7100ShellCorrelationAnalysis, Box<dyn Error>> {
    let path = crate::repository_fixture_path(SAMPLE_PCAP_REL);
    analyze_auth_7100_shell_correlation_from_pcap(path)
}

pub fn analyze_auth_7100_client_shell_from_pcap(
    path: impl AsRef<Path>,
) -> Result<Auth7100ClientShellAnalysis, Box<dyn Error>> {
    let path = path.as_ref();
    let src = Endpoint {
        ip: Ipv4Addr::new(192, 168, 3, 38),
        port: 2697,
    };
    let dst = Endpoint {
        ip: Ipv4Addr::new(39, 108, 103, 69),
        port: 7100,
    };

    let flow = reassemble_tcp_payload(path, src, dst)?;
    analyze_client_shell_flow(path, src, dst, &flow)
}

pub fn analyze_auth_7100_shell_correlation_from_pcap(
    path: impl AsRef<Path>,
) -> Result<Auth7100ShellCorrelationAnalysis, Box<dyn Error>> {
    let path = path.as_ref();
    let server = Endpoint {
        ip: Ipv4Addr::new(39, 108, 103, 69),
        port: 7100,
    };
    let local_host = Ipv4Addr::new(192, 168, 3, 38);
    let flow_matrix = crate::auth_7100_flow_matrix::analyze_auth_7100_flow_matrix(path)?;
    let session_meta = flow_matrix
        .sessions
        .into_iter()
        .map(|session| {
            (
                session.local_endpoint,
                (session.session_role, session.started_with_handshake),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let bytes = read_capture_file_as_pcap_bytes(path)?;
    let parsed = parse_pcap(&bytes)?;
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

    let mut out_sessions = Vec::new();
    let mut findings = Vec::new();
    for (local_endpoint, packets) in sessions {
        let client_stream = reassemble_stream(&packets.client_packets);
        let server_stream = reassemble_stream(&packets.server_packets);
        let local_endpoint_text = display_ep(local_endpoint);
        let (session_role, started_with_handshake) = session_meta
            .get(&local_endpoint_text)
            .cloned()
            .unwrap_or_else(|| ("unknown".to_string(), packets.started_with_handshake));

        let mut directions = Vec::new();
        if !client_stream.is_empty() {
            let subject = format!("方向 {} -> {}", local_endpoint_text, display_ep(server));
            let analysis = analyze_shell_direction_flow(
                path,
                local_endpoint,
                server,
                &client_stream,
                &subject,
            )?;
            findings.extend(analysis.findings.iter().cloned());
            directions.push(analysis);
        }
        if !server_stream.is_empty() {
            let subject = format!("方向 {} -> {}", display_ep(server), local_endpoint_text);
            let analysis = analyze_shell_direction_flow(
                path,
                server,
                local_endpoint,
                &server_stream,
                &subject,
            )?;
            findings.extend(analysis.findings.iter().cloned());
            directions.push(analysis);
        }

        out_sessions.push(Auth7100ShellSessionAnalysis {
            local_endpoint: local_endpoint_text,
            session_role,
            started_with_handshake,
            directions,
        });
    }

    out_sessions.sort_by(|left, right| left.local_endpoint.cmp(&right.local_endpoint));

    Ok(Auth7100ShellCorrelationAnalysis {
        pcap_path: path.display().to_string(),
        server_endpoint: display_ep(server),
        sessions: out_sessions,
        findings,
    })
}

fn analyze_client_shell_flow(
    path: &Path,
    src: Endpoint,
    dst: Endpoint,
    bytes: &[u8],
) -> Result<Auth7100ClientShellAnalysis, Box<dyn Error>> {
    let manifest_dir = crate::repository_fixture_path("");
    let (packets, packet_matches) = collect_flow_packets_and_matches(&manifest_dir, bytes, false)?;
    let findings = build_findings("客户端", &packet_matches);

    Ok(Auth7100ClientShellAnalysis {
        pcap_path: path.display().to_string(),
        source_endpoint: display_ep(src),
        destination_endpoint: display_ep(dst),
        tcp_payload_bytes: bytes.len(),
        packets,
        packet_matches,
        findings,
    })
}

fn analyze_shell_direction_flow(
    path: &Path,
    src: Endpoint,
    dst: Endpoint,
    bytes: &[u8],
    subject: &str,
) -> Result<Auth7100ShellDirectionAnalysis, Box<dyn Error>> {
    let manifest_dir = crate::repository_fixture_path("");
    let (packets, packet_matches) = collect_flow_packets_and_matches(&manifest_dir, bytes, true)?;
    let findings = build_findings(subject, &packet_matches);

    let _ = path;
    Ok(Auth7100ShellDirectionAnalysis {
        source_endpoint: display_ep(src),
        destination_endpoint: display_ep(dst),
        tcp_payload_bytes: bytes.len(),
        packets,
        packet_matches,
        findings,
    })
}

fn collect_flow_packets_and_matches(
    manifest_dir: &Path,
    bytes: &[u8],
    shell_only: bool,
) -> Result<
    (
        Vec<Auth7100ClientPacketAnalysis>,
        Vec<Auth7100ClientPacketMatches>,
    ),
    Box<dyn Error>,
> {
    let mut packets = Vec::new();
    let mut packet_bytes = Vec::new();
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

        let packet = &bytes[offset..offset + prefix.packet_len];
        let analysis = analyze_packet(offset, packet, &prefix);
        if !shell_only || is_shell_packet(&analysis) {
            packets.push(analysis);
            packet_bytes.push(packet.to_vec());
        }
        offset += prefix.packet_len;
    }

    let mut packet_matches = Vec::new();
    for (packet, packet_bytes) in packets.iter().zip(packet_bytes.iter()) {
        packet_matches.push(match_packet_candidates(manifest_dir, packet, packet_bytes)?);
    }

    Ok((packets, packet_matches))
}

fn build_findings(subject: &str, packet_matches: &[Auth7100ClientPacketMatches]) -> Vec<String> {
    let mut findings = Vec::new();

    for matches in packet_matches {
        if matches.exact_match_count > 0 {
            let exact_names = matches
                .exact_matches
                .iter()
                .take(4)
                .map(|item| {
                    Path::new(&item.candidate_path)
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| item.candidate_path.clone())
                })
                .collect::<Vec<_>>()
                .join(", ");
            findings.push(format!(
                "{subject} 上的 {len} 字节壳包已命中 {count} 份完全一致的 Windows dump：{exact_names}",
                subject = subject,
                len = matches.packet_len,
                count = matches.exact_match_count,
            ));
        }

        if let Some(best_same_size_near) = matches
            .top_matches
            .iter()
            .find(|item| !item.exact_match && item.same_size)
        {
            if best_same_size_near.diff_bytes > 0
                && best_same_size_near.diff_bytes <= 4
                && best_same_size_near.diff_bytes == best_same_size_near.zstd_block_body_diff_bytes
            {
                findings.push(format!(
                    "{subject} 上的 {len} 字节壳包同长度近邻 {name} 只差 {diff} 字节，且全部落在 zstd-like 块体内部{offset_hint}",
                    subject = subject,
                    len = matches.packet_len,
                    name = Path::new(&best_same_size_near.candidate_path)
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| best_same_size_near.candidate_path.clone()),
                    diff = best_same_size_near.diff_bytes,
                    offset_hint = best_same_size_near
                        .zstd_block_body_compare
                        .as_ref()
                        .and_then(|summary| summary.first_diff_offset)
                        .map(|offset| format!("；首个差异位于块体偏移 {offset}"))
                        .unwrap_or_default(),
                ));
            }
        }

        if let Some(best_near) = matches.top_matches.iter().find(|item| !item.exact_match) {
            if best_near.zstd_block_body_diff_bytes > 0
                && best_near.zstd_block_body_diff_bytes
                    > best_near.outer_header_diff_bytes + best_near.zstd_frame_header_diff_bytes
            {
                findings.push(format!(
                    "{subject} 上的 {len} 字节壳包最近似样本 {name} 主要差异仍落在 zstd-like 块体内部，而不是外层网络包头",
                    subject = subject,
                    len = matches.packet_len,
                    name = Path::new(&best_near.candidate_path)
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| best_near.candidate_path.clone()),
                ));
            }

            if let Some(summary) = &best_near.zstd_block_body_compare {
                if summary.diff_bytes > 0 {
                    findings.push(format!(
                        "{subject} 上的 {len} 字节壳包最近似样本 {name} 在 zstd-like 块体里仍保留前 {prefix} / 后 {suffix} 字节公共区，中间共有 {diff} 字节差异",
                        subject = subject,
                        len = matches.packet_len,
                        name = Path::new(&best_near.candidate_path)
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| best_near.candidate_path.clone()),
                        prefix = summary.equal_prefix_len,
                        suffix = summary.equal_suffix_len,
                        diff = summary.diff_bytes,
                    ));
                }
            }
        }
    }

    findings
}

fn is_shell_packet(packet: &Auth7100ClientPacketAnalysis) -> bool {
    packet.zstd_frame.is_some() || packet.outer_tail.contains("ZSTD")
}

fn match_packet_candidates(
    manifest_dir: &Path,
    packet: &Auth7100ClientPacketAnalysis,
    packet_bytes: &[u8],
) -> Result<Auth7100ClientPacketMatches, Box<dyn Error>> {
    let candidates = discover_candidate_files(manifest_dir, packet.packet_len)?;
    let layout = packet
        .zstd_frame
        .as_ref()
        .and_then(|frame| compute_zstd_layout(packet_bytes, frame.zstd_magic_offset));
    let mut matches = Vec::new();

    for candidate in candidates {
        let candidate_bytes = fs::read(&candidate)?;
        let compare_len = candidate_bytes.len().min(packet_bytes.len());
        let compare = compare_blob_bytes(
            candidate.display().to_string(),
            &candidate_bytes,
            format!(
                "client_packet_{}_{}",
                packet.packet_offset, packet.packet_len
            ),
            packet_bytes,
            0,
            0,
            Some(compare_len),
            8,
        )?;

        let (
            outer_header_diff_bytes,
            pre_zstd_metadata_diff_bytes,
            zstd_frame_header_diff_bytes,
            zstd_block_body_diff_bytes,
        ) = classify_diff_zones(
            &candidate_bytes[..compare_len],
            &packet_bytes[..compare_len],
            layout,
        );

        let same_size = candidate_bytes.len() == packet_bytes.len();
        matches.push(Auth7100DumpCandidateMatch {
            candidate_path: candidate.display().to_string(),
            candidate_size: candidate_bytes.len(),
            size_delta: candidate_bytes.len() as isize - packet_bytes.len() as isize,
            same_size,
            exact_match: same_size && compare.exact_match,
            equal_prefix_len: compare.equal_prefix_len,
            equal_suffix_len: compare.equal_suffix_len,
            diff_bytes: compare.diff_bytes,
            first_diff_offset: compare.first_diff_offset,
            first_diff_zone: compare
                .first_diff_offset
                .map(|offset| diff_zone_name(offset, layout).to_string()),
            outer_header_diff_bytes,
            pre_zstd_metadata_diff_bytes,
            zstd_frame_header_diff_bytes,
            zstd_block_body_diff_bytes,
            zstd_block_body_compare: summarize_zstd_block_body_compare(
                &candidate_bytes[..compare_len],
                &packet_bytes[..compare_len],
                layout,
            )?,
            diff_runs: compare.diff_runs,
        });
    }

    matches.sort_by(|left, right| {
        right
            .exact_match
            .cmp(&left.exact_match)
            .then_with(|| left.diff_bytes.cmp(&right.diff_bytes))
            .then_with(|| left.size_delta.abs().cmp(&right.size_delta.abs()))
            .then_with(|| right.equal_prefix_len.cmp(&left.equal_prefix_len))
            .then_with(|| left.candidate_path.cmp(&right.candidate_path))
    });

    let exact_matches = matches
        .iter()
        .filter(|item| item.exact_match)
        .take(MAX_MATCHES_PER_PACKET)
        .cloned()
        .collect::<Vec<_>>();
    let top_matches = matches
        .iter()
        .take(MAX_MATCHES_PER_PACKET)
        .cloned()
        .collect::<Vec<_>>();

    Ok(Auth7100ClientPacketMatches {
        packet_offset: packet.packet_offset,
        packet_len: packet.packet_len,
        candidate_count: matches.len(),
        exact_match_count: exact_matches.len(),
        exact_matches,
        top_matches,
    })
}

fn summarize_zstd_block_body_compare(
    left: &[u8],
    right: &[u8],
    layout: Option<ZstdLayout>,
) -> Result<Option<Auth7100ZoneCompareSummary>, Box<dyn Error>> {
    let Some(layout) = layout else {
        return Ok(None);
    };
    let compare_len = left.len().min(right.len());
    if layout.block_data_offset >= compare_len {
        return Ok(None);
    }

    let compare = compare_blob_bytes(
        "candidate_zstd_block_body".to_string(),
        left,
        "packet_zstd_block_body".to_string(),
        right,
        layout.block_data_offset,
        layout.block_data_offset,
        Some(compare_len - layout.block_data_offset),
        8,
    )?;

    Ok(Some(Auth7100ZoneCompareSummary {
        compare_len: compare.compare_len,
        equal_prefix_len: compare.equal_prefix_len,
        equal_suffix_len: compare.equal_suffix_len,
        diff_bytes: compare.diff_bytes,
        first_diff_offset: compare.first_diff_offset,
        diff_runs: compare.diff_runs,
    }))
}

fn classify_diff_zones(
    left: &[u8],
    right: &[u8],
    layout: Option<ZstdLayout>,
) -> (usize, usize, usize, usize) {
    let mut outer_header = 0usize;
    let mut pre_zstd_metadata = 0usize;
    let mut zstd_frame_header = 0usize;
    let mut zstd_block_body = 0usize;

    for (offset, (lhs, rhs)) in left.iter().zip(right.iter()).enumerate() {
        if lhs == rhs {
            continue;
        }
        match diff_zone_name(offset, layout) {
            "outer_netpacket_header" => outer_header += 1,
            "pre_zstd_metadata" => pre_zstd_metadata += 1,
            "zstd_frame_header" => zstd_frame_header += 1,
            _ => zstd_block_body += 1,
        }
    }

    (
        outer_header,
        pre_zstd_metadata,
        zstd_frame_header,
        zstd_block_body,
    )
}

fn diff_zone_name(offset: usize, layout: Option<ZstdLayout>) -> &'static str {
    if offset < NET_PACKET_PREFIX_LEN {
        return "outer_netpacket_header";
    }
    let Some(layout) = layout else {
        return "payload";
    };
    if offset < layout.zstd_magic_offset {
        return "pre_zstd_metadata";
    }
    if offset < layout.block_data_offset {
        return "zstd_frame_header";
    }
    "zstd_block_body"
}

fn discover_candidate_files(
    root: &Path,
    packet_len: usize,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut lens = vec![packet_len];
    if packet_len > 0 {
        lens.push(packet_len - 1);
    }
    lens.push(packet_len + 1);
    lens.sort_unstable();
    lens.dedup();

    let prefixes = lens
        .into_iter()
        .flat_map(|len| [format!("dump_{len}_"), format!("spawn_dump_{len}_")])
        .collect::<Vec<_>>();

    let mut out = Vec::new();
    let mut fallback = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(".bin") {
            continue;
        }
        if prefixes.iter().any(|prefix| name.starts_with(prefix)) {
            out.push(entry.path());
            continue;
        }

        let Some(candidate_len) = parse_candidate_len_from_name(&name) else {
            continue;
        };
        let delta = candidate_len.abs_diff(packet_len);
        if delta <= FALLBACK_MAX_SIZE_DELTA {
            fallback.push((delta, entry.path()));
        }
    }

    out.sort();
    if !out.is_empty() {
        return Ok(out);
    }

    fallback.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    Ok(fallback
        .into_iter()
        .take(FALLBACK_MAX_CANDIDATES)
        .map(|(_, path)| path)
        .collect())
}

fn parse_candidate_len_from_name(name: &str) -> Option<usize> {
    let trimmed = name
        .strip_prefix("spawn_dump_")
        .or_else(|| name.strip_prefix("dump_"))?;
    let (len_text, _) = trimmed.split_once('_')?;
    len_text.parse::<usize>().ok()
}

fn analyze_packet(
    packet_offset: usize,
    packet_bytes: &[u8],
    prefix: &NetPacketPrefix,
) -> Auth7100ClientPacketAnalysis {
    let utf16_strings = scan_utf16_strings(packet_bytes, 2);
    let ascii_strings = scan_ascii_strings(packet_bytes, 4);
    let zstd_frame = find_zstd(packet_bytes).map(|zstd_magic_offset| {
        summarize_zstd_frame(&packet_bytes[zstd_magic_offset..], zstd_magic_offset)
    });

    Auth7100ClientPacketAnalysis {
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
        utf16_strings,
        ascii_strings,
        zstd_frame,
    }
}

fn classify_packet(prefix: &NetPacketPrefix) -> String {
    if prefix.tail.contains("ZSTD") {
        return "zstd_dict_envelope".to_string();
    }
    "unknown".to_string()
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

fn compute_zstd_layout(bytes: &[u8], zstd_magic_offset: usize) -> Option<ZstdLayout> {
    let zstd = bytes.get(zstd_magic_offset..)?;
    if zstd.len() < 8 {
        return None;
    }
    let frame_descriptor = *zstd.get(4)?;
    let single_segment = frame_descriptor & 0x20 != 0;
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
    if !single_segment && zstd.len() > cursor {
        cursor += 1;
    }
    cursor += dictionary_id_size;
    cursor += frame_content_size_size;

    if zstd.len() < cursor + 3 {
        return None;
    }

    Some(ZstdLayout {
        zstd_magic_offset,
        block_data_offset: zstd_magic_offset + cursor + 3,
    })
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

fn reassemble_tcp_payload(
    pcap_path: &Path,
    src: Endpoint,
    dst: Endpoint,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let bytes = read_capture_file_as_pcap_bytes(pcap_path)?;
    let packets = parse_pcap(&bytes)?;

    let mut seen = HashSet::new();
    let mut selected = Vec::new();
    for packet in packets {
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
        if packet.src == src && packet.dst == dst && !packet.captured_payload.is_empty() {
            selected.push(packet);
        }
    }

    if selected.is_empty() {
        return Err(format!(
            "no TCP payload packets for {} -> {}",
            display_ep(src),
            display_ep(dst)
        )
        .into());
    }

    selected.sort_by_key(|packet| packet.seq);
    let first_seq = selected[0].seq;
    let mut stream = Vec::new();
    let mut expected_seq = first_seq;

    for packet in &selected {
        let payload = &packet.captured_payload;
        let packet_start = packet.seq;
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

    Ok(stream)
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
        let payload = &packet.captured_payload;
        let packet_start = packet.seq;
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
    use super::{
        analyze_auth_7100_client_shell_sample, analyze_auth_7100_shell_correlation_sample,
    };

    #[test]
    fn finds_exact_dump_matches_for_client_shell_packets() {
        let analysis = analyze_auth_7100_client_shell_sample().expect("client shell analysis");
        assert_eq!(analysis.packets.len(), 3);

        let packet271 = analysis
            .packet_matches
            .iter()
            .find(|packet| packet.packet_len == 271)
            .expect("271 packet matches");
        assert!(packet271.exact_match_count >= 2);
        assert!(packet271.exact_matches.iter().any(|candidate| {
            candidate
                .candidate_path
                .ends_with("spawn_dump_271_1774689981.bin")
        }));

        let packet333 = analysis
            .packet_matches
            .iter()
            .find(|packet| packet.packet_len == 333)
            .expect("333 packet matches");
        assert!(packet333.exact_match_count >= 2);
        assert!(packet333.exact_matches.iter().any(|candidate| {
            candidate
                .candidate_path
                .ends_with("spawn_dump_333_1774689982.bin")
        }));

        let packet271_same_size_near = packet271
            .top_matches
            .iter()
            .find(|candidate| candidate.same_size && !candidate.exact_match)
            .expect("same-size near match for 271");
        assert_eq!(packet271_same_size_near.diff_bytes, 1);
        assert_eq!(packet271_same_size_near.zstd_block_body_diff_bytes, 1);
        let block_compare = packet271_same_size_near
            .zstd_block_body_compare
            .as_ref()
            .expect("271 block compare");
        assert_eq!(block_compare.diff_bytes, 1);
        assert_eq!(block_compare.first_diff_offset, Some(58));
    }

    #[test]
    fn summarizes_shell_correlation_for_both_login_directions() {
        let analysis = analyze_auth_7100_shell_correlation_sample().expect("shell correlation");
        assert_eq!(analysis.sessions.len(), 2);

        let login = analysis
            .sessions
            .iter()
            .find(|session| session.session_role == "auth_login")
            .expect("login session");
        assert_eq!(login.directions.len(), 2);

        let client = login
            .directions
            .iter()
            .find(|direction| direction.source_endpoint.ends_with(":2697"))
            .expect("client direction");
        assert_eq!(
            client
                .packets
                .iter()
                .map(|packet| packet.packet_len)
                .collect::<Vec<_>>(),
            vec![611, 271, 333]
        );

        let server = login
            .directions
            .iter()
            .find(|direction| direction.source_endpoint.ends_with(":7100"))
            .expect("server direction");
        assert_eq!(
            server
                .packets
                .iter()
                .map(|packet| packet.packet_len)
                .collect::<Vec<_>>(),
            vec![443, 395]
        );
        assert_eq!(server.packet_matches.len(), 2);
        assert!(
            server
                .packet_matches
                .iter()
                .all(|packet| packet.candidate_count > 0)
        );

        let server395 = server
            .packet_matches
            .iter()
            .find(|packet| packet.packet_len == 395)
            .expect("395 packet matches");
        let best_near = server395
            .top_matches
            .iter()
            .find(|candidate| !candidate.exact_match)
            .expect("395 near match");
        let block_compare = best_near
            .zstd_block_body_compare
            .as_ref()
            .expect("395 block compare");
        assert!(block_compare.compare_len > 0);
        assert!(block_compare.diff_bytes > 0);
    }
}
