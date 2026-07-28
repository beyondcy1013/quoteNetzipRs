use std::env;
use std::error::Error;
use std::path::PathBuf;

use netzipapi_rust_demo::compare_blob_files;

#[derive(Debug)]
struct Config {
    left: PathBuf,
    right: PathBuf,
    left_offset: usize,
    right_offset: usize,
    compare_len: Option<usize>,
    block_size: usize,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;
    let result = compare_blob_files(
        &config.left,
        &config.right,
        config.left_offset,
        config.right_offset,
        config.compare_len,
        config.block_size,
    )?;

    println!("left: {}", result.left_input);
    println!("right: {}", result.right_input);
    println!(
        "sizes: left={} right={} compare={} exact_match={}",
        result.left_size, result.right_size, result.compare_len, result.exact_match
    );
    println!(
        "prefix={} suffix={} equal_bytes={} diff_bytes={} first_diff={:?}",
        result.equal_prefix_len,
        result.equal_suffix_len,
        result.equal_bytes,
        result.diff_bytes,
        result.first_diff_offset
    );
    println!("left-head: {}", result.left_head_hex);
    println!("right-head: {}", result.right_head_hex);
    println!("xor-head: {}", result.xor_head_hex);

    if !result.diff_runs.is_empty() {
        println!("diff-runs:");
        for run in &result.diff_runs {
            println!("  offset={} len={}", run.offset, run.len);
        }
    }

    if let Some(block) = &result.block_summary {
        println!(
            "blocks: size={} total={} equal={} different={}",
            block.block_size, block.total_blocks, block.equal_blocks, block.differing_blocks
        );
        println!(
            "left-unique={} right-unique={} xor-unique={}",
            block.left_unique_blocks, block.right_unique_blocks, block.xor_unique_blocks
        );
        println!(
            "left-top={} x{}",
            block.left_most_common_hex, block.left_most_common_count
        );
        println!(
            "right-top={} x{}",
            block.right_most_common_hex, block.right_most_common_count
        );
        println!(
            "xor-top={} x{}",
            block.xor_most_common_hex, block.xor_most_common_count
        );
    }

    Ok(())
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut left = None;
    let mut right = None;
    let mut left_offset = 0usize;
    let mut right_offset = 0usize;
    let mut compare_len = None;
    let mut block_size = 8usize;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--left-offset" => {
                left_offset = args
                    .next()
                    .ok_or("missing value after --left-offset")?
                    .parse()?;
            }
            "--right-offset" => {
                right_offset = args
                    .next()
                    .ok_or("missing value after --right-offset")?
                    .parse()?;
            }
            "--len" => {
                compare_len = Some(args.next().ok_or("missing value after --len")?.parse()?);
            }
            "--block-size" => {
                block_size = args
                    .next()
                    .ok_or("missing value after --block-size")?
                    .parse()?;
            }
            other if other.starts_with("--") => return Err(format!("unknown arg: {other}").into()),
            other => {
                if left.is_none() {
                    left = Some(PathBuf::from(other));
                } else if right.is_none() {
                    right = Some(PathBuf::from(other));
                } else {
                    return Err(format!("unexpected extra positional arg: {other}").into());
                }
            }
        }
    }

    Ok(Config {
        left: left.ok_or("missing left blob path")?,
        right: right.ok_or("missing right blob path")?,
        left_offset,
        right_offset,
        compare_len,
        block_size,
    })
}

fn print_help() {
    println!("blob_compare <left.bin> <right.bin> [options]");
    println!("options:");
    println!("  --left-offset <n>            start offset into left blob");
    println!("  --right-offset <n>           start offset into right blob");
    println!("  --len <n>                    compare at most n bytes");
    println!("  --block-size <n>             block size for repeat/xor stats, default 8");
    println!("example:");
    println!("  cargo run --example blob_compare -- plain.bin cipher.bin --len 280 --block-size 8");
}
