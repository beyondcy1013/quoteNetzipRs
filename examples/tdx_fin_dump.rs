use std::env;
use std::error::Error;
use std::path::PathBuf;

use netzipapi_rust_demo::{parse_fin_file, write_fin_csv};

#[derive(Debug)]
struct Config {
    input: PathBuf,
    csv: Option<PathBuf>,
    limit: usize,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;
    let parsed = parse_fin_file(&config.input)?;

    println!("file: {}", config.input.display());
    println!("magic: 0x{:08x}", parsed.magic);
    println!("record-size: {}", parsed.record_size);
    println!("records-total: {}", parsed.records.len());
    println!("trailing-bytes: {}", parsed.trailing_bytes.len());

    if let Some(first) = parsed.records.first() {
        println!(
            "first-record: {} time={} bao_gao={} quarter={} mg_shou_yi={:.4} zong_gu={:.2}",
            first.symbol,
            first.time,
            first.bao_gao,
            first.quarter(),
            first.mg_shou_yi,
            first.zong_gu
        );
    }
    if let Some(last) = parsed.records.last() {
        println!(
            "last-record: {} time={} bao_gao={} quarter={} liu_tong_ag={:.2} jing_li_run={:.2}",
            last.symbol,
            last.time,
            last.bao_gao,
            last.quarter(),
            last.liu_tong_ag,
            last.jing_li_run
        );
    }

    for (index, record) in parsed.records.iter().take(config.limit).enumerate() {
        println!(
            "#{index}: {} market={} code={} time={} bao_gao={} eps={:.4} net_assets={:.4} total_shares={:.2}",
            record.symbol,
            record.market().unwrap_or(""),
            record.code().unwrap_or(""),
            record.time,
            record.bao_gao,
            record.mg_shou_yi,
            record.mg_jing_zhi,
            record.zong_gu
        );
    }

    if let Some(path) = &config.csv {
        write_fin_csv(path, &parsed.records)?;
        println!("saved: {}", path.display());
    }

    Ok(())
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut input = None;
    let mut csv = None;
    let mut limit = 5usize;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--csv" => {
                csv = Some(PathBuf::from(
                    args.next().ok_or("missing value after --csv")?,
                ))
            }
            "--limit" => {
                limit = args.next().ok_or("missing value after --limit")?.parse()?;
            }
            other if other.starts_with("--") => return Err(format!("unknown arg: {other}").into()),
            other => {
                if input.is_some() {
                    return Err(format!("unexpected extra positional arg: {other}").into());
                }
                input = Some(PathBuf::from(other));
            }
        }
    }

    Ok(Config {
        input: input.ok_or("missing FIN file path")?,
        csv,
        limit,
    })
}

fn print_help() {
    println!("tdx_fin_dump <full_sh.FIN|full_sz.FIN> [options]");
    println!("options:");
    println!("  --csv <path>                 save parsed rows as CSV");
    println!("  --limit <n>                  print first n rows, default 5");
    println!("example:");
    println!("  cargo run --example tdx_fin_dump -- /tmp/full_sh.FIN --csv /tmp/full_sh.csv");
}
