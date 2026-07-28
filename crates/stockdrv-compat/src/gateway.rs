use std::env;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::Value;
use tuwenca_codec::{InstrumentId, QuoteSnapshot, encode_realtime_packet};

const STOCKDRV_PUSH_MAGIC: u32 = 0x3f00_1234;
const STOCKDRV_CONTAINER_SIZE: usize = 0x124;
const STOCKDRV_REPORT_SIZE: usize = 0x9e;
const STOCKDRV_REPORT_KIND: &[u8] = b"\xca\xb5\xca\xb1\xca\xfd\xbe\xdd RCV_REPORTV3\0";

const DEFAULT_GATEWAY: &str = "192.168.3.2:16886";

pub(crate) struct PushQuote {
    pub instrument: String,
    pub name: String,
    pub timestamp: u32,
    pub price: f32,
    pub last_close: f32,
    pub volume: f32,
    pub amount: f32,
}

pub(crate) fn encode_push_packet(quotes: &[PushQuote]) -> Result<Vec<u8>, String> {
    let count = u32::try_from(quotes.len()).map_err(|_| "too many push quotes".to_owned())?;
    let mut packet = vec![0; STOCKDRV_CONTAINER_SIZE + STOCKDRV_REPORT_SIZE * quotes.len()];
    packet[0..4].copy_from_slice(&STOCKDRV_PUSH_MAGIC.to_le_bytes());
    packet[4..8].copy_from_slice(&count.to_le_bytes());
    packet[0x14..0x14 + STOCKDRV_REPORT_KIND.len()].copy_from_slice(STOCKDRV_REPORT_KIND);
    for (index, quote) in quotes.iter().enumerate() {
        InstrumentId::parse(&quote.instrument).map_err(|error| error.to_string())?;
        let record = &mut packet[STOCKDRV_CONTAINER_SIZE + index * STOCKDRV_REPORT_SIZE..]
            [..STOCKDRV_REPORT_SIZE];
        record[0..2].copy_from_slice(&(STOCKDRV_REPORT_SIZE as u16).to_le_bytes());
        record[2..6].copy_from_slice(&quote.timestamp.to_le_bytes());
        write_ansi(&mut record[6..18], quote.instrument.as_bytes());
        let (encoded_name, _, _) = encoding_rs::GBK.encode(&quote.name);
        write_ansi(&mut record[18..50], &encoded_name);
        record[50..54].copy_from_slice(&quote.last_close.to_le_bytes());
        record[66..70].copy_from_slice(&quote.price.to_le_bytes());
        record[70..74].copy_from_slice(&quote.volume.to_le_bytes());
        record[74..78].copy_from_slice(&quote.amount.to_le_bytes());
    }
    Ok(packet)
}

#[cfg(target_os = "windows")]
pub(crate) fn prepare_push_packet(packet: &mut [u8]) -> Result<(), String> {
    if packet.len() < STOCKDRV_CONTAINER_SIZE {
        return Err("Stockdrv push packet is shorter than its container".to_owned());
    }
    let record_address = (packet.as_ptr() as usize)
        .checked_add(STOCKDRV_CONTAINER_SIZE)
        .and_then(|address| u32::try_from(address).ok())
        .ok_or("Stockdrv push packet address is outside x86 range")?;
    packet[0x11c..0x120].copy_from_slice(&record_address.to_le_bytes());
    Ok(())
}

fn write_ansi(output: &mut [u8], input: &[u8]) {
    let length = input.len().min(output.len().saturating_sub(1));
    output[..length].copy_from_slice(&input[..length]);
}

pub(crate) fn fetch_report(instrument: &str) -> Result<Vec<u8>, String> {
    let id = InstrumentId::parse(instrument).map_err(|error| error.to_string())?;
    let address = env::var("TUWENCA_GATEWAY_ADDR").unwrap_or_else(|_| DEFAULT_GATEWAY.to_owned());
    let path = format!("/api/quotes?codes={}&refresh=false", &instrument[2..]);
    let body = http_get(&address, &path)?;
    let quote = parse_quote(&body, id)?;
    let packet = encode_realtime_packet(&[quote], 0).map_err(|error| error.to_string())?;
    Ok(packet[200..].to_vec())
}

pub(crate) fn fetch_worklist() -> Result<Vec<PushQuote>, String> {
    let address = env::var("TUWENCA_GATEWAY_ADDR").unwrap_or_else(|_| DEFAULT_GATEWAY.to_owned());
    let body = http_get(
        &address,
        "/api/codes/worklist?include_unknown=true&limit=6000",
    )?;
    parse_worklist(&body)
}

fn http_get(address: &str, path: &str) -> Result<String, String> {
    let timeout = Duration::from_secs(3);
    let mut stream = TcpStream::connect_timeout(
        &address
            .parse()
            .map_err(|_| format!("invalid gateway address: {address}"))?,
        timeout,
    )
    .map_err(|error| format!("connect gateway: {error}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    write!(
        stream,
        "GET {path} HTTP/1.0\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| error.to_string())?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| error.to_string())?;
    let (head, body) = response
        .split_once("\r\n\r\n")
        .or_else(|| response.split_once("\n\n"))
        .ok_or_else(|| {
            format!(
                "invalid HTTP response: {:?}",
                response.chars().take(80).collect::<String>()
            )
        })?;
    if !head
        .lines()
        .next()
        .is_some_and(|line| line.contains(" 200 "))
    {
        return Err(format!(
            "gateway HTTP failure: {}",
            head.lines().next().unwrap_or("unknown")
        ));
    }
    Ok(body.to_owned())
}

fn parse_quote(body: &str, instrument: InstrumentId) -> Result<QuoteSnapshot, String> {
    let root: Value = serde_json::from_str(body).map_err(|error| error.to_string())?;
    let row = root
        .get("data")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .ok_or("gateway returned no quote")?;
    let requested_market = &instrument.as_str()[..2];
    let response_market = row
        .get("market")
        .and_then(market_text)
        .ok_or("quote missing market")?;
    if response_market != requested_market {
        return Err(format!(
            "market mismatch: requested {requested_market}, got {response_market}"
        ));
    }
    let numbers = |key: &str| -> Result<[f32; 10], String> {
        let values = row
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("quote missing {key}"))?;
        if values.len() != 10 {
            return Err(format!("quote {key} must contain 10 values"));
        }
        let mut output = [0.0; 10];
        for (index, value) in values.iter().enumerate() {
            output[index] = value.as_f64().ok_or_else(|| format!("invalid {key}"))? as f32;
        }
        Ok(output)
    };
    let number = |key: &str| {
        row.get(key)
            .and_then(Value::as_f64)
            .map(|v| v as f32)
            .ok_or_else(|| format!("quote missing {key}"))
    };
    Ok(QuoteSnapshot {
        instrument,
        name: row
            .get("name")
            .and_then(Value::as_str)
            .ok_or("quote missing name")?
            .to_owned(),
        timestamp: parse_datetime(
            row.get("quote_datetime")
                .or_else(|| row.get("datetime"))
                .and_then(Value::as_str)
                .ok_or("quote missing datetime")?,
        )?,
        open: number("open")?,
        high: number("high")?,
        low: number("low")?,
        close: row
            .get("close")
            .or_else(|| row.get("price"))
            .and_then(Value::as_f64)
            .map(|v| v as f32)
            .ok_or("quote missing close")?,
        last_close: number("last_close")?,
        volume: number("volume")?,
        amount: number("amount")?,
        ask_prices: numbers("ask_prices")?,
        ask_volumes: numbers("ask_volumes")?,
        bid_prices: numbers("bid_prices")?,
        bid_volumes: numbers("bid_volumes")?,
    })
}

fn parse_worklist(body: &str) -> Result<Vec<PushQuote>, String> {
    let root: Value = serde_json::from_str(body).map_err(|error| error.to_string())?;
    let rows = root
        .pointer("/payload/data")
        .and_then(Value::as_array)
        .ok_or("worklist missing data")?;
    rows.iter()
        .map(|row| {
            let code = row
                .get("code")
                .and_then(Value::as_str)
                .ok_or("worklist row missing code")?;
            let market = match row.get("market").and_then(Value::as_i64) {
                Some(0) => "SZ",
                Some(1) => "SH",
                Some(2) => "BJ",
                _ => return Err("worklist row has unknown market".to_owned()),
            };
            Ok(PushQuote {
                instrument: format!("{market}{code}"),
                name: row
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                timestamp: row
                    .get("quote_datetime")
                    .or_else(|| row.get("datetime"))
                    .and_then(Value::as_str)
                    .and_then(|value| parse_datetime(value).ok())
                    .unwrap_or(0),
                price: row.get("price").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                last_close: row.get("last_close").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                volume: row.get("volume").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                amount: row.get("amount").and_then(Value::as_f64).unwrap_or(0.0) as f32,
            })
        })
        .collect()
}

fn market_text(value: &Value) -> Option<&'static str> {
    match value {
        Value::String(value) if value.eq_ignore_ascii_case("SH") => Some("SH"),
        Value::String(value) if value.eq_ignore_ascii_case("SZ") => Some("SZ"),
        Value::String(value) if value.eq_ignore_ascii_case("BJ") => Some("BJ"),
        Value::Number(value) if value.as_i64() == Some(0) => Some("SZ"),
        Value::Number(value) if value.as_i64() == Some(1) => Some("SH"),
        _ => None,
    }
}

fn parse_datetime(value: &str) -> Result<u32, String> {
    let digits: Vec<i64> = value
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .take(6)
        .map(|part| part.parse::<i64>())
        .collect::<Result<_, _>>()
        .map_err(|_| "invalid datetime")?;
    if digits.len() != 6 {
        return Err("invalid datetime".to_owned());
    }
    let (year, month, day, hour, minute, second) = (
        digits[0], digits[1], digits[2], digits[3], digits[4], digits[5],
    );
    let y = year - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let days = era * 146097 + (yoe * 365 + yoe / 4 - yoe / 100) + doy - 719468;
    let unix = days * 86400 + hour * 3600 + minute * 60 + second - 8 * 3600;
    u32::try_from(unix).map_err(|_| "datetime outside OEM range".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_cross_market_collision() {
        let body = r#"{"data":[{"market":"SZ"}]}"#;
        let error = parse_quote(body, InstrumentId::parse("SH000001").unwrap()).unwrap_err();
        assert!(error.contains("market mismatch"));
    }

    #[test]
    fn converts_china_local_datetime_to_unix_time() {
        assert_eq!(
            parse_datetime("2026-07-24 15:00:00").unwrap(),
            1_784_876_400
        );
    }

    #[test]
    fn worklist_preserves_market_qualified_codes() {
        let body = r#"{"payload":{"data":[
            {"code":"000001","market":0,"name":"平安银行","price":11.1,"volume":10,"amount":20},
            {"code":"600000","market":1,"name":"浦发银行","price":9.04,"volume":30,"amount":40}
        ]}}"#;
        let rows = parse_worklist(body).unwrap();
        assert_eq!(rows[0].instrument, "SZ000001");
        assert_eq!(rows[1].instrument, "SH600000");
    }
}
