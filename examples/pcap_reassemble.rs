use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fs;
use std::net::Ipv4Addr;
use std::path::PathBuf;

#[derive(Debug)]
struct Config {
    input: PathBuf,
    src: Endpoint,
    dst: Endpoint,
    out: Option<PathBuf>,
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
    incl_len: u32,
    orig_len: u32,
    wire_payload_len: usize,
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

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;
    let bytes = fs::read(&config.input)?;
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
        if packet.src == config.src
            && packet.dst == config.dst
            && !packet.captured_payload.is_empty()
        {
            selected.push(packet);
        }
    }

    if selected.is_empty() {
        return Err(format!(
            "no TCP payload packets for {} -> {}",
            display_ep(config.src),
            display_ep(config.dst)
        )
        .into());
    }

    selected.sort_by_key(|packet| packet.seq);
    let first_seq = selected[0].seq;
    let mut stream = Vec::new();
    let mut expected_seq = first_seq;
    let mut gaps = Vec::new();

    for packet in &selected {
        let payload = &packet.captured_payload;
        let packet_start = packet.seq;
        let packet_end = packet.seq.wrapping_add(payload.len() as u32);

        if packet_start > expected_seq {
            gaps.push((expected_seq, packet_start));
            expected_seq = packet_start;
        }

        let overlap = expected_seq.saturating_sub(packet_start) as usize;
        if overlap >= payload.len() {
            continue;
        }

        stream.extend_from_slice(&payload[overlap..]);
        expected_seq = packet_end;
    }

    println!("input: {}", config.input.display());
    println!(
        "flow: {} -> {}",
        display_ep(config.src),
        display_ep(config.dst)
    );
    println!("selected-packets: {}", selected.len());
    println!("first-seq: {}", first_seq);
    println!("reassembled-bytes: {}", stream.len());
    if !gaps.is_empty() {
        println!("gaps: {}", gaps.len());
        for (start, end) in gaps.iter().take(8) {
            println!("  seq {} .. {}", start, end);
        }
    } else {
        println!("gaps: 0");
    }

    for packet in selected.iter().take(12) {
        println!(
            "  seg seq={} ack={} flags=0x{:02x} wire={} captured={} incl={} orig={}",
            packet.seq,
            packet.ack,
            packet.flags,
            packet.wire_payload_len,
            packet.captured_payload.len(),
            packet.incl_len,
            packet.orig_len
        );
    }

    if let Some(out) = config.out {
        fs::write(&out, &stream)?;
        println!("wrote: {}", out.display());
    } else {
        println!("{}", hex_dump(&stream[..stream.len().min(256)], 16));
    }

    Ok(())
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let mut input = None;
    let mut src = None;
    let mut dst = None;
    let mut out = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--src" => {
                let value = args.next().ok_or("missing value after --src")?;
                src = Some(parse_endpoint(&value)?);
            }
            "--dst" => {
                let value = args.next().ok_or("missing value after --dst")?;
                dst = Some(parse_endpoint(&value)?);
            }
            "--out" => {
                let value = args.next().ok_or("missing value after --out")?;
                out = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            value if input.is_none() => input = Some(PathBuf::from(value)),
            other => return Err(format!("unknown arg: {other}").into()),
        }
    }

    Ok(Config {
        input: input.ok_or("missing input pcap path")?,
        src: src.ok_or("missing --src ip:port")?,
        dst: dst.ok_or("missing --dst ip:port")?,
        out,
    })
}

fn print_help() {
    println!("pcap_reassemble <capture.pcap> --src IP:PORT --dst IP:PORT [--out file]");
    println!("  parse classic pcap, dedupe repeated packets, and reassemble TCP payload");
}

fn parse_endpoint(value: &str) -> Result<Endpoint, Box<dyn Error>> {
    let (ip, port) = value
        .rsplit_once(':')
        .ok_or_else(|| format!("endpoint must look like ip:port, got {value}"))?;
    Ok(Endpoint {
        ip: ip.parse()?,
        port: port.parse()?,
    })
}

fn display_ep(endpoint: Endpoint) -> String {
    format!("{}:{}", endpoint.ip, endpoint.port)
}

fn parse_pcap(bytes: &[u8]) -> Result<Vec<TcpPacket>, Box<dyn Error>> {
    if bytes.len() < 24 {
        return Err("pcap file too small".into());
    }

    let (endian, scale) = parse_global_header(&bytes[..24])?;
    let link_type = read_u32(endian, &bytes[20..24])?;
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

        let Some(packet) =
            parse_tcp_packet(frame, incl_len, orig_len, ts_sec, ts_frac, scale, link_type)
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
    _ts_sec: u32,
    _ts_frac: u32,
    _scale: TimestampScale,
    link_type: u32,
) -> Option<TcpPacket> {
    let (network_offset, protocol_offset) = match link_type {
        1 => (14usize, 12usize),  // DLT_EN10MB
        276 => (20usize, 0usize), // DLT_LINUX_SLL2
        _ => return None,
    };
    if frame.len() < network_offset + 20 + 20 {
        return None;
    }
    if frame[protocol_offset..protocol_offset + 2] != [0x08, 0x00] {
        return None;
    }

    let ihl = ((frame[network_offset] & 0x0f) as usize) * 4;
    if frame.len() < network_offset + ihl + 20 || frame[network_offset + 9] != 6 {
        return None;
    }

    let ip_total_len =
        u16::from_be_bytes([frame[network_offset + 2], frame[network_offset + 3]]) as usize;
    let src = Endpoint {
        ip: Ipv4Addr::new(
            frame[network_offset + 12],
            frame[network_offset + 13],
            frame[network_offset + 14],
            frame[network_offset + 15],
        ),
        port: u16::from_be_bytes([frame[network_offset + ihl], frame[network_offset + ihl + 1]]),
    };
    let dst = Endpoint {
        ip: Ipv4Addr::new(
            frame[network_offset + 16],
            frame[network_offset + 17],
            frame[network_offset + 18],
            frame[network_offset + 19],
        ),
        port: u16::from_be_bytes([
            frame[network_offset + ihl + 2],
            frame[network_offset + ihl + 3],
        ]),
    };

    let tcp_start = network_offset + ihl;
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
    let payload_end = frame.len().min(network_offset + ip_total_len);
    let captured_payload = if payload_start < payload_end {
        frame[payload_start..payload_end].to_vec()
    } else {
        Vec::new()
    };

    Some(TcpPacket {
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
