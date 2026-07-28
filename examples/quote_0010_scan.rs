use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use netzipapi_rust_demo::parse_wire_finance_batch_body;

#[derive(Debug)]
struct Config {
    inputs: Vec<PathBuf>,
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
    let files = collect_files(&config.inputs)?;
    if files.is_empty() {
        return Err("no files found".into());
    }

    for path in files {
        let bytes = fs::read(&path)?;
        let records = parse_wire_finance_batch_body(&bytes)?;
        println!(
            "{} size={} count={} first={} last={}",
            path.display(),
            bytes.len(),
            records.len(),
            records
                .first()
                .map(|row| format!("{}{}", market_prefix(row.market), row.code))
                .unwrap_or_else(|| "-".to_string()),
            records
                .last()
                .map(|row| format!("{}{}", market_prefix(row.market), row.code))
                .unwrap_or_else(|| "-".to_string()),
        );
        for (index, row) in records.iter().take(config.limit).enumerate() {
            println!(
                "  [{index}] {}{} liutongguben={:.3} zongguben={:.3} jingzichan={:.3} zhuyingshouru={:.3} ipo_date={} updated_date={}",
                market_prefix(row.market),
                row.code,
                row.liutongguben,
                row.float_field("zongguben").unwrap_or_default(),
                row.float_field("jingzichan").unwrap_or_default(),
                row.float_field("zhuyingshouru").unwrap_or_default(),
                row.ipo_date,
                row.updated_date,
            );
        }
    }

    Ok(())
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h") {
        print_help();
        std::process::exit(0);
    }

    let mut inputs = Vec::new();
    let mut limit = 3usize;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--limit" => {
                let Some(value) = iter.next() else {
                    return Err("missing value after --limit".into());
                };
                limit = value.parse::<usize>()?;
            }
            _ => inputs.push(PathBuf::from(arg)),
        }
    }

    Ok(Config { inputs, limit })
}

fn print_help() {
    println!("quote_0010_scan <inflated-file-or-dir> [more...] [--limit N]");
    println!("  parses extracted 0x0010 finance reply bodies");
    println!("  if a directory is passed, it auto-loads *_inflated.bin");
}

fn collect_files(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut out = Vec::new();
    for input in inputs {
        if input.is_dir() {
            out.append(&mut glob_in_dir(input, "_inflated.bin")?);
        } else {
            out.push(input.clone());
        }
    }
    out.sort();
    Ok(out)
}

fn glob_in_dir(dir: &Path, suffix: &str) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("")
                .ends_with(suffix)
        {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

fn market_prefix(market: u8) -> &'static str {
    match market {
        0 => "SZ",
        1 => "SH",
        2 => "BJ",
        _ => "",
    }
}
