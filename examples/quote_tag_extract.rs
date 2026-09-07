use std::env;
use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

use flate2::read::ZlibDecoder;

#[derive(Debug)]
struct Config {
    input: PathBuf,
    out_dir: PathBuf,
    tag: u16,
}

#[derive(Clone, Copy, Debug)]
struct Client10Frame {
    offset: usize,
    op: u16,
    sub: u16,
    flags: u16,
    payload_len: usize,
}

#[derive(Clone, Copy, Debug)]
struct Server16Frame {
    offset: usize,
    op: u16,
    sub: u16,
    flags: u16,
    tag: u16,
    compressed_len: usize,
    original_len: usize,
}

#[derive(Debug)]
struct Tag0547Item {
    market_flag: u8,
    code: String,
    token: u32,
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
    fs::create_dir_all(&config.out_dir)?;

    println!("input: {}", config.input.display());
    println!("out-dir: {}", config.out_dir.display());
    println!("tag: 0x{:04x}", config.tag);
    println!("size: {}", bytes.len());

    if let Some(frames) = parse_server16_frames(&bytes) {
        let mut matches = 0usize;
        println!("mode: server16");
        for (index, frame) in frames.iter().enumerate() {
            if frame.tag != config.tag {
                continue;
            }
            matches += 1;
            let body_start = frame.offset + 16;
            let body_end = body_start + frame.compressed_len;
            let body = &bytes[body_start..body_end];
            let stem = format!(
                "frame{index:03}_sub{:04x}_tag{:04x}_off{}",
                frame.sub, frame.tag, frame.offset
            );
            let body_path = config.out_dir.join(format!("{stem}_body.bin"));
            fs::write(&body_path, body)?;

            println!(
                "frame[{index}] op=0x{:04x} sub=0x{:04x} flags=0x{:04x} tag=0x{:04x} body={} original={} -> {}",
                frame.op,
                frame.sub,
                frame.flags,
                frame.tag,
                frame.compressed_len,
                frame.original_len,
                body_path.display()
            );

            if let Some(inflated) = zlib_decode(body) {
                let inflated_path = config.out_dir.join(format!("{stem}_inflated.bin"));
                fs::write(&inflated_path, &inflated)?;
                println!(
                    "  zlib: ok inflated={} -> {}",
                    inflated.len(),
                    inflated_path.display()
                );
            } else {
                println!("  zlib: skipped-or-failed");
            }
        }
        println!("matches: {matches}");
        return Ok(());
    }

    if let Some(frames) = parse_client10_frames(&bytes) {
        let mut matches = 0usize;
        println!("mode: client10");
        for (index, frame) in frames.iter().enumerate() {
            let body_start = frame.offset + 10;
            let body_end = body_start + frame.payload_len;
            let body = &bytes[body_start..body_end];
            if body.len() < 2 || le_u16(&body[0..2]) != config.tag {
                continue;
            }
            matches += 1;
            let stem = format!(
                "frame{index:03}_sub{:04x}_tag{:04x}_off{}",
                frame.sub, config.tag, frame.offset
            );
            let payload_path = config.out_dir.join(format!("{stem}_payload.bin"));
            fs::write(&payload_path, body)?;

            let tokens = extract_numeric_tokens(body);
            print!(
                "frame[{index}] op=0x{:04x} sub=0x{:04x} flags=0x{:04x} payload={} numeric-tokens={}",
                frame.op,
                frame.sub,
                frame.flags,
                frame.payload_len,
                tokens.len()
            );
            if let Some(items) = parse_tag_0547_items(body) {
                let nonzero = items.iter().filter(|item| item.token != 0).count();
                let preview = items
                    .iter()
                    .take(4)
                    .map(|item| {
                        format!(
                            "{}{}@0x{:08x}",
                            market_prefix(item.market_flag),
                            item.code,
                            item.token
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                print!(
                    " item-count={} token-nonzero={} first=[{}]",
                    items.len(),
                    nonzero,
                    preview
                );
            }
            println!(" -> {}", payload_path.display());
        }
        println!("matches: {matches}");
        return Ok(());
    }

    Err("unknown flow format".into())
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let Some(first) = args.next() else {
        return Err("missing input file path".into());
    };
    if matches!(first.as_str(), "--help" | "-h") {
        print_help();
        std::process::exit(0);
    }

    let mut input = PathBuf::from(first);
    let mut out_dir = PathBuf::from("tmp/quote_tag_extract");
    let mut tag = 0x0547u16;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out-dir" => {
                let Some(value) = args.next() else {
                    return Err("missing value after --out-dir".into());
                };
                out_dir = PathBuf::from(value);
            }
            "--tag" => {
                let Some(value) = args.next() else {
                    return Err("missing value after --tag".into());
                };
                tag = parse_u16(&value)?;
            }
            "--input" => {
                let Some(value) = args.next() else {
                    return Err("missing value after --input".into());
                };
                input = PathBuf::from(value);
            }
            other => return Err(format!("unknown arg: {other}").into()),
        }
    }

    Ok(Config {
        input,
        out_dir,
        tag,
    })
}

fn print_help() {
    println!("quote_tag_extract <raw-flow.bin> [--out-dir dir] [--tag 0x0547]");
    println!("  extracts matching tag frames from client10/server16 quote flows");
    println!("  default tag: 0x0547");
}

fn parse_server16_frames(bytes: &[u8]) -> Option<Vec<Server16Frame>> {
    if bytes.len() < 16 || !bytes.starts_with(&[0xb1, 0xcb, 0x74, 0x00]) {
        return None;
    }

    let mut out = Vec::new();
    let mut offset = 0usize;
    while offset < bytes.len() {
        if offset + 16 > bytes.len() || bytes[offset..offset + 4] != [0xb1, 0xcb, 0x74, 0x00] {
            return None;
        }
        let compressed_len = le_u16(&bytes[offset + 12..offset + 14]) as usize;
        let total = 16 + compressed_len;
        if offset + total > bytes.len() {
            return None;
        }
        out.push(Server16Frame {
            offset,
            op: le_u16(&bytes[offset + 4..offset + 6]),
            sub: le_u16(&bytes[offset + 6..offset + 8]),
            flags: le_u16(&bytes[offset + 8..offset + 10]),
            tag: le_u16(&bytes[offset + 10..offset + 12]),
            compressed_len,
            original_len: le_u16(&bytes[offset + 14..offset + 16]) as usize,
        });
        offset += total;
    }
    Some(out)
}

fn parse_client10_frames(bytes: &[u8]) -> Option<Vec<Client10Frame>> {
    if bytes.len() < 10 {
        return None;
    }

    let mut out = Vec::new();
    let mut offset = 0usize;
    while offset < bytes.len() {
        if offset + 10 > bytes.len() {
            return None;
        }
        let len1 = le_u16(&bytes[offset + 6..offset + 8]) as usize;
        let len2 = le_u16(&bytes[offset + 8..offset + 10]) as usize;
        if len1 == 0 || len1 != len2 {
            return None;
        }
        let total = 10 + len1;
        if offset + total > bytes.len() {
            return None;
        }
        out.push(Client10Frame {
            offset,
            op: le_u16(&bytes[offset..offset + 2]),
            sub: le_u16(&bytes[offset + 2..offset + 4]),
            flags: le_u16(&bytes[offset + 4..offset + 6]),
            payload_len: len1,
        });
        offset += total;
    }
    Some(out)
}

fn zlib_decode(bytes: &[u8]) -> Option<Vec<u8>> {
    if !(bytes.starts_with(&[0x78, 0x01])
        || bytes.starts_with(&[0x78, 0x5e])
        || bytes.starts_with(&[0x78, 0x9c])
        || bytes.starts_with(&[0x78, 0xda]))
    {
        return None;
    }
    let mut decoder = ZlibDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

fn extract_numeric_tokens(bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, byte) in bytes.iter().copied().enumerate() {
        if byte.is_ascii_digit() {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take()
            && i - s >= 6
        {
            out.push(String::from_utf8_lossy(&bytes[s..i]).into_owned());
        }
    }
    if let Some(s) = start
        && bytes.len() - s >= 6
    {
        out.push(String::from_utf8_lossy(&bytes[s..]).into_owned());
    }
    out
}

fn parse_tag_0547_items(body: &[u8]) -> Option<Vec<Tag0547Item>> {
    if body.len() < 4 || !(body.len() - 4).is_multiple_of(11) {
        return None;
    }
    let mut out = Vec::new();
    for chunk in body[4..].as_chunks::<11>().0 {
        let market_flag = chunk[0];
        let code = String::from_utf8_lossy(&chunk[1..7]).into_owned();
        let token = u32::from_le_bytes([chunk[7], chunk[8], chunk[9], chunk[10]]);
        out.push(Tag0547Item {
            market_flag,
            code,
            token,
        });
    }
    Some(out)
}

fn market_prefix(flag: u8) -> &'static str {
    match flag {
        0 => "SZ",
        1 => "SH",
        _ => "??",
    }
}

fn parse_u16(value: &str) -> Result<u16, Box<dyn Error>> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        return Ok(u16::from_str_radix(hex, 16)?);
    }
    Ok(value.parse()?)
}

fn le_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}
