use std::collections::{BTreeMap, HashSet};
use std::env;
use std::error::Error;
use std::fs;
use std::net::Ipv4Addr;
use std::path::PathBuf;

#[derive(Debug)]
struct Config {
    input: PathBuf,
    host: Option<Ipv4Addr>,
    ports: Vec<u16>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Endpoint {
    ip: Ipv4Addr,
    port: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Timestamp {
    sec: u32,
    frac: u32,
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
    ts: Timestamp,
    src: Endpoint,
    dst: Endpoint,
    seq: u32,
    ack: u32,
    flags: u8,
    wire_payload_len: usize,
    captured_payload_len: usize,
    captured_payload: Vec<u8>,
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct FlowKey {
    a: Endpoint,
    b: Endpoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Direction {
    AtoB,
    BtoA,
}

#[derive(Debug)]
struct FlowSummary {
    first_ts: Timestamp,
    last_ts: Timestamp,
    first_dir: Direction,
    first_flags: u8,
    first_payload_len: usize,
    packets: usize,
    syn_a_to_b: usize,
    syn_b_to_a: usize,
    syn_ack_a_to_b: usize,
    syn_ack_b_to_a: usize,
    fin_rst_packets: usize,
    payload_packets_a_to_b: usize,
    payload_packets_b_to_a: usize,
    payload_bytes_a_to_b: usize,
    payload_bytes_b_to_a: usize,
    pure_ack_packets: usize,
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
    let mut packets = parse_pcap(&bytes)?;
    packets.sort_by_key(|packet| packet.ts);

    let mut seen = HashSet::new();
    let mut flows = BTreeMap::<FlowKey, FlowSummary>::new();
    let mut first_seen = None::<Timestamp>;

    for packet in packets {
        if !matches_filters(&config, &packet) {
            continue;
        }

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

        let key = canonical_flow(packet.src, packet.dst);
        let direction = if packet.src == key.a {
            Direction::AtoB
        } else {
            Direction::BtoA
        };

        let entry = flows.entry(key).or_insert_with(|| FlowSummary {
            first_ts: packet.ts,
            last_ts: packet.ts,
            first_dir: direction,
            first_flags: packet.flags,
            first_payload_len: packet.captured_payload_len,
            packets: 0,
            syn_a_to_b: 0,
            syn_b_to_a: 0,
            syn_ack_a_to_b: 0,
            syn_ack_b_to_a: 0,
            fin_rst_packets: 0,
            payload_packets_a_to_b: 0,
            payload_packets_b_to_a: 0,
            payload_bytes_a_to_b: 0,
            payload_bytes_b_to_a: 0,
            pure_ack_packets: 0,
        });

        if first_seen.is_none() {
            first_seen = Some(packet.ts);
        }

        entry.last_ts = packet.ts;
        entry.packets += 1;

        let syn = packet.flags & 0x02 != 0;
        let ack = packet.flags & 0x10 != 0;
        let fin_rst = packet.flags & 0x05 != 0;
        let payload_len = packet.captured_payload_len;

        match direction {
            Direction::AtoB => {
                if syn && ack {
                    entry.syn_ack_a_to_b += 1;
                } else if syn {
                    entry.syn_a_to_b += 1;
                }
                if payload_len > 0 {
                    entry.payload_packets_a_to_b += 1;
                    entry.payload_bytes_a_to_b += payload_len;
                }
            }
            Direction::BtoA => {
                if syn && ack {
                    entry.syn_ack_b_to_a += 1;
                } else if syn {
                    entry.syn_b_to_a += 1;
                }
                if payload_len > 0 {
                    entry.payload_packets_b_to_a += 1;
                    entry.payload_bytes_b_to_a += payload_len;
                }
            }
        }

        if fin_rst {
            entry.fin_rst_packets += 1;
        } else if ack && !syn && payload_len == 0 {
            entry.pure_ack_packets += 1;
        }
    }

    let Some(first_seen) = first_seen else {
        return Err("no matching TCP packets after filtering".into());
    };

    println!("input: {}", config.input.display());
    if let Some(host) = config.host {
        println!("host-filter: {host}");
    }
    if !config.ports.is_empty() {
        let ports = config
            .ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(",");
        println!("ports-filter: {ports}");
    }
    println!("flows: {}", flows.len());

    for (index, (key, summary)) in flows.iter().enumerate() {
        let start_ms = elapsed_millis(first_seen, summary.first_ts);
        let duration_ms = elapsed_millis(summary.first_ts, summary.last_ts);
        println!(
            "flow[{index}] t+{start_ms:>6}ms dur={duration_ms:>5}ms {} <-> {}",
            display_ep(key.a),
            display_ep(key.b)
        );
        println!(
            "  first={} flags={} payload={} state={}",
            display_dir(summary.first_dir, key),
            format_flags(summary.first_flags),
            summary.first_payload_len,
            classify_flow(summary)
        );
        println!(
            "  syn={} syn-ack={} payload={} bytes={} fin/rst={} pure-ack={}",
            format_dir_counts(summary.syn_a_to_b, summary.syn_b_to_a),
            format_dir_counts(summary.syn_ack_a_to_b, summary.syn_ack_b_to_a),
            format_dir_counts(
                summary.payload_packets_a_to_b,
                summary.payload_packets_b_to_a
            ),
            format_dir_counts(summary.payload_bytes_a_to_b, summary.payload_bytes_b_to_a),
            summary.fin_rst_packets,
            summary.pure_ack_packets
        );
    }

    Ok(())
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut input = None;
    let mut host = None;
    let mut ports = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--host" => {
                host = Some(args.next().ok_or("missing value after --host")?.parse()?);
            }
            "--ports" => {
                let spec = args.next().ok_or("missing value after --ports")?;
                ports = spec
                    .split(',')
                    .filter(|item| !item.is_empty())
                    .map(str::parse)
                    .collect::<Result<Vec<u16>, _>>()?;
            }
            value if input.is_none() => input = Some(PathBuf::from(value)),
            other => return Err(format!("unknown arg: {other}").into()),
        }
    }

    Ok(Config {
        input: input.ok_or("missing input pcap path")?,
        host,
        ports,
    })
}

fn print_help() {
    println!("pcap_flow_timeline <capture.pcap> [--host ip] [--ports p1,p2,...]");
    println!(
        "  summarize TCP flows, dedupe repeated packets, and flag whether capture started midstream"
    );
}

fn matches_filters(config: &Config, packet: &TcpPacket) -> bool {
    if let Some(host) = config.host
        && packet.src.ip != host
        && packet.dst.ip != host
    {
        return false;
    }
    !(!config.ports.is_empty()
        && !config.ports.contains(&packet.src.port)
        && !config.ports.contains(&packet.dst.port))
}

fn canonical_flow(src: Endpoint, dst: Endpoint) -> FlowKey {
    if src <= dst {
        FlowKey { a: src, b: dst }
    } else {
        FlowKey { a: dst, b: src }
    }
}

fn classify_flow(summary: &FlowSummary) -> &'static str {
    let saw_handshake = (summary.syn_a_to_b > 0 && summary.syn_ack_b_to_a > 0)
        || (summary.syn_b_to_a > 0 && summary.syn_ack_a_to_b > 0);
    let only_syn = summary.payload_bytes_a_to_b == 0
        && summary.payload_bytes_b_to_a == 0
        && summary.pure_ack_packets == 0;

    if only_syn {
        return "connect-attempt-only";
    }
    if summary.first_flags & 0x02 != 0 && saw_handshake {
        return "started-with-handshake";
    }
    if summary.first_payload_len > 0 {
        return "midstream-first-packet-had-payload";
    }
    if summary.first_flags & 0x10 != 0 {
        return "midstream-first-packet-was-ack";
    }
    "unknown"
}

fn format_dir_counts(a_to_b: usize, b_to_a: usize) -> String {
    format!("{a_to_b}/{b_to_a}")
}

fn display_dir(direction: Direction, key: &FlowKey) -> String {
    match direction {
        Direction::AtoB => format!("{} -> {}", display_ep(key.a), display_ep(key.b)),
        Direction::BtoA => format!("{} -> {}", display_ep(key.b), display_ep(key.a)),
    }
}

fn display_ep(endpoint: Endpoint) -> String {
    format!("{}:{}", endpoint.ip, endpoint.port)
}

fn format_flags(flags: u8) -> String {
    let mut out = String::new();
    if flags & 0x02 != 0 {
        out.push('S');
    }
    if flags & 0x10 != 0 {
        out.push('A');
    }
    if flags & 0x08 != 0 {
        out.push('P');
    }
    if flags & 0x01 != 0 {
        out.push('F');
    }
    if flags & 0x04 != 0 {
        out.push('R');
    }
    if flags & 0x20 != 0 {
        out.push('U');
    }
    if out.is_empty() {
        out.push('-');
    }
    out
}

fn elapsed_millis(start: Timestamp, end: Timestamp) -> i128 {
    let start_ns = timestamp_to_nanos(start);
    let end_ns = timestamp_to_nanos(end);
    (end_ns - start_ns) / 1_000_000
}

fn timestamp_to_nanos(ts: Timestamp) -> i128 {
    (ts.sec as i128) * 1_000_000_000 + (ts.frac as i128)
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
        let _orig_len = read_u32(endian, &bytes[offset + 12..offset + 16])?;
        offset += 16;

        let incl_len_usize = incl_len as usize;
        if offset + incl_len_usize > bytes.len() {
            break;
        }
        let frame = &bytes[offset..offset + incl_len_usize];
        offset += incl_len_usize;

        let Some(packet) = parse_tcp_packet(
            frame,
            Timestamp {
                sec: ts_sec,
                frac: match scale {
                    TimestampScale::Micros => ts_frac.saturating_mul(1_000),
                    TimestampScale::Nanos => ts_frac,
                },
            },
        ) else {
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

fn parse_tcp_packet(frame: &[u8], ts: Timestamp) -> Option<TcpPacket> {
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
    let payload_end = frame.len().min(14 + ip_total_len);
    let captured_payload_len = payload_end.saturating_sub(payload_start);
    let wire_payload_len = ip_total_len.saturating_sub(ihl + tcp_header_len);

    Some(TcpPacket {
        ts,
        src,
        dst,
        seq,
        ack,
        flags,
        wire_payload_len,
        captured_payload_len,
        captured_payload: if payload_start < payload_end {
            frame[payload_start..payload_end].to_vec()
        } else {
            Vec::new()
        },
    })
}
