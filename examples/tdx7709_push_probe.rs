use std::error::Error;
use std::time::Duration;

use netzipapi_rust_demo::tdx_0547_delivery::Tdx0547DeliveryKind;
use netzipapi_rust_demo::{Tdx7709Config, Tdx7709QuoteRequestItem, Tdx7709Session};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let symbols = std::env::args().skip(1).collect::<Vec<_>>();
    let symbols = if symbols.is_empty() {
        vec!["SH600000".to_string(), "SZ000001".to_string()]
    } else {
        symbols
    };
    let items = symbols
        .iter()
        .map(|symbol| parse_symbol(symbol))
        .collect::<Result<Vec<_>, _>>()?;

    let mut session = Tdx7709Session::open(&Tdx7709Config::default())?;
    let initial = session.request_live_quotes(&items)?;
    let deliveries = session.collect_quote_deliveries(Duration::from_secs(5))?;
    let unsolicited = deliveries
        .iter()
        .filter(|delivery| delivery.kind == Tdx0547DeliveryKind::UnsolicitedUpdate)
        .count();
    let solicited = deliveries.len().saturating_sub(unsolicited);
    let records = deliveries
        .iter()
        .map(|delivery| delivery.body.records.len())
        .sum::<usize>();
    println!(
        "subscribed={} initial_bodies={} observed_deliveries={} unsolicited={} solicited={} records={}",
        items.len(),
        initial.quote_bodies.len(),
        deliveries.len(),
        unsolicited,
        solicited,
        records
    );
    for delivery in deliveries.iter().take(10) {
        for record in delivery.body.records.iter().take(3) {
            println!(
                "kind={:?} market={} code={} time={}",
                delivery.kind,
                record.market,
                record.code,
                record
                    .time_hhmmss_raw
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string())
            );
        }
    }
    Ok(())
}

fn parse_symbol(symbol: &str) -> Result<Tdx7709QuoteRequestItem, Box<dyn Error>> {
    let symbol = symbol.trim().to_ascii_uppercase();
    let (market, code) = if let Some(code) = symbol.strip_prefix("SZ") {
        (0, code)
    } else if let Some(code) = symbol.strip_prefix("SH") {
        (1, code)
    } else {
        return Err(format!("unsupported symbol: {symbol}").into());
    };
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("invalid symbol: {symbol}").into());
    }
    Ok(Tdx7709QuoteRequestItem {
        market,
        code: code.to_string(),
        token: 0,
    })
}
