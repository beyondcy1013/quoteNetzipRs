//! Lists bounded, non-text numeric traits of Wine 311-byte state candidates.

use netzip_fullpull::{OFFICIAL_5188_INTERNAL_RECORD_LEN, Official5188InternalRecord};
use std::{env, error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let core = fs::read(
        args.next()
            .ok_or("usage: official_5188_core_candidates CORE MARKET INDEX")?,
    )?;
    let market_arg = args.next().ok_or("missing MARKET")?;
    let market_text = market_arg.to_string_lossy();
    let market: [u8; 2] = market_text
        .as_bytes()
        .try_into()
        .map_err(|_| "MARKET must be two bytes")?;
    let symbol_index: u16 = args
        .next()
        .ok_or("missing INDEX")?
        .to_string_lossy()
        .parse()?;
    if args.next().is_some() {
        return Err("unexpected extra argument".into());
    }

    let mut found = 0usize;
    for position in 0xe1..core.len().saturating_sub(2) {
        if core[position..position + 2] != market
            || core[position - 2..position] != symbol_index.to_le_bytes()
        {
            continue;
        }
        let start = position - 0xe1;
        let end = start + OFFICIAL_5188_INTERNAL_RECORD_LEN;
        if end > core.len() {
            continue;
        }
        let record = Official5188InternalRecord::decode(&core[start..end])?;
        let bytes = record.as_bytes();
        let reference = i32::from_le_bytes(bytes[0x12b..0x12f].try_into()?);
        let nonzero = bytes.iter().filter(|byte| **byte != 0).count();
        println!(
            "offset={start} timestamp={} marker={} nonzero={nonzero} price={} volume={} amount={} reference={reference}",
            record.timestamp(),
            bytes[0xde],
            record.last_price_integer(),
            record.volume_integer(),
            record.amount_integer(),
        );
        found += 1;
    }
    eprintln!("candidates={found}");
    Ok(())
}
