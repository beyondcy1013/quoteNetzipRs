use std::env;
use std::error::Error;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use netzipapi_rust_demo::{parse_answer_buffer, to_wide_null};

const DEFAULT_HOST: &str = "121.41.70.217";
const DEFAULT_PORT: u16 = 6100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Encoding {
    Utf8,
    Utf16Le,
    Hex,
}

#[derive(Debug)]
struct Config {
    host: String,
    port: u16,
    payload: Option<String>,
    encoding: Encoding,
    nul_terminate: bool,
    prefix_u32le: bool,
    read_secs: u64,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;
    let addr = format!("{}:{}", config.host, config.port);
    println!("connect {addr}");

    let mut stream = TcpStream::connect(&addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(config.read_secs)))?;
    stream.set_write_timeout(Some(Duration::from_secs(config.read_secs)))?;

    if let Some(payload) = &config.payload {
        let request = build_request(payload, &config)?;
        println!("send {} bytes", request.len());
        println!("{}", hex_dump(&request, 16));
        stream.write_all(&request)?;
        stream.flush()?;
    } else {
        println!("no payload, read-only probe");
    }

    let mut response = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => response.extend_from_slice(&buf[..n]),
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(err) => return Err(err.into()),
        }
    }

    println!("recv {} bytes", response.len());
    if !response.is_empty() {
        println!("{}", hex_dump(&response, 16));
        if let Some(packet) = unsafe { parse_answer_buffer(&response) } {
            println!("parsed: {}", packet.summary());
            println!("{packet:#?}");
        } else {
            println!("parsed: no OEM_DATA_HEAD packet detected");
        }
    }

    Ok(())
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let mut host = DEFAULT_HOST.to_string();
    let mut port = DEFAULT_PORT;
    let mut payload = None;
    let mut encoding = Encoding::Utf8;
    let mut nul_terminate = false;
    let mut prefix_u32le = false;
    let mut read_secs = 3u64;

    let mut args = env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--host" => host = next_arg(&mut args, "--host")?,
            "--port" => port = next_arg(&mut args, "--port")?.parse()?,
            "--payload" => payload = Some(next_arg(&mut args, "--payload")?),
            "--utf8" => encoding = Encoding::Utf8,
            "--utf16le" => encoding = Encoding::Utf16Le,
            "--hex" => encoding = Encoding::Hex,
            "--nul" => nul_terminate = true,
            "--u32le-len" => prefix_u32le = true,
            "--read-secs" => read_secs = next_arg(&mut args, "--read-secs")?.parse()?,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other if !other.starts_with('-') && host == DEFAULT_HOST => host = other.to_string(),
            other if !other.starts_with('-') && port == DEFAULT_PORT => port = other.parse()?,
            other => return Err(format!("unknown arg: {other}").into()),
        }
    }

    Ok(Config {
        host,
        port,
        payload,
        encoding,
        nul_terminate,
        prefix_u32le,
        read_secs,
    })
}

fn next_arg(
    args: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    flag: &str,
) -> Result<String, Box<dyn Error>> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value").into())
}

fn build_request(payload: &str, config: &Config) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut body = match config.encoding {
        Encoding::Utf8 => {
            let mut bytes = payload.as_bytes().to_vec();
            if config.nul_terminate {
                bytes.push(0);
            }
            bytes
        }
        Encoding::Utf16Le => {
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
        Encoding::Hex => parse_hex(payload)?,
    };

    if config.prefix_u32le {
        let len = u32::try_from(body.len())?;
        let mut with_prefix = Vec::with_capacity(body.len() + 4);
        with_prefix.extend_from_slice(&len.to_le_bytes());
        with_prefix.append(&mut body);
        Ok(with_prefix)
    } else {
        Ok(body)
    }
}

fn parse_hex(value: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let compact: String = value
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect();
    if compact.len() % 2 != 0 {
        return Err("hex payload must have even length".into());
    }

    let mut out = Vec::with_capacity(compact.len() / 2);
    let bytes = compact.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hex = std::str::from_utf8(&bytes[i..i + 2])?;
        out.push(u8::from_str_radix(hex, 16)?);
    }
    Ok(out)
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

fn print_help() {
    println!("proto_probe [host] [port] [options]");
    println!("  --host <host>          default 121.41.70.217");
    println!("  --port <port>          default 6100");
    println!("  --payload <text>       request body to send");
    println!("  --utf8                 payload encoding utf-8");
    println!("  --utf16le              payload encoding utf-16le");
    println!("  --hex                  payload is hex bytes");
    println!("  --nul                  append string terminator");
    println!("  --u32le-len            prepend little-endian u32 length");
    println!("  --read-secs <n>        socket read timeout");
}
