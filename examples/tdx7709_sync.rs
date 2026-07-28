use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;

use netzipapi_rust_demo::{
    TDX7709_DEFAULT_HOST, TDX7709_DEFAULT_PORT, Tdx7709Config, sync_code_table,
    tdx7709_ascii_preview_from_zlib, write_code_table_csv,
};

#[derive(Debug)]
struct Config {
    client: Tdx7709Config,
    out: Option<PathBuf>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;
    println!("target: {}:{}", config.client.host, config.client.port);

    let result = sync_code_table(&config.client)?;
    println!("reply-bytes: {}", result.raw_reply.len());
    println!("reply-frames: {}", result.frames.len());

    if let Some(reply0) = result.frames.first() {
        println!(
            "bootstrap[0]-reply: op=0x{:04x} sub=0x{:04x} tag=0x{:04x} body={}",
            reply0.op,
            reply0.sub,
            reply0.tag,
            reply0.body.len()
        );
    }
    if let Some(reply1) = result.frames.get(1) {
        println!(
            "bootstrap[1]-reply: op=0x{:04x} sub=0x{:04x} flags=0x{:04x} tag=0x{:04x} body={} ascii={}",
            reply1.op,
            reply1.sub,
            reply1.flags,
            reply1.tag,
            reply1.body.len(),
            tdx7709_ascii_preview_from_zlib(&reply1.body).unwrap_or_else(|| "<none>".to_string())
        );
    }
    if let Some(reply2) = result.frames.get(2) {
        println!(
            "bootstrap[2]-reply: op=0x{:04x} sub=0x{:04x} flags=0x{:04x} tag=0x{:04x} body={}",
            reply2.op,
            reply2.sub,
            reply2.flags,
            reply2.tag,
            reply2.body.len()
        );
    }

    println!("records-total: {}", result.records.len());
    if let Some(first) = result.records.first() {
        println!("first-record: {},{}", first.code, first.name);
    }
    if let Some(last) = result.records.last() {
        println!("last-record: {},{}", last.code, last.name);
    }

    if let Some(path) = &config.out {
        write_code_table_csv(path, &result.records)?;
        println!("saved: {}", path.display());
    }

    Ok(())
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut host = TDX7709_DEFAULT_HOST.to_string();
    let mut port = TDX7709_DEFAULT_PORT;
    let mut out = None;
    let mut read_timeout_ms = 1_000u64;
    let mut connect_timeout_ms = 5_000u64;
    let mut settle_ms = 120u64;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--host" => host = args.next().ok_or("missing value after --host")?,
            "--port" => port = args.next().ok_or("missing value after --port")?.parse()?,
            "--out" => {
                out = Some(PathBuf::from(
                    args.next().ok_or("missing value after --out")?,
                ))
            }
            "--read-timeout-ms" => {
                read_timeout_ms = args
                    .next()
                    .ok_or("missing value after --read-timeout-ms")?
                    .parse()?
            }
            "--connect-timeout-ms" => {
                connect_timeout_ms = args
                    .next()
                    .ok_or("missing value after --connect-timeout-ms")?
                    .parse()?
            }
            "--settle-ms" => {
                settle_ms = args
                    .next()
                    .ok_or("missing value after --settle-ms")?
                    .parse()?
            }
            other => return Err(format!("unknown arg: {other}").into()),
        }
    }

    Ok(Config {
        client: Tdx7709Config {
            host,
            port,
            read_timeout: Duration::from_millis(read_timeout_ms),
            connect_timeout: Duration::from_millis(connect_timeout_ms),
            settle_delay: Duration::from_millis(settle_ms),
        },
        out,
    })
}

fn print_help() {
    println!("tdx7709_sync [options]");
    println!("options:");
    println!("  --host <ip>                   default {TDX7709_DEFAULT_HOST}");
    println!("  --port <port>                 default {TDX7709_DEFAULT_PORT}");
    println!("  --out <csv>                   save decoded code table");
    println!("  --read-timeout-ms <n>         default 1000");
    println!("  --connect-timeout-ms <n>      default 5000");
    println!("  --settle-ms <n>               sleep after each reply, default 120");
    println!("example:");
    println!("  cargo run --example tdx7709_sync -- --out /tmp/tdx7709_codes.csv");
}
