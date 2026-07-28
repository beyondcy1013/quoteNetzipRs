use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
struct Config {
    input: PathBuf,
    host: String,
    port: u16,
    segments: Vec<Segment>,
    recv_ms: u64,
    connect_timeout_ms: u64,
    save: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug)]
struct Segment {
    offset: usize,
    len: usize,
    delay_ms: u64,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;
    let payload = fs::read(&config.input)?;
    let segments = if config.segments.is_empty() {
        vec![Segment {
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

    let addr = format!("{}:{}", config.host, config.port);
    let socket_addr = addr
        .to_socket_addrs()?
        .next()
        .ok_or("failed to resolve target address")?;

    println!("input: {}", config.input.display());
    println!("target: {socket_addr}");
    println!("payload-size: {}", payload.len());
    println!("segments: {}", segments.len());

    let mut stream = TcpStream::connect_timeout(
        &socket_addr,
        Duration::from_millis(config.connect_timeout_ms),
    )?;
    stream.set_read_timeout(Some(Duration::from_millis(config.recv_ms)))?;
    stream.set_write_timeout(Some(Duration::from_millis(config.connect_timeout_ms)))?;

    let mut reply = Vec::new();
    let mut transport_error = None;
    for (index, segment) in segments.iter().enumerate() {
        if segment.delay_ms > 0 {
            thread::sleep(Duration::from_millis(segment.delay_ms));
        }

        let part = &payload[segment.offset..segment.offset + segment.len];
        if let Err(err) = stream.write_all(part) {
            println!("send-error[{index}]: {err}");
            transport_error = Some(err.to_string());
            break;
        }
        println!(
            "sent[{index}] offset={} len={} delay_ms={}",
            segment.offset, segment.len, segment.delay_ms
        );

        let mut buf = [0u8; 65_536];
        let recv_deadline = Instant::now() + Duration::from_millis(config.recv_ms);
        while Instant::now() < recv_deadline {
            match stream.read(&mut buf) {
                Ok(0) => {
                    println!("recv: eof");
                    break;
                }
                Ok(n) => {
                    println!("recv: {n}");
                    reply.extend_from_slice(&buf[..n]);
                }
                Err(err)
                    if matches!(
                        err.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    ) =>
                {
                    break;
                }
                Err(err) => {
                    println!("recv-error: {err}");
                    transport_error = Some(err.to_string());
                    break;
                }
            }
        }

        if transport_error.is_some() {
            break;
        }
    }

    let _ = stream.shutdown(Shutdown::Both);

    println!("reply-total: {}", reply.len());
    println!("{}", hex_dump(&reply[..reply.len().min(128)]));

    if let Some(path) = &config.save {
        fs::write(path, &reply)?;
        println!("saved: {}", path.display());
    }

    if let Some(err) = transport_error {
        println!("transport-error: {err}");
    }

    Ok(())
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut input = None;
    let mut host = None;
    let mut port = None;
    let mut segments = Vec::new();
    let mut recv_ms = 250u64;
    let mut connect_timeout_ms = 5000u64;
    let mut save = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--segments" => {
                let spec = args.next().ok_or("missing value after --segments")?;
                segments = parse_segments(&spec)?;
            }
            "--recv-ms" => {
                recv_ms = args
                    .next()
                    .ok_or("missing value after --recv-ms")?
                    .parse()?;
            }
            "--connect-timeout-ms" => {
                connect_timeout_ms = args
                    .next()
                    .ok_or("missing value after --connect-timeout-ms")?
                    .parse()?;
            }
            "--save" => {
                save = Some(PathBuf::from(
                    args.next().ok_or("missing value after --save")?,
                ));
            }
            value if input.is_none() => input = Some(PathBuf::from(value)),
            value if host.is_none() => host = Some(value.to_string()),
            value if port.is_none() => port = Some(value.parse()?),
            other => return Err(format!("unknown arg: {other}").into()),
        }
    }

    Ok(Config {
        input: input.ok_or("missing input file path")?,
        host: host.ok_or("missing host")?,
        port: port.ok_or("missing port")?,
        segments,
        recv_ms,
        connect_timeout_ms,
        save,
    })
}

fn print_help() {
    println!("quote_replay <raw-flow.bin> <host> <port> [options]");
    println!("options:");
    println!("  --segments <offset:len:delay_ms,...>");
    println!("  --recv-ms <n>                per-segment receive window, default 250");
    println!("  --connect-timeout-ms <n>     default 5000");
    println!("  --save <file>                save reply bytes");
    println!("examples:");
    println!(
        "  quote_replay /tmp/flow_9278_7719.bin 110.41.14.158 7719 --segments '0:582:0,582:260:420,842:58:550,900:582:2010,1482:260:444'"
    );
}

fn parse_segments(spec: &str) -> Result<Vec<Segment>, Box<dyn Error>> {
    let mut out = Vec::new();
    for item in spec.split(',').filter(|item| !item.is_empty()) {
        let parts = item.split(':').collect::<Vec<_>>();
        if parts.len() != 3 {
            return Err(format!("invalid segment spec: {item}").into());
        }
        out.push(Segment {
            offset: parts[0].parse()?,
            len: parts[1].parse()?,
            delay_ms: parts[2].parse()?,
        });
    }
    Ok(out)
}

fn hex_dump(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "reply-head: <empty>".to_string();
    }

    let mut out = String::from("reply-head:\n");
    for (row, chunk) in bytes.chunks(16).enumerate() {
        out.push_str(&format!("{:08x}  ", row * 16));
        for byte in chunk {
            out.push_str(&format!("{byte:02x} "));
        }
        out.push('\n');
    }
    out
}
