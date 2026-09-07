use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::io::Read;
use std::net::Ipv4Addr;
use std::path::Path;

use serde::Serialize;

const NET_PACKET_PREFIX_LEN: usize = 74;
const MAX_CANDIDATE_DECODE_LEN: u64 = 8 * 1024 * 1024;

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

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Official5188PcapFlow {
    pub src: String,
    pub dst: String,
    pub first_payload_at_micros: Option<u64>,
    pub last_payload_at_micros: Option<u64>,
    pub frame_count: usize,
    pub client_frames: usize,
    pub server_frames: usize,
    pub unknown_frames: usize,
    pub kind_counts: BTreeMap<String, usize>,
    pub wire_kind_counts: BTreeMap<String, usize>,
    pub payload_hint_counts: BTreeMap<String, usize>,
    pub zstd_decode_successes: usize,
    pub zstd_decode_lengths: Vec<usize>,
    pub zlib_decode_successes: usize,
    pub zlib_decode_lengths: Vec<usize>,
    pub embedded_zlib_candidates: Vec<netzip_fullpull::Official5188EmbeddedZlibCandidate>,
    pub embedded_zlib_by_kind: BTreeMap<String, usize>,
    pub zlib_object_by_kind: BTreeMap<String, usize>,
    pub zlib_object_decoded_lengths: Vec<usize>,
    pub zlib_object_samples: Vec<Official5188ZlibObjectSample>,
    pub delta_envelope_count: usize,
    pub delta_headers: Vec<Official5188DeltaHeader>,
    pub delta_samples: Vec<Official5188DeltaSample>,
    pub bulk_envelope_count: usize,
    pub bulk_envelope_headers: Vec<[u32; 3]>,
    pub subscription_frame_count: usize,
    pub subscription_entry_count: usize,
    pub subscription_ranges: Vec<netzip_fullpull::Official5188SubscriptionRange>,
    pub frame_samples: Vec<Official5188FrameSample>,
    pub trailing_bytes: usize,
    pub reassembly_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Official5188FrameSample {
    pub completed_at_micros: Option<u64>,
    pub kind: String,
    pub wire_kind: String,
    pub metadata_hex: String,
    pub payload_len: usize,
    pub payload_head_hex: String,
    pub zlib_object_uncompressed_len: Option<usize>,
    pub zlib_object_compressed_len: Option<usize>,
    pub delta_header: Option<Official5188DeltaHeader>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Official5188ZlibObjectSample {
    pub kind: String,
    pub uncompressed_len: usize,
    pub compressed_len: usize,
    pub decoded_head_hex: String,
    pub six_digit_code_samples: Vec<Official5188DecodedCodeSample>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Official5188DecodedCodeSample {
    pub offset: usize,
    pub code: String,
    pub following_68_hex: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Official5188DeltaSample {
    pub completed_at_micros: Option<u64>,
    pub payload_len: usize,
    pub header: Official5188DeltaHeader,
    pub body_81_count: usize,
    pub body_head_hex: String,
    pub body_hex: String,
    pub value_stream_len: Option<usize>,
    pub index_stream_len: Option<usize>,
    pub index_stream_head_hex: Option<String>,
    pub decoded_index_count: Option<usize>,
    pub decoded_index_head: Vec<netzip_fullpull::Official5188DeltaIndexState>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct Official5188DeltaHeader {
    pub record_count: u16,
    pub value_end_offset: u32,
}

/// Complete evidence retained for one reassembled 5188 application frame.
///
/// Unlike [`Official5188FrameSample`], this type is intended for offline
/// protocol reconstruction and therefore retains the complete payload and
/// any length-closed zlib object. Production status endpoints should continue
/// to use the bounded summary API.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Official5188CapturedFrame {
    pub src: String,
    pub dst: String,
    pub frame_index: usize,
    pub completed_at_micros: Option<u64>,
    pub kind: String,
    pub wire_kind: String,
    pub metadata: [u8; 4],
    pub payload: Vec<u8>,
    pub decoded_zlib_object: Option<Vec<u8>>,
    pub code_table: Option<netzip_fullpull::Official5188CodeTable>,
    pub delta_value_stream: Option<Vec<u8>>,
    pub delta_index_stream: Option<Vec<u8>>,
    pub delta_indexes: Option<Vec<netzip_fullpull::Official5188DeltaIndexState>>,
}

/// Extracts complete 5188 frames for offline Wine/callback parity work.
///
/// TCP retransmissions are removed and segments are reassembled with the same
/// rules as [`scan_official_5188_pcap`]. Flows with a sequence gap or malformed
/// application frame are omitted. A trailing partial frame caused by capture
/// shutdown is ignored; only frames already closed by their declared lengths
/// are returned, so partial bytes cannot become fixtures.
pub fn extract_official_5188_frames(
    path: impl AsRef<Path>,
) -> Result<Vec<Official5188CapturedFrame>, Box<dyn Error>> {
    let bytes = crate::capture_input::read_capture_file_as_pcap_bytes(path)?;
    let packets = parse_pcap(&bytes)?;
    let mut flows: HashMap<(Endpoint, Endpoint), Vec<TcpPacket>> = HashMap::new();
    for packet in dedupe_packets(packets).into_iter().filter(|packet| {
        (packet.src.port == 5188 || packet.dst.port == 5188) && !packet.captured_payload.is_empty()
    }) {
        flows
            .entry((packet.src, packet.dst))
            .or_default()
            .push(packet);
    }

    let mut output = Vec::new();
    for ((src, dst), segments) in flows {
        let (frames, completion_times, _trailing_bytes, reassembly_error) =
            reassemble_official_5188_segments(segments);
        if reassembly_error.is_some() {
            continue;
        }
        for (frame_index, (frame, completed_at_micros)) in
            frames.into_iter().zip(completion_times).enumerate()
        {
            let decoded_zlib_object =
                netzip_fullpull::Official5188ZlibObjectEnvelope::decode(&frame)
                    .ok()
                    .map(|object| object.decoded);
            let code_table = decoded_zlib_object
                .as_deref()
                .and_then(|decoded| netzip_fullpull::Official5188CodeTable::decode(decoded).ok());
            let (delta_value_stream, delta_index_stream, delta_indexes) =
                match netzip_fullpull::Official5188DeltaEnvelope::decode(&frame).and_then(
                    |envelope| {
                        let streams = envelope.split_streams()?;
                        let indexes = netzip_fullpull::decode_official_5188_delta_indexes(streams)?;
                        Ok((
                            streams.value_stream.to_vec(),
                            streams.index_stream.to_vec(),
                            indexes,
                        ))
                    },
                ) {
                    Ok((value, index, indexes)) => (Some(value), Some(index), Some(indexes)),
                    Err(_) => (None, None, None),
                };
            output.push(Official5188CapturedFrame {
                src: src.to_string(),
                dst: dst.to_string(),
                frame_index,
                completed_at_micros,
                kind: frame.kind.to_string(),
                wire_kind: frame.kind.wire_hex(),
                metadata: frame.metadata,
                payload: frame.payload,
                decoded_zlib_object,
                code_table,
                delta_value_stream,
                delta_index_stream,
                delta_indexes,
            });
        }
    }
    output.sort_by(|a, b| {
        a.completed_at_micros
            .cmp(&b.completed_at_micros)
            .then_with(|| a.src.cmp(&b.src))
            .then_with(|| a.dst.cmp(&b.dst))
            .then_with(|| a.frame_index.cmp(&b.frame_index))
    });
    Ok(output)
}

fn reassemble_official_5188_segments(
    mut segments: Vec<TcpPacket>,
) -> (
    Vec<netzip_fullpull::Official5188Frame>,
    Vec<Option<u64>>,
    usize,
    Option<String>,
) {
    segments.sort_by_key(|packet| packet.seq);
    let mut stream = Vec::new();
    let mut stream_completion_times = Vec::new();
    let mut expected = None;
    let mut sequence_gap = false;
    for segment in segments {
        let start = segment.seq;
        let end = start.wrapping_add(segment.captured_payload.len() as u32);
        let next = expected.unwrap_or(start);
        if start >= next {
            if start > next {
                sequence_gap = true;
                break;
            }
            stream.extend_from_slice(&segment.captured_payload);
            stream_completion_times.push((stream.len(), packet_micros(segment.ts)));
            expected = Some(end);
        } else {
            let overlap = (next - start) as usize;
            if overlap < segment.captured_payload.len() {
                stream.extend_from_slice(&segment.captured_payload[overlap..]);
                stream_completion_times.push((stream.len(), packet_micros(segment.ts)));
                expected = Some(end.max(next));
            }
        }
    }
    let mut decoder = netzip_fullpull::Official5188Reassembler::new();
    let (frames, reassembly_error) = match decoder.push(&stream) {
        Ok(frames) if sequence_gap => (
            frames,
            Some("TCP sequence gap before next segment".to_string()),
        ),
        Ok(frames) => (frames, None),
        Err(error) => (Vec::new(), Some(error)),
    };
    let trailing_bytes = decoder.buffered_len();
    let mut frame_end_offset = 0usize;
    let completion_times = frames
        .iter()
        .map(|frame| {
            frame_end_offset += 8 + frame.payload.len();
            stream_completion_times
                .iter()
                .find(|(end_offset, _)| *end_offset >= frame_end_offset)
                .map(|(_, timestamp)| *timestamp)
        })
        .collect();
    (frames, completion_times, trailing_bytes, reassembly_error)
}

/// Reassembles complete 5188 application frames from a pcap/pcapng capture.
/// Payloads remain opaque; this is an evidence and boundary tool only.
pub fn scan_official_5188_pcap(
    path: impl AsRef<Path>,
) -> Result<Vec<Official5188PcapFlow>, Box<dyn Error>> {
    let bytes = crate::capture_input::read_capture_file_as_pcap_bytes(path)?;
    let packets = parse_pcap(&bytes)?;
    let mut flows: HashMap<(Endpoint, Endpoint), Vec<TcpPacket>> = HashMap::new();
    for packet in dedupe_packets(packets).into_iter().filter(|packet| {
        (packet.src.port == 5188 || packet.dst.port == 5188) && !packet.captured_payload.is_empty()
    }) {
        flows
            .entry((packet.src, packet.dst))
            .or_default()
            .push(packet);
    }

    let mut output = Vec::new();
    for ((src, dst), mut segments) in flows {
        let first_payload_at_micros = segments
            .iter()
            .map(|segment| (segment.ts.max(0.0) * 1_000_000.0).round() as u64)
            .min();
        let last_payload_at_micros = segments
            .iter()
            .map(|segment| (segment.ts.max(0.0) * 1_000_000.0).round() as u64)
            .max();
        segments.sort_by_key(|packet| packet.seq);
        let mut stream = Vec::new();
        let mut stream_completion_times = Vec::new();
        let mut expected = None;
        let mut sequence_gap = false;
        for segment in segments {
            let start = segment.seq;
            let end = start.wrapping_add(segment.captured_payload.len() as u32);
            let next = expected.unwrap_or(start);
            if start >= next {
                if start > next {
                    sequence_gap = true;
                    break;
                }
                stream.extend_from_slice(&segment.captured_payload);
                stream_completion_times.push((stream.len(), packet_micros(segment.ts)));
                expected = Some(end);
            } else {
                let overlap = (next - start) as usize;
                if overlap < segment.captured_payload.len() {
                    stream.extend_from_slice(&segment.captured_payload[overlap..]);
                    stream_completion_times.push((stream.len(), packet_micros(segment.ts)));
                    expected = Some(end.max(next));
                }
            }
        }
        let mut decoder = netzip_fullpull::Official5188Reassembler::new();
        let (frames, reassembly_error) = match decoder.push(&stream) {
            Ok(frames) if sequence_gap => (
                frames,
                Some("TCP sequence gap before next segment".to_string()),
            ),
            Ok(frames) => (frames, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let trailing_bytes = decoder.buffered_len();
        let mut kind_counts = BTreeMap::new();
        let mut wire_kind_counts = BTreeMap::new();
        let mut payload_hint_counts = BTreeMap::new();
        let mut zstd_decode_lengths = Vec::new();
        let mut zlib_decode_lengths = Vec::new();
        let mut zstd_decode_successes = 0;
        let mut zlib_decode_successes = 0;
        let mut embedded_zlib_candidates = Vec::new();
        let mut embedded_zlib_by_kind = BTreeMap::new();
        let mut zlib_object_by_kind = BTreeMap::new();
        let mut zlib_object_decoded_lengths = Vec::new();
        let mut zlib_object_samples = Vec::new();
        let mut delta_envelope_count = 0;
        let mut delta_headers = Vec::new();
        let mut delta_samples = Vec::new();
        let mut bulk_envelope_count = 0;
        let mut bulk_envelope_headers = Vec::new();
        let mut subscription_frame_count = 0;
        let mut subscription_entry_count = 0;
        let mut subscription_ranges = Vec::new();
        let mut frame_end_offset = 0usize;
        let frame_completion_times = frames
            .iter()
            .map(|frame| {
                frame_end_offset += 8 + frame.payload.len();
                stream_completion_times
                    .iter()
                    .find(|(end_offset, _)| *end_offset >= frame_end_offset)
                    .map(|(_, timestamp)| *timestamp)
            })
            .collect::<Vec<_>>();
        let frame_samples = frames
            .iter()
            .zip(&frame_completion_times)
            .take(128)
            .map(|(frame, completed_at_micros)| {
                let zlib_object =
                    netzip_fullpull::Official5188ZlibObjectEnvelope::decode(frame).ok();
                let delta = netzip_fullpull::Official5188DeltaEnvelope::decode(frame).ok();
                Official5188FrameSample {
                    completed_at_micros: *completed_at_micros,
                    kind: frame.kind.to_string(),
                    wire_kind: frame.kind.wire_hex(),
                    metadata_hex: hex_prefix(&frame.metadata, 4),
                    payload_len: frame.payload.len(),
                    payload_head_hex: hex_prefix(&frame.payload, 32),
                    zlib_object_uncompressed_len: zlib_object
                        .as_ref()
                        .map(|object| object.uncompressed_len),
                    zlib_object_compressed_len: zlib_object
                        .as_ref()
                        .map(|object| object.compressed_len),
                    delta_header: delta.as_ref().map(|envelope| Official5188DeltaHeader {
                        record_count: envelope.record_count,
                        value_end_offset: envelope.value_end_offset,
                    }),
                }
            })
            .collect();
        let mut client_frames = 0;
        let mut server_frames = 0;
        let mut unknown_frames = 0;
        for (frame, completed_at_micros) in frames.iter().zip(&frame_completion_times) {
            *kind_counts.entry(frame.kind.to_string()).or_insert(0) += 1;
            *wire_kind_counts.entry(frame.kind.wire_hex()).or_insert(0) += 1;
            let hint = netzip_fullpull::payload_hint(&frame.payload);
            let hint_name = match hint {
                netzip_fullpull::Official5188PayloadHint::ZstdMagic => "zstd-magic",
                netzip_fullpull::Official5188PayloadHint::ZlibMagic => "zlib-magic",
                netzip_fullpull::Official5188PayloadHint::Opaque => "opaque",
            };
            *payload_hint_counts
                .entry(hint_name.to_string())
                .or_insert(0) += 1;
            match hint {
                netzip_fullpull::Official5188PayloadHint::ZstdMagic => {
                    if let Ok(decoded) = bounded_zstd_decode(&frame.payload) {
                        zstd_decode_successes += 1;
                        if zstd_decode_lengths.len() < 16 {
                            zstd_decode_lengths.push(decoded.len());
                        }
                    }
                }
                netzip_fullpull::Official5188PayloadHint::ZlibMagic => {
                    if let Ok(decoded) = bounded_zlib_decode(&frame.payload) {
                        zlib_decode_successes += 1;
                        if zlib_decode_lengths.len() < 16 {
                            zlib_decode_lengths.push(decoded.len());
                        }
                    }
                }
                netzip_fullpull::Official5188PayloadHint::Opaque => {}
            }
            for candidate in netzip_fullpull::embedded_zlib_candidates(&frame.payload) {
                *embedded_zlib_by_kind
                    .entry(frame.kind.to_string())
                    .or_insert(0) += 1;
                if embedded_zlib_candidates.len() < 16
                    && !embedded_zlib_candidates.contains(&candidate)
                {
                    embedded_zlib_candidates.push(candidate);
                }
            }
            if let Ok(object) = netzip_fullpull::Official5188ZlibObjectEnvelope::decode(frame) {
                *zlib_object_by_kind
                    .entry(frame.kind.to_string())
                    .or_insert(0) += 1;
                if zlib_object_decoded_lengths.len() < 16 {
                    zlib_object_decoded_lengths.push(object.decoded.len());
                }
                if zlib_object_samples.len() < 16 {
                    zlib_object_samples.push(Official5188ZlibObjectSample {
                        kind: frame.kind.to_string(),
                        uncompressed_len: object.uncompressed_len,
                        compressed_len: object.compressed_len,
                        decoded_head_hex: hex_prefix(&object.decoded, 32),
                        six_digit_code_samples: six_digit_code_samples(&object.decoded, 16),
                    });
                }
            }
            match frame.direction() {
                netzip_fullpull::Official5188Direction::Client => client_frames += 1,
                netzip_fullpull::Official5188Direction::Server => server_frames += 1,
                netzip_fullpull::Official5188Direction::Unknown => unknown_frames += 1,
            }
            if let Ok(envelope) = netzip_fullpull::Official5188BulkEnvelope::decode(frame) {
                bulk_envelope_count += 1;
                if bulk_envelope_headers.len() < 16 {
                    bulk_envelope_headers.push([
                        envelope.block_offset,
                        envelope.header_word,
                        envelope.sequence_word,
                    ]);
                }
            }
            if let Ok(envelope) = netzip_fullpull::Official5188DeltaEnvelope::decode(frame) {
                let streams = envelope.split_streams().ok();
                let decoded_indexes = streams.and_then(|streams| {
                    netzip_fullpull::decode_official_5188_delta_indexes(streams).ok()
                });
                delta_envelope_count += 1;
                if delta_headers.len() < 16 {
                    let header = Official5188DeltaHeader {
                        record_count: envelope.record_count,
                        value_end_offset: envelope.value_end_offset,
                    };
                    delta_headers.push(header);
                    delta_samples.push(Official5188DeltaSample {
                        completed_at_micros: *completed_at_micros,
                        payload_len: frame.payload.len(),
                        header,
                        body_81_count: envelope.body.iter().filter(|byte| **byte == 0x81).count(),
                        body_head_hex: hex_prefix(&envelope.body, 64),
                        body_hex: hex_prefix(&envelope.body, 8 * 1024),
                        value_stream_len: streams
                            .as_ref()
                            .map(|streams| streams.value_stream.len()),
                        index_stream_len: streams
                            .as_ref()
                            .map(|streams| streams.index_stream.len()),
                        index_stream_head_hex: streams
                            .as_ref()
                            .map(|streams| hex_prefix(streams.index_stream, 64)),
                        decoded_index_count: decoded_indexes.as_ref().map(Vec::len),
                        decoded_index_head: decoded_indexes
                            .unwrap_or_default()
                            .into_iter()
                            .take(16)
                            .collect(),
                    });
                }
            }
            if let Ok(subscription) =
                netzip_fullpull::Official5188SubscriptionEnvelope::decode(frame)
            {
                subscription_frame_count += 1;
                subscription_entry_count += subscription.entries.len();
                subscription_ranges.extend(subscription.consecutive_ranges());
            }
        }
        output.push(Official5188PcapFlow {
            src: src.to_string(),
            dst: dst.to_string(),
            first_payload_at_micros,
            last_payload_at_micros,
            frame_count: frames.len(),
            client_frames,
            server_frames,
            unknown_frames,
            kind_counts,
            wire_kind_counts,
            payload_hint_counts,
            zstd_decode_successes,
            zstd_decode_lengths,
            zlib_decode_successes,
            zlib_decode_lengths,
            embedded_zlib_candidates,
            embedded_zlib_by_kind,
            zlib_object_by_kind,
            zlib_object_decoded_lengths,
            zlib_object_samples,
            delta_envelope_count,
            delta_headers,
            delta_samples,
            bulk_envelope_count,
            bulk_envelope_headers,
            subscription_frame_count,
            subscription_entry_count,
            subscription_ranges,
            frame_samples,
            trailing_bytes,
            reassembly_error,
        });
    }
    output.sort_by(|a, b| a.src.cmp(&b.src).then_with(|| a.dst.cmp(&b.dst)));
    Ok(output)
}

fn packet_micros(timestamp: f64) -> u64 {
    (timestamp.max(0.0) * 1_000_000.0).round() as u64
}

fn six_digit_code_samples(bytes: &[u8], limit: usize) -> Vec<Official5188DecodedCodeSample> {
    let mut samples = Vec::new();
    for (offset, window) in bytes.windows(6).enumerate() {
        if samples.len() >= limit {
            break;
        }
        if window.iter().all(u8::is_ascii_digit)
            && (offset == 0 || !bytes[offset - 1].is_ascii_digit())
            && (offset + 6 == bytes.len() || !bytes[offset + 6].is_ascii_digit())
        {
            samples.push(Official5188DecodedCodeSample {
                offset,
                code: String::from_utf8_lossy(window).into_owned(),
                following_68_hex: hex_prefix(&bytes[offset..], 68),
            });
        }
    }
    samples
}

fn bounded_zstd_decode(payload: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let decoder = zstd::stream::read::Decoder::new(std::io::Cursor::new(payload))?;
    let mut decoded = Vec::new();
    decoder
        .take(MAX_CANDIDATE_DECODE_LEN + 1)
        .read_to_end(&mut decoded)?;
    if decoded.len() as u64 > MAX_CANDIDATE_DECODE_LEN {
        return Err("zstd candidate exceeds bounded decode length".into());
    }
    Ok(decoded)
}

fn bounded_zlib_decode(payload: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let decoder = flate2::read::ZlibDecoder::new(std::io::Cursor::new(payload));
    let mut decoded = Vec::new();
    decoder
        .take(MAX_CANDIDATE_DECODE_LEN + 1)
        .read_to_end(&mut decoded)?;
    if decoded.len() as u64 > MAX_CANDIDATE_DECODE_LEN {
        return Err("zlib candidate exceeds bounded decode length".into());
    }
    Ok(decoded)
}

fn dedupe_packets(packets: Vec<TcpPacket>) -> Vec<TcpPacket> {
    let mut seen = HashSet::new();
    packets
        .into_iter()
        .filter(|packet| {
            seen.insert(PacketIdentity {
                src: packet.src,
                dst: packet.dst,
                seq: packet.seq,
                ack: packet.ack,
                flags: packet.flags,
                wire_payload_len: packet.wire_payload_len,
                captured_payload: packet.captured_payload.clone(),
            })
        })
        .collect()
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
    // Real vendor captures are pcapng; normalize them through the shared
    // converter before applying the classic-pcap parser below.
    let bytes = crate::capture_input::read_capture_file_as_pcap_bytes(path)?;
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
    let link_offset = find_ipv4_tcp_offset(frame)?;
    if frame.len() < link_offset + 20 + 20 {
        return None;
    }

    let ihl = ((frame[link_offset] & 0x0f) as usize) * 4;
    if ihl < 20 || frame.len() < link_offset + ihl + 20 || frame[link_offset + 9] != 6 {
        return None;
    }

    let ip_total_len =
        u16::from_be_bytes([frame[link_offset + 2], frame[link_offset + 3]]) as usize;
    let src = Endpoint {
        ip: Ipv4Addr::new(
            frame[link_offset + 12],
            frame[link_offset + 13],
            frame[link_offset + 14],
            frame[link_offset + 15],
        ),
        port: 0,
    };
    let dst = Endpoint {
        ip: Ipv4Addr::new(
            frame[link_offset + 16],
            frame[link_offset + 17],
            frame[link_offset + 18],
            frame[link_offset + 19],
        ),
        port: 0,
    };

    let tcp_start = link_offset + ihl;
    let (src_port, dst_port) = (
        u16::from_be_bytes([frame[tcp_start], frame[tcp_start + 1]]),
        u16::from_be_bytes([frame[tcp_start + 2], frame[tcp_start + 3]]),
    );
    let src = Endpoint {
        port: src_port,
        ..src
    };
    let dst = Endpoint {
        port: dst_port,
        ..dst
    };
    let tcp_header_len = (((frame[tcp_start + 12] >> 4) & 0x0f) as usize) * 4;
    if tcp_header_len < 20 || frame.len() < tcp_start + tcp_header_len {
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
    let payload_end = frame.len().min(link_offset + ip_total_len);
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

fn find_ipv4_tcp_offset(frame: &[u8]) -> Option<usize> {
    let max_offset = frame.len().saturating_sub(40).min(64);
    (0..=max_offset).find(|offset| {
        let ip = &frame[*offset..];
        if ip.len() < 40 || ip[0] >> 4 != 4 || ip[9] != 6 {
            return false;
        }
        let ihl = usize::from(ip[0] & 0x0f) * 4;
        if ihl < 20 || ip.len() < ihl + 20 {
            return false;
        }
        let total_len = usize::from(u16::from_be_bytes([ip[2], ip[3]]));
        total_len >= ihl + 20 && total_len <= ip.len()
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

    use super::{Endpoint, TcpPacket, dedupe_packets, summarize_pcap_file};

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

    #[test]
    fn deduplicates_identical_5188_segments() {
        let packet = TcpPacket {
            ts: 1.0,
            src: Endpoint {
                ip: "192.0.2.1".parse().unwrap(),
                port: 5188,
            },
            dst: Endpoint {
                ip: "192.0.2.2".parse().unwrap(),
                port: 40000,
            },
            seq: 10,
            ack: 20,
            flags: 0x18,
            incl_len: 0,
            orig_len: 0,
            wire_payload_len: 8,
            captured_payload: vec![1, 2, 3, 4],
        };
        let unique = dedupe_packets(vec![packet.clone(), packet]);
        assert_eq!(unique.len(), 1);
    }

    #[test]
    fn parses_bare_ipv4_tcp_frame_used_by_pktmon() {
        let mut frame = vec![0u8; 40];
        frame[0] = 0x45;
        frame[2..4].copy_from_slice(&(40u16).to_be_bytes());
        frame[9] = 6;
        frame[12..16].copy_from_slice(&[192, 0, 2, 1]);
        frame[16..20].copy_from_slice(&[192, 0, 2, 2]);
        frame[20..22].copy_from_slice(&(5188u16).to_be_bytes());
        frame[22..24].copy_from_slice(&(40000u16).to_be_bytes());
        frame[32] = 0x50;
        let parsed = super::parse_tcp_packet(&frame, 40, 40, 0, 0, super::TimestampScale::Micros)
            .expect("bare IPv4 TCP frame");
        assert_eq!(parsed.src.port, 5188);
        assert_eq!(parsed.dst.port, 40000);
    }

    #[test]
    fn parses_ipv4_tcp_after_pktmon_metadata_prefix() {
        let mut frame = vec![0xa5u8; 24];
        frame.extend_from_slice(&[0x45, 0, 0, 40, 0, 0, 0, 0, 64, 6, 0, 0]);
        frame.extend_from_slice(&[192, 0, 2, 1, 192, 0, 2, 2]);
        frame.extend_from_slice(&5188u16.to_be_bytes());
        frame.extend_from_slice(&40000u16.to_be_bytes());
        frame.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 2, 0x50, 0x18, 0, 0, 0, 0, 0, 0]);
        let parsed = super::parse_tcp_packet(
            &frame,
            frame.len() as u32,
            frame.len() as u32,
            0,
            0,
            super::TimestampScale::Micros,
        )
        .expect("prefixed bare IPv4 TCP frame");
        assert_eq!(parsed.src.port, 5188);
        assert_eq!(parsed.dst.port, 40000);
    }

    #[test]
    fn rejects_tcp_header_shorter_than_minimum() {
        let mut frame = vec![0u8; 40];
        frame[0] = 0x45;
        frame[2..4].copy_from_slice(&(40u16).to_be_bytes());
        frame[9] = 6;
        frame[20..22].copy_from_slice(&(5188u16).to_be_bytes());
        frame[22..24].copy_from_slice(&(40000u16).to_be_bytes());
        frame[32] = 0x40;
        assert!(
            super::parse_tcp_packet(&frame, 40, 40, 0, 0, super::TimestampScale::Micros).is_none()
        );
    }

    #[test]
    fn bounds_candidate_zstd_expansion() {
        let source = vec![0u8; (super::MAX_CANDIDATE_DECODE_LEN + 1) as usize];
        let compressed = zstd::stream::encode_all(std::io::Cursor::new(source), 1)
            .expect("compress candidate fixture");
        assert!(super::bounded_zstd_decode(&compressed).is_err());
    }

    #[test]
    fn samples_only_standalone_six_digit_codes_with_offsets() {
        let samples = super::six_digit_code_samples(b"x600000\0y1234567z000001", 8);
        assert_eq!(
            samples,
            vec![
                super::Official5188DecodedCodeSample {
                    offset: 1,
                    code: "600000".to_string(),
                    following_68_hex: super::hex_prefix(&b"600000\0y1234567z000001"[..], 68),
                },
                super::Official5188DecodedCodeSample {
                    offset: 17,
                    code: "000001".to_string(),
                    following_68_hex: super::hex_prefix(b"000001", 68),
                },
            ]
        );
    }

    #[test]
    #[ignore = "large forensic fixture; run explicitly when capture analysis is requested"]
    fn scans_vendor_pm_5188_capture() {
        let path = crate::repository_fixture_path(
            "diagnostics/20260831-netzip-windows-vs-rust/captures/vendor_pm.pcapng",
        );
        let flows = super::scan_official_5188_pcap(path).expect("scan vendor 5188 capture");
        assert!(!flows.is_empty(), "expected at least one 5188 flow");
        let server_flow = flows
            .iter()
            .find(|flow| flow.src.ends_with(":5188") && flow.frame_count > 100)
            .expect("expected a long-lived 5188 server flow");
        assert!(server_flow.server_frames > 0);
        assert!(server_flow.wire_kind_counts.contains_key("3e04"));
        assert!(server_flow.first_payload_at_micros.is_some());
        assert!(server_flow.last_payload_at_micros.is_some());
        let initialized_server_flows = flows
            .iter()
            .filter(|flow| {
                flow.src.ends_with(":5188")
                    && flow.wire_kind_counts.get("3110") == Some(&2)
                    && flow.wire_kind_counts.get("3210") == Some(&1)
            })
            .collect::<Vec<_>>();
        assert_eq!(initialized_server_flows.len(), 10);
        assert!(initialized_server_flows.iter().all(|flow| {
            flow.server_frames >= 3
                && flow.kind_counts.contains_key(
                    &netzip_fullpull::Official5188Kind::SERVER_INIT_CONTROL.to_string(),
                )
                && flow.kind_counts.contains_key(
                    &netzip_fullpull::Official5188Kind::SERVER_INIT_CONTINUE.to_string(),
                )
        }));
        let delta_flow = flows
            .iter()
            .find(|flow| flow.delta_envelope_count > 0)
            .expect("expected at least one 2704 delta flow");
        assert!(!delta_flow.delta_headers.is_empty());
        println!("vendor_pm_5188_flows={flows:#?}");
    }
}
