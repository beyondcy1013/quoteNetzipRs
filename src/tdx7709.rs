use crate::tdx_0547::{Tdx0547Body, parse_tdx_0547_body};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::{Duration, Instant};

use encoding_rs::GBK;
use flate2::read::ZlibDecoder;

pub const DEFAULT_HOST: &str = "120.195.71.160";
pub const DEFAULT_PORT: u16 = 7709;
pub const PROBE_HELLO_HEX: &str = "0c0100000000020002001500";
pub const BOOTSTRAP0_BODY_HEX: &str = concat!(
    "0b00a874d31627619ee5dfb79187649c8289749933ae27700357749933ae27700357",
    "749933ae27700357749933ae2770035734e3083347ada5c085dc768d4872f9f37499",
    "33ae27700357749933ae27700357749933ae27700357749933ae277003578d658dbd",
    "bce0ca224ea7ed2855de0d46506d3e50bc102df77a4d9332edf78068749933ae2770",
    "0357749933ae27700357749933ae27700357749933ae27700357749933ae27700357",
    "749933ae27700357749933ae27700357a874d31627619ee5dfb79187649c82897499",
    "33ae27700357749933ae27700357749933ae27700357749933ae27700357749933ae",
    "27700357749933ae27700357749933ae27700357749933ae27700357749933ae2770",
    "0357749933ae27700357"
);
pub const BOOTSTRAP2_BODY_HEX: &str =
    "db0f7464786c6576656c3200009a99e940045400000000000000000000000003";

#[derive(Clone, Debug)]
pub struct Tdx7709BootstrapPacket {
    pub label: &'static str,
    pub op: u16,
    pub sub: u16,
    pub flags: u16,
    pub body_len: usize,
    pub body_hex: &'static str,
    pub request: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Tdx7709Config {
    pub host: String,
    pub port: u16,
    pub read_timeout: Duration,
    pub connect_timeout: Duration,
    pub settle_delay: Duration,
}

impl Default for Tdx7709Config {
    fn default() -> Self {
        Self {
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
            read_timeout: Duration::from_millis(1_000),
            connect_timeout: Duration::from_millis(5_000),
            settle_delay: Duration::from_millis(120),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Tdx7709ServerFrame {
    pub op: u16,
    pub sub: u16,
    pub flags: u16,
    pub tag: u16,
    pub body: Vec<u8>,
    pub original_len: u16,
}

#[derive(Clone, Debug)]
pub struct Tdx7709CodeTableRecord {
    pub market: u8,
    pub code: String,
    pub name: String,
    pub meta: [u8; 13],
}

impl Tdx7709CodeTableRecord {
    pub fn meta_hex(&self) -> String {
        hex_line(&self.meta)
    }

    pub fn decimal_point(&self) -> u8 {
        self.meta[4]
    }

    pub fn pre_close(&self) -> f32 {
        f32::from_le_bytes(self.meta[5..9].try_into().unwrap())
    }
}

#[derive(Clone, Debug)]
pub struct Tdx7709SyncResult {
    pub raw_reply: Vec<u8>,
    pub frames: Vec<Tdx7709ServerFrame>,
    pub records: Vec<Tdx7709CodeTableRecord>,
}

#[derive(Clone, Debug)]
pub struct Tdx7709QuoteRequestItem {
    pub market: u8,
    pub code: String,
    pub token: u32,
}

#[derive(Clone, Debug)]
pub struct Tdx7709LiveQuoteResult {
    pub code_table_reply: Vec<u8>,
    pub code_table_frames: Vec<Tdx7709ServerFrame>,
    pub code_table_records: Vec<Tdx7709CodeTableRecord>,
    pub quote_reply: Vec<u8>,
    pub quote_frames: Vec<Tdx7709ServerFrame>,
    pub quote_bodies: Vec<Tdx0547Body>,
}

#[derive(Clone, Debug)]
pub struct Tdx7709KlineBar {
    pub market: u8,
    pub code: String,
    pub category: u16,
    pub datetime: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub amount: f64,
}

#[derive(Clone, Debug)]
pub struct Tdx7709KlineResult {
    pub code_table_reply: Vec<u8>,
    pub code_table_frames: Vec<Tdx7709ServerFrame>,
    pub code_table_records: Vec<Tdx7709CodeTableRecord>,
    pub kline_reply: Vec<u8>,
    pub kline_frames: Vec<Tdx7709ServerFrame>,
    pub bars: Vec<Tdx7709KlineBar>,
}

#[derive(Clone, Debug)]
pub struct Tdx7709F10Category {
    pub name: String,
    pub filename: String,
    pub start: u32,
    pub length: u32,
}

#[derive(Clone, Debug)]
pub struct Tdx7709F10CategoriesResult {
    pub code_table_reply: Vec<u8>,
    pub code_table_frames: Vec<Tdx7709ServerFrame>,
    pub code_table_records: Vec<Tdx7709CodeTableRecord>,
    pub category_reply: Vec<u8>,
    pub category_frames: Vec<Tdx7709ServerFrame>,
    pub categories: Vec<Tdx7709F10Category>,
}

#[derive(Clone, Debug)]
pub struct Tdx7709F10ContentResult {
    pub code_table_reply: Vec<u8>,
    pub code_table_frames: Vec<Tdx7709ServerFrame>,
    pub code_table_records: Vec<Tdx7709CodeTableRecord>,
    pub content_reply: Vec<u8>,
    pub content_frames: Vec<Tdx7709ServerFrame>,
    pub content: String,
    pub requested_bytes: u32,
}

pub struct Tdx7709Session {
    stream: TcpStream,
    read_timeout: Duration,
    settle_delay: Duration,
    raw_reply: Vec<u8>,
    frames: Vec<Tdx7709ServerFrame>,
    records: Vec<Tdx7709CodeTableRecord>,
}

impl Tdx7709Session {
    pub fn open(config: &Tdx7709Config) -> Result<Self, Box<dyn Error>> {
        open_tdx7709_session_impl(config)
    }

    pub fn sync_result(&self) -> Tdx7709SyncResult {
        Tdx7709SyncResult {
            raw_reply: self.raw_reply.clone(),
            frames: self.frames.clone(),
            records: self.records.clone(),
        }
    }

    pub fn request_live_quotes(
        &mut self,
        items: &[Tdx7709QuoteRequestItem],
    ) -> Result<Tdx7709LiveQuoteResult, Box<dyn Error>> {
        validate_quote_items(items)?;

        let payload = build_quote_0547_request_payload(items)?;
        let request = build_client10_frame(0x400c, 0x2900, 0x0100, &payload);
        let mut quote_reply = Vec::new();
        send_and_collect_live_quotes(
            &mut self.stream,
            &request,
            self.read_timeout,
            max_request_settle_delay(self.settle_delay),
            items,
            &mut quote_reply,
        )?;

        let quote_frames = parse_server16_stream(&quote_reply)?;
        let mut quote_bodies = Vec::new();
        for frame in &quote_frames {
            if frame.tag != 0x0547 {
                continue;
            }
            if let Ok(inflated) = zlib_decode(&frame.body) {
                quote_bodies.push(parse_tdx_0547_body(&inflated));
            }
        }

        if quote_bodies.is_empty() {
            return Err("no zlib-decodable tag=0x0547 quote frames received".into());
        }

        Ok(Tdx7709LiveQuoteResult {
            code_table_reply: self.raw_reply.clone(),
            code_table_frames: self.frames.clone(),
            code_table_records: self.records.clone(),
            quote_reply,
            quote_frames,
            quote_bodies,
        })
    }

    pub fn request_kline(
        &mut self,
        market: u8,
        code: &str,
        category: u16,
        start: u16,
        count: u16,
    ) -> Result<Tdx7709KlineResult, Box<dyn Error>> {
        validate_market_code(market, code)?;

        let request = build_security_bars_packet(category, market.into(), code, start, count);
        let mut kline_reply = Vec::new();
        send_and_collect(
            &mut self.stream,
            &request,
            self.read_timeout,
            max_request_settle_delay(self.settle_delay),
            &mut kline_reply,
        )?;

        let kline_frames = parse_server16_stream(&kline_reply)?;
        let body = decode_single_response_body(&kline_frames)?;
        let bars = parse_kline_body(code, market, category, &body, is_index_code(market, code))?;

        Ok(Tdx7709KlineResult {
            code_table_reply: self.raw_reply.clone(),
            code_table_frames: self.frames.clone(),
            code_table_records: self.records.clone(),
            kline_reply,
            kline_frames,
            bars,
        })
    }

    pub fn request_f10_categories(
        &mut self,
        market: u8,
        code: &str,
    ) -> Result<Tdx7709F10CategoriesResult, Box<dyn Error>> {
        validate_market_code(market, code)?;

        let request = build_company_info_category_packet(market.into(), code);
        let mut category_reply = Vec::new();
        send_and_collect(
            &mut self.stream,
            &request,
            self.read_timeout,
            max_request_settle_delay(self.settle_delay),
            &mut category_reply,
        )?;

        let category_frames = parse_server16_stream(&category_reply)?;
        let body = decode_single_response_body(&category_frames)?;
        let categories = parse_company_info_category_body(&body)?;

        Ok(Tdx7709F10CategoriesResult {
            code_table_reply: self.raw_reply.clone(),
            code_table_frames: self.frames.clone(),
            code_table_records: self.records.clone(),
            category_reply,
            category_frames,
            categories,
        })
    }

    pub fn request_f10_content(
        &mut self,
        market: u8,
        code: &str,
        filename: &str,
        start: u32,
        length: u32,
    ) -> Result<Tdx7709F10ContentResult, Box<dyn Error>> {
        validate_market_code(market, code)?;
        if filename.trim().is_empty() {
            return Err("filename must not be empty".into());
        }

        let mut content_reply = Vec::new();
        let mut content = String::new();
        let mut current_start = start;
        let mut remaining = length.max(1);

        while remaining > 0 {
            let request_size = remaining.min(30_000);
            let request = build_company_info_content_packet(
                market.into(),
                code,
                filename,
                current_start,
                request_size,
            );
            let mut chunk_reply = Vec::new();
            send_and_collect(
                &mut self.stream,
                &request,
                self.read_timeout,
                max_request_settle_delay(self.settle_delay),
                &mut chunk_reply,
            )?;
            content_reply.extend_from_slice(&chunk_reply);
            let frames = parse_server16_stream(&chunk_reply)?;
            let body = decode_single_response_body(&frames)?;
            let chunk = parse_company_info_content_body(&body)?;
            if chunk.is_empty() {
                break;
            }
            content.push_str(&chunk);
            if request_size >= remaining {
                break;
            }
            current_start = current_start.saturating_add(request_size);
            remaining = remaining.saturating_sub(request_size);
        }
        let content_frames = parse_server16_stream(&content_reply)?;

        Ok(Tdx7709F10ContentResult {
            code_table_reply: self.raw_reply.clone(),
            code_table_frames: self.frames.clone(),
            code_table_records: self.records.clone(),
            content_reply,
            content_frames,
            content,
            requested_bytes: length.max(1),
        })
    }
}

pub fn open_tdx7709_session(config: &Tdx7709Config) -> Result<Tdx7709Session, Box<dyn Error>> {
    Tdx7709Session::open(config)
}

pub fn sync_code_table(config: &Tdx7709Config) -> Result<Tdx7709SyncResult, Box<dyn Error>> {
    let session = Tdx7709Session::open(config)?;
    Ok(session.sync_result())
}

pub fn fetch_live_quotes(
    config: &Tdx7709Config,
    items: &[Tdx7709QuoteRequestItem],
) -> Result<Tdx7709LiveQuoteResult, Box<dyn Error>> {
    let mut session = Tdx7709Session::open(config)?;
    session.request_live_quotes(items)
}

pub fn fetch_kline(
    config: &Tdx7709Config,
    market: u8,
    code: &str,
    category: u16,
    start: u16,
    count: u16,
) -> Result<Tdx7709KlineResult, Box<dyn Error>> {
    let mut session = Tdx7709Session::open(config)?;
    session.request_kline(market, code, category, start, count)
}

pub fn fetch_f10_categories(
    config: &Tdx7709Config,
    market: u8,
    code: &str,
) -> Result<Tdx7709F10CategoriesResult, Box<dyn Error>> {
    let mut session = Tdx7709Session::open(config)?;
    session.request_f10_categories(market, code)
}

pub fn fetch_f10_content(
    config: &Tdx7709Config,
    market: u8,
    code: &str,
    filename: &str,
    start: u32,
    length: u32,
) -> Result<Tdx7709F10ContentResult, Box<dyn Error>> {
    let mut session = Tdx7709Session::open(config)?;
    session.request_f10_content(market, code, filename, start, length)
}

pub fn build_probe_hello() -> Result<Vec<u8>, Box<dyn Error>> {
    decode_hex(PROBE_HELLO_HEX)
}

pub fn build_bootstrap_packets() -> Result<Vec<Tdx7709BootstrapPacket>, Box<dyn Error>> {
    let bootstrap0 = decode_hex(BOOTSTRAP0_BODY_HEX)?;
    let bootstrap1 = vec![0x0d, 0x00, 0x00];
    let bootstrap2 = decode_hex(BOOTSTRAP2_BODY_HEX)?;

    Ok(vec![
        Tdx7709BootstrapPacket {
            label: "probe.hello",
            op: 0x010c,
            sub: 0x0000,
            flags: 0x0002,
            body_len: 2,
            body_hex: "1500",
            request: build_probe_hello()?,
        },
        Tdx7709BootstrapPacket {
            label: "bootstrap.main-site-validate",
            op: 0x010c,
            sub: 0x7b00,
            flags: 0x0100,
            body_len: bootstrap0.len(),
            body_hex: BOOTSTRAP0_BODY_HEX,
            request: build_client10_frame(0x010c, 0x7b00, 0x0100, &bootstrap0),
        },
        Tdx7709BootstrapPacket {
            label: "bootstrap.server-info",
            op: 0x020c,
            sub: 0x9400,
            flags: 0x0100,
            body_len: bootstrap1.len(),
            body_hex: "0d0000",
            request: build_client10_frame(0x020c, 0x9400, 0x0100, &bootstrap1),
        },
        Tdx7709BootstrapPacket {
            label: "bootstrap.ticket-register",
            op: 0x030c,
            sub: 0x9900,
            flags: 0x0100,
            body_len: bootstrap2.len(),
            body_hex: BOOTSTRAP2_BODY_HEX,
            request: build_client10_frame(0x030c, 0x9900, 0x0100, &bootstrap2),
        },
    ])
}

pub fn write_code_table_csv(
    path: impl AsRef<Path>,
    records: &[Tdx7709CodeTableRecord],
) -> Result<(), Box<dyn Error>> {
    let mut out = String::from("code,name,meta_hex\n");
    for record in records {
        out.push_str(&csv_escape(&record.code));
        out.push(',');
        out.push_str(&csv_escape(&record.name));
        out.push(',');
        out.push_str(&csv_escape(&record.meta_hex()));
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(())
}

pub fn ascii_preview_from_zlib(bytes: &[u8]) -> Option<String> {
    let inflated = zlib_decode(bytes).ok()?;
    let parts = extract_ascii_strings(&inflated, 6);
    if parts.is_empty() {
        return None;
    }
    Some(parts.into_iter().take(4).collect::<Vec<_>>().join(", "))
}

fn build_client10_frame(op: u16, sub: u16, flags: u16, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(10 + body.len());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&sub.to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&(body.len() as u16).to_le_bytes());
    out.extend_from_slice(&(body.len() as u16).to_le_bytes());
    out.extend_from_slice(body);
    out
}

fn build_quote_0547_request_payload(
    items: &[Tdx7709QuoteRequestItem],
) -> Result<Vec<u8>, Box<dyn Error>> {
    if items.is_empty() {
        return Err("quote payload requires at least one item".into());
    }
    if items.len() > 100 {
        return Err(format!(
            "quote payload supports at most 100 items, got {}",
            items.len()
        )
        .into());
    }

    let mut out = Vec::with_capacity(4 + items.len() * 11);
    out.extend_from_slice(&0x0547u16.to_le_bytes());
    out.extend_from_slice(&(items.len() as u16).to_le_bytes());
    for item in items {
        if item.market > 2 {
            return Err(format!("unsupported market flag {}", item.market).into());
        }
        if item.code.len() != 6 || !item.code.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(format!("quote code must be exactly 6 digits, got {}", item.code).into());
        }
        out.push(item.market);
        out.extend_from_slice(item.code.as_bytes());
        out.extend_from_slice(&item.token.to_le_bytes());
    }
    Ok(out)
}

fn build_security_bars_packet(
    category: u16,
    market: u16,
    code: &str,
    start: u16,
    count: u16,
) -> Vec<u8> {
    let mut packet = Vec::with_capacity(30);
    packet.extend_from_slice(&0x010cu16.to_le_bytes());
    packet.extend_from_slice(&0x0101_6408u32.to_le_bytes());
    packet.extend_from_slice(&0x001cu16.to_le_bytes());
    packet.extend_from_slice(&0x001cu16.to_le_bytes());
    packet.extend_from_slice(&0x052du16.to_le_bytes());
    packet.extend_from_slice(&market.to_le_bytes());
    packet.extend_from_slice(&padded_ascii_bytes::<6>(code.as_bytes()));
    packet.extend_from_slice(&category.to_le_bytes());
    packet.extend_from_slice(&1u16.to_le_bytes());
    packet.extend_from_slice(&start.to_le_bytes());
    packet.extend_from_slice(&count.to_le_bytes());
    packet.extend_from_slice(&0u32.to_le_bytes());
    packet.extend_from_slice(&0u32.to_le_bytes());
    packet.extend_from_slice(&0u16.to_le_bytes());
    packet
}

fn build_company_info_category_packet(market: u16, code: &str) -> Vec<u8> {
    let mut packet = decode_hex("0c0f109b00010e000e00cf02").expect("valid static hex");
    packet.extend_from_slice(&market.to_le_bytes());
    packet.extend_from_slice(&padded_ascii_bytes::<6>(code.as_bytes()));
    packet.extend_from_slice(&0u32.to_le_bytes());
    packet
}

fn build_company_info_content_packet(
    market: u16,
    code: &str,
    filename: &str,
    start: u32,
    length: u32,
) -> Vec<u8> {
    let mut packet = decode_hex("0c07109c000168006800d002").expect("valid static hex");
    packet.extend_from_slice(&market.to_le_bytes());
    packet.extend_from_slice(&padded_ascii_bytes::<6>(code.as_bytes()));
    packet.extend_from_slice(&0u16.to_le_bytes());
    packet.extend_from_slice(&padded_ascii_bytes::<80>(filename.as_bytes()));
    packet.extend_from_slice(&start.to_le_bytes());
    packet.extend_from_slice(&length.to_le_bytes());
    packet.extend_from_slice(&0u32.to_le_bytes());
    packet
}

fn validate_market_code(market: u8, code: &str) -> Result<(), Box<dyn Error>> {
    if market > 2 {
        return Err(format!("unsupported market flag {market}").into());
    }
    if code.len() != 6 || !code.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("code must be exactly 6 digits, got {code}").into());
    }
    Ok(())
}

fn padded_ascii_bytes<const N: usize>(bytes: &[u8]) -> [u8; N] {
    let mut out = [0u8; N];
    let len = bytes.len().min(N);
    out[..len].copy_from_slice(&bytes[..len]);
    out
}

fn decode_single_response_body(frames: &[Tdx7709ServerFrame]) -> Result<Vec<u8>, Box<dyn Error>> {
    let frame = frames
        .iter()
        .find(|frame| !frame.body.is_empty())
        .ok_or("no non-empty response frames received")?;
    if frame.body.len() == frame.original_len as usize {
        return Ok(frame.body.clone());
    }
    zlib_decode(&frame.body)
}

fn parse_kline_body(
    code: &str,
    market: u8,
    category: u16,
    body: &[u8],
    is_index: bool,
) -> Result<Vec<Tdx7709KlineBar>, Box<dyn Error>> {
    if body.len() < 2 {
        return Err("kline body too short for count".into());
    }

    let count = u16::from_le_bytes([body[0], body[1]]) as usize;
    let mut pos = 2usize;
    let mut pre_diff_base = 0i32;
    let mut bars = Vec::with_capacity(count);

    for _ in 0..count {
        let datetime = parse_kline_datetime(category, body, &mut pos)?;
        let open_diff = parse_price_varint(body, &mut pos)?;
        let close_diff = parse_price_varint(body, &mut pos)?;
        let high_diff = parse_price_varint(body, &mut pos)?;
        let low_diff = parse_price_varint(body, &mut pos)?;

        if pos + 8 > body.len() {
            return Err("kline body truncated while reading volume/amount".into());
        }
        let volume_raw = u32::from_le_bytes(body[pos..pos + 4].try_into().unwrap());
        let amount_raw = u32::from_le_bytes(body[pos + 4..pos + 8].try_into().unwrap());
        pos += 8;

        let open = calculate_price_1000(open_diff, pre_diff_base);
        let open_base = open_diff + pre_diff_base;
        let close = calculate_price_1000(open_base, close_diff);
        let high = calculate_price_1000(open_base, high_diff);
        let low = calculate_price_1000(open_base, low_diff);
        pre_diff_base = open_base + close_diff;

        if is_index {
            if pos + 4 > body.len() {
                return Err("kline body truncated while reading index payload".into());
            }
            pos += 4;
        }

        bars.push(Tdx7709KlineBar {
            market,
            code: code.to_string(),
            category,
            datetime,
            open,
            high,
            low,
            close,
            volume: parse_volume_f64(volume_raw),
            amount: parse_volume_f64(amount_raw),
        });
    }

    Ok(bars)
}

fn parse_company_info_category_body(
    body: &[u8],
) -> Result<Vec<Tdx7709F10Category>, Box<dyn Error>> {
    if body.len() < 2 {
        return Err("company info category body too short for count".into());
    }

    let count = u16::from_le_bytes([body[0], body[1]]) as usize;
    let mut pos = 2usize;
    let mut rows = Vec::with_capacity(count);
    for _ in 0..count {
        if pos + 152 > body.len() {
            break;
        }
        let name = decode_gbk_trimmed(&body[pos..pos + 64]);
        pos += 64;
        let filename = decode_gbk_trimmed(&body[pos..pos + 80]);
        pos += 80;
        let start = u32::from_le_bytes(body[pos..pos + 4].try_into().unwrap());
        pos += 4;
        let length = u32::from_le_bytes(body[pos..pos + 4].try_into().unwrap());
        pos += 4;
        rows.push(Tdx7709F10Category {
            name,
            filename,
            start,
            length,
        });
    }
    Ok(rows)
}

fn parse_company_info_content_body(body: &[u8]) -> Result<String, Box<dyn Error>> {
    if body.len() < 12 {
        return Err("company info content body too short".into());
    }
    let content_len = u16::from_le_bytes([body[10], body[11]]) as usize;
    if content_len == 0 {
        return Ok(String::new());
    }
    if body.len() < 12 + content_len {
        return Err("company info content body truncated".into());
    }
    let (decoded, _, _) = GBK.decode(&body[12..12 + content_len]);
    Ok(decoded.into_owned())
}

fn parse_kline_datetime(
    category: u16,
    body: &[u8],
    pos: &mut usize,
) -> Result<String, Box<dyn Error>> {
    if *pos + 4 > body.len() {
        return Err("kline body truncated while reading datetime".into());
    }

    let text = if category < 4 || category == 7 || category == 8 {
        let zip_day = u16::from_le_bytes(body[*pos..*pos + 2].try_into().unwrap());
        let minutes = u16::from_le_bytes(body[*pos + 2..*pos + 4].try_into().unwrap());
        let month = ((zip_day % 2048) / 100) as u8;
        let year = i32::from(zip_day >> 11) + 2004;
        let day = ((zip_day % 2048) % 100) as u8;
        let hour = (minutes / 60) as u8;
        let minute = (minutes % 60) as u8;
        format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:00")
    } else {
        let zip_day = u32::from_le_bytes(body[*pos..*pos + 4].try_into().unwrap());
        let year = zip_day / 10_000;
        let month = (zip_day % 10_000) / 100;
        let day = zip_day % 100;
        format!("{year:04}-{month:02}-{day:02} 15:00:00")
    };

    *pos += 4;
    Ok(text)
}

fn parse_price_varint(data: &[u8], pos: &mut usize) -> Result<i32, Box<dyn Error>> {
    if *pos >= data.len() {
        return Err("body truncated while reading price".into());
    }

    let mut shift = 6;
    let mut byte = data[*pos];
    let mut value = i32::from(byte & 0x3f);
    let negative = byte & 0x40 != 0;

    if byte & 0x80 != 0 {
        loop {
            *pos += 1;
            if *pos >= data.len() {
                return Err("body truncated inside varint price".into());
            }
            byte = data[*pos];
            value += i32::from(byte & 0x7f) << shift;
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
        }
    }

    *pos += 1;
    if negative {
        value = -value;
    }
    Ok(value)
}

fn calculate_price_1000(base: i32, diff: i32) -> f64 {
    f64::from(base + diff) / 1000.0
}

fn parse_volume_f64(raw: u32) -> f64 {
    if raw == 0 {
        return 0.0;
    }

    let logpoint = (raw >> 24) as u8;
    let hleax = ((raw >> 16) & 0xff) as u8;
    let lheax = ((raw >> 8) & 0xff) as u8;
    let lleax = (raw & 0xff) as u8;

    let dw_ecx = i32::from(logpoint) * 2 - 0x7f;
    let dw_edx = i32::from(logpoint) * 2 - 0x86;
    let dw_esi = i32::from(logpoint) * 2 - 0x8e;
    let dw_eax = i32::from(logpoint) * 2 - 0x96;

    let magnitude = 2_f64.powi(dw_ecx.unsigned_abs() as i32);
    let base = if dw_ecx < 0 {
        1.0 / magnitude
    } else {
        magnitude
    };

    let mid = if hleax > 0x80 {
        let tmp = 2_f64.powi(dw_edx + 1);
        2_f64.powi(dw_edx) * 128.0 + f64::from(hleax & 0x7f) * tmp
    } else if dw_edx >= 0 {
        2_f64.powi(dw_edx) * f64::from(hleax)
    } else {
        (1.0 / 2_f64.powi(-dw_edx)) * f64::from(hleax)
    };

    let mut lower_mid = 2_f64.powi(dw_esi) * f64::from(lheax);
    let mut lower = 2_f64.powi(dw_eax) * f64::from(lleax);
    if hleax & 0x80 != 0 {
        lower_mid *= 2.0;
        lower *= 2.0;
    }

    base + mid + lower_mid + lower
}

fn decode_gbk_trimmed(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    let (decoded, _, _) = GBK.decode(&bytes[..end]);
    decoded.trim().to_string()
}

fn is_index_code(market: u8, code: &str) -> bool {
    match market {
        1 => code.starts_with("000"),
        0 => code.starts_with("399"),
        2 => code.starts_with("899"),
        _ => false,
    }
}

fn build_code_table_chunk_request(bucket: u16, offset: u16) -> [u8; 6] {
    let mut out = [0u8; 6];
    out[0..2].copy_from_slice(&0x0450u16.to_le_bytes());
    out[2..4].copy_from_slice(&bucket.to_le_bytes());
    out[4..6].copy_from_slice(&offset.to_le_bytes());
    out
}

fn open_tdx7709_session_impl(config: &Tdx7709Config) -> Result<Tdx7709Session, Box<dyn Error>> {
    let bootstrap0 = decode_hex(BOOTSTRAP0_BODY_HEX)?;
    let bootstrap2 = decode_hex(BOOTSTRAP2_BODY_HEX)?;

    let socket_addr = format!("{}:{}", config.host, config.port)
        .to_socket_addrs()?
        .next()
        .ok_or("failed to resolve target address")?;
    let mut stream = TcpStream::connect_timeout(&socket_addr, config.connect_timeout)?;
    stream.set_write_timeout(Some(config.connect_timeout))?;

    let mut reply_stream = Vec::new();
    send_and_collect(
        &mut stream,
        &build_client10_frame(0x010c, 0x7b00, 0x0100, &bootstrap0),
        config.read_timeout,
        config.settle_delay,
        &mut reply_stream,
    )?;
    send_and_collect(
        &mut stream,
        &build_client10_frame(0x020c, 0x9400, 0x0100, &[0x0d, 0x00, 0x00]),
        config.read_timeout,
        config.settle_delay,
        &mut reply_stream,
    )?;
    send_and_collect(
        &mut stream,
        &build_client10_frame(0x030c, 0x9900, 0x0100, &bootstrap2),
        config.read_timeout,
        config.settle_delay,
        &mut reply_stream,
    )?;

    let mut chunk_index = 0u16;
    for bucket in 0u16..=1 {
        let max_offset = if bucket == 0 { 22_000 } else { 26_000 };
        for offset in (0u16..=max_offset).step_by(1_000) {
            let request = build_client10_frame(
                0x040c + (chunk_index << 8),
                if bucket == 0 { 0x6d00 } else { 0x6e00 },
                0x0100,
                &build_code_table_chunk_request(bucket, offset),
            );
            send_and_collect(
                &mut stream,
                &request,
                config.read_timeout,
                config.settle_delay,
                &mut reply_stream,
            )?;
            chunk_index += 1;
        }
    }

    let frames = parse_server16_stream(&reply_stream)?;
    if frames.len() < 53 {
        return Err(format!("expected at least 53 reply frames, got {}", frames.len()).into());
    }

    let mut records = BTreeMap::<(u8, String), Tdx7709CodeTableRecord>::new();
    for (chunk_index, frame) in frames.iter().skip(3).enumerate() {
        let market = if chunk_index < 23 { 0 } else { 1 };
        ingest_code_table_reply(frame, market, &mut records)?;
    }

    Ok(Tdx7709Session {
        stream,
        read_timeout: config.read_timeout,
        settle_delay: config.settle_delay,
        raw_reply: reply_stream,
        frames,
        records: records.into_values().collect(),
    })
}

fn validate_quote_items(items: &[Tdx7709QuoteRequestItem]) -> Result<(), Box<dyn Error>> {
    if items.is_empty() {
        return Err("quote request requires at least one item".into());
    }
    if items.len() > 100 {
        return Err(format!(
            "quote request supports at most 100 items, got {}",
            items.len()
        )
        .into());
    }
    Ok(())
}

fn max_request_settle_delay(settle_delay: Duration) -> Duration {
    settle_delay.max(Duration::from_millis(250))
}

fn live_quote_bodies_cover_items(
    bodies: &[Tdx0547Body],
    items: &[Tdx7709QuoteRequestItem],
) -> bool {
    let expected = items
        .iter()
        .map(|item| (item.market, item.code.as_str()))
        .collect::<BTreeSet<_>>();
    let received = bodies
        .iter()
        .flat_map(|body| &body.records)
        .map(|record| (record.market, record.code.as_str()))
        .collect::<BTreeSet<_>>();
    expected.is_subset(&received)
}

fn send_and_collect(
    stream: &mut TcpStream,
    request: &[u8],
    read_timeout: Duration,
    settle_delay: Duration,
    out: &mut Vec<u8>,
) -> Result<(), Box<dyn Error>> {
    stream.write_all(request)?;
    let deadline = Instant::now() + settle_delay.max(Duration::from_millis(1));
    let mut buf = [0u8; 65_536];
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        stream.set_read_timeout(Some(
            remaining.min(read_timeout.max(Duration::from_millis(1))),
        ))?;
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

fn send_and_collect_live_quotes(
    stream: &mut TcpStream,
    request: &[u8],
    read_timeout: Duration,
    settle_delay: Duration,
    items: &[Tdx7709QuoteRequestItem],
    out: &mut Vec<u8>,
) -> Result<(), Box<dyn Error>> {
    stream.write_all(request)?;
    let deadline = Instant::now() + settle_delay.max(Duration::from_millis(1));
    let mut buf = [0u8; 65_536];
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        stream.set_read_timeout(Some(
            remaining.min(read_timeout.max(Duration::from_millis(1))),
        ))?;
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                out.extend_from_slice(&buf[..n]);
                if live_quote_reply_covers_items(out, items) {
                    break;
                }
            }
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

fn live_quote_reply_covers_items(bytes: &[u8], items: &[Tdx7709QuoteRequestItem]) -> bool {
    let Ok(frames) = parse_server16_stream(bytes) else {
        return false;
    };
    let mut bodies = Vec::new();
    for frame in frames.iter().filter(|frame| frame.tag == 0x0547) {
        let Ok(inflated) = zlib_decode(&frame.body) else {
            return false;
        };
        bodies.push(parse_tdx_0547_body(&inflated));
    }
    !bodies.is_empty() && live_quote_bodies_cover_items(&bodies, items)
}

fn parse_server16_stream(bytes: &[u8]) -> Result<Vec<Tdx7709ServerFrame>, Box<dyn Error>> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while offset < bytes.len() {
        if offset + 16 > bytes.len() {
            return Err(format!("truncated server16 frame at offset {offset}").into());
        }
        if bytes[offset..offset + 4] != [0xb1, 0xcb, 0x74, 0x00] {
            return Err(format!("unexpected server16 magic at offset {offset}").into());
        }
        let body_len = u16::from_le_bytes([bytes[offset + 12], bytes[offset + 13]]) as usize;
        let total = 16 + body_len;
        if offset + total > bytes.len() {
            return Err(format!("truncated server16 body at offset {offset}").into());
        }
        out.push(Tdx7709ServerFrame {
            op: u16::from_le_bytes([bytes[offset + 4], bytes[offset + 5]]),
            sub: u16::from_le_bytes([bytes[offset + 6], bytes[offset + 7]]),
            flags: u16::from_le_bytes([bytes[offset + 8], bytes[offset + 9]]),
            tag: u16::from_le_bytes([bytes[offset + 10], bytes[offset + 11]]),
            body: bytes[offset + 16..offset + total].to_vec(),
            original_len: u16::from_le_bytes([bytes[offset + 14], bytes[offset + 15]]),
        });
        offset += total;
    }
    Ok(out)
}

fn ingest_code_table_reply(
    frame: &Tdx7709ServerFrame,
    market: u8,
    records: &mut BTreeMap<(u8, String), Tdx7709CodeTableRecord>,
) -> Result<(), Box<dyn Error>> {
    if frame.tag != 0x0450 {
        return Ok(());
    }

    let inflated = zlib_decode(&frame.body)?;
    if inflated.len() != frame.original_len as usize {
        return Err(format!(
            "inflated size mismatch: got {} expected {}",
            inflated.len(),
            frame.original_len
        )
        .into());
    }

    let count = u16::from_le_bytes([inflated[0], inflated[1]]) as usize;
    let expected = 2 + count * 29;
    if inflated.len() != expected {
        return Err(format!(
            "unexpected code-table size: count={} inflated={}",
            count,
            inflated.len()
        )
        .into());
    }

    for index in 0..count {
        let start = 2 + index * 29;
        let record = parse_code_table_record(market, &inflated[start..start + 29])?;
        records.insert((record.market, record.code.clone()), record);
    }
    Ok(())
}

fn parse_code_table_record(
    market: u8,
    bytes: &[u8],
) -> Result<Tdx7709CodeTableRecord, Box<dyn Error>> {
    if bytes.len() != 29 {
        return Err(format!("unexpected code-table row length {}", bytes.len()).into());
    }

    let digits = std::str::from_utf8(&bytes[0..6])?;
    let suffix = bytes[6];
    let code = if suffix.is_ascii_alphanumeric() {
        format!("{digits}{}", suffix as char)
    } else {
        digits.to_string()
    };
    let name = decode_gbk_name_slot(&bytes[8..16])
        .ok_or_else(|| format!("failed to decode name for code {code}"))?;
    let mut meta = [0u8; 13];
    meta.copy_from_slice(&bytes[16..29]);

    Ok(Tdx7709CodeTableRecord {
        market,
        code,
        name,
        meta,
    })
}

fn decode_gbk_name_slot(bytes: &[u8]) -> Option<String> {
    let field = bytes
        .iter()
        .position(|byte| *byte == 0)
        .map(|end| &bytes[..end])
        .unwrap_or(bytes);

    for end in (0..=field.len()).rev() {
        let (decoded, _, had_errors) = GBK.decode(&field[..end]);
        if !had_errors && !decoded.is_empty() {
            return Some(decoded.into_owned());
        }
    }

    None
}

fn zlib_decode(bytes: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut decoder = ZlibDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    Ok(out)
}

fn extract_ascii_strings(bytes: &[u8], min_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = Vec::new();
    for &byte in bytes {
        if byte.is_ascii_graphic() || byte == b' ' {
            current.push(byte);
            continue;
        }
        if current.len() >= min_len {
            out.push(String::from_utf8_lossy(&current).to_string());
        }
        current.clear();
    }
    if current.len() >= min_len {
        out.push(String::from_utf8_lossy(&current).to_string());
    }
    out
}

fn decode_hex(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let compact: String = input
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect();
    if compact.len() % 2 != 0 {
        return Err("hex string must contain an even number of digits".into());
    }
    let mut out = Vec::with_capacity(compact.len() / 2);
    for chunk in compact.as_bytes().chunks_exact(2) {
        let text = std::str::from_utf8(chunk)?;
        out.push(u8::from_str_radix(text, 16)?);
    }
    Ok(out)
}

fn csv_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

fn hex_line(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{
        build_bootstrap_packets, build_company_info_category_packet, build_probe_hello,
        build_security_bars_packet, live_quote_bodies_cover_items,
        parse_company_info_category_body, parse_company_info_content_body, parse_kline_body,
    };
    use crate::{Tdx0547Body, Tdx0547Record, Tdx7709QuoteRequestItem};

    #[test]
    fn complete_quote_reply_requires_every_requested_market_code_pair() {
        let body = Tdx0547Body {
            xor93_count: Some(2),
            printable_ratio: 0.0,
            records: vec![
                Tdx0547Record {
                    start: 2,
                    len: 100,
                    market: 0,
                    code: "000001".to_string(),
                    active1_raw: None,
                    time_hhmmss_raw: None,
                    extra0_raw: None,
                    extra0_time_hhmmss: None,
                    extra1_raw: None,
                    extra2_raw: None,
                    extra3_raw: None,
                    volume: None,
                    current_volume: None,
                    amount: None,
                    amount_raw: None,
                    quote_head: None,
                },
                Tdx0547Record {
                    start: 102,
                    len: 100,
                    market: 1,
                    code: "600000".to_string(),
                    active1_raw: None,
                    time_hhmmss_raw: None,
                    extra0_raw: None,
                    extra0_time_hhmmss: None,
                    extra1_raw: None,
                    extra2_raw: None,
                    extra3_raw: None,
                    volume: None,
                    current_volume: None,
                    amount: None,
                    amount_raw: None,
                    quote_head: None,
                },
            ],
        };
        let requested = vec![
            Tdx7709QuoteRequestItem {
                market: 0,
                code: "000001".to_string(),
                token: 0,
            },
            Tdx7709QuoteRequestItem {
                market: 1,
                code: "600000".to_string(),
                token: 0,
            },
        ];

        assert!(live_quote_bodies_cover_items(&[body.clone()], &requested));
        assert!(!live_quote_bodies_cover_items(
            &[body],
            &[Tdx7709QuoteRequestItem {
                market: 0,
                code: "600000".to_string(),
                token: 0
            }],
        ));
    }

    #[test]
    fn probe_hello_matches_known_bytes() {
        let probe = build_probe_hello().expect("probe hex");
        assert_eq!(
            probe,
            vec![
                0x0c, 0x01, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00, 0x15, 0x00
            ]
        );
    }

    #[test]
    fn bootstrap_packets_have_expected_lengths() {
        let packets = build_bootstrap_packets().expect("bootstrap packets");
        assert_eq!(packets.len(), 4);
        assert_eq!(packets[0].label, "probe.hello");
        assert_eq!(packets[0].request.len(), 12);
        assert_eq!(packets[1].label, "bootstrap.main-site-validate");
        assert_eq!(packets[1].request.len(), 292);
        assert_eq!(packets[2].label, "bootstrap.server-info");
        assert_eq!(packets[2].request.len(), 13);
        assert_eq!(packets[3].label, "bootstrap.ticket-register");
        assert_eq!(packets[3].request.len(), 42);
    }

    #[test]
    fn kline_parser_decodes_single_minute_bar() {
        let mut body = Vec::new();
        body.extend_from_slice(&1u16.to_le_bytes());
        let zip_day = ((2026 - 2004) as u16) << 11 | 3 * 100 + 10;
        body.extend_from_slice(&zip_day.to_le_bytes());
        body.extend_from_slice(&(9 * 60 + 31u16).to_le_bytes());
        body.extend_from_slice(&encode_price(10));
        body.extend_from_slice(&encode_price(1));
        body.extend_from_slice(&encode_price(2));
        body.extend_from_slice(&encode_price(-1));
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());

        let rows = parse_kline_body("000001", 0, 7, &body, false).expect("kline rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].datetime, "2026-03-10 09:31:00");
        assert_eq!(rows[0].open, 0.010);
        assert_eq!(rows[0].close, 0.011);
        assert_eq!(rows[0].high, 0.012);
        assert_eq!(rows[0].low, 0.009);
    }

    #[test]
    fn company_info_category_parser_decodes_single_row() {
        let mut body = Vec::new();
        body.extend_from_slice(&1u16.to_le_bytes());
        let mut name = [0u8; 64];
        name[..8].copy_from_slice(b"Profile ");
        body.extend_from_slice(&name);
        let mut filename = [0u8; 80];
        filename[..10].copy_from_slice(b"000001.txt");
        body.extend_from_slice(&filename);
        body.extend_from_slice(&100u32.to_le_bytes());
        body.extend_from_slice(&200u32.to_le_bytes());

        let rows = parse_company_info_category_body(&body).expect("category rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Profile");
        assert_eq!(rows[0].filename, "000001.txt");
        assert_eq!(rows[0].start, 100);
        assert_eq!(rows[0].length, 200);
    }

    #[test]
    fn company_info_content_parser_decodes_single_chunk() {
        let mut body = vec![0u8; 10];
        body.extend_from_slice(&5u16.to_le_bytes());
        body.extend_from_slice(b"Hello");
        let text = parse_company_info_content_body(&body).expect("content");
        assert_eq!(text, "Hello");
    }

    #[test]
    fn security_bars_packet_contains_expected_tag_and_code() {
        let packet = build_security_bars_packet(7, 1, "600000", 0, 32);
        assert_eq!(&packet[0..2], &0x010cu16.to_le_bytes());
        assert_eq!(&packet[10..12], &0x052du16.to_le_bytes());
        assert_eq!(&packet[14..20], b"600000");
    }

    #[test]
    fn company_info_category_packet_contains_expected_tag_and_code() {
        let packet = build_company_info_category_packet(1, "600000");
        assert_eq!(&packet[0..2], &0x0f0cu16.to_le_bytes());
        assert_eq!(&packet[10..12], &0x02cfu16.to_le_bytes());
        assert_eq!(&packet[14..20], b"600000");
    }

    fn encode_price(value: i32) -> Vec<u8> {
        let negative = value < 0;
        let mut remaining = value.unsigned_abs();
        let mut bytes = Vec::new();

        let mut first = (remaining as u8) & 0x3f;
        remaining >>= 6;
        if negative {
            first |= 0x40;
        }
        if remaining > 0 {
            first |= 0x80;
        }
        bytes.push(first);

        while remaining > 0 {
            let mut next = (remaining as u8) & 0x7f;
            remaining >>= 7;
            if remaining > 0 {
                next |= 0x80;
            }
            bytes.push(next);
        }

        bytes
    }
}
