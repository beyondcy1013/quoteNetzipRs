use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::http::header;
use axum::response::{Html, IntoResponse, Response};
use axum::{
    Json, Router,
    routing::{get, post},
};
use netzipapi_rust_demo::tdx_0547_scheduler::{QuoteRenewalScheduler, RenewalRateGate};
use netzipapi_rust_demo::tdx_push_coalescer::{TdxPushCoalescer, TdxPushEvent};
use netzipapi_rust_demo::{
    FIN_GETTER_UNRESOLVED_IDS, ProtoProbeConfig, ProtoProbeEncoding as ProbeEncoding,
    QuoteReplayConfig, SH_FIN_URL, SZ_FIN_URL, Tdx7709Config, Tdx7709QuoteRequestItem,
    Tdx7709Session, analyze_auth_7100_client_shell_sample, analyze_auth_7100_flow_matrix,
    analyze_auth_7100_flow_matrix_sample, analyze_auth_7100_server_sample,
    analyze_auth_7100_shell_correlation_from_pcap, analyze_auth_7100_shell_correlation_sample,
    analyze_local_2000_log_file, analyze_local_2000_vs_auth7100,
    analyze_local_2000_vs_auth7100_sample, analyze_stream_file, build_bootstrap_packets,
    build_probe_hello, compare_blob_files, connect_legacy_panel, disconnect_legacy_panel,
    fetch_f10_categories, fetch_f10_content, fetch_kline, fetch_live_quotes, fin_getter_specs,
    load_legacy_panel_bootstrap, parse_answer_buffer, parse_fin_file, parse_quote_segments,
    parse_tdx_0547_body, probe_legacy_servers, probe_proto, query_tdx_0547_records,
    replay_quote_file, save_legacy_panel_config, scan_quote_frame_file,
    summarize_from_answer_buffer, summarize_pcap_file, sync_code_table,
    tdx_0547_extra0_time_hint_seconds, tdx_0547_format_hhmmss_raw, tdx_0547_hhmmss_raw_to_seconds,
    tdx_0547_market_name, tdx_0547_normalize_quote_head, tdx_0547_public_time_hhmmss,
    tdx_0547_record_symbol, write_code_table_csv, write_fin_csv,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

static FULL_PUSH_SESSION_CACHE: OnceLock<Mutex<BTreeMap<String, Tdx7709Session>>> = OnceLock::new();
static FULL_PUSH_LAST_PUBLISHED_AT: OnceLock<Mutex<BTreeMap<String, String>>> = OnceLock::new();
static NETZIP_RUST_TCP_CLIENTS: OnceLock<
    Mutex<BTreeMap<String, Arc<Mutex<Option<NetzipRustTcpClient>>>>>,
> = OnceLock::new();
static NETZIP_RUST_PUBLISH_METRICS: OnceLock<Mutex<BTreeMap<String, GatewayPublishMetrics>>> =
    OnceLock::new();
static NATIVE_PUSH_CODE_TABLE_LOOKUP: OnceLock<BTreeMap<String, Quote0547CodeTableInfo>> =
    OnceLock::new();

const NETZIP_RUST_TCP_MAGIC: &[u8; 4] = b"NZRS";
const NETZIP_RUST_TCP_VERSION: u16 = 2;
const NETZIP_RUST_TCP_MSG_QUOTES: u16 = 1;
const NETZIP_RUST_TCP_MSG_ACK: u16 = 2;
const NETZIP_RUST_TCP_HEADER_LEN: usize = 24;
const NETZIP_RUST_TCP_MAX_BODY: usize = 16 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
struct NetzipRustTcpAck {
    sequence: u64,
    applied: u32,
    cached_quotes: u32,
}

struct NetzipRustTcpSendResult {
    ack: NetzipRustTcpAck,
    wire_bytes: usize,
    json_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GatewayPublishLane {
    Main,
    Bj,
    Manual,
}

impl GatewayPublishLane {
    fn label(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Bj => "bj",
            Self::Manual => "manual",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
struct GatewayPublishMetrics {
    attempts: u64,
    tcp_successes: u64,
    tcp_failures: u64,
    http_fallbacks: u64,
    successes: u64,
    failures: u64,
    consecutive_failures: u64,
    last_transport: Option<String>,
    last_error: Option<String>,
}

struct NetzipRustTcpClient {
    addr: String,
    stream: TcpStream,
    next_sequence: u64,
}

fn encode_netzip_rust_tcp_frame(
    message_type: u16,
    sequence: u64,
    body: &[u8],
) -> Result<Vec<u8>, ApiError> {
    if body.len() > NETZIP_RUST_TCP_MAX_BODY {
        return Err(ApiError::internal(format!(
            "netzip rust tcp body too large: {}",
            body.len()
        )));
    }
    let body_len = u32::try_from(body.len())
        .map_err(|_| ApiError::internal("netzip rust tcp body length overflow"))?;
    let mut frame = Vec::with_capacity(NETZIP_RUST_TCP_HEADER_LEN + body.len());
    frame.extend_from_slice(NETZIP_RUST_TCP_MAGIC);
    frame.extend_from_slice(&NETZIP_RUST_TCP_VERSION.to_be_bytes());
    frame.extend_from_slice(&message_type.to_be_bytes());
    frame.extend_from_slice(&sequence.to_be_bytes());
    frame.extend_from_slice(&body_len.to_be_bytes());
    frame.extend_from_slice(&crc32fast::hash(body).to_be_bytes());
    frame.extend_from_slice(body);
    Ok(frame)
}

fn decode_netzip_rust_tcp_frame(
    frame: &[u8],
    expected_type: u16,
) -> Result<(u64, &[u8]), ApiError> {
    if frame.len() < NETZIP_RUST_TCP_HEADER_LEN {
        return Err(ApiError::internal("netzip rust tcp frame header truncated"));
    }
    if &frame[0..4] != NETZIP_RUST_TCP_MAGIC {
        return Err(ApiError::internal("netzip rust tcp frame magic mismatch"));
    }
    let version = u16::from_be_bytes(frame[4..6].try_into().unwrap());
    let message_type = u16::from_be_bytes(frame[6..8].try_into().unwrap());
    if version != NETZIP_RUST_TCP_VERSION || message_type != expected_type {
        return Err(ApiError::internal(format!(
            "unexpected netzip rust tcp version/type {version}/{message_type}"
        )));
    }
    let sequence = u64::from_be_bytes(frame[8..16].try_into().unwrap());
    let body_len = u32::from_be_bytes(frame[16..20].try_into().unwrap()) as usize;
    if body_len > NETZIP_RUST_TCP_MAX_BODY || frame.len() != NETZIP_RUST_TCP_HEADER_LEN + body_len {
        return Err(ApiError::internal(format!(
            "invalid netzip rust tcp body length {body_len}"
        )));
    }
    let body = &frame[NETZIP_RUST_TCP_HEADER_LEN..];
    let expected_crc = u32::from_be_bytes(frame[20..24].try_into().unwrap());
    if crc32fast::hash(body) != expected_crc {
        return Err(ApiError::internal("netzip rust tcp crc mismatch"));
    }
    Ok((sequence, body))
}

fn encode_netzip_rust_tcp_quote_frame(
    sequence: u64,
    payload: &serde_json::Value,
) -> Result<Vec<u8>, ApiError> {
    let message_pack = rmp_serde::to_vec_named(payload).map_err(|err| {
        ApiError::internal(format!("encode MessagePack quote batch failed: {err}"))
    })?;
    let body = zstd::stream::encode_all(message_pack.as_slice(), 1).map_err(|err| {
        ApiError::internal(format!("compress MessagePack quote batch failed: {err}"))
    })?;
    encode_netzip_rust_tcp_frame(NETZIP_RUST_TCP_MSG_QUOTES, sequence, &body)
}

fn encode_netzip_rust_tcp_ack(
    sequence: u64,
    applied: usize,
    cached_quotes: usize,
) -> Result<Vec<u8>, ApiError> {
    let applied = u32::try_from(applied)
        .map_err(|_| ApiError::internal("netzip rust ack applied overflow"))?;
    let cached_quotes = u32::try_from(cached_quotes)
        .map_err(|_| ApiError::internal("netzip rust ack cache overflow"))?;
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&applied.to_be_bytes());
    body.extend_from_slice(&cached_quotes.to_be_bytes());
    encode_netzip_rust_tcp_frame(NETZIP_RUST_TCP_MSG_ACK, sequence, &body)
}

fn decode_netzip_rust_tcp_ack(frame: &[u8]) -> Result<NetzipRustTcpAck, ApiError> {
    let (sequence, body) = decode_netzip_rust_tcp_frame(frame, NETZIP_RUST_TCP_MSG_ACK)?;
    if body.len() != 8 {
        return Err(ApiError::internal(format!(
            "invalid netzip rust tcp ack length {}",
            body.len()
        )));
    }
    Ok(NetzipRustTcpAck {
        sequence,
        applied: u32::from_be_bytes(body[0..4].try_into().unwrap()),
        cached_quotes: u32::from_be_bytes(body[4..8].try_into().unwrap()),
    })
}

fn read_netzip_rust_tcp_frame(stream: &mut TcpStream) -> Result<Vec<u8>, ApiError> {
    let mut header = [0u8; NETZIP_RUST_TCP_HEADER_LEN];
    stream.read_exact(&mut header).map_err(|err| {
        ApiError::internal(format!("read netzip rust tcp ack header failed: {err}"))
    })?;
    let body_len = u32::from_be_bytes(header[16..20].try_into().unwrap()) as usize;
    if body_len > NETZIP_RUST_TCP_MAX_BODY {
        return Err(ApiError::internal(format!(
            "netzip rust tcp ack too large: {body_len}"
        )));
    }
    let mut frame = Vec::with_capacity(NETZIP_RUST_TCP_HEADER_LEN + body_len);
    frame.extend_from_slice(&header);
    frame.resize(NETZIP_RUST_TCP_HEADER_LEN + body_len, 0);
    stream
        .read_exact(&mut frame[NETZIP_RUST_TCP_HEADER_LEN..])
        .map_err(|err| {
            ApiError::internal(format!("read netzip rust tcp ack body failed: {err}"))
        })?;
    Ok(frame)
}

impl NetzipRustTcpClient {
    fn connect(addr: &str) -> Result<Self, ApiError> {
        let stream = TcpStream::connect(addr).map_err(|err| {
            ApiError::internal(format!("connect netzip rust tcp {addr} failed: {err}"))
        })?;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .map_err(|err| {
                ApiError::internal(format!("set netzip tcp read timeout failed: {err}"))
            })?;
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .map_err(|err| {
                ApiError::internal(format!("set netzip tcp write timeout failed: {err}"))
            })?;
        stream
            .set_nodelay(true)
            .map_err(|err| ApiError::internal(format!("set netzip tcp nodelay failed: {err}")))?;
        Ok(Self {
            addr: addr.to_string(),
            stream,
            next_sequence: 1,
        })
    }

    fn send(&mut self, payload: &serde_json::Value) -> Result<NetzipRustTcpSendResult, ApiError> {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1).max(1);
        let frame = encode_netzip_rust_tcp_quote_frame(sequence, payload)?;
        let json_bytes = serde_json::to_vec(payload)
            .map_err(|err| ApiError::internal(format!("measure JSON quote batch failed: {err}")))?
            .len();
        let wire_bytes = frame.len();
        self.stream.write_all(&frame).map_err(|err| {
            ApiError::internal(format!("write netzip rust tcp frame failed: {err}"))
        })?;
        let ack = decode_netzip_rust_tcp_ack(&read_netzip_rust_tcp_frame(&mut self.stream)?)?;
        if ack.sequence != sequence {
            return Err(ApiError::internal(format!(
                "netzip rust tcp ack sequence mismatch: sent={sequence} received={}",
                ack.sequence
            )));
        }
        Ok(NetzipRustTcpSendResult {
            ack,
            wire_bytes,
            json_bytes,
        })
    }
}

fn netzip_rust_tcp_client_slot(
    lane: GatewayPublishLane,
    tcp_addr: &str,
) -> Arc<Mutex<Option<NetzipRustTcpClient>>> {
    let clients = NETZIP_RUST_TCP_CLIENTS.get_or_init(|| Mutex::new(BTreeMap::new()));
    let key = netzip_rust_tcp_client_key(lane, tcp_addr);
    let mut clients = clients.lock().unwrap_or_else(|err| err.into_inner());
    clients
        .entry(key)
        .or_insert_with(|| Arc::new(Mutex::new(None)))
        .clone()
}

fn netzip_rust_tcp_client_key(lane: GatewayPublishLane, tcp_addr: &str) -> String {
    // Keep one bounded persistent client per lane and gateway address. Including
    // a request thread ID retained sockets after short-lived audit/API threads exited.
    format!("{}:{tcp_addr}", lane.label())
}

fn netzip_rust_tcp_ack_timeout() -> Duration {
    let seconds = std::env::var("NETZIP_QUOTE_GATEWAY_TCP_ACK_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(30)
        .clamp(5, 120);
    Duration::from_secs(seconds)
}

fn record_gateway_publish_metric(
    lane: GatewayPublishLane,
    update: impl FnOnce(&mut GatewayPublishMetrics),
) {
    let metrics = NETZIP_RUST_PUBLISH_METRICS.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut metrics = metrics.lock().unwrap_or_else(|err| err.into_inner());
    update(metrics.entry(lane.label().to_string()).or_default());
}

fn gateway_publish_metrics_snapshot() -> BTreeMap<String, GatewayPublishMetrics> {
    NETZIP_RUST_PUBLISH_METRICS
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clone()
}

#[derive(Clone)]
struct AppState {
    service_name: &'static str,
    version: &'static str,
}

#[cfg(test)]
mod netzip_rust_tcp_transport_contract_tests {
    use serde_json::json;

    #[test]
    fn persistent_tcp_frame_is_smaller_than_http_json_and_ack_is_correlated() {
        let mut payload = json!({
            "schema": "netzipRust7709.quote_batch.v1",
            "event": "quote_batch",
            "source": "netzipRust7709",
            "batch_id": "20260727-1",
            "quotes": [{
                "market": "SH",
                "code": "600000",
                "name": "Pudong Development Bank",
                "datetime": "2026-07-27 10:00:01",
                "price": 9.02,
                "last_close": 9.01,
                "open": 9.00,
                "high": 9.05,
                "low": 8.99,
                "volume": 2345678.0,
                "amount": 21123456.0,
                "source_protocol": "netzip-rust-7709-0547.v2"
            }]
        });
        let row = payload["quotes"][0].clone();
        payload["quotes"] = json!(vec![row; 100]);
        let json_bytes = serde_json::to_vec(&payload).expect("json");
        let frame = super::encode_netzip_rust_tcp_quote_frame(19, &payload).expect("frame");
        assert!(frame.len() * 100 <= json_bytes.len() * 60);

        let ack = super::encode_netzip_rust_tcp_ack(19, 1, 5522).expect("ack");
        let decoded = super::decode_netzip_rust_tcp_ack(&ack).expect("decode ack");
        assert_eq!(decoded.sequence, 19);
        assert_eq!(decoded.applied, 1);
        assert_eq!(decoded.cached_quotes, 5522);
    }

    #[test]
    fn main_and_bj_lanes_use_distinct_transport_slots() {
        assert_ne!(
            super::netzip_rust_tcp_client_key(super::GatewayPublishLane::Main, "127.0.0.1:16889"),
            super::netzip_rust_tcp_client_key(super::GatewayPublishLane::Bj, "127.0.0.1:16889")
        );
    }

    #[test]
    fn transport_slot_is_stable_across_short_lived_threads() {
        let expected =
            super::netzip_rust_tcp_client_key(super::GatewayPublishLane::Main, "127.0.0.1:16889");
        let actual = std::thread::spawn(|| {
            super::netzip_rust_tcp_client_key(super::GatewayPublishLane::Main, "127.0.0.1:16889")
        })
        .join()
        .expect("transport key thread");

        assert_eq!(actual, expected);
    }
}

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
    service: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct CapabilitiesResponse {
    service: &'static str,
    stable_endpoints: Vec<&'static str>,
    linux_native_endpoints: Vec<&'static str>,
    windows_bridge_endpoints: Vec<&'static str>,
    research_endpoints: Vec<&'static str>,
    recommended_delivery_tracks: Vec<DeliveryTrack>,
    pure_rust_linux_feasibility: &'static str,
    pure_rust_linux_summary: &'static str,
    pure_rust_linux_hard_blockers: Vec<&'static str>,
    pure_rust_linux_non_blocking_gaps: Vec<&'static str>,
    fin_urls: [&'static str; 2],
}

#[derive(Serialize)]
struct DeliveryTrack {
    name: &'static str,
    priority: &'static str,
    status: &'static str,
    scope: &'static str,
    notes: Vec<&'static str>,
}

#[derive(Deserialize)]
struct LegacyPanelConfigRequest {
    config: netzipapi_rust_demo::LegacyPanelConfig,
}

#[derive(Deserialize)]
struct LegacyPanelActionRequest {
    config: netzipapi_rust_demo::LegacyPanelConfig,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
struct FinParseRequest {
    path: String,
    limit: Option<usize>,
    out_csv_path: Option<String>,
}

#[derive(Serialize)]
struct FinRecordPreview {
    symbol: String,
    market: Option<String>,
    code: Option<String>,
    time: u32,
    bao_gao: u32,
    quarter: u8,
    mg_shou_yi: f32,
    mg_jing_zhi: f32,
    zong_gu: f32,
    liu_tong_ag: f32,
    jing_li_run: f32,
    trailer_hex: String,
}

#[derive(Serialize)]
struct FinParseResponse {
    path: String,
    magic_hex: String,
    record_size: u32,
    records_total: usize,
    trailing_bytes: usize,
    out_csv_path: Option<String>,
    first_record: Option<FinRecordPreview>,
    last_record: Option<FinRecordPreview>,
    preview: Vec<FinRecordPreview>,
}

#[derive(Deserialize)]
struct FinGetterValueRequest {
    path: String,
    symbol: String,
    field_id: u8,
}

#[derive(Deserialize)]
struct FinRecordQueryRequest {
    path: String,
    symbol: String,
}

#[derive(Serialize)]
struct FinGetterSpecPreview {
    field_id: u8,
    field_hex: String,
    name: String,
    display_name: String,
    status: String,
    raw_offset: Option<usize>,
    note: String,
}

#[derive(Serialize)]
struct FinGetterValueResponse {
    path: String,
    symbol: String,
    field_id: u8,
    record_found: bool,
    record_symbol: Option<String>,
    record_market: Option<String>,
    record_code: Option<String>,
    getter_name: Option<String>,
    raw_offset: Option<usize>,
    note: Option<String>,
    quarter: Option<u8>,
    value: Option<f32>,
}

#[derive(Serialize)]
struct FinRecordQueryData {
    symbol: String,
    market: Option<String>,
    code: Option<String>,
    time: u32,
    bao_gao: u32,
    quarter: u8,
    mg_shou_yi: f32,
    mg_jing_zhi: f32,
    zong_gu: f32,
    liu_tong_ag: f32,
    shou_ru: f32,
    zong_zc: f32,
    zong_fu_zhai: f32,
    quan_yi: f32,
    jing_li_run: f32,
    wei_fen_pei: f32,
}

#[derive(Serialize)]
struct FinRecordQueryResponse {
    path: String,
    symbol: String,
    record_found: bool,
    record: Option<FinRecordQueryData>,
}

#[derive(Serialize)]
struct FinGetterSpecsResponse {
    specs: Vec<FinGetterSpecPreview>,
    unresolved_field_ids: Vec<u8>,
    unresolved_field_hex: Vec<String>,
    confirmed_count: usize,
    derived_count: usize,
    unresolved_count: usize,
}

#[derive(Deserialize)]
struct Tdx7709SyncRequest {
    host: Option<String>,
    port: Option<u16>,
    read_timeout_ms: Option<u64>,
    connect_timeout_ms: Option<u64>,
    settle_ms: Option<u64>,
    preview_limit: Option<usize>,
    out_csv_path: Option<String>,
}

#[derive(Deserialize)]
struct Tdx7709CodeQueryRequest {
    host: Option<String>,
    port: Option<u16>,
    query: Option<String>,
    limit: Option<usize>,
    read_timeout_ms: Option<u64>,
    connect_timeout_ms: Option<u64>,
    settle_ms: Option<u64>,
}

#[derive(Deserialize)]
struct Tdx7709LiveQuoteRequest {
    host: Option<String>,
    port: Option<u16>,
    symbols: Vec<String>,
    limit: Option<usize>,
    read_timeout_ms: Option<u64>,
    connect_timeout_ms: Option<u64>,
    settle_ms: Option<u64>,
}

#[derive(Serialize)]
struct CodeTableRecordPreview {
    market: String,
    code: String,
    name: String,
    decimal_point: u8,
    pre_close: f32,
    meta_hex: String,
}

#[derive(Serialize)]
struct Tdx7709LiveQuoteResponse {
    host: String,
    port: u16,
    requested_symbols: Vec<String>,
    transport_symbols: Vec<String>,
    unmatched_symbols: Vec<String>,
    code_table_reply_bytes: usize,
    code_table_reply_frames: usize,
    code_table_records_total: usize,
    quote_reply_bytes: usize,
    quote_reply_frames: usize,
    parsed_quote_bodies: usize,
    parsed_quote_records: usize,
    matched_records: usize,
    preview: Vec<Quote0547RecordPreview>,
    first_match: Option<Quote0547RecordPreview>,
    last_match: Option<Quote0547RecordPreview>,
}

#[derive(Deserialize)]
struct CompactQuotesQuery {
    codes: String,
}

#[derive(Debug, Serialize)]
struct CompactQuote {
    code: String,
    symbol: String,
    market: String,
    name: Option<String>,
    price: f64,
    last_close: f64,
    open: f64,
    high: f64,
    low: f64,
    volume: Option<f64>,
    amount: Option<f64>,
    datetime: Option<String>,
    quote_datetime: Option<String>,
    source: &'static str,
}

#[derive(Debug, Serialize)]
struct CompactQuotesResponse {
    success: bool,
    source: &'static str,
    count: usize,
    missing_codes: Vec<String>,
    partial: bool,
    data: Vec<CompactQuote>,
}

#[derive(Clone, Copy)]
enum OemPublicAmountMode {
    Index,
    PriceRelative,
    Direct,
}

#[derive(Deserialize)]
struct HqwPublishRequest {
    symbols: Vec<String>,
    trade_date: String,
}

#[derive(Debug, Serialize)]
struct HqwPublishResponse {
    success: bool,
    gateway_addr: String,
    gateway_protocol: &'static str,
    requested_count: usize,
    published_count: usize,
    missing_codes: Vec<String>,
    gateway_response: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct HqwPublishWorklistRequest {
    batch_size: Option<usize>,
    limit: Option<usize>,
    worker_count: Option<usize>,
}

#[derive(Debug, Serialize)]
struct HqwPublishWorklistResponse {
    success: bool,
    gateway_addr: String,
    gateway_protocol: &'static str,
    trade_date: String,
    worklist_count: usize,
    batch_size: usize,
    worker_count: usize,
    batch_count: usize,
    primary_batch_count: usize,
    primary_worker_count: usize,
    primary_elapsed_ms: u128,
    primary_slowest_batch_ms: u128,
    fallback_batch_count: usize,
    fallback_worker_count: usize,
    fallback_elapsed_ms: u128,
    fallback_slowest_batch_ms: u128,
    elapsed_ms: u128,
    fallback_host: String,
    fallback_published_count: usize,
    published_count: usize,
    unchanged_count: usize,
    no_current_quote_count: usize,
    no_current_quote_codes: Vec<String>,
    last_gateway_response: Option<serde_json::Value>,
    gateway_publish_metrics: BTreeMap<String, GatewayPublishMetrics>,
}

#[derive(Debug, Deserialize)]
struct HqwPushWorklistRequest {
    duration_secs: Option<u64>,
    audit_interval_secs: Option<u64>,
    bj_poll_interval_secs: Option<u64>,
    publish: Option<bool>,
}

#[derive(Debug, Serialize)]
struct HqwPushWorklistResponse {
    success: bool,
    publish: bool,
    trade_date: String,
    worklist_count: usize,
    subscribed_count: usize,
    shard_count: usize,
    received_records: usize,
    converted_records: usize,
    unconverted_symbols: usize,
    unconverted_symbol_sample: Vec<String>,
    published_records: usize,
    unchanged_records: usize,
    publish_batches: usize,
    publish_failures: usize,
    reader_failures: usize,
    reader_recoveries: usize,
    renewal_requests: usize,
    audit_runs: usize,
    audit_failures: usize,
    bj_poll_symbols: usize,
    bj_poll_runs: usize,
    bj_poll_failures: usize,
    bj_published_records: usize,
    bj_unchanged_records: usize,
    gateway_publish_metrics: BTreeMap<String, GatewayPublishMetrics>,
    elapsed_ms: u128,
}

#[derive(Debug, PartialEq, Eq)]
struct GatewayWorklist {
    trade_date: String,
    fallback_quote_time: String,
    symbols: Vec<String>,
    names: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BjPollPlan {
    symbols: Vec<String>,
    batch_count: usize,
    worker_count: usize,
    interval: Duration,
}

#[derive(Debug, Default)]
struct BjPollSummary {
    runs: usize,
    failures: usize,
    published_records: usize,
    unchanged_records: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GatewayPublishProtocol {
    Auto,
    NetzipRust7709,
    NetzipRust7709Tcp,
}

impl GatewayPublishProtocol {
    fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::NetzipRust7709 => "netzipRust7709.quote_batch.v1",
            Self::NetzipRust7709Tcp => "netzipRust7709.tcp.msgpack.zstd.v2",
        }
    }
}

struct FullPushStageResult {
    batch_count: usize,
    worker_count: usize,
    elapsed_ms: u128,
    slowest_batch_ms: u128,
    total_batch_elapsed_ms: u128,
    published_count: usize,
    unchanged_count: usize,
    missing_codes: Vec<String>,
    last_gateway_response: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Tdx7709KlineRequest {
    host: Option<String>,
    port: Option<u16>,
    symbol: String,
    category: Option<u16>,
    kline_type: Option<String>,
    start: Option<u16>,
    count: Option<u16>,
    limit: Option<usize>,
    read_timeout_ms: Option<u64>,
    connect_timeout_ms: Option<u64>,
    settle_ms: Option<u64>,
}

#[derive(Serialize)]
struct Tdx7709KlineBarPreview {
    symbol: String,
    market: u8,
    market_name: String,
    code: String,
    category: u16,
    kline_type: String,
    datetime: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    amount: f64,
}

#[derive(Serialize)]
struct Tdx7709KlineResponse {
    host: String,
    port: u16,
    symbol: String,
    category: u16,
    kline_type: String,
    start: u16,
    requested_count: u16,
    code_table_reply_bytes: usize,
    code_table_reply_frames: usize,
    code_table_records_total: usize,
    kline_reply_bytes: usize,
    kline_reply_frames: usize,
    bars_total: usize,
    preview: Vec<Tdx7709KlineBarPreview>,
    first_bar: Option<Tdx7709KlineBarPreview>,
    last_bar: Option<Tdx7709KlineBarPreview>,
}

#[derive(Deserialize)]
struct Tdx7709F10CategoriesRequest {
    host: Option<String>,
    port: Option<u16>,
    symbol: String,
    limit: Option<usize>,
    read_timeout_ms: Option<u64>,
    connect_timeout_ms: Option<u64>,
    settle_ms: Option<u64>,
}

#[derive(Serialize)]
struct Tdx7709F10CategoryPreview {
    name: String,
    filename: String,
    start: u32,
    length: u32,
}

#[derive(Serialize)]
struct Tdx7709F10CategoriesResponse {
    host: String,
    port: u16,
    symbol: String,
    code_table_reply_bytes: usize,
    code_table_reply_frames: usize,
    code_table_records_total: usize,
    category_reply_bytes: usize,
    category_reply_frames: usize,
    categories_total: usize,
    preview: Vec<Tdx7709F10CategoryPreview>,
    first_category: Option<Tdx7709F10CategoryPreview>,
    last_category: Option<Tdx7709F10CategoryPreview>,
}

#[derive(Deserialize)]
struct Tdx7709F10ContentRequest {
    host: Option<String>,
    port: Option<u16>,
    symbol: String,
    filename: Option<String>,
    category_name: Option<String>,
    start: Option<u32>,
    length: Option<u32>,
    preview_chars: Option<usize>,
    read_timeout_ms: Option<u64>,
    connect_timeout_ms: Option<u64>,
    settle_ms: Option<u64>,
}

#[derive(Deserialize)]
struct Tdx7709SnapshotRequest {
    host: Option<String>,
    port: Option<u16>,
    symbol: String,
    category: Option<u16>,
    kline_type: Option<String>,
    kline_start: Option<u16>,
    kline_count: Option<u16>,
    kline_limit: Option<usize>,
    f10_limit: Option<usize>,
    include_live_quote: Option<bool>,
    include_kline: Option<bool>,
    include_f10: Option<bool>,
    read_timeout_ms: Option<u64>,
    connect_timeout_ms: Option<u64>,
    settle_ms: Option<u64>,
}

#[derive(Serialize)]
struct Tdx7709F10ContentResponse {
    host: String,
    port: u16,
    symbol: String,
    resolved_category_name: Option<String>,
    filename: String,
    start: u32,
    length: u32,
    preview_chars: usize,
    code_table_reply_bytes: usize,
    code_table_reply_frames: usize,
    code_table_records_total: usize,
    content_reply_bytes: usize,
    content_reply_frames: usize,
    content_chars_total: usize,
    content_preview: String,
}

#[derive(Serialize)]
struct Tdx7709SnapshotResponse {
    host: String,
    port: u16,
    symbol: String,
    category: u16,
    kline_type: String,
    kline_start: u16,
    kline_count: u16,
    shared_session_attempted: bool,
    shared_session_established: bool,
    shared_session_fallback_phases: Vec<String>,
    include_live_quote: bool,
    include_kline: bool,
    include_f10: bool,
    live_quote: Option<Tdx7709LiveQuoteResponse>,
    live_quote_error: Option<String>,
    kline: Option<Tdx7709KlineResponse>,
    kline_error: Option<String>,
    f10_categories: Option<Tdx7709F10CategoriesResponse>,
    f10_categories_error: Option<String>,
}

#[derive(Deserialize)]
struct LinuxPureRustMvpRequest {
    host: Option<String>,
    port: Option<u16>,
    symbols: Vec<String>,
    kline_type: Option<String>,
    kline_count: Option<u16>,
    sync_preview_limit: Option<usize>,
    quote_limit: Option<usize>,
    kline_limit: Option<usize>,
    f10_limit: Option<usize>,
    include_sync: Option<bool>,
    include_live_quote: Option<bool>,
    include_kline: Option<bool>,
    include_f10: Option<bool>,
    read_timeout_ms: Option<u64>,
    connect_timeout_ms: Option<u64>,
    settle_ms: Option<u64>,
}

#[derive(Serialize)]
struct LinuxPureRustMvpPhase<T: Serialize> {
    enabled: bool,
    skipped: bool,
    ok: bool,
    error: Option<String>,
    data: Option<T>,
}

#[derive(Serialize)]
struct LinuxPureRustMvpResponse {
    host: String,
    port: u16,
    requested_symbols: Vec<String>,
    probe_symbol: String,
    kline_type: String,
    shared_session_attempted: bool,
    shared_session_established: bool,
    shared_session_fallback_phases: Vec<String>,
    include_sync: bool,
    include_live_quote: bool,
    include_kline: bool,
    include_f10: bool,
    overall_ok: bool,
    findings: Vec<String>,
    sync: LinuxPureRustMvpPhase<Tdx7709SyncResponse>,
    live_quote: LinuxPureRustMvpPhase<Tdx7709LiveQuoteResponse>,
    kline: LinuxPureRustMvpPhase<Tdx7709KlineResponse>,
    f10_categories: LinuxPureRustMvpPhase<Tdx7709F10CategoriesResponse>,
}

#[derive(Serialize)]
struct Tdx7709SyncResponse {
    host: String,
    port: u16,
    reply_bytes: usize,
    reply_frames: usize,
    records_total: usize,
    out_csv_path: Option<String>,
    first_record: Option<CodeTableRecordPreview>,
    last_record: Option<CodeTableRecordPreview>,
    preview: Vec<CodeTableRecordPreview>,
}

#[derive(Serialize)]
struct Tdx7709CodeQueryResponse {
    host: String,
    port: u16,
    query: String,
    records_total: usize,
    matches_total: usize,
    reply_bytes: usize,
    reply_frames: usize,
    preview: Vec<CodeTableRecordPreview>,
}

#[derive(Serialize)]
struct Tdx7709BootstrapPacketResponse {
    label: String,
    op: u16,
    sub: u16,
    flags: u16,
    body_len: usize,
    request_len: usize,
    body_hex: String,
    request_hex: String,
}

#[derive(Serialize)]
struct Tdx7709BootstrapPlanResponse {
    probe_hello_hex: String,
    probe_hello_len: usize,
    packets: Vec<Tdx7709BootstrapPacketResponse>,
}

#[derive(Serialize)]
struct Tdx118DumpComparePlanResponse {
    status: &'static str,
    ready: bool,
    expected_plain_len: usize,
    expected_cipher_len: usize,
    tag_len: usize,
    plain_source_hint: &'static str,
    cipher_target_hint: &'static str,
    known_inputs: Vec<&'static str>,
    suggested_artifacts: Vec<&'static str>,
    notes: Vec<&'static str>,
}

#[derive(Serialize)]
struct Auth7100ClientShellResponse {
    analysis: netzipapi_rust_demo::Auth7100ClientShellAnalysis,
}

#[derive(Serialize)]
struct Auth7100ShellCorrelationResponse {
    analysis: netzipapi_rust_demo::Auth7100ShellCorrelationAnalysis,
}

#[derive(Serialize)]
struct Auth7100FlowMatrixResponse {
    analysis: netzipapi_rust_demo::Auth7100FlowMatrix,
}

#[derive(Deserialize)]
struct Auth7100PathFilterRequest {
    path: String,
    local_endpoint: Option<String>,
    session_role: Option<String>,
    source_endpoint: Option<String>,
    destination_endpoint: Option<String>,
}

#[derive(Deserialize)]
struct AnswerSummaryRequest {
    path: String,
}

#[derive(Deserialize)]
struct BlobCompareRequest {
    left_path: String,
    right_path: String,
    left_offset: Option<usize>,
    right_offset: Option<usize>,
    compare_len: Option<usize>,
    block_size: Option<usize>,
}

#[derive(Serialize)]
struct AnswerSummaryResponse {
    path: String,
    size_bytes: usize,
    packet_kind: Option<String>,
    summary: Option<String>,
}

#[derive(Deserialize)]
struct PcapSummaryRequest {
    path: String,
    segment_limit: Option<usize>,
}

#[derive(Serialize)]
struct PcapSummaryResponse {
    path: String,
    pcap_packets: usize,
    unique_packets: usize,
    duplicate_packets: usize,
    truncated_unique_packets: usize,
    flows: Vec<netzipapi_rust_demo::PcapFlowSummary>,
}

#[derive(Deserialize)]
struct StreamAnalyzeRequest {
    path: String,
    is_hex: Option<bool>,
}

#[derive(Deserialize)]
struct QuoteFrameScanRequest {
    path: String,
}

#[derive(Deserialize)]
struct Quote0547DecodeRequest {
    path: String,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct Quote0547QueryRequest {
    path: String,
    query: String,
    limit: Option<usize>,
    decimal_point: Option<u8>,
    decimal_point_filter: Option<u8>,
    prefix3: Option<String>,
    pattern_bucket: Option<String>,
    pattern_subbucket: Option<String>,
    state_matrix: Option<String>,
    quote_head_state: Option<String>,
    time_presence: Option<String>,
    name_keyword_tag: Option<String>,
}

#[derive(Deserialize)]
struct Quote0547ExtraProfileRequest {
    path: String,
    anomaly_limit: Option<usize>,
}

#[derive(Serialize)]
struct Quote0547HeadPreview {
    active1: u16,
    price: f64,
    last_close: f64,
    open: f64,
    high: f64,
    low: f64,
}

#[derive(Serialize)]
struct Quote0547RecordPreview {
    source_name: Option<String>,
    source_path: Option<String>,
    market: u8,
    market_name: Option<String>,
    code: String,
    start: usize,
    len: usize,
    decimal_point_used: Option<u8>,
    code_table_name: Option<String>,
    code_table_name_keyword_tag: Option<String>,
    code_table_name_keyword_tags: Vec<String>,
    code_table_pre_close: Option<f32>,
    code_table_pre_close_delta: Option<f64>,
    code_table_pre_close_matches: Option<bool>,
    active1_raw: Option<u16>,
    time_hhmmss_raw: Option<u32>,
    time_hhmmss: Option<String>,
    time_fields_match: Option<bool>,
    time_fields_delta_seconds: Option<i32>,
    extra0_raw: Option<i32>,
    extra0_time_hhmmss: Option<String>,
    extra1_raw: Option<i32>,
    extra2_raw: Option<i32>,
    extra3_raw: Option<i32>,
    volume: Option<f64>,
    current_volume: Option<f64>,
    amount: Option<f64>,
    amount_raw: Option<u32>,
    extra3_positive: Option<bool>,
    special_zero_bucket: bool,
    pattern_bucket: String,
    pattern_subbucket: String,
    quote_head: Option<Quote0547HeadPreview>,
    normalized_quote_head: Option<Quote0547HeadPreview>,
    normalized_change_value: Option<f64>,
    normalized_change_percent: Option<f64>,
    normalized_amplitude_percent: Option<f64>,
    normalized_open_gap_value: Option<f64>,
    normalized_open_gap_percent: Option<f64>,
    normalized_intraday_range_value: Option<f64>,
    normalized_return_from_open_percent: Option<f64>,
    normalized_drawdown_from_high_percent: Option<f64>,
}

#[derive(Serialize)]
struct Quote0547DecodeResponse {
    path: String,
    size: usize,
    decimal_point_used: Option<u8>,
    auto_decimal_lookup: bool,
    decimal_point_lookup_error: Option<String>,
    xor93_count: Option<u16>,
    printable_ratio: f64,
    filtered_records: usize,
    count_matches_records: bool,
    preview: Vec<Quote0547RecordPreview>,
    first_record: Option<Quote0547RecordPreview>,
    last_record: Option<Quote0547RecordPreview>,
}

#[derive(Serialize)]
struct Quote0547QueryResponse {
    path: String,
    source_mode: String,
    source_files_total: usize,
    size: usize,
    query: String,
    decimal_point_filter: Option<u8>,
    prefix3_filter: Option<String>,
    pattern_bucket_filter: Option<String>,
    pattern_subbucket_filter: Option<String>,
    state_matrix_filter: Option<String>,
    quote_head_state_filter: Option<String>,
    time_presence_filter: Option<String>,
    name_keyword_tag_filter: Option<String>,
    decimal_point_used: Option<u8>,
    auto_decimal_lookup: bool,
    decimal_point_lookup_error: Option<String>,
    xor93_count: Option<u16>,
    filtered_records: usize,
    matched_records: usize,
    count_matches_records: bool,
    matched_prefix3_top: Vec<Quote0547LabelSummary>,
    matched_decimal_point_top: Vec<Quote0547LabelSummary>,
    matched_pattern_bucket_top: Vec<Quote0547LabelSummary>,
    matched_pattern_subbucket_top: Vec<Quote0547LabelSummary>,
    matched_quote_head_state_top: Vec<Quote0547LabelSummary>,
    matched_time_presence_top: Vec<Quote0547LabelSummary>,
    matched_state_matrix_top: Vec<Quote0547LabelSummary>,
    matched_name_keyword_top: Vec<Quote0547LabelSummary>,
    matched_source_top: Vec<Quote0547LabelSummary>,
    preview: Vec<Quote0547RecordPreview>,
    first_match: Option<Quote0547RecordPreview>,
    last_match: Option<Quote0547RecordPreview>,
}

#[derive(Serialize)]
struct Quote0547ValueCount {
    value: i32,
    count: usize,
}

#[derive(Serialize)]
struct Quote0547ExtraPatternSummary {
    extra1_raw: i32,
    extra2_raw: i32,
    extra3_raw: i32,
    count: usize,
    example_codes: Vec<String>,
}

#[derive(Serialize)]
struct Quote0547LabelSummary {
    label: String,
    count: usize,
    example_codes: Vec<String>,
}

#[derive(Serialize)]
struct Quote0547PatternCorrelationSummary {
    extra1_raw: i32,
    extra2_raw: i32,
    extra3_raw: i32,
    count: usize,
    prefix3_top: Vec<Quote0547LabelSummary>,
    market_top: Vec<Quote0547LabelSummary>,
    decimal_point_top: Vec<Quote0547LabelSummary>,
    name_keyword_top: Vec<Quote0547LabelSummary>,
    time_hhmmss_top: Vec<Quote0547LabelSummary>,
    extra0_time_hint_top: Vec<Quote0547LabelSummary>,
}

#[derive(Serialize)]
struct Quote0547LabelCorrelationSummary {
    label: String,
    count: usize,
    prefix3_top: Vec<Quote0547LabelSummary>,
    market_top: Vec<Quote0547LabelSummary>,
    decimal_point_top: Vec<Quote0547LabelSummary>,
    name_keyword_top: Vec<Quote0547LabelSummary>,
    time_hhmmss_top: Vec<Quote0547LabelSummary>,
    extra0_time_hint_top: Vec<Quote0547LabelSummary>,
}

#[derive(Serialize)]
struct Quote0547ExtraProfileResponse {
    path: String,
    source_mode: String,
    source_files_total: usize,
    size: usize,
    xor93_count: Option<u16>,
    filtered_records: usize,
    records_with_complete_extra_tuple: usize,
    default_pattern_count: usize,
    default_quote_head_count: usize,
    default_quote_head_time_present_count: usize,
    default_quote_head_time_absent_count: usize,
    anomaly_count: usize,
    anomaly_extra3_positive_count: usize,
    anomaly_special_zero_bucket_count: usize,
    default_no_quote_head_count: usize,
    code_table_lookup_error: Option<String>,
    extra0_top: Vec<Quote0547ValueCount>,
    extra1_top: Vec<Quote0547ValueCount>,
    extra2_top: Vec<Quote0547ValueCount>,
    extra3_top: Vec<Quote0547ValueCount>,
    pattern_top: Vec<Quote0547ExtraPatternSummary>,
    default_pattern_subbucket_top: Vec<Quote0547LabelSummary>,
    default_pattern_subbucket_correlations: Vec<Quote0547LabelCorrelationSummary>,
    default_state_matrix_top: Vec<Quote0547LabelSummary>,
    default_quote_head_subbucket_top: Vec<Quote0547LabelSummary>,
    default_quote_head_name_keyword_top: Vec<Quote0547LabelSummary>,
    default_quote_head_time_present_examples: Vec<Quote0547RecordPreview>,
    default_quote_head_time_absent_examples: Vec<Quote0547RecordPreview>,
    default_no_quote_head_subbucket_top: Vec<Quote0547LabelSummary>,
    default_no_quote_head_name_keyword_top: Vec<Quote0547LabelSummary>,
    default_no_quote_head_examples: Vec<Quote0547RecordPreview>,
    anomaly_prefix3_top: Vec<Quote0547LabelSummary>,
    anomaly_market_top: Vec<Quote0547LabelSummary>,
    anomaly_decimal_point_top: Vec<Quote0547LabelSummary>,
    anomaly_pattern_bucket_top: Vec<Quote0547LabelSummary>,
    anomaly_pattern_subbucket_top: Vec<Quote0547LabelSummary>,
    anomaly_name_keyword_top: Vec<Quote0547LabelSummary>,
    anomaly_time_hhmmss_top: Vec<Quote0547LabelSummary>,
    anomaly_extra0_time_hint_top: Vec<Quote0547LabelSummary>,
    source_top: Vec<Quote0547LabelSummary>,
    anomaly_source_top: Vec<Quote0547LabelSummary>,
    anomaly_pattern_correlations: Vec<Quote0547PatternCorrelationSummary>,
    anomaly_special_zero_bucket_examples: Vec<Quote0547RecordPreview>,
    anomaly_examples: Vec<Quote0547RecordPreview>,
}

#[derive(Clone)]
struct Quote0547CodeTableInfo {
    name: String,
    decimal_point: u8,
    pre_close: f32,
}

struct Quote0547LoadedSource {
    source_path: String,
    source_name: String,
    size: usize,
    parsed: netzipapi_rust_demo::Tdx0547Body,
}

#[derive(Clone, Copy)]
struct Quote0547ScopedRecord<'a> {
    source_path: &'a str,
    source_name: &'a str,
    record: &'a netzipapi_rust_demo::Tdx0547Record,
}

#[derive(Clone)]
struct NormalizedLiveQuoteSymbol {
    market: u8,
    code: String,
    symbol: String,
}

#[derive(Serialize)]
struct QuoteFrameScanResponse {
    input: String,
    size: usize,
    mode: String,
    summary: QuoteFrameScanSummary,
    server_frames: Vec<netzipapi_rust_demo::Server16FrameSummary>,
    client_frames: Vec<netzipapi_rust_demo::Client10FrameSummary>,
    head_hex: String,
}

#[derive(Serialize)]
struct QuoteFrameScanSummary {
    server_frames: usize,
    client_frames: usize,
    labeled_server_frames: usize,
    labeled_client_frames: usize,
    code_table_requests: usize,
    bootstrap_labels: Vec<String>,
    phase_order: Vec<String>,
    phase_counts: BTreeMap<String, usize>,
    main_site_validate_after_tag_block8: Option<netzipapi_rust_demo::BlockPatternSummary>,
}

#[derive(Deserialize)]
struct QuoteReplayRequest {
    path: String,
    host: Option<String>,
    port: Option<u16>,
    segments: Option<String>,
    recv_ms: Option<u64>,
    connect_timeout_ms: Option<u64>,
    save_path: Option<String>,
}

#[derive(Serialize)]
struct QuoteReplayResponse {
    input: String,
    target: String,
    payload_size: usize,
    segment_count: usize,
    sent_bytes: usize,
    reply_bytes: usize,
    reply_head_hex: String,
    saved_path: Option<String>,
    transport_error: Option<String>,
}

#[derive(Deserialize)]
struct ProtoProbeRequest {
    host: Option<String>,
    port: Option<u16>,
    payload: Option<String>,
    encoding: Option<String>,
    nul_terminate: Option<bool>,
    prefix_u32le: Option<bool>,
    read_secs: Option<u64>,
}

#[derive(Serialize)]
struct ProtoProbeResponse {
    target: String,
    payload_len: Option<usize>,
    request_bytes: Option<usize>,
    request_hex: Option<String>,
    reply_bytes: usize,
    reply_head_hex: String,
    parsed_kind: Option<String>,
    parsed_summary: Option<String>,
    transport_error: Option<String>,
}

#[derive(Serialize)]
struct StreamAnalyzeResponse {
    input: String,
    is_hex: bool,
    size: usize,
    byte_profile: netzipapi_rust_demo::ByteProfile,
    utf8_preview: Option<String>,
    utf16le_preview: Option<String>,
    http_headers: Option<String>,
    netpacket_runs: Vec<netzipapi_rust_demo::NetPacketRun>,
    packet_summaries: Vec<netzipapi_rust_demo::NetPacketSliceSummary>,
    zlib_hits: Vec<netzipapi_rust_demo::ZlibHit>,
    oem_at_0: Option<String>,
    oem_at_4: Option<String>,
    candidate_packets: Vec<netzipapi_rust_demo::CandidatePacket>,
}

#[derive(Deserialize)]
struct Local2000LogScanRequest {
    path: String,
    port: Option<u16>,
    small_max: Option<usize>,
    code_preview_limit: Option<usize>,
}

#[derive(Serialize)]
struct Local2000LogScanResponse {
    analysis: netzipapi_rust_demo::Local2000LogAnalysis,
}

#[derive(Deserialize)]
struct Local2000VsAuth7100Request {
    local_log_path: Option<String>,
    local_port: Option<u16>,
    small_max: Option<usize>,
    code_preview_limit: Option<usize>,
    auth_pcap_path: Option<String>,
}

#[derive(Serialize)]
struct Local2000VsAuth7100Response {
    analysis: netzipapi_rust_demo::Local2000VsAuth7100Analysis,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let listen = parse_listen()?;

    let state = AppState {
        service_name: "netzip-rs",
        version: env!("CARGO_PKG_VERSION"),
    };

    let app = Router::new()
        .route("/", get(web_index))
        .route("/favicon.ico", get(web_favicon))
        .route("/webgui", get(web_index))
        .route("/webgui/app.js", get(web_app_js))
        .route("/webgui/favicon.ico", get(web_favicon))
        .route("/webgui/styles.css", get(web_styles))
        .route("/health", get(health))
        .route("/api/capabilities", get(capabilities))
        .route("/api/quotes", get(compact_quotes))
        .route("/api/hqw/publish", post(hqw_publish))
        .route("/api/hqw/publish-worklist", post(hqw_publish_worklist))
        .route("/api/hqw/push-worklist", post(hqw_push_worklist))
        .route("/api/legacy/panel/bootstrap", get(legacy_panel_bootstrap))
        .route("/api/legacy/panel/save", post(legacy_panel_save))
        .route("/api/legacy/panel/probe", post(legacy_panel_probe))
        .route("/api/legacy/panel/connect", post(legacy_panel_connect))
        .route(
            "/api/legacy/panel/disconnect",
            post(legacy_panel_disconnect),
        )
        .route("/api/fin/parse", post(fin_parse))
        .route("/api/fin/query-record", post(fin_record_query))
        .route("/api/fin/getter-value", post(fin_getter_value))
        .route("/api/fin/getter-specs", post(fin_getter_specs_api))
        .route("/api/tdx7709/sync-code-table", post(tdx7709_sync))
        .route("/api/tdx7709/query-code-table", post(tdx7709_code_query))
        .route("/api/tdx7709/live-quote", post(tdx7709_live_quote))
        .route("/api/tdx7709/snapshot", post(tdx7709_snapshot))
        .route("/api/tdx7709/kline", post(tdx7709_kline))
        .route("/api/tdx7709/f10/categories", post(tdx7709_f10_categories))
        .route("/api/tdx7709/f10/content", post(tdx7709_f10_content))
        .route("/api/tdx7709/bootstrap-plan", get(tdx7709_bootstrap_plan))
        .route("/api/linux/pure-rust-mvp", post(linux_pure_rust_mvp))
        .route(
            "/api/debug/tdx118-dump-compare-plan",
            get(tdx118_dump_compare_plan),
        )
        .route("/api/debug/answer-summary", post(answer_summary))
        .route("/api/debug/blob-compare", post(blob_compare))
        .route(
            "/api/debug/auth-7100-client-shell-correlation",
            get(auth_7100_client_shell_correlation),
        )
        .route(
            "/api/debug/auth-7100-shell-correlation",
            get(auth_7100_shell_correlation).post(auth_7100_shell_correlation_for_path),
        )
        .route(
            "/api/debug/auth-7100-flow-matrix",
            get(auth_7100_flow_matrix).post(auth_7100_flow_matrix_for_path),
        )
        .route(
            "/api/debug/local-2000-vs-auth-7100",
            get(local_2000_vs_auth_7100).post(local_2000_vs_auth_7100_for_path),
        )
        .route(
            "/api/debug/auth-7100-server-sample",
            get(auth_7100_server_sample),
        )
        .route("/api/debug/quote-replay", post(quote_replay))
        .route("/api/debug/proto-probe", post(proto_probe))
        .route("/api/debug/quote-frame-scan", post(quote_frame_scan))
        .route("/api/debug/local-2000-log-scan", post(local_2000_log_scan))
        .route("/api/debug/quote-0547-decode", post(quote_0547_decode))
        .route("/api/debug/quote-0547-query", post(quote_0547_query))
        .route(
            "/api/debug/quote-0547-extra-profile",
            post(quote_0547_extra_profile),
        )
        .route("/api/quote/0547/query", post(quote_0547_query))
        .route(
            "/api/quote/0547/extra-profile",
            post(quote_0547_extra_profile),
        )
        .route("/api/debug/pcap-summary", post(pcap_summary))
        .route("/api/debug/stream-analyze", post(stream_analyze))
        .with_state(state);

    println!("listening: http://{listen}");
    let listener = tokio::net::TcpListener::bind(listen).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: state.service_name,
        version: state.version,
    })
}

async fn capabilities() -> Json<CapabilitiesResponse> {
    Json(CapabilitiesResponse {
        service: "netzip-rs",
        stable_endpoints: stable_endpoints(),
        linux_native_endpoints: linux_native_endpoints(),
        windows_bridge_endpoints: windows_bridge_endpoints(),
        research_endpoints: research_endpoints(),
        recommended_delivery_tracks: recommended_delivery_tracks(),
        pure_rust_linux_feasibility: "partial_now_full_replacement_pending",
        pure_rust_linux_summary: "纯 Rust + Linux 已可覆盖 7709 行情/K线/F10 与 FIN/0547 解析；要完整替代 Windows 主链，仍卡在 7100 登录后壳层与上游初始化对象链的纯 Rust 闭环。",
        pure_rust_linux_hard_blockers: pure_rust_linux_hard_blockers(),
        pure_rust_linux_non_blocking_gaps: pure_rust_linux_non_blocking_gaps(),
        fin_urls: [SH_FIN_URL, SZ_FIN_URL],
    })
}

fn stable_endpoints() -> Vec<&'static str> {
    vec![
        "GET /",
        "GET /webgui",
        "GET /webgui/app.js",
        "GET /webgui/styles.css",
        "GET /health",
        "GET /api/capabilities",
        "GET /api/quotes",
        "POST /api/hqw/publish",
        "POST /api/hqw/publish-worklist",
        "POST /api/hqw/push-worklist",
        "GET /api/legacy/panel/bootstrap",
        "POST /api/legacy/panel/save",
        "POST /api/legacy/panel/probe",
        "POST /api/legacy/panel/connect",
        "POST /api/legacy/panel/disconnect",
        "POST /api/fin/parse",
        "POST /api/fin/query-record",
        "POST /api/fin/getter-value",
        "POST /api/fin/getter-specs",
        "POST /api/tdx7709/sync-code-table",
        "POST /api/tdx7709/query-code-table",
        "POST /api/tdx7709/live-quote",
        "POST /api/tdx7709/snapshot",
        "POST /api/tdx7709/kline",
        "POST /api/tdx7709/f10/categories",
        "POST /api/tdx7709/f10/content",
        "GET /api/tdx7709/bootstrap-plan",
        "POST /api/linux/pure-rust-mvp",
        "GET /api/debug/tdx118-dump-compare-plan",
        "POST /api/debug/answer-summary",
        "POST /api/debug/blob-compare",
        "GET /api/debug/auth-7100-client-shell-correlation",
        "GET /api/debug/auth-7100-shell-correlation",
        "POST /api/debug/auth-7100-shell-correlation",
        "GET /api/debug/auth-7100-flow-matrix",
        "POST /api/debug/auth-7100-flow-matrix",
        "GET /api/debug/local-2000-vs-auth-7100",
        "POST /api/debug/local-2000-vs-auth-7100",
        "GET /api/debug/auth-7100-server-sample",
        "POST /api/debug/quote-replay",
        "POST /api/debug/proto-probe",
        "POST /api/debug/quote-frame-scan",
        "POST /api/debug/local-2000-log-scan",
        "POST /api/debug/quote-0547-decode",
        "POST /api/debug/quote-0547-query",
        "POST /api/debug/quote-0547-extra-profile",
        "POST /api/quote/0547/query",
        "POST /api/quote/0547/extra-profile",
        "POST /api/debug/pcap-summary",
        "POST /api/debug/stream-analyze",
    ]
}

fn linux_native_endpoints() -> Vec<&'static str> {
    vec![
        "GET /health",
        "GET /api/capabilities",
        "GET /api/quotes",
        "POST /api/hqw/publish",
        "POST /api/hqw/publish-worklist",
        "POST /api/hqw/push-worklist",
        "POST /api/fin/parse",
        "POST /api/fin/query-record",
        "POST /api/fin/getter-value",
        "POST /api/fin/getter-specs",
        "POST /api/tdx7709/sync-code-table",
        "POST /api/tdx7709/query-code-table",
        "POST /api/tdx7709/live-quote",
        "POST /api/tdx7709/snapshot",
        "POST /api/tdx7709/kline",
        "POST /api/tdx7709/f10/categories",
        "POST /api/tdx7709/f10/content",
        "GET /api/tdx7709/bootstrap-plan",
        "POST /api/linux/pure-rust-mvp",
        "POST /api/quote/0547/query",
        "POST /api/quote/0547/extra-profile",
        "POST /api/debug/pcap-summary",
        "POST /api/debug/stream-analyze",
    ]
}

fn windows_bridge_endpoints() -> Vec<&'static str> {
    vec![
        "GET /api/legacy/panel/bootstrap",
        "POST /api/legacy/panel/save",
        "POST /api/legacy/panel/probe",
        "POST /api/legacy/panel/connect",
        "POST /api/legacy/panel/disconnect",
        "POST /api/debug/local-2000-log-scan",
        "GET /api/debug/local-2000-vs-auth-7100",
        "POST /api/debug/local-2000-vs-auth-7100",
    ]
}

fn research_endpoints() -> Vec<&'static str> {
    vec![
        "GET /api/debug/tdx118-dump-compare-plan",
        "POST /api/debug/answer-summary",
        "POST /api/debug/blob-compare",
        "GET /api/debug/auth-7100-client-shell-correlation",
        "GET /api/debug/auth-7100-shell-correlation",
        "POST /api/debug/auth-7100-shell-correlation",
        "GET /api/debug/auth-7100-flow-matrix",
        "POST /api/debug/auth-7100-flow-matrix",
        "GET /api/debug/auth-7100-server-sample",
        "POST /api/debug/quote-replay",
        "POST /api/debug/proto-probe",
        "POST /api/debug/quote-frame-scan",
        "POST /api/debug/quote-0547-decode",
        "POST /api/debug/quote-0547-query",
        "POST /api/debug/quote-0547-extra-profile",
    ]
}

fn recommended_delivery_tracks() -> Vec<DeliveryTrack> {
    vec![
        DeliveryTrack {
            name: "Pure Rust + Linux partial delivery",
            priority: "highest",
            status: "available_now",
            scope: "7709 行情/K线/F10 + FIN/0547 解析 + 本地 HTTP 服务",
            notes: vec![
                "不依赖 Windows DLL",
                "适合先交付 Linux-only 行情服务或数据中台",
            ],
        },
        DeliveryTrack {
            name: "Windows-first compatibility bridge",
            priority: "medium",
            status: "available_now",
            scope: "x86 Stock.dll + 网际风.exe + 本地 2000 + Rust 封装",
            notes: vec!["最接近原厂行为", "不应再作为当前最高优先级，但仍是对账锚点"],
        },
        DeliveryTrack {
            name: "Pure Rust full upstream replacement",
            priority: "long_term",
            status: "research_in_progress",
            scope: "完整替代 7100 登录、初始化对象链和后续行情链",
            notes: vec!["仍卡在 7100 登录后壳层", "不应阻塞 Linux-only 子集先交付"],
        },
    ]
}

fn pure_rust_linux_hard_blockers() -> Vec<&'static str> {
    vec![
        "7100 登录后 Tdx_Encrypt / penc / ZSTD 字典壳层尚未完整可逆",
        "纯 Rust 初始化对象链还未替代 Windows 现场的 代码表/除权/财务/文件/实时数据 全流程",
        "上游认证/下载链仍缺稳定的无 DLL 会话闭环",
    ]
}

fn pure_rust_linux_non_blocking_gaps() -> Vec<&'static str> {
    vec![
        "local-2000 的 hypenc 与后续 penc 语义仍在研究，但这不阻塞 Linux-only 路线",
        "旧客户端字节级兼容不是当前 pure Rust + Linux MVP 的必要条件",
        "财务V6.fin 剩余个别字段口径尚未最终坐实，但不阻塞实时行情/K线/F10 主路径",
    ]
}

fn build_tdx7709_config(
    host: &str,
    port: u16,
    read_timeout_ms: u64,
    connect_timeout_ms: u64,
    settle_ms: u64,
) -> Tdx7709Config {
    Tdx7709Config {
        host: host.to_string(),
        port,
        read_timeout: Duration::from_millis(read_timeout_ms),
        connect_timeout: Duration::from_millis(connect_timeout_ms),
        settle_delay: Duration::from_millis(settle_ms),
    }
}

fn linux_phase_ok<T: Serialize>(data: T) -> LinuxPureRustMvpPhase<T> {
    LinuxPureRustMvpPhase {
        enabled: true,
        skipped: false,
        ok: true,
        error: None,
        data: Some(data),
    }
}

fn linux_phase_err<T: Serialize>(message: impl Into<String>) -> LinuxPureRustMvpPhase<T> {
    LinuxPureRustMvpPhase {
        enabled: true,
        skipped: false,
        ok: false,
        error: Some(message.into()),
        data: None,
    }
}

fn linux_phase_skipped<T: Serialize>(message: impl Into<String>) -> LinuxPureRustMvpPhase<T> {
    LinuxPureRustMvpPhase {
        enabled: false,
        skipped: true,
        ok: false,
        error: Some(message.into()),
        data: None,
    }
}

fn tdx7709_sync_response_from_result(
    host: String,
    port: u16,
    result: netzipapi_rust_demo::Tdx7709SyncResult,
    preview_limit: usize,
    out_csv_path: Option<String>,
) -> Result<Tdx7709SyncResponse, ApiError> {
    if let Some(csv_path) = &out_csv_path {
        write_code_table_csv(csv_path, &result.records)
            .map_err(|err| ApiError::internal(format!("write CSV failed: {err}")))?;
    }

    let preview = result
        .records
        .iter()
        .take(preview_limit)
        .map(code_table_preview)
        .collect::<Vec<_>>();

    Ok(Tdx7709SyncResponse {
        host,
        port,
        reply_bytes: result.raw_reply.len(),
        reply_frames: result.frames.len(),
        records_total: result.records.len(),
        out_csv_path,
        first_record: result.records.first().map(code_table_preview),
        last_record: result.records.last().map(code_table_preview),
        preview,
    })
}

fn tdx7709_live_quote_response_from_result(
    host: String,
    port: u16,
    normalized_symbols: &[NormalizedLiveQuoteSymbol],
    limit: usize,
    result: netzipapi_rust_demo::Tdx7709LiveQuoteResult,
) -> Tdx7709LiveQuoteResponse {
    let transport_symbols = augment_live_quote_symbols(normalized_symbols);
    let code_table_lookup = build_quote_0547_code_table_lookup(&result.code_table_records);
    let requested_set = normalized_symbols
        .iter()
        .map(|item| item.symbol.clone())
        .collect::<BTreeSet<_>>();
    let mut seen_symbols = BTreeSet::new();
    let mut matched_records = Vec::new();
    let mut parsed_quote_records = 0usize;

    for body in &result.quote_bodies {
        parsed_quote_records += body.records.len();
        for record in &body.records {
            let Some(symbol) = tdx_0547_record_symbol(record) else {
                continue;
            };
            if !requested_set.contains(&symbol) || !seen_symbols.insert(symbol) {
                continue;
            }
            matched_records.push(record);
        }
    }

    let preview = matched_records
        .iter()
        .take(limit)
        .map(|record| {
            quote_0547_record_preview(
                record,
                resolve_quote_0547_decimal_point(record, None, &code_table_lookup),
                lookup_quote_0547_code_table_info(record, &code_table_lookup),
                None,
                None,
            )
        })
        .collect::<Vec<_>>();
    let unmatched_symbols = normalized_symbols
        .iter()
        .filter(|item| !seen_symbols.contains(&item.symbol))
        .map(|item| item.symbol.clone())
        .collect::<Vec<_>>();

    Tdx7709LiveQuoteResponse {
        host,
        port,
        requested_symbols: normalized_symbols
            .iter()
            .map(|item| item.symbol.clone())
            .collect(),
        transport_symbols: transport_symbols
            .iter()
            .map(|item| item.symbol.clone())
            .collect(),
        unmatched_symbols,
        code_table_reply_bytes: result.code_table_reply.len(),
        code_table_reply_frames: result.code_table_frames.len(),
        code_table_records_total: result.code_table_records.len(),
        quote_reply_bytes: result.quote_reply.len(),
        quote_reply_frames: result.quote_frames.len(),
        parsed_quote_bodies: result.quote_bodies.len(),
        parsed_quote_records,
        matched_records: matched_records.len(),
        preview,
        first_match: matched_records.first().map(|record| {
            quote_0547_record_preview(
                record,
                resolve_quote_0547_decimal_point(record, None, &code_table_lookup),
                lookup_quote_0547_code_table_info(record, &code_table_lookup),
                None,
                None,
            )
        }),
        last_match: matched_records.last().map(|record| {
            quote_0547_record_preview(
                record,
                resolve_quote_0547_decimal_point(record, None, &code_table_lookup),
                lookup_quote_0547_code_table_info(record, &code_table_lookup),
                None,
                None,
            )
        }),
    }
}

fn compact_quotes_response_from_result(
    normalized_symbols: &[NormalizedLiveQuoteSymbol],
    result: &netzipapi_rust_demo::Tdx7709LiveQuoteResult,
) -> CompactQuotesResponse {
    const SOURCE: &str = "netzip-rust-7709";
    let requested = normalized_symbols
        .iter()
        .map(|item| item.symbol.clone())
        .collect::<BTreeSet<_>>();
    let code_table_lookup = build_quote_0547_code_table_lookup(&result.code_table_records);
    let mut seen = BTreeSet::new();
    let mut data = Vec::new();

    for record in result.quote_bodies.iter().flat_map(|body| &body.records) {
        let Some(symbol) = tdx_0547_record_symbol(record) else {
            continue;
        };
        if !requested.contains(&symbol) || seen.contains(&symbol) {
            continue;
        }
        let Some(decimal_point) =
            resolve_quote_0547_decimal_point(record, None, &code_table_lookup)
        else {
            continue;
        };
        let Some(head) = record
            .quote_head
            .as_ref()
            .and_then(|head| tdx_0547_normalize_quote_head(head, decimal_point))
        else {
            continue;
        };
        let Some(market) = tdx_0547_market_name(record.market) else {
            continue;
        };
        let name = lookup_quote_0547_code_table_info(record, &code_table_lookup)
            .map(|info| info.name.clone());
        seen.insert(symbol.clone());
        let quote_datetime = tdx_0547_public_time_hhmmss(record);
        let amount = oem_public_amount(record, decimal_point);
        data.push(CompactQuote {
            code: record.code.clone(),
            symbol,
            market: market.to_string(),
            name,
            price: head.price,
            last_close: head.last_close,
            open: head.open,
            high: head.high,
            low: head.low,
            volume: record.volume,
            amount,
            datetime: quote_datetime.clone(),
            quote_datetime,
            source: SOURCE,
        });
    }

    let missing_codes = normalized_symbols
        .iter()
        .filter(|item| !seen.contains(&item.symbol))
        .map(|item| item.symbol.clone())
        .collect::<Vec<_>>();
    CompactQuotesResponse {
        success: true,
        source: SOURCE,
        count: data.len(),
        partial: !data.is_empty() && !missing_codes.is_empty(),
        missing_codes,
        data,
    }
}

fn oem_public_amount(
    record: &netzipapi_rust_demo::Tdx0547Record,
    decimal_point: u8,
) -> Option<f64> {
    let amount = record.amount?;
    // Production 实时.dat category values and 网际风.exe 0x40a130 agree on these
    // mode boundaries; unknown security classes retain the decoded 0547 value.
    let mode = match record.market {
        1 if record.code.starts_with("000") => OemPublicAmountMode::Index,
        0 if record.code.starts_with("399") => OemPublicAmountMode::Index,
        2 if record.code.starts_with("899") => OemPublicAmountMode::Index,
        1 if ["60", "68", "90", "51", "56", "58"]
            .iter()
            .any(|prefix| record.code.starts_with(prefix)) =>
        {
            OemPublicAmountMode::PriceRelative
        }
        0 if ["00", "20", "15", "30"]
            .iter()
            .any(|prefix| record.code.starts_with(prefix)) =>
        {
            OemPublicAmountMode::PriceRelative
        }
        _ => OemPublicAmountMode::Direct,
    };
    if matches!(mode, OemPublicAmountMode::Direct) {
        return Some(amount);
    }

    let volume = record.volume? as f32;
    let amount = amount as f32;
    if volume == 0.0 {
        return Some(0.0);
    }

    let public = match mode {
        OemPublicAmountMode::Index => {
            let per_unit = amount / volume;
            let offset = per_unit - 1_000.0_f32;
            let scaled = offset * 100.0_f32;
            let encoded = (scaled + 0.5_f32).trunc() as i32;
            let decoded = encoded as f32 / 100.0_f32;
            let per_unit = decoded + 1_000.0_f32;
            per_unit * volume
        }
        OemPublicAmountMode::PriceRelative => {
            let price_raw = (record.quote_head.as_ref()?.price * 100.0).round() as i32;
            let hand = 100.0_f32;
            let point_scale = 10_f32.powi(i32::from(decimal_point));
            let per_unit = amount / volume;
            let per_share = per_unit / hand;
            let scaled_price = per_share * point_scale;
            let price_offset = scaled_price - price_raw as f32;
            let scaled_offset = price_offset * 30.0_f32;
            let encoded = (scaled_offset + 0.5_f32).trunc() as i32;
            let decoded_offset = encoded as f32 / 30.0_f32;
            let decoded_price = decoded_offset + price_raw as f32;
            let decoded_per_share = decoded_price * volume;
            let decoded_per_hand = decoded_per_share * hand;
            decoded_per_hand / point_scale
        }
        OemPublicAmountMode::Direct => unreachable!(),
    };
    Some(f64::from(public))
}

fn tdx7709_kline_response_from_result(
    host: String,
    port: u16,
    normalized: &NormalizedLiveQuoteSymbol,
    category: u16,
    start: u16,
    count: u16,
    limit: usize,
    result: netzipapi_rust_demo::Tdx7709KlineResult,
) -> Tdx7709KlineResponse {
    let kline_type = describe_kline_category(category).to_string();
    let preview = result
        .bars
        .iter()
        .take(limit)
        .map(tdx7709_kline_bar_preview)
        .collect::<Vec<_>>();

    Tdx7709KlineResponse {
        host,
        port,
        symbol: normalized.symbol.clone(),
        category,
        kline_type,
        start,
        requested_count: count,
        code_table_reply_bytes: result.code_table_reply.len(),
        code_table_reply_frames: result.code_table_frames.len(),
        code_table_records_total: result.code_table_records.len(),
        kline_reply_bytes: result.kline_reply.len(),
        kline_reply_frames: result.kline_frames.len(),
        bars_total: result.bars.len(),
        preview,
        first_bar: result.bars.first().map(tdx7709_kline_bar_preview),
        last_bar: result.bars.last().map(tdx7709_kline_bar_preview),
    }
}

fn tdx7709_f10_categories_response_from_result(
    host: String,
    port: u16,
    normalized: &NormalizedLiveQuoteSymbol,
    limit: usize,
    result: netzipapi_rust_demo::Tdx7709F10CategoriesResult,
) -> Tdx7709F10CategoriesResponse {
    let preview = result
        .categories
        .iter()
        .take(limit)
        .map(tdx7709_f10_category_preview)
        .collect::<Vec<_>>();

    Tdx7709F10CategoriesResponse {
        host,
        port,
        symbol: normalized.symbol.clone(),
        code_table_reply_bytes: result.code_table_reply.len(),
        code_table_reply_frames: result.code_table_frames.len(),
        code_table_records_total: result.code_table_records.len(),
        category_reply_bytes: result.category_reply.len(),
        category_reply_frames: result.category_frames.len(),
        categories_total: result.categories.len(),
        preview,
        first_category: result.categories.first().map(tdx7709_f10_category_preview),
        last_category: result.categories.last().map(tdx7709_f10_category_preview),
    }
}

fn execute_tdx7709_sync(
    host: String,
    port: u16,
    read_timeout_ms: u64,
    connect_timeout_ms: u64,
    settle_ms: u64,
    preview_limit: usize,
    out_csv_path: Option<String>,
) -> Result<Tdx7709SyncResponse, ApiError> {
    let config = build_tdx7709_config(&host, port, read_timeout_ms, connect_timeout_ms, settle_ms);
    let result = sync_code_table(&config)
        .map_err(|err| ApiError::internal(format!("7709 sync failed: {err}")))?;
    tdx7709_sync_response_from_result(host, port, result, preview_limit, out_csv_path)
}

fn execute_tdx7709_live_quote(
    host: String,
    port: u16,
    read_timeout_ms: u64,
    connect_timeout_ms: u64,
    settle_ms: u64,
    normalized_symbols: Vec<NormalizedLiveQuoteSymbol>,
    limit: usize,
) -> Result<Tdx7709LiveQuoteResponse, ApiError> {
    let config = build_tdx7709_config(&host, port, read_timeout_ms, connect_timeout_ms, settle_ms);
    let request_items = augment_live_quote_symbols(&normalized_symbols)
        .iter()
        .map(|item| Tdx7709QuoteRequestItem {
            market: item.market,
            code: item.code.clone(),
            token: 0,
        })
        .collect::<Vec<_>>();

    let result = fetch_live_quotes(&config, &request_items)
        .map_err(|err| ApiError::internal(format!("7709 live quote failed: {err}")))?;
    Ok(tdx7709_live_quote_response_from_result(
        host,
        port,
        &normalized_symbols,
        limit,
        result,
    ))
}

fn execute_compact_quotes(
    normalized_symbols: Vec<NormalizedLiveQuoteSymbol>,
) -> Result<CompactQuotesResponse, ApiError> {
    let config = Tdx7709Config::default();
    let request_items = augment_live_quote_symbols(&normalized_symbols)
        .iter()
        .map(|item| Tdx7709QuoteRequestItem {
            market: item.market,
            code: item.code.clone(),
            token: 0,
        })
        .collect::<Vec<_>>();
    let result = fetch_live_quotes(&config, &request_items)
        .map_err(|err| ApiError::internal(format!("7709 compact quotes failed: {err}")))?;
    Ok(compact_quotes_response_from_result(
        &normalized_symbols,
        &result,
    ))
}

fn execute_tdx7709_snapshot(
    host: String,
    port: u16,
    read_timeout_ms: u64,
    connect_timeout_ms: u64,
    settle_ms: u64,
    normalized: NormalizedLiveQuoteSymbol,
    category: u16,
    kline_start: u16,
    kline_count: u16,
    kline_limit: usize,
    f10_limit: usize,
    include_live_quote: bool,
    include_kline: bool,
    include_f10: bool,
) -> Tdx7709SnapshotResponse {
    let config = build_tdx7709_config(&host, port, read_timeout_ms, connect_timeout_ms, settle_ms);
    let shared_session_attempted = include_live_quote || include_kline || include_f10;
    let mut shared_session = if shared_session_attempted {
        Tdx7709Session::open(&config).ok()
    } else {
        None
    };
    let shared_session_established = shared_session.is_some();
    let mut shared_session_fallback_phases = Vec::new();

    let kline = if include_kline {
        if shared_session.is_some() {
            let shared_result = shared_session.as_mut().unwrap().request_kline(
                normalized.market,
                &normalized.code,
                category,
                kline_start,
                kline_count,
            );
            match shared_result {
                Ok(result) => Some(Ok(tdx7709_kline_response_from_result(
                    host.clone(),
                    port,
                    &normalized,
                    category,
                    kline_start,
                    kline_count,
                    kline_limit,
                    result,
                ))),
                Err(_) => {
                    shared_session = None;
                    shared_session_fallback_phases.push("kline".to_string());
                    Some(execute_tdx7709_kline(
                        host.clone(),
                        port,
                        read_timeout_ms,
                        connect_timeout_ms,
                        settle_ms,
                        normalized.clone(),
                        category,
                        kline_start,
                        kline_count,
                        kline_limit,
                    ))
                }
            }
        } else {
            Some(execute_tdx7709_kline(
                host.clone(),
                port,
                read_timeout_ms,
                connect_timeout_ms,
                settle_ms,
                normalized.clone(),
                category,
                kline_start,
                kline_count,
                kline_limit,
            ))
        }
    } else {
        None
    };

    let f10_categories = if include_f10 {
        if shared_session.is_some() {
            let shared_result = shared_session
                .as_mut()
                .unwrap()
                .request_f10_categories(normalized.market, &normalized.code);
            match shared_result {
                Ok(result) => Some(Ok(tdx7709_f10_categories_response_from_result(
                    host.clone(),
                    port,
                    &normalized,
                    f10_limit,
                    result,
                ))),
                Err(_) => {
                    shared_session = None;
                    shared_session_fallback_phases.push("f10-categories".to_string());
                    Some(execute_tdx7709_f10_categories(
                        host.clone(),
                        port,
                        read_timeout_ms,
                        connect_timeout_ms,
                        settle_ms,
                        normalized.clone(),
                        f10_limit,
                    ))
                }
            }
        } else {
            Some(execute_tdx7709_f10_categories(
                host.clone(),
                port,
                read_timeout_ms,
                connect_timeout_ms,
                settle_ms,
                normalized.clone(),
                f10_limit,
            ))
        }
    } else {
        None
    };

    let live_quote = if include_live_quote {
        let normalized_symbols = vec![normalized.clone()];
        if shared_session.is_some() {
            let request_items = augment_live_quote_symbols(&normalized_symbols)
                .iter()
                .map(|item| Tdx7709QuoteRequestItem {
                    market: item.market,
                    code: item.code.clone(),
                    token: 0,
                })
                .collect::<Vec<_>>();
            let shared_result = shared_session
                .as_mut()
                .unwrap()
                .request_live_quotes(&request_items);
            match shared_result {
                Ok(result) => Some(Ok(tdx7709_live_quote_response_from_result(
                    host.clone(),
                    port,
                    &normalized_symbols,
                    1,
                    result,
                ))),
                Err(_) => {
                    shared_session_fallback_phases.push("live-quote".to_string());
                    Some(execute_tdx7709_live_quote(
                        host.clone(),
                        port,
                        read_timeout_ms,
                        connect_timeout_ms,
                        settle_ms,
                        normalized_symbols,
                        1,
                    ))
                }
            }
        } else {
            Some(execute_tdx7709_live_quote(
                host.clone(),
                port,
                read_timeout_ms,
                connect_timeout_ms,
                settle_ms,
                normalized_symbols,
                1,
            ))
        }
    } else {
        None
    };

    let (live_quote, live_quote_error) = match live_quote {
        Some(Ok(value)) => (Some(value), None),
        Some(Err(err)) => (None, Some(err.message)),
        None => (None, Some("skipped by request".to_string())),
    };
    let (kline, kline_error) = match kline {
        Some(Ok(value)) => (Some(value), None),
        Some(Err(err)) => (None, Some(err.message)),
        None => (None, Some("skipped by request".to_string())),
    };
    let (f10_categories, f10_categories_error) = match f10_categories {
        Some(Ok(value)) => (Some(value), None),
        Some(Err(err)) => (None, Some(err.message)),
        None => (None, Some("skipped by request".to_string())),
    };

    let kline_type = describe_kline_category(category).to_string();
    Tdx7709SnapshotResponse {
        host,
        port,
        symbol: normalized.symbol,
        category,
        kline_type,
        kline_start,
        kline_count,
        shared_session_attempted,
        shared_session_established,
        shared_session_fallback_phases,
        include_live_quote,
        include_kline,
        include_f10,
        live_quote,
        live_quote_error,
        kline,
        kline_error,
        f10_categories,
        f10_categories_error,
    }
}

fn execute_tdx7709_kline(
    host: String,
    port: u16,
    read_timeout_ms: u64,
    connect_timeout_ms: u64,
    settle_ms: u64,
    normalized: NormalizedLiveQuoteSymbol,
    category: u16,
    start: u16,
    count: u16,
    limit: usize,
) -> Result<Tdx7709KlineResponse, ApiError> {
    let config = build_tdx7709_config(&host, port, read_timeout_ms, connect_timeout_ms, settle_ms);
    let result = fetch_kline(
        &config,
        normalized.market,
        &normalized.code,
        category,
        start,
        count,
    )
    .map_err(|err| ApiError::internal(format!("7709 kline failed: {err}")))?;
    Ok(tdx7709_kline_response_from_result(
        host,
        port,
        &normalized,
        category,
        start,
        count,
        limit,
        result,
    ))
}

fn execute_tdx7709_f10_categories(
    host: String,
    port: u16,
    read_timeout_ms: u64,
    connect_timeout_ms: u64,
    settle_ms: u64,
    normalized: NormalizedLiveQuoteSymbol,
    limit: usize,
) -> Result<Tdx7709F10CategoriesResponse, ApiError> {
    let config = build_tdx7709_config(&host, port, read_timeout_ms, connect_timeout_ms, settle_ms);
    let result = fetch_f10_categories(&config, normalized.market, &normalized.code)
        .map_err(|err| ApiError::internal(format!("7709 f10 categories failed: {err}")))?;
    Ok(tdx7709_f10_categories_response_from_result(
        host,
        port,
        &normalized,
        limit,
        result,
    ))
}

async fn legacy_panel_bootstrap()
-> Result<Json<netzipapi_rust_demo::LegacyPanelBootstrap>, ApiError> {
    let response = tokio::task::spawn_blocking(|| {
        load_legacy_panel_bootstrap()
            .map_err(|err| ApiError::internal(format!("load legacy panel bootstrap failed: {err}")))
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn legacy_panel_save(
    Json(request): Json<LegacyPanelConfigRequest>,
) -> Result<Json<netzipapi_rust_demo::LegacyPanelSaveResponse>, ApiError> {
    let response = tokio::task::spawn_blocking(move || {
        save_legacy_panel_config(request.config)
            .map_err(|err| ApiError::bad_request(format!("save legacy panel config failed: {err}")))
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn legacy_panel_probe(
    Json(request): Json<LegacyPanelActionRequest>,
) -> Result<Json<netzipapi_rust_demo::LegacyPanelProbeResponse>, ApiError> {
    let timeout_ms = request.timeout_ms.unwrap_or(1500).clamp(200, 10_000);
    let response = tokio::task::spawn_blocking(move || {
        probe_legacy_servers(request.config, timeout_ms)
            .map_err(|err| ApiError::bad_request(format!("probe legacy panel failed: {err}")))
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn legacy_panel_connect(
    Json(request): Json<LegacyPanelActionRequest>,
) -> Result<Json<netzipapi_rust_demo::LegacyPanelConnectResponse>, ApiError> {
    let timeout_ms = request.timeout_ms.unwrap_or(1500).clamp(200, 10_000);
    let response = tokio::task::spawn_blocking(move || {
        connect_legacy_panel(request.config, timeout_ms)
            .map_err(|err| ApiError::bad_request(format!("connect legacy panel failed: {err}")))
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn legacy_panel_disconnect(
    Json(request): Json<LegacyPanelConfigRequest>,
) -> Result<Json<netzipapi_rust_demo::LegacyPanelDisconnectResponse>, ApiError> {
    let response = tokio::task::spawn_blocking(move || {
        disconnect_legacy_panel(request.config)
            .map_err(|err| ApiError::bad_request(format!("disconnect legacy panel failed: {err}")))
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn auth_7100_server_sample()
-> Result<Json<netzipapi_rust_demo::Auth7100ServerSampleAnalysis>, ApiError> {
    let response = tokio::task::spawn_blocking(|| {
        analyze_auth_7100_server_sample()
            .map_err(|err| ApiError::internal(format!("analyze auth 7100 sample failed: {err}")))
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn auth_7100_client_shell_correlation() -> Result<Json<Auth7100ClientShellResponse>, ApiError>
{
    let analysis = tokio::task::spawn_blocking(|| {
        analyze_auth_7100_client_shell_sample().map_err(|err| {
            ApiError::internal(format!(
                "analyze auth 7100 client shell correlation failed: {err}"
            ))
        })
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(Auth7100ClientShellResponse { analysis }))
}

async fn auth_7100_shell_correlation() -> Result<Json<Auth7100ShellCorrelationResponse>, ApiError> {
    let analysis = tokio::task::spawn_blocking(|| {
        analyze_auth_7100_shell_correlation_sample().map_err(|err| {
            ApiError::internal(format!("analyze auth 7100 shell correlation failed: {err}"))
        })
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(Auth7100ShellCorrelationResponse { analysis }))
}

async fn auth_7100_shell_correlation_for_path(
    Json(request): Json<Auth7100PathFilterRequest>,
) -> Result<Json<Auth7100ShellCorrelationResponse>, ApiError> {
    let path = request.path.clone();
    let analysis = tokio::task::spawn_blocking(move || {
        analyze_auth_7100_shell_correlation_from_pcap(&path).map_err(|err| {
            ApiError::bad_request(format!(
                "analyze auth 7100 shell correlation for path failed: {err}"
            ))
        })
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;
    let analysis = filter_auth_7100_shell_correlation(analysis, &request);

    Ok(Json(Auth7100ShellCorrelationResponse { analysis }))
}

async fn auth_7100_flow_matrix() -> Result<Json<Auth7100FlowMatrixResponse>, ApiError> {
    let analysis = tokio::task::spawn_blocking(|| {
        analyze_auth_7100_flow_matrix_sample().map_err(|err| {
            ApiError::internal(format!("analyze auth 7100 flow matrix failed: {err}"))
        })
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(Auth7100FlowMatrixResponse { analysis }))
}

async fn auth_7100_flow_matrix_for_path(
    Json(request): Json<Auth7100PathFilterRequest>,
) -> Result<Json<Auth7100FlowMatrixResponse>, ApiError> {
    let path = request.path.clone();
    let analysis = tokio::task::spawn_blocking(move || {
        analyze_auth_7100_flow_matrix(&path).map_err(|err| {
            ApiError::bad_request(format!("analyze auth 7100 flow matrix failed: {err}"))
        })
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;
    let analysis = filter_auth_7100_flow_matrix(analysis, &request);

    Ok(Json(Auth7100FlowMatrixResponse { analysis }))
}

async fn local_2000_log_scan(
    Json(request): Json<Local2000LogScanRequest>,
) -> Result<Json<Local2000LogScanResponse>, ApiError> {
    let path = request.path.clone();
    let port = request.port.unwrap_or(2000);
    let small_max = request
        .small_max
        .unwrap_or(4096)
        .clamp(128, 16 * 1024 * 1024);
    let code_preview_limit = request.code_preview_limit.unwrap_or(20).clamp(1, 200);

    let analysis = tokio::task::spawn_blocking(move || {
        analyze_local_2000_log_file(&path, port, small_max, code_preview_limit)
            .map_err(|err| ApiError::bad_request(format!("analyze local 2000 log failed: {err}")))
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(Local2000LogScanResponse { analysis }))
}

async fn local_2000_vs_auth_7100() -> Result<Json<Local2000VsAuth7100Response>, ApiError> {
    let analysis = tokio::task::spawn_blocking(|| {
        analyze_local_2000_vs_auth7100_sample().map_err(|err| {
            ApiError::internal(format!(
                "analyze local 2000 vs auth 7100 sample failed: {err}"
            ))
        })
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(Local2000VsAuth7100Response { analysis }))
}

async fn local_2000_vs_auth_7100_for_path(
    Json(request): Json<Local2000VsAuth7100Request>,
) -> Result<Json<Local2000VsAuth7100Response>, ApiError> {
    let local_log_path = request
        .local_log_path
        .ok_or_else(|| ApiError::bad_request("missing local_log_path".to_string()))?;
    let auth_pcap_path = request
        .auth_pcap_path
        .ok_or_else(|| ApiError::bad_request("missing auth_pcap_path".to_string()))?;
    let local_port = request.local_port.unwrap_or(2000);
    let small_max = request
        .small_max
        .unwrap_or(4096)
        .clamp(128, 16 * 1024 * 1024);
    let code_preview_limit = request.code_preview_limit.unwrap_or(20).clamp(1, 200);

    let analysis = tokio::task::spawn_blocking(move || {
        analyze_local_2000_vs_auth7100(
            &local_log_path,
            local_port,
            small_max,
            code_preview_limit,
            &auth_pcap_path,
        )
        .map_err(|err| {
            ApiError::bad_request(format!("analyze local 2000 vs auth 7100 failed: {err}"))
        })
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(Local2000VsAuth7100Response { analysis }))
}

fn filter_auth_7100_shell_correlation(
    mut analysis: netzipapi_rust_demo::Auth7100ShellCorrelationAnalysis,
    request: &Auth7100PathFilterRequest,
) -> netzipapi_rust_demo::Auth7100ShellCorrelationAnalysis {
    analysis.sessions.retain(|session| {
        matches_optional_filter(&request.local_endpoint, &session.local_endpoint)
            && matches_optional_filter(&request.session_role, &session.session_role)
    });

    for session in &mut analysis.sessions {
        session.directions.retain(|direction| {
            matches_optional_filter(&request.source_endpoint, &direction.source_endpoint)
                && matches_optional_filter(
                    &request.destination_endpoint,
                    &direction.destination_endpoint,
                )
        });
    }

    if request.source_endpoint.is_some() || request.destination_endpoint.is_some() {
        analysis
            .sessions
            .retain(|session| !session.directions.is_empty());
    }

    analysis.findings = analysis
        .sessions
        .iter()
        .flat_map(|session| session.directions.iter())
        .flat_map(|direction| direction.findings.iter().cloned())
        .collect();
    analysis
}

fn filter_auth_7100_flow_matrix(
    mut analysis: netzipapi_rust_demo::Auth7100FlowMatrix,
    request: &Auth7100PathFilterRequest,
) -> netzipapi_rust_demo::Auth7100FlowMatrix {
    let server_endpoint = analysis.server_endpoint.clone();
    analysis.sessions.retain(|session| {
        matches_optional_filter(&request.local_endpoint, &session.local_endpoint)
            && matches_optional_filter(&request.session_role, &session.session_role)
            && matches_matrix_direction_filters(
                &server_endpoint,
                &session.local_endpoint,
                request.source_endpoint.as_deref(),
                request.destination_endpoint.as_deref(),
            )
    });
    analysis
}

fn matches_optional_filter(filter: &Option<String>, value: &str) -> bool {
    filter
        .as_deref()
        .is_none_or(|expected| expected.eq_ignore_ascii_case(value))
}

fn matches_matrix_direction_filters(
    server_endpoint: &str,
    local_endpoint: &str,
    source_endpoint: Option<&str>,
    destination_endpoint: Option<&str>,
) -> bool {
    match (source_endpoint, destination_endpoint) {
        (None, None) => true,
        (Some(source), None) => {
            source.eq_ignore_ascii_case(server_endpoint)
                || source.eq_ignore_ascii_case(local_endpoint)
        }
        (None, Some(destination)) => {
            destination.eq_ignore_ascii_case(server_endpoint)
                || destination.eq_ignore_ascii_case(local_endpoint)
        }
        (Some(source), Some(destination)) => {
            (source.eq_ignore_ascii_case(server_endpoint)
                && destination.eq_ignore_ascii_case(local_endpoint))
                || (source.eq_ignore_ascii_case(local_endpoint)
                    && destination.eq_ignore_ascii_case(server_endpoint))
        }
    }
}

async fn web_index() -> Html<&'static str> {
    Html(include_str!("../../webgui/index.html"))
}

async fn web_styles() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../webgui/styles.css"),
    )
}

async fn web_app_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../../webgui/app.js"),
    )
}

async fn web_favicon() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/x-icon")],
        include_bytes!("../../webgui/favicon.ico").as_slice(),
    )
}

async fn fin_parse(
    Json(request): Json<FinParseRequest>,
) -> Result<Json<FinParseResponse>, ApiError> {
    let path = request.path.clone();
    let limit = request.limit.unwrap_or(10).min(100);
    let out_csv_path = request.out_csv_path.clone();

    let response = tokio::task::spawn_blocking(move || -> Result<FinParseResponse, ApiError> {
        let parsed = parse_fin_file(&path)
            .map_err(|err| ApiError::bad_request(format!("parse FIN failed: {err}")))?;

        if let Some(csv_path) = &out_csv_path {
            write_fin_csv(csv_path, &parsed.records)
                .map_err(|err| ApiError::internal(format!("write CSV failed: {err}")))?;
        }

        let preview = parsed
            .records
            .iter()
            .take(limit)
            .map(fin_record_preview)
            .collect::<Vec<_>>();

        Ok(FinParseResponse {
            path,
            magic_hex: format!("0x{:08x}", parsed.magic),
            record_size: parsed.record_size,
            records_total: parsed.records.len(),
            trailing_bytes: parsed.trailing_bytes.len(),
            out_csv_path,
            first_record: parsed.records.first().map(fin_record_preview),
            last_record: parsed.records.last().map(fin_record_preview),
            preview,
        })
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn fin_record_query(
    Json(request): Json<FinRecordQueryRequest>,
) -> Result<Json<FinRecordQueryResponse>, ApiError> {
    let path = PathBuf::from(request.path.clone());
    let symbol = request.symbol.clone();

    let response =
        tokio::task::spawn_blocking(move || -> Result<FinRecordQueryResponse, ApiError> {
            let parsed = parse_fin_file(&path)
                .map_err(|err| ApiError::bad_request(format!("parse FIN failed: {err}")))?;
            let record = find_fin_record(&parsed.records, &symbol);

            Ok(FinRecordQueryResponse {
                path: path.display().to_string(),
                symbol,
                record_found: record.is_some(),
                record: record.map(fin_record_query_data),
            })
        })
        .await
        .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn fin_getter_value(
    Json(request): Json<FinGetterValueRequest>,
) -> Result<Json<FinGetterValueResponse>, ApiError> {
    let path = PathBuf::from(request.path.clone());
    let symbol = request.symbol.clone();
    let field_id = request.field_id;

    let response =
        tokio::task::spawn_blocking(move || -> Result<FinGetterValueResponse, ApiError> {
            let parsed = parse_fin_file(&path)
                .map_err(|err| ApiError::bad_request(format!("parse FIN failed: {err}")))?;
            let record = find_fin_record(&parsed.records, &symbol);

            let Some(record) = record else {
                return Ok(FinGetterValueResponse {
                    path: path.display().to_string(),
                    symbol,
                    field_id,
                    record_found: false,
                    record_symbol: None,
                    record_market: None,
                    record_code: None,
                    getter_name: None,
                    raw_offset: None,
                    note: None,
                    quarter: None,
                    value: None,
                });
            };

            let spec = fin_getter_specs().find(|spec| spec.field_id == field_id);
            let value = record.getter_value(field_id);

            Ok(FinGetterValueResponse {
                path: path.display().to_string(),
                symbol,
                field_id,
                record_found: true,
                record_symbol: Some(record.symbol.clone()),
                record_market: record.market().map(str::to_string),
                record_code: record.code().map(str::to_string),
                getter_name: spec.map(|spec| spec.name.to_string()),
                raw_offset: spec.and_then(|spec| spec.raw_offset),
                note: spec.map(|spec| spec.note.to_string()),
                quarter: Some(record.quarter()),
                value,
            })
        })
        .await
        .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn fin_getter_specs_api() -> Result<Json<FinGetterSpecsResponse>, ApiError> {
    let unresolved_field_ids = FIN_GETTER_UNRESOLVED_IDS.to_vec();
    let unresolved_lookup = unresolved_field_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let specs = fin_getter_specs()
        .map(|spec| FinGetterSpecPreview {
            field_id: spec.field_id,
            field_hex: format!("0x{:02x}", spec.field_id),
            name: spec.name.to_string(),
            display_name: humanize_snake(spec.name),
            status: if unresolved_lookup.contains(&spec.field_id) {
                "unresolved".to_string()
            } else if spec.raw_offset.is_some() {
                "confirmed".to_string()
            } else {
                "derived".to_string()
            },
            raw_offset: spec.raw_offset,
            note: spec.note.to_string(),
        })
        .collect::<Vec<_>>();
    let unresolved_field_hex = unresolved_field_ids
        .iter()
        .map(|field_id| format!("0x{:02x}", field_id))
        .collect::<Vec<_>>();
    let confirmed_count = specs
        .iter()
        .filter(|spec| spec.status == "confirmed")
        .count();
    let derived_count = specs.iter().filter(|spec| spec.status == "derived").count();
    let unresolved_count = unresolved_field_ids.len();
    Ok(Json(FinGetterSpecsResponse {
        specs,
        unresolved_field_ids,
        unresolved_field_hex,
        confirmed_count,
        derived_count,
        unresolved_count,
    }))
}

async fn tdx7709_sync(
    Json(request): Json<Tdx7709SyncRequest>,
) -> Result<Json<Tdx7709SyncResponse>, ApiError> {
    let host = request.host.unwrap_or_else(|| "120.195.71.160".to_string());
    let port = request.port.unwrap_or(7709);
    let read_timeout = request.read_timeout_ms.unwrap_or(1_000);
    let connect_timeout = request.connect_timeout_ms.unwrap_or(5_000);
    let settle_ms = request.settle_ms.unwrap_or(120);
    let preview_limit = request.preview_limit.unwrap_or(10).min(100);
    let out_csv_path = request.out_csv_path.clone();

    let response = tokio::task::spawn_blocking(move || {
        execute_tdx7709_sync(
            host,
            port,
            read_timeout,
            connect_timeout,
            settle_ms,
            preview_limit,
            out_csv_path,
        )
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn tdx7709_code_query(
    Json(request): Json<Tdx7709CodeQueryRequest>,
) -> Result<Json<Tdx7709CodeQueryResponse>, ApiError> {
    let host = request.host.unwrap_or_else(|| "120.195.71.160".to_string());
    let port = request.port.unwrap_or(7709);
    let query = request.query.unwrap_or_default();
    let limit = request.limit.unwrap_or(20).clamp(1, 200);
    let read_timeout = request.read_timeout_ms.unwrap_or(1_000);
    let connect_timeout = request.connect_timeout_ms.unwrap_or(5_000);
    let settle_ms = request.settle_ms.unwrap_or(120);

    let response =
        tokio::task::spawn_blocking(move || -> Result<Tdx7709CodeQueryResponse, ApiError> {
            let config = Tdx7709Config {
                host: host.clone(),
                port,
                read_timeout: Duration::from_millis(read_timeout),
                connect_timeout: Duration::from_millis(connect_timeout),
                settle_delay: Duration::from_millis(settle_ms),
            };

            let result = sync_code_table(&config)
                .map_err(|err| ApiError::internal(format!("7709 query failed: {err}")))?;

            let matches = result
                .records
                .iter()
                .filter(|record| matches_code_query(record, &query))
                .take(limit)
                .map(code_table_preview)
                .collect::<Vec<_>>();
            let matches_total = result
                .records
                .iter()
                .filter(|record| matches_code_query(record, &query))
                .count();

            Ok(Tdx7709CodeQueryResponse {
                host,
                port,
                query,
                records_total: result.records.len(),
                matches_total,
                reply_bytes: result.raw_reply.len(),
                reply_frames: result.frames.len(),
                preview: matches,
            })
        })
        .await
        .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn tdx7709_live_quote(
    Json(request): Json<Tdx7709LiveQuoteRequest>,
) -> Result<Json<Tdx7709LiveQuoteResponse>, ApiError> {
    let host = request.host.unwrap_or_else(|| "120.195.71.160".to_string());
    let port = request.port.unwrap_or(7709);
    let limit = request.limit.unwrap_or(20).clamp(1, 200);
    let read_timeout = request.read_timeout_ms.unwrap_or(1_000);
    let connect_timeout = request.connect_timeout_ms.unwrap_or(5_000);
    let settle_ms = request.settle_ms.unwrap_or(120);

    let normalized_symbols = request
        .symbols
        .into_iter()
        .map(|value| normalize_live_quote_symbol(&value))
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApiError::bad_request)?;

    if normalized_symbols.is_empty() {
        return Err(ApiError::bad_request(
            "symbols must contain at least one stock code",
        ));
    }
    if normalized_symbols.len() > 100 {
        return Err(ApiError::bad_request(format!(
            "symbols supports at most 100 items, got {}",
            normalized_symbols.len()
        )));
    }

    let response = tokio::task::spawn_blocking(move || {
        execute_tdx7709_live_quote(
            host,
            port,
            read_timeout,
            connect_timeout,
            settle_ms,
            normalized_symbols,
            limit,
        )
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn compact_quotes(
    Query(query): Query<CompactQuotesQuery>,
) -> Result<Json<CompactQuotesResponse>, ApiError> {
    let normalized_symbols =
        parse_compact_quote_codes(&query.codes).map_err(ApiError::bad_request)?;
    let response = tokio::task::spawn_blocking(move || execute_compact_quotes(normalized_symbols))
        .await
        .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn hqw_publish(
    Json(request): Json<HqwPublishRequest>,
) -> Result<Json<HqwPublishResponse>, ApiError> {
    let normalized_symbols = request
        .symbols
        .iter()
        .map(|value| normalize_live_quote_symbol(value))
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApiError::bad_request)?;
    if normalized_symbols.is_empty() || normalized_symbols.len() > 100 {
        return Err(ApiError::bad_request(
            "symbols must contain between 1 and 100 market-qualified codes",
        ));
    }
    let trade_date = normalize_trade_date(&request.trade_date).map_err(ApiError::bad_request)?;
    let gateway_addr = std::env::var("NETZIP_QUOTE_GATEWAY_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:16886".to_string());
    let current_token = std::env::var("NETZIP_QUOTE_GATEWAY_NETZIP_RUST_7709_TOKEN").ok();
    let requested_count = normalized_symbols.len();
    let response = tokio::task::spawn_blocking(move || {
        let quotes = execute_compact_quotes(normalized_symbols)?;
        let (publishable, missing_codes) = partition_hqw_quotes(quotes.data, quotes.missing_codes);
        let current_payload = build_netzip_rust_7709_quote_batch(&publishable, &trade_date)?;
        let mut protocol = GatewayPublishProtocol::Auto;
        let gateway_response = post_quote_gateway_batch_auto(
            &gateway_addr,
            current_token.as_deref(),
            &current_payload,
            &mut protocol,
            GatewayPublishLane::Manual,
        )?;
        Ok::<_, ApiError>(HqwPublishResponse {
            success: true,
            gateway_addr,
            gateway_protocol: protocol.label(),
            requested_count,
            published_count: publishable.len(),
            missing_codes,
            gateway_response,
        })
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;
    Ok(Json(response))
}

async fn hqw_publish_worklist(
    Json(request): Json<HqwPublishWorklistRequest>,
) -> Result<Json<HqwPublishWorklistResponse>, ApiError> {
    let batch_size = request.batch_size.unwrap_or(100);
    if !(1..=100).contains(&batch_size) {
        return Err(ApiError::bad_request(
            "batch_size must be between 1 and 100",
        ));
    }
    let limit = request.limit.unwrap_or(6_000);
    if !(1..=6_000).contains(&limit) {
        return Err(ApiError::bad_request("limit must be between 1 and 6000"));
    }
    let worker_count = request
        .worker_count
        .or_else(|| {
            std::env::var("NETZIP_FULL_PUSH_WORKERS")
                .ok()
                .and_then(|value| value.parse().ok())
        })
        .unwrap_or(4);
    validated_full_push_worker_count(worker_count, 1)?;
    let gateway_addr = std::env::var("NETZIP_QUOTE_GATEWAY_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:16886".to_string());
    let current_token = std::env::var("NETZIP_QUOTE_GATEWAY_NETZIP_RUST_7709_TOKEN").ok();
    let response = tokio::task::spawn_blocking(move || {
        execute_hqw_publish_worklist(
            gateway_addr,
            current_token.as_deref(),
            batch_size,
            limit,
            worker_count,
            true,
        )
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;
    Ok(Json(response))
}

async fn hqw_push_worklist(
    Json(request): Json<HqwPushWorklistRequest>,
) -> Result<Json<HqwPushWorklistResponse>, ApiError> {
    let duration_secs = request.duration_secs.unwrap_or(240);
    if !(5..=600).contains(&duration_secs) {
        return Err(ApiError::bad_request(
            "duration_secs must be between 5 and 600",
        ));
    }
    let audit_interval_secs = request.audit_interval_secs.unwrap_or(30);
    if !(10..=300).contains(&audit_interval_secs) {
        return Err(ApiError::bad_request(
            "audit_interval_secs must be between 10 and 300",
        ));
    }
    let bj_poll_interval_secs = request.bj_poll_interval_secs.unwrap_or(3);
    if !(1..=30).contains(&bj_poll_interval_secs) {
        return Err(ApiError::bad_request(
            "bj_poll_interval_secs must be between 1 and 30",
        ));
    }
    let publish = request.publish.unwrap_or(false);
    if publish && std::env::var("NETZIP_NATIVE_PUSH_PUBLISH_ENABLE").as_deref() != Ok("1") {
        return Err(ApiError::bad_request(
            "native push publication requires NETZIP_NATIVE_PUSH_PUBLISH_ENABLE=1",
        ));
    }
    let gateway_addr = std::env::var("NETZIP_QUOTE_GATEWAY_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:16886".to_string());
    let current_token = std::env::var("NETZIP_QUOTE_GATEWAY_NETZIP_RUST_7709_TOKEN").ok();
    let response = tokio::task::spawn_blocking(move || {
        execute_hqw_push_worklist(
            gateway_addr,
            current_token,
            Duration::from_secs(duration_secs),
            Duration::from_secs(audit_interval_secs),
            Duration::from_secs(bj_poll_interval_secs),
            publish,
        )
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;
    Ok(Json(response))
}

async fn tdx7709_snapshot(
    Json(request): Json<Tdx7709SnapshotRequest>,
) -> Result<Json<Tdx7709SnapshotResponse>, ApiError> {
    let host = request
        .host
        .unwrap_or_else(|| netzipapi_rust_demo::TDX7709_DEFAULT_HOST.to_string());
    let port = request
        .port
        .unwrap_or(netzipapi_rust_demo::TDX7709_DEFAULT_PORT);
    let read_timeout = request.read_timeout_ms.unwrap_or(1_000);
    let connect_timeout = request.connect_timeout_ms.unwrap_or(5_000);
    let settle_ms = request.settle_ms.unwrap_or(300);
    let kline_start = request.kline_start.unwrap_or(0);
    let kline_count = request.kline_count.unwrap_or(3).max(1);
    let kline_limit = request.kline_limit.unwrap_or(3).min(200);
    let f10_limit = request.f10_limit.unwrap_or(8).min(200);
    let include_live_quote = request.include_live_quote.unwrap_or(true);
    let include_kline = request.include_kline.unwrap_or(true);
    let include_f10 = request.include_f10.unwrap_or(true);
    let normalized = normalize_live_quote_symbol(&request.symbol).map_err(ApiError::bad_request)?;
    let category = resolve_kline_category(request.category, request.kline_type.as_deref())
        .map_err(ApiError::bad_request)?;

    let response = tokio::task::spawn_blocking(move || {
        execute_tdx7709_snapshot(
            host,
            port,
            read_timeout,
            connect_timeout,
            settle_ms,
            normalized,
            category,
            kline_start,
            kline_count,
            kline_limit,
            f10_limit,
            include_live_quote,
            include_kline,
            include_f10,
        )
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))?;

    Ok(Json(response))
}

async fn tdx7709_kline(
    Json(request): Json<Tdx7709KlineRequest>,
) -> Result<Json<Tdx7709KlineResponse>, ApiError> {
    let host = request
        .host
        .unwrap_or_else(|| netzipapi_rust_demo::TDX7709_DEFAULT_HOST.to_string());
    let port = request
        .port
        .unwrap_or(netzipapi_rust_demo::TDX7709_DEFAULT_PORT);
    let read_timeout = request.read_timeout_ms.unwrap_or(1_000);
    let connect_timeout = request.connect_timeout_ms.unwrap_or(5_000);
    let settle_ms = request.settle_ms.unwrap_or(300);
    let start = request.start.unwrap_or(0);
    let count = request.count.unwrap_or(32).max(1);
    let limit = request.limit.unwrap_or(20).min(200);
    let normalized = normalize_live_quote_symbol(&request.symbol).map_err(ApiError::bad_request)?;
    let category = resolve_kline_category(request.category, request.kline_type.as_deref())
        .map_err(ApiError::bad_request)?;

    let response = tokio::task::spawn_blocking(move || {
        execute_tdx7709_kline(
            host,
            port,
            read_timeout,
            connect_timeout,
            settle_ms,
            normalized,
            category,
            start,
            count,
            limit,
        )
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn tdx7709_f10_categories(
    Json(request): Json<Tdx7709F10CategoriesRequest>,
) -> Result<Json<Tdx7709F10CategoriesResponse>, ApiError> {
    let host = request
        .host
        .unwrap_or_else(|| netzipapi_rust_demo::TDX7709_DEFAULT_HOST.to_string());
    let port = request
        .port
        .unwrap_or(netzipapi_rust_demo::TDX7709_DEFAULT_PORT);
    let read_timeout = request.read_timeout_ms.unwrap_or(1_000);
    let connect_timeout = request.connect_timeout_ms.unwrap_or(5_000);
    let settle_ms = request.settle_ms.unwrap_or(300);
    let limit = request.limit.unwrap_or(20).min(200);
    let normalized = normalize_live_quote_symbol(&request.symbol).map_err(ApiError::bad_request)?;

    let response = tokio::task::spawn_blocking(move || {
        execute_tdx7709_f10_categories(
            host,
            port,
            read_timeout,
            connect_timeout,
            settle_ms,
            normalized,
            limit,
        )
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn tdx7709_f10_content(
    Json(request): Json<Tdx7709F10ContentRequest>,
) -> Result<Json<Tdx7709F10ContentResponse>, ApiError> {
    let host = request
        .host
        .unwrap_or_else(|| netzipapi_rust_demo::TDX7709_DEFAULT_HOST.to_string());
    let port = request
        .port
        .unwrap_or(netzipapi_rust_demo::TDX7709_DEFAULT_PORT);
    let read_timeout = request.read_timeout_ms.unwrap_or(1_000);
    let connect_timeout = request.connect_timeout_ms.unwrap_or(5_000);
    let settle_ms = request.settle_ms.unwrap_or(300);
    let preview_chars = request.preview_chars.unwrap_or(600).min(20_000);
    let normalized = normalize_live_quote_symbol(&request.symbol).map_err(ApiError::bad_request)?;

    let response =
        tokio::task::spawn_blocking(move || -> Result<Tdx7709F10ContentResponse, ApiError> {
            let config = Tdx7709Config {
                host: host.clone(),
                port,
                read_timeout: Duration::from_millis(read_timeout),
                connect_timeout: Duration::from_millis(connect_timeout),
                settle_delay: Duration::from_millis(settle_ms),
            };

            let mut resolved_category_name = None;
            let (filename, start, length) = if let Some(filename) = request
                .filename
                .as_ref()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
            {
                (
                    filename.to_string(),
                    request.start.unwrap_or(0),
                    request.length.unwrap_or(30_000).max(1),
                )
            } else if let Some(category_name) = request
                .category_name
                .as_ref()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
            {
                let categories = fetch_f10_categories(&config, normalized.market, &normalized.code)
                    .map_err(|err| {
                        ApiError::internal(format!("7709 f10 category lookup failed: {err}"))
                    })?;
                let matched = categories
                    .categories
                    .into_iter()
                    .find(|item| item.name == category_name)
                    .ok_or_else(|| {
                        ApiError::bad_request(format!(
                            "category_name not found for {}: {}",
                            normalized.symbol, category_name
                        ))
                    })?;
                resolved_category_name = Some(matched.name.clone());
                (matched.filename, matched.start, matched.length.max(1))
            } else {
                return Err(ApiError::bad_request(
                    "filename or category_name is required for F10 content",
                ));
            };

            let result = fetch_f10_content(
                &config,
                normalized.market,
                &normalized.code,
                &filename,
                start,
                length,
            )
            .map_err(|err| ApiError::internal(format!("7709 f10 content failed: {err}")))?;

            let content_preview = result
                .content
                .chars()
                .take(preview_chars)
                .collect::<String>();

            Ok(Tdx7709F10ContentResponse {
                host,
                port,
                symbol: normalized.symbol,
                resolved_category_name,
                filename,
                start,
                length,
                preview_chars,
                code_table_reply_bytes: result.code_table_reply.len(),
                code_table_reply_frames: result.code_table_frames.len(),
                code_table_records_total: result.code_table_records.len(),
                content_reply_bytes: result.content_reply.len(),
                content_reply_frames: result.content_frames.len(),
                content_chars_total: result.content.chars().count(),
                content_preview,
            })
        })
        .await
        .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn tdx7709_bootstrap_plan() -> Result<Json<Tdx7709BootstrapPlanResponse>, ApiError> {
    let response =
        tokio::task::spawn_blocking(move || -> Result<Tdx7709BootstrapPlanResponse, ApiError> {
            let probe = build_probe_hello()
                .map_err(|err| ApiError::internal(format!("build probe failed: {err}")))?;
            let packets = build_bootstrap_packets().map_err(|err| {
                ApiError::internal(format!("build bootstrap packets failed: {err}"))
            })?;

            Ok(Tdx7709BootstrapPlanResponse {
                probe_hello_hex: hex_line(&probe),
                probe_hello_len: probe.len(),
                packets: packets
                    .into_iter()
                    .map(|packet| Tdx7709BootstrapPacketResponse {
                        label: packet.label.to_string(),
                        op: packet.op,
                        sub: packet.sub,
                        flags: packet.flags,
                        body_len: packet.body_len,
                        request_len: packet.request.len(),
                        body_hex: spaced_hex(packet.body_hex),
                        request_hex: hex_line(&packet.request),
                    })
                    .collect(),
            })
        })
        .await
        .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn linux_pure_rust_mvp(
    Json(request): Json<LinuxPureRustMvpRequest>,
) -> Result<Json<LinuxPureRustMvpResponse>, ApiError> {
    let host = request
        .host
        .unwrap_or_else(|| netzipapi_rust_demo::TDX7709_DEFAULT_HOST.to_string());
    let port = request
        .port
        .unwrap_or(netzipapi_rust_demo::TDX7709_DEFAULT_PORT);
    let read_timeout = request.read_timeout_ms.unwrap_or(1_000);
    let connect_timeout = request.connect_timeout_ms.unwrap_or(5_000);
    let settle_ms = request.settle_ms.unwrap_or(300);
    let sync_preview_limit = request.sync_preview_limit.unwrap_or(10).clamp(1, 100);
    let quote_limit = request.quote_limit.unwrap_or(20).clamp(1, 100);
    let kline_limit = request.kline_limit.unwrap_or(10).clamp(1, 100);
    let f10_limit = request.f10_limit.unwrap_or(10).clamp(1, 100);
    let include_sync = request.include_sync.unwrap_or(true);
    let include_live_quote = request.include_live_quote.unwrap_or(true);
    let include_kline = request.include_kline.unwrap_or(true);
    let include_f10 = request.include_f10.unwrap_or(true);
    let kline_count = request.kline_count.unwrap_or(16).max(1);
    let category = resolve_kline_category(None, request.kline_type.as_deref())
        .map_err(ApiError::bad_request)?;
    let kline_type = describe_kline_category(category).to_string();

    let normalized_symbols = request
        .symbols
        .into_iter()
        .map(|value| normalize_live_quote_symbol(&value))
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApiError::bad_request)?;
    if normalized_symbols.is_empty() {
        return Err(ApiError::bad_request(
            "symbols must contain at least one stock code",
        ));
    }
    if normalized_symbols.len() > 100 {
        return Err(ApiError::bad_request(format!(
            "symbols supports at most 100 items, got {}",
            normalized_symbols.len()
        )));
    }
    let probe_symbol = normalized_symbols
        .first()
        .cloned()
        .ok_or_else(|| ApiError::bad_request("symbols must contain at least one stock code"))?;

    let response = tokio::task::spawn_blocking(move || {
        let config =
            build_tdx7709_config(&host, port, read_timeout, connect_timeout, settle_ms);
        let shared_session_attempted =
            include_sync || include_live_quote || include_kline || include_f10;
        let mut shared_session = if shared_session_attempted {
            Tdx7709Session::open(&config).ok()
        } else {
            None
        };
        let shared_session_established = shared_session.is_some();
        let mut shared_session_fallback_phases = Vec::new();

        let sync = if include_sync {
            if let Some(session) = shared_session.as_ref() {
                match tdx7709_sync_response_from_result(
                    host.clone(),
                    port,
                    session.sync_result(),
                    sync_preview_limit,
                    None,
                ) {
                    Ok(data) => linux_phase_ok(data),
                    Err(err) => linux_phase_err(err.message),
                }
            } else {
                match execute_tdx7709_sync(
                    host.clone(),
                    port,
                    read_timeout,
                    connect_timeout,
                    settle_ms,
                    sync_preview_limit,
                    None,
                ) {
                    Ok(data) => linux_phase_ok(data),
                    Err(err) => linux_phase_err(err.message),
                }
            }
        } else {
            linux_phase_skipped("skipped by request")
        };

        let kline = if include_kline {
            if shared_session.is_some() {
                let shared_result = shared_session.as_mut().unwrap().request_kline(
                    probe_symbol.market,
                    &probe_symbol.code,
                    category,
                    0,
                    kline_count,
                );
                match shared_result {
                    Ok(result) => linux_phase_ok(tdx7709_kline_response_from_result(
                        host.clone(),
                        port,
                        &probe_symbol,
                        category,
                        0,
                        kline_count,
                        kline_limit,
                        result,
                    )),
                    Err(_) => {
                        shared_session = None;
                        shared_session_fallback_phases.push("kline".to_string());
                        match execute_tdx7709_kline(
                            host.clone(),
                            port,
                            read_timeout,
                            connect_timeout,
                            settle_ms,
                            probe_symbol.clone(),
                            category,
                            0,
                            kline_count,
                            kline_limit,
                        ) {
                            Ok(data) => linux_phase_ok(data),
                            Err(err) => linux_phase_err(err.message),
                        }
                    }
                }
            } else {
                match execute_tdx7709_kline(
                    host.clone(),
                    port,
                    read_timeout,
                    connect_timeout,
                    settle_ms,
                    probe_symbol.clone(),
                    category,
                    0,
                    kline_count,
                    kline_limit,
                ) {
                    Ok(data) => linux_phase_ok(data),
                    Err(err) => linux_phase_err(err.message),
                }
            }
        } else {
            linux_phase_skipped("skipped by request")
        };

        let f10_categories = if include_f10 {
            if shared_session.is_some() {
                let shared_result = shared_session
                    .as_mut()
                    .unwrap()
                    .request_f10_categories(probe_symbol.market, &probe_symbol.code);
                match shared_result {
                    Ok(result) => linux_phase_ok(tdx7709_f10_categories_response_from_result(
                        host.clone(),
                        port,
                        &probe_symbol,
                        f10_limit,
                        result,
                    )),
                    Err(_) => {
                        shared_session = None;
                        shared_session_fallback_phases.push("f10-categories".to_string());
                        match execute_tdx7709_f10_categories(
                            host.clone(),
                            port,
                            read_timeout,
                            connect_timeout,
                            settle_ms,
                            probe_symbol.clone(),
                            f10_limit,
                        ) {
                            Ok(data) => linux_phase_ok(data),
                            Err(err) => linux_phase_err(err.message),
                        }
                    }
                }
            } else {
                match execute_tdx7709_f10_categories(
                    host.clone(),
                    port,
                    read_timeout,
                    connect_timeout,
                    settle_ms,
                    probe_symbol.clone(),
                    f10_limit,
                ) {
                    Ok(data) => linux_phase_ok(data),
                    Err(err) => linux_phase_err(err.message),
                }
            }
        } else {
            linux_phase_skipped("skipped by request")
        };

        let live_quote = if include_live_quote {
            if shared_session.is_some() {
                let request_items = augment_live_quote_symbols(&normalized_symbols)
                    .iter()
                    .map(|item| Tdx7709QuoteRequestItem {
                        market: item.market,
                        code: item.code.clone(),
                        token: 0,
                    })
                    .collect::<Vec<_>>();
                let shared_result = shared_session
                    .as_mut()
                    .unwrap()
                    .request_live_quotes(&request_items);
                match shared_result {
                    Ok(result) => linux_phase_ok(tdx7709_live_quote_response_from_result(
                        host.clone(),
                        port,
                        &normalized_symbols,
                        quote_limit,
                        result,
                    )),
                    Err(_) => {
                        shared_session_fallback_phases.push("live-quote".to_string());
                        match execute_tdx7709_live_quote(
                            host.clone(),
                            port,
                            read_timeout,
                            connect_timeout,
                            settle_ms,
                            normalized_symbols.clone(),
                            quote_limit,
                        ) {
                            Ok(data) => linux_phase_ok(data),
                            Err(err) => linux_phase_err(err.message),
                        }
                    }
                }
            } else {
                match execute_tdx7709_live_quote(
                    host.clone(),
                    port,
                    read_timeout,
                    connect_timeout,
                    settle_ms,
                    normalized_symbols.clone(),
                    quote_limit,
                ) {
                    Ok(data) => linux_phase_ok(data),
                    Err(err) => linux_phase_err(err.message),
                }
            }
        } else {
            linux_phase_skipped("skipped by request")
        };

        let overall_ok = (!include_sync || sync.ok)
            && (!include_live_quote || live_quote.ok)
            && (!include_kline || kline.ok)
            && (!include_f10 || f10_categories.ok);

        let mut phase_chain = Vec::new();
        if include_sync {
            phase_chain.push("sync-code-table".to_string());
        }
        if include_live_quote {
            phase_chain.push(format!(
                "live-quote({})",
                normalized_symbols
                    .iter()
                    .map(|item| item.symbol.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if include_kline {
            phase_chain.push(format!("kline({})", probe_symbol.symbol));
        }
        if include_f10 {
            phase_chain.push(format!("f10-categories({})", probe_symbol.symbol));
        }

        let mut findings = vec![
            "这条探针只走纯 Rust + Linux 侧已经落地的 7709/F10/0547 能力，不依赖 Windows DLL。"
                .to_string(),
            format!(
                "当前验证链：{}",
                if phase_chain.is_empty() {
                    "all phases skipped".to_string()
                } else {
                    phase_chain.join(" -> ")
                }
            ),
        ];
        if shared_session_established {
            findings.push(
                "本次探针优先复用了单个 7709 会话，避免重复 bootstrap/代码表同步。"
                    .to_string(),
            );
        }
        if !shared_session_fallback_phases.is_empty() {
            findings.push(format!(
                "共享会话在这些阶段回退为独立请求：{}。",
                shared_session_fallback_phases.join(", ")
            ));
        }
        if overall_ok {
            let mut success_parts = Vec::new();
            if include_sync {
                success_parts.push("代码表".to_string());
            }
            if include_live_quote {
                success_parts.push("实时行情".to_string());
            }
            if include_kline {
                success_parts.push("K线".to_string());
            }
            if include_f10 {
                success_parts.push("F10 栏目查询".to_string());
            }
            findings.push(format!(
                "当前样本已证明：Linux 侧可以在不依赖 DLL 的前提下跑通 {}。",
                if success_parts.is_empty() {
                    "所选阶段".to_string()
                } else {
                    success_parts.join("、")
                }
            ));
        } else {
            findings.push(
                "当前探针出现部分失败；失败不会自动推翻纯 Rust + Linux 路线，但会暴露当前节点、超时或未解协议层缺口。"
                    .to_string(),
            );
        }
        if !include_f10 {
            findings.push("本次请求显式跳过了 F10 栏目阶段。".to_string());
        }
        if !include_sync {
            findings.push("本次请求显式跳过了代码表同步阶段。".to_string());
            if include_live_quote || include_kline || include_f10 {
                findings.push(
                    "但后续 7709 请求仍会在 transport 层完成内部 bootstrap/代码表准备。"
                        .to_string(),
                );
            }
        }
        if !include_live_quote {
            findings.push("本次请求显式跳过了实时行情阶段。".to_string());
        }
        if !include_kline {
            findings.push("本次请求显式跳过了 K线阶段。".to_string());
        }
        if sync.enabled && !sync.ok {
            findings.push("代码表同步失败会直接影响后续 Linux-only 路线的符号解析与小数位校准。".to_string());
        }
        if live_quote.enabled && !live_quote.ok {
            findings.push("实时行情失败通常优先回查 7709 节点可用性、超时和 0547 回复路径。".to_string());
        }
        if kline.enabled && !kline.ok {
            findings.push("K线失败通常优先回查 7709 bootstrap、证券类别和 category/kline_type 映射。".to_string());
        }
        if f10_categories.enabled && !f10_categories.ok {
            findings.push("F10 栏目失败不阻塞 Linux-only 行情/K线 MVP，但会阻塞公告资料闭环。".to_string());
        }

        LinuxPureRustMvpResponse {
            host,
            port,
            requested_symbols: normalized_symbols
                .iter()
                .map(|item| item.symbol.clone())
                .collect(),
            probe_symbol: probe_symbol.symbol,
            kline_type,
            shared_session_attempted,
            shared_session_established,
            shared_session_fallback_phases,
            include_sync,
            include_live_quote,
            include_kline,
            include_f10,
            overall_ok,
            findings,
            sync,
            live_quote,
            kline,
            f10_categories,
        }
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))?;

    Ok(Json(response))
}

async fn tdx118_dump_compare_plan() -> Result<Json<Tdx118DumpComparePlanResponse>, ApiError> {
    Ok(Json(Tdx118DumpComparePlanResponse {
        status: "planned",
        ready: false,
        expected_plain_len: 0x118,
        expected_cipher_len: 0x118,
        tag_len: 2,
        plain_source_hint: "0x10066b60 -> 0x118 plaintext body before Tdx_Encrypt",
        cipher_target_hint: "0x7b00 payload[2..] -> 0x118 body after fixed-block processing",
        known_inputs: vec![
            "中信证券",
            "NetCardMac",
            "current server ip text",
            "account",
            "password",
        ],
        suggested_artifacts: vec![
            "Windows dump from 0x10066b60",
            "Windows dump from 0x1007b7c0 / 0x1002bf90",
            "captured_windows_traffic/client_to_server_full.raw",
            "tmp/flow_2400_7709.bin",
        ],
        notes: vec![
            "This is a staging endpoint for the upcoming 0x118 compare workflow.",
            "No protocol core was changed here.",
        ],
    }))
}

async fn answer_summary(
    Json(request): Json<AnswerSummaryRequest>,
) -> Result<Json<AnswerSummaryResponse>, ApiError> {
    let path = PathBuf::from(request.path.clone());

    let response =
        tokio::task::spawn_blocking(move || -> Result<AnswerSummaryResponse, ApiError> {
            let bytes = std::fs::read(&path).map_err(|err| {
                ApiError::bad_request(format!("read answer buffer failed: {err}"))
            })?;

            let summary = unsafe { summarize_from_answer_buffer(&bytes) };
            let packet_kind = unsafe { parse_answer_buffer(&bytes) }.map(packet_kind_name);

            Ok(AnswerSummaryResponse {
                path: path.display().to_string(),
                size_bytes: bytes.len(),
                packet_kind,
                summary,
            })
        })
        .await
        .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn blob_compare(
    Json(request): Json<BlobCompareRequest>,
) -> Result<Json<netzipapi_rust_demo::BlobCompareResult>, ApiError> {
    let left_path = PathBuf::from(request.left_path);
    let right_path = PathBuf::from(request.right_path);
    let left_offset = request.left_offset.unwrap_or(0);
    let right_offset = request.right_offset.unwrap_or(0);
    let compare_len = request.compare_len;
    let block_size = request.block_size.unwrap_or(8);

    let response = tokio::task::spawn_blocking(
        move || -> Result<netzipapi_rust_demo::BlobCompareResult, ApiError> {
            compare_blob_files(
                &left_path,
                &right_path,
                left_offset,
                right_offset,
                compare_len,
                block_size,
            )
            .map_err(|err| ApiError::bad_request(format!("blob compare failed: {err}")))
        },
    )
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn quote_replay(
    Json(request): Json<QuoteReplayRequest>,
) -> Result<Json<QuoteReplayResponse>, ApiError> {
    let path = PathBuf::from(request.path.clone());
    let host = request.host.unwrap_or_else(|| "110.41.14.158".to_string());
    let port = request.port.unwrap_or(7719);
    let recv_ms = request.recv_ms.unwrap_or(250);
    let connect_timeout_ms = request.connect_timeout_ms.unwrap_or(5_000);
    let save_path = request.save_path.clone().map(PathBuf::from);
    let segments = match request.segments.as_deref() {
        Some(spec) if !spec.trim().is_empty() => parse_quote_segments(spec)
            .map_err(|err| ApiError::bad_request(format!("parse segments failed: {err}")))?,
        _ => Vec::new(),
    };

    let response = tokio::task::spawn_blocking(move || -> Result<QuoteReplayResponse, ApiError> {
        let result = replay_quote_file(&QuoteReplayConfig {
            input: path,
            host,
            port,
            segments,
            recv_ms,
            connect_timeout_ms,
            save_path,
        })
        .map_err(|err| ApiError::bad_request(format!("quote replay failed: {err}")))?;

        Ok(QuoteReplayResponse {
            input: result.input,
            target: result.target,
            payload_size: result.payload_size,
            segment_count: result.segment_count,
            sent_bytes: result.sent_bytes,
            reply_bytes: result.reply_bytes,
            reply_head_hex: result.reply_head_hex,
            saved_path: result.saved_path,
            transport_error: result.transport_error,
        })
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn proto_probe(
    Json(request): Json<ProtoProbeRequest>,
) -> Result<Json<ProtoProbeResponse>, ApiError> {
    let host = request.host.unwrap_or_else(|| "121.41.70.217".to_string());
    let port = request.port.unwrap_or(6100);
    let payload = request.payload.clone();
    let encoding = parse_probe_encoding(request.encoding.as_deref())?;
    let nul_terminate = request.nul_terminate.unwrap_or(false);
    let prefix_u32le = request.prefix_u32le.unwrap_or(false);
    let read_secs = request.read_secs.unwrap_or(3);

    let response = tokio::task::spawn_blocking(move || -> Result<ProtoProbeResponse, ApiError> {
        let result = probe_proto(&ProtoProbeConfig {
            host,
            port,
            payload,
            encoding,
            nul_terminate,
            prefix_u32le,
            read_secs,
        })
        .map_err(|err| ApiError::bad_request(format!("proto probe failed: {err}")))?;

        Ok(ProtoProbeResponse {
            target: result.target,
            payload_len: result.payload_len,
            request_bytes: result.request_bytes,
            request_hex: result.request_hex,
            reply_bytes: result.reply_bytes,
            reply_head_hex: result.reply_head_hex,
            parsed_kind: result.parsed_kind,
            parsed_summary: result.parsed_summary,
            transport_error: result.transport_error,
        })
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn pcap_summary(
    Json(request): Json<PcapSummaryRequest>,
) -> Result<Json<PcapSummaryResponse>, ApiError> {
    let path = PathBuf::from(request.path.clone());
    let segment_limit = request.segment_limit.unwrap_or(12).min(128);

    let response = tokio::task::spawn_blocking(move || -> Result<PcapSummaryResponse, ApiError> {
        let summary = summarize_pcap_file(&path, segment_limit)
            .map_err(|err| ApiError::bad_request(format!("pcap summary failed: {err}")))?;

        Ok(PcapSummaryResponse {
            path: path.display().to_string(),
            pcap_packets: summary.pcap_packets,
            unique_packets: summary.unique_packets,
            duplicate_packets: summary.duplicate_packets,
            truncated_unique_packets: summary.truncated_unique_packets,
            flows: summary.flows,
        })
    })
    .await
    .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn quote_frame_scan(
    Json(request): Json<QuoteFrameScanRequest>,
) -> Result<Json<QuoteFrameScanResponse>, ApiError> {
    let path = PathBuf::from(request.path.clone());

    let response =
        tokio::task::spawn_blocking(move || -> Result<QuoteFrameScanResponse, ApiError> {
            let scanned = scan_quote_frame_file(&path)
                .map_err(|err| ApiError::bad_request(format!("quote frame scan failed: {err}")))?;
            let summary = quote_frame_scan_summary(&scanned);

            Ok(QuoteFrameScanResponse {
                input: scanned.input,
                size: scanned.size,
                mode: scanned.mode,
                summary,
                server_frames: scanned.server_frames,
                client_frames: scanned.client_frames,
                head_hex: scanned.head_hex,
            })
        })
        .await
        .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn quote_0547_decode(
    Json(request): Json<Quote0547DecodeRequest>,
) -> Result<Json<Quote0547DecodeResponse>, ApiError> {
    let path = PathBuf::from(request.path.clone());
    let limit = request.limit.unwrap_or(20).max(1);

    let response =
        tokio::task::spawn_blocking(move || -> Result<Quote0547DecodeResponse, ApiError> {
            let bytes = std::fs::read(&path).map_err(|err| {
                ApiError::bad_request(format!("read quote 0547 body failed: {err}"))
            })?;
            let parsed = parse_tdx_0547_body(&bytes);
            let filtered_records = parsed.records.len();
            let source_path = path.display().to_string();
            let source_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| source_path.clone());
            let preview = parsed
                .records
                .iter()
                .take(limit)
                .map(|record| {
                    quote_0547_record_preview(
                        record,
                        None,
                        None,
                        Some(&source_name),
                        Some(&source_path),
                    )
                })
                .collect::<Vec<_>>();
            let count_matches_records = parsed
                .xor93_count
                .map(|count| count as usize == filtered_records)
                .unwrap_or(false);

            Ok(Quote0547DecodeResponse {
                path: path.display().to_string(),
                size: bytes.len(),
                decimal_point_used: None,
                auto_decimal_lookup: false,
                decimal_point_lookup_error: None,
                xor93_count: parsed.xor93_count,
                printable_ratio: parsed.printable_ratio,
                filtered_records,
                count_matches_records,
                preview,
                first_record: parsed.records.first().map(|record| {
                    quote_0547_record_preview(
                        record,
                        None,
                        None,
                        Some(&source_name),
                        Some(&source_path),
                    )
                }),
                last_record: parsed.records.last().map(|record| {
                    quote_0547_record_preview(
                        record,
                        None,
                        None,
                        Some(&source_name),
                        Some(&source_path),
                    )
                }),
            })
        })
        .await
        .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn quote_0547_query(
    Json(request): Json<Quote0547QueryRequest>,
) -> Result<Json<Quote0547QueryResponse>, ApiError> {
    let path = PathBuf::from(request.path.clone());
    let query = request.query.clone();
    let decimal_point_filter = request.decimal_point_filter;
    let prefix3_filter = request
        .prefix3
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let pattern_bucket_filter = request
        .pattern_bucket
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let pattern_subbucket_filter = request
        .pattern_subbucket
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let state_matrix_filter = request
        .state_matrix
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let quote_head_state_filter = request
        .quote_head_state
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let time_presence_filter = request
        .time_presence
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let name_keyword_tag_filter = request
        .name_keyword_tag
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let limit = request.limit.unwrap_or(20).clamp(1, 500);
    let decimal_point_used = request
        .decimal_point
        .map(|decimal_point| {
            if (2..=6).contains(&decimal_point) {
                Ok(decimal_point)
            } else {
                Err(ApiError::bad_request(format!(
                    "decimal_point must be between 2 and 6, got {decimal_point}"
                )))
            }
        })
        .transpose()?;

    let response =
        tokio::task::spawn_blocking(move || -> Result<Quote0547QueryResponse, ApiError> {
            let (source_mode, sources) = load_quote_0547_sources(&path)?;
            let total_size = sources.iter().map(|source| source.size).sum::<usize>();
            let filtered_records = sources
                .iter()
                .map(|source| source.parsed.records.len())
                .sum::<usize>();
            let matches = sources
                .iter()
                .flat_map(|source| {
                    query_tdx_0547_records(&source.parsed, &query)
                        .into_iter()
                        .map(|record| Quote0547ScopedRecord {
                            source_path: &source.source_path,
                            source_name: &source.source_name,
                            record,
                        })
                })
                .collect::<Vec<_>>();
            let matched_records = matches.len();
            let auto_decimal_lookup = decimal_point_used.is_none();
            let mut decimal_point_lookup_error = None;
            let code_table_lookup = if matched_records > 0 {
                match sync_code_table(&Tdx7709Config::default()) {
                    Ok(result) => build_quote_0547_code_table_lookup(&result.records),
                    Err(err) => {
                        decimal_point_lookup_error =
                            Some(format!("7709 code-table lookup failed: {err}"));
                        BTreeMap::new()
                    }
                }
            } else {
                BTreeMap::new()
            };
            let filtered_matches = matches
                .into_iter()
                .filter(|scoped| {
                    let record = scoped.record;
                    let bucket_ok = pattern_bucket_filter
                        .as_deref()
                        .map(|filter| {
                            quote_0547_pattern_bucket(record, &code_table_lookup) == filter
                        })
                        .unwrap_or(true);
                    let prefix3_ok = prefix3_filter
                        .as_deref()
                        .map(|filter| {
                            normalize_code_prefix(&record.code, 3).as_deref() == Some(filter)
                        })
                        .unwrap_or(true);
                    let decimal_point_ok = decimal_point_filter
                        .map(|filter| {
                            lookup_quote_0547_code_table_info(record, &code_table_lookup)
                                .map(|info| info.decimal_point == filter)
                                .unwrap_or(false)
                        })
                        .unwrap_or(true);
                    let subbucket_ok = pattern_subbucket_filter
                        .as_deref()
                        .map(|filter| {
                            quote_0547_pattern_subbucket(record, &code_table_lookup) == filter
                        })
                        .unwrap_or(true);
                    let state_matrix_ok = state_matrix_filter
                        .as_deref()
                        .map(|filter| quote_0547_state_matrix_label(record) == filter)
                        .unwrap_or(true);
                    let quote_head_ok = quote_head_state_filter
                        .as_deref()
                        .map(|filter| match filter {
                            "with" | "present" | "has" | "quote_head" => {
                                record.quote_head.is_some()
                            }
                            "without" | "absent" | "none" | "no_quote_head" => {
                                record.quote_head.is_none()
                            }
                            _ => true,
                        })
                        .unwrap_or(true);
                    let time_presence_ok = time_presence_filter
                        .as_deref()
                        .map(|filter| match filter {
                            "with" | "present" | "has" => record.time_hhmmss_raw.is_some(),
                            "without" | "absent" | "none" => record.time_hhmmss_raw.is_none(),
                            _ => true,
                        })
                        .unwrap_or(true);
                    let keyword_ok = name_keyword_tag_filter
                        .as_deref()
                        .map(|filter| {
                            lookup_quote_0547_code_table_info(record, &code_table_lookup)
                                .and_then(|info| {
                                    let tags = infer_name_keyword_tags(&info.name);
                                    tags.into_iter().find(|tag| tag == filter)
                                })
                                .is_some()
                        })
                        .unwrap_or(true);
                    prefix3_ok
                        && decimal_point_ok
                        && bucket_ok
                        && subbucket_ok
                        && state_matrix_ok
                        && quote_head_ok
                        && time_presence_ok
                        && keyword_ok
                })
                .collect::<Vec<_>>();
            let matched_records = filtered_matches.len();
            let filtered_match_records = filtered_matches
                .iter()
                .map(|scoped| scoped.record)
                .collect::<Vec<_>>();
            let preview = filtered_matches
                .iter()
                .take(limit)
                .map(|scoped| {
                    quote_0547_record_preview(
                        scoped.record,
                        resolve_quote_0547_decimal_point(
                            scoped.record,
                            decimal_point_used,
                            &code_table_lookup,
                        ),
                        lookup_quote_0547_code_table_info(scoped.record, &code_table_lookup),
                        Some(scoped.source_name),
                        Some(scoped.source_path),
                    )
                })
                .collect::<Vec<_>>();
            let xor93_count = if sources.len() == 1 {
                sources.first().and_then(|source| source.parsed.xor93_count)
            } else {
                None
            };
            let count_matches_records = if sources.len() == 1 {
                sources
                    .first()
                    .and_then(|source| {
                        source
                            .parsed
                            .xor93_count
                            .map(|count| count as usize == source.parsed.records.len())
                    })
                    .unwrap_or(false)
            } else {
                false
            };

            Ok(Quote0547QueryResponse {
                path: path.display().to_string(),
                source_mode,
                source_files_total: sources.len(),
                size: total_size,
                query,
                decimal_point_filter,
                prefix3_filter,
                pattern_bucket_filter,
                pattern_subbucket_filter,
                state_matrix_filter,
                quote_head_state_filter,
                time_presence_filter,
                name_keyword_tag_filter,
                decimal_point_used,
                auto_decimal_lookup,
                decimal_point_lookup_error,
                xor93_count,
                filtered_records,
                matched_records,
                count_matches_records,
                matched_prefix3_top: top_label_groups(
                    &filtered_match_records,
                    &code_table_lookup,
                    |record, _| normalize_code_prefix(&record.code, 3),
                    8,
                ),
                matched_decimal_point_top: top_label_groups(
                    &filtered_match_records,
                    &code_table_lookup,
                    |record, lookup| {
                        lookup
                            .get(&record.code)
                            .map(|info| info.decimal_point.to_string())
                    },
                    8,
                ),
                matched_pattern_bucket_top: top_label_groups(
                    &filtered_match_records,
                    &code_table_lookup,
                    |record, lookup| Some(quote_0547_pattern_bucket(record, lookup).to_string()),
                    8,
                ),
                matched_pattern_subbucket_top: top_label_groups(
                    &filtered_match_records,
                    &code_table_lookup,
                    |record, lookup| Some(quote_0547_pattern_subbucket(record, lookup).to_string()),
                    8,
                ),
                matched_quote_head_state_top: top_label_groups(
                    &filtered_match_records,
                    &code_table_lookup,
                    |record, _| Some(quote_0547_quote_head_state_label(record).to_string()),
                    4,
                ),
                matched_time_presence_top: top_label_groups(
                    &filtered_match_records,
                    &code_table_lookup,
                    |record, _| Some(quote_0547_time_presence_label(record).to_string()),
                    4,
                ),
                matched_state_matrix_top: top_label_groups(
                    &filtered_match_records,
                    &code_table_lookup,
                    |record, _| Some(quote_0547_state_matrix_label(record).to_string()),
                    4,
                ),
                matched_name_keyword_top: top_label_groups(
                    &filtered_match_records,
                    &code_table_lookup,
                    |record, lookup| {
                        lookup
                            .get(&record.code)
                            .and_then(|info| infer_name_keyword_tag(&info.name))
                    },
                    8,
                ),
                matched_source_top: top_scoped_source_groups(&filtered_matches, 8),
                preview,
                first_match: filtered_matches.first().map(|scoped| {
                    quote_0547_record_preview(
                        scoped.record,
                        resolve_quote_0547_decimal_point(
                            scoped.record,
                            decimal_point_used,
                            &code_table_lookup,
                        ),
                        lookup_quote_0547_code_table_info(scoped.record, &code_table_lookup),
                        Some(scoped.source_name),
                        Some(scoped.source_path),
                    )
                }),
                last_match: filtered_matches.last().map(|scoped| {
                    quote_0547_record_preview(
                        scoped.record,
                        resolve_quote_0547_decimal_point(
                            scoped.record,
                            decimal_point_used,
                            &code_table_lookup,
                        ),
                        lookup_quote_0547_code_table_info(scoped.record, &code_table_lookup),
                        Some(scoped.source_name),
                        Some(scoped.source_path),
                    )
                }),
            })
        })
        .await
        .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn quote_0547_extra_profile(
    Json(request): Json<Quote0547ExtraProfileRequest>,
) -> Result<Json<Quote0547ExtraProfileResponse>, ApiError> {
    let path = PathBuf::from(request.path.clone());
    let anomaly_limit = request.anomaly_limit.unwrap_or(12).clamp(1, 100);

    let response =
        tokio::task::spawn_blocking(move || -> Result<Quote0547ExtraProfileResponse, ApiError> {
            let bytes = std::fs::read(&path).map_err(|err| {
                ApiError::bad_request(format!("read quote 0547 body failed: {err}"))
            })?;
            let parsed = parse_tdx_0547_body(&bytes);
            let filtered_records = parsed.records.len();

            let mut code_table_lookup_error = None;
            let code_table_lookup = if filtered_records > 0 {
                match sync_code_table(&Tdx7709Config::default()) {
                    Ok(result) => build_quote_0547_code_table_lookup(&result.records),
                    Err(err) => {
                        code_table_lookup_error =
                            Some(format!("7709 code-table lookup failed: {err}"));
                        BTreeMap::new()
                    }
                }
            } else {
                BTreeMap::new()
            };

            let complete_records = parsed
                .records
                .iter()
                .filter(|record| {
                    record.extra1_raw.is_some()
                        && record.extra2_raw.is_some()
                        && record.extra3_raw.is_some()
                })
                .collect::<Vec<_>>();
            let records_with_complete_extra_tuple = complete_records.len();
            let default_pattern_count = complete_records
                .iter()
                .filter(|record| is_default_extra_pattern(record))
                .count();
            let anomaly_records = complete_records
                .iter()
                .copied()
                .filter(|record| !is_default_extra_pattern(record))
                .collect::<Vec<_>>();
            let anomaly_count = anomaly_records.len();
            let default_records = complete_records
                .iter()
                .copied()
                .filter(|record| is_default_extra_pattern(record))
                .collect::<Vec<_>>();
            let default_quote_head_records = default_records
                .iter()
                .copied()
                .filter(|record| record.quote_head.is_some())
                .collect::<Vec<_>>();
            let default_quote_head_time_present_records = default_quote_head_records
                .iter()
                .copied()
                .filter(|record| record.time_hhmmss_raw.is_some())
                .collect::<Vec<_>>();
            let default_quote_head_time_absent_records = default_quote_head_records
                .iter()
                .copied()
                .filter(|record| record.time_hhmmss_raw.is_none())
                .collect::<Vec<_>>();
            let default_no_quote_head_records = default_records
                .iter()
                .copied()
                .filter(|record| record.quote_head.is_none())
                .collect::<Vec<_>>();
            let anomaly_extra3_positive_count = anomaly_records
                .iter()
                .filter(|record| record.extra3_raw.is_some_and(|value| value > 0))
                .count();
            let anomaly_special_zero_bucket = anomaly_records
                .iter()
                .copied()
                .filter(|record| is_special_zero_bucket(record, &code_table_lookup))
                .collect::<Vec<_>>();
            let anomaly_special_zero_bucket_count = anomaly_special_zero_bucket.len();
            let source_label = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| path.display().to_string());
            let source_path = path.display().to_string();

            Ok(Quote0547ExtraProfileResponse {
                path: path.display().to_string(),
                source_mode: "single_path".to_string(),
                source_files_total: 1,
                size: bytes.len(),
                xor93_count: parsed.xor93_count,
                filtered_records,
                records_with_complete_extra_tuple,
                default_pattern_count,
                default_quote_head_count: default_quote_head_records.len(),
                default_quote_head_time_present_count: default_quote_head_time_present_records
                    .len(),
                default_quote_head_time_absent_count: default_quote_head_time_absent_records.len(),
                anomaly_count,
                anomaly_extra3_positive_count,
                anomaly_special_zero_bucket_count,
                default_no_quote_head_count: default_no_quote_head_records.len(),
                code_table_lookup_error,
                extra0_top: top_i32_values(
                    parsed.records.iter().filter_map(|record| record.extra0_raw),
                    12,
                ),
                extra1_top: top_i32_values(
                    parsed.records.iter().filter_map(|record| record.extra1_raw),
                    12,
                ),
                extra2_top: top_i32_values(
                    parsed.records.iter().filter_map(|record| record.extra2_raw),
                    12,
                ),
                extra3_top: top_i32_values(
                    parsed.records.iter().filter_map(|record| record.extra3_raw),
                    12,
                ),
                pattern_top: top_extra_patterns(&complete_records, 12),
                default_pattern_subbucket_top: top_label_groups(
                    &default_records,
                    &code_table_lookup,
                    |record, lookup| Some(quote_0547_pattern_subbucket(record, lookup).to_string()),
                    12,
                ),
                default_pattern_subbucket_correlations: top_label_correlations(
                    &default_records,
                    &code_table_lookup,
                    |record, lookup| Some(quote_0547_pattern_subbucket(record, lookup).to_string()),
                    8,
                ),
                default_state_matrix_top: top_label_groups(
                    &default_records,
                    &code_table_lookup,
                    |record, _| Some(quote_0547_state_matrix_label(record).to_string()),
                    8,
                ),
                default_quote_head_subbucket_top: top_label_groups(
                    &default_quote_head_records,
                    &code_table_lookup,
                    |record, lookup| Some(quote_0547_pattern_subbucket(record, lookup).to_string()),
                    8,
                ),
                default_quote_head_name_keyword_top: top_label_groups(
                    &default_quote_head_records,
                    &code_table_lookup,
                    |record, lookup| {
                        lookup
                            .get(&record.code)
                            .and_then(|info| infer_name_keyword_tag(&info.name))
                    },
                    8,
                ),
                default_quote_head_time_present_examples: default_quote_head_time_present_records
                    .iter()
                    .take(anomaly_limit)
                    .map(|record| {
                        quote_0547_record_preview(
                            record,
                            resolve_quote_0547_decimal_point(record, None, &code_table_lookup),
                            lookup_quote_0547_code_table_info(record, &code_table_lookup),
                            Some(&source_label),
                            Some(&source_path),
                        )
                    })
                    .collect(),
                default_quote_head_time_absent_examples: default_quote_head_time_absent_records
                    .iter()
                    .take(anomaly_limit)
                    .map(|record| {
                        quote_0547_record_preview(
                            record,
                            resolve_quote_0547_decimal_point(record, None, &code_table_lookup),
                            lookup_quote_0547_code_table_info(record, &code_table_lookup),
                            Some(&source_label),
                            Some(&source_path),
                        )
                    })
                    .collect(),
                default_no_quote_head_subbucket_top: top_label_groups(
                    &default_no_quote_head_records,
                    &code_table_lookup,
                    |record, lookup| Some(quote_0547_pattern_subbucket(record, lookup).to_string()),
                    8,
                ),
                default_no_quote_head_name_keyword_top: top_label_groups(
                    &default_no_quote_head_records,
                    &code_table_lookup,
                    |record, lookup| {
                        lookup
                            .get(&record.code)
                            .and_then(|info| infer_name_keyword_tag(&info.name))
                    },
                    8,
                ),
                default_no_quote_head_examples: default_no_quote_head_records
                    .iter()
                    .take(anomaly_limit)
                    .map(|record| {
                        quote_0547_record_preview(
                            record,
                            resolve_quote_0547_decimal_point(record, None, &code_table_lookup),
                            lookup_quote_0547_code_table_info(record, &code_table_lookup),
                            Some(&source_label),
                            Some(&source_path),
                        )
                    })
                    .collect(),
                anomaly_prefix3_top: top_label_groups(
                    &anomaly_records,
                    &code_table_lookup,
                    |record, _| normalize_code_prefix(&record.code, 3),
                    12,
                ),
                anomaly_market_top: top_label_groups(
                    &anomaly_records,
                    &code_table_lookup,
                    |record, _| {
                        Some(
                            tdx_0547_market_name(record.market)
                                .unwrap_or("?")
                                .to_string(),
                        )
                    },
                    12,
                ),
                anomaly_decimal_point_top: top_label_groups(
                    &anomaly_records,
                    &code_table_lookup,
                    |record, lookup| {
                        lookup
                            .get(&record.code)
                            .map(|info| info.decimal_point.to_string())
                    },
                    12,
                ),
                anomaly_pattern_bucket_top: top_label_groups(
                    &anomaly_records,
                    &code_table_lookup,
                    |record, lookup| Some(quote_0547_pattern_bucket(record, lookup).to_string()),
                    12,
                ),
                anomaly_pattern_subbucket_top: top_label_groups(
                    &anomaly_records,
                    &code_table_lookup,
                    |record, lookup| Some(quote_0547_pattern_subbucket(record, lookup).to_string()),
                    12,
                ),
                anomaly_name_keyword_top: top_label_groups(
                    &anomaly_records,
                    &code_table_lookup,
                    |record, lookup| {
                        lookup
                            .get(&record.code)
                            .and_then(|info| infer_name_keyword_tag(&info.name))
                    },
                    12,
                ),
                anomaly_source_top: vec![Quote0547LabelSummary {
                    label: source_label.clone(),
                    count: anomaly_count,
                    example_codes: anomaly_records
                        .iter()
                        .take(3)
                        .map(|record| record.code.clone())
                        .collect(),
                }],
                anomaly_time_hhmmss_top: top_label_groups(
                    &anomaly_records,
                    &code_table_lookup,
                    |record, _| record.time_hhmmss_raw.and_then(tdx_0547_format_hhmmss_raw),
                    12,
                ),
                anomaly_extra0_time_hint_top: top_label_groups(
                    &anomaly_records,
                    &code_table_lookup,
                    |record, _| record.extra0_time_hhmmss.clone(),
                    12,
                ),
                source_top: vec![Quote0547LabelSummary {
                    label: source_label.clone(),
                    count: filtered_records,
                    example_codes: parsed
                        .records
                        .iter()
                        .take(3)
                        .map(|record| record.code.clone())
                        .collect(),
                }],
                anomaly_pattern_correlations: top_pattern_correlations(
                    &anomaly_records,
                    &code_table_lookup,
                    8,
                ),
                anomaly_special_zero_bucket_examples: anomaly_special_zero_bucket
                    .iter()
                    .take(4)
                    .map(|record| {
                        quote_0547_record_preview(
                            record,
                            resolve_quote_0547_decimal_point(record, None, &code_table_lookup),
                            lookup_quote_0547_code_table_info(record, &code_table_lookup),
                            Some(&source_label),
                            Some(&source_path),
                        )
                    })
                    .collect(),
                anomaly_examples: anomaly_records
                    .into_iter()
                    .take(anomaly_limit)
                    .map(|record| {
                        quote_0547_record_preview(
                            record,
                            resolve_quote_0547_decimal_point(record, None, &code_table_lookup),
                            lookup_quote_0547_code_table_info(record, &code_table_lookup),
                            Some(&source_label),
                            Some(&source_path),
                        )
                    })
                    .collect(),
            })
        })
        .await
        .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

async fn stream_analyze(
    Json(request): Json<StreamAnalyzeRequest>,
) -> Result<Json<StreamAnalyzeResponse>, ApiError> {
    let path = PathBuf::from(request.path.clone());
    let is_hex = request.is_hex.unwrap_or(false);

    let response =
        tokio::task::spawn_blocking(move || -> Result<StreamAnalyzeResponse, ApiError> {
            let analysis = analyze_stream_file(&path, is_hex)
                .map_err(|err| ApiError::bad_request(format!("stream analyze failed: {err}")))?;

            Ok(StreamAnalyzeResponse {
                input: analysis.input,
                is_hex: analysis.is_hex,
                size: analysis.size,
                byte_profile: analysis.byte_profile,
                utf8_preview: analysis.utf8_preview,
                utf16le_preview: analysis.utf16le_preview,
                http_headers: analysis.http_headers,
                netpacket_runs: analysis.netpacket_runs,
                packet_summaries: analysis.packet_summaries,
                zlib_hits: analysis.zlib_hits,
                oem_at_0: analysis.oem_at_0,
                oem_at_4: analysis.oem_at_4,
                candidate_packets: analysis.candidate_packets,
            })
        })
        .await
        .map_err(|err| ApiError::internal(format!("join error: {err}")))??;

    Ok(Json(response))
}

fn fin_record_preview(record: &netzipapi_rust_demo::TdxFinRecord) -> FinRecordPreview {
    FinRecordPreview {
        symbol: record.symbol.clone(),
        market: record.market().map(str::to_string),
        code: record.code().map(str::to_string),
        time: record.time,
        bao_gao: record.bao_gao,
        quarter: record.quarter(),
        mg_shou_yi: record.mg_shou_yi,
        mg_jing_zhi: record.mg_jing_zhi,
        zong_gu: record.zong_gu,
        liu_tong_ag: record.liu_tong_ag,
        jing_li_run: record.jing_li_run,
        trailer_hex: record
            .trailer
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn fin_record_query_data(record: &netzipapi_rust_demo::TdxFinRecord) -> FinRecordQueryData {
    FinRecordQueryData {
        symbol: record.symbol.clone(),
        market: record.market().map(str::to_string),
        code: record.code().map(str::to_string),
        time: record.time,
        bao_gao: record.bao_gao,
        quarter: record.quarter(),
        mg_shou_yi: record.mg_shou_yi,
        mg_jing_zhi: record.mg_jing_zhi,
        zong_gu: record.zong_gu,
        liu_tong_ag: record.liu_tong_ag,
        shou_ru: record.shou_ru,
        zong_zc: record.zong_zc,
        zong_fu_zhai: record.zong_fu_zhai,
        quan_yi: record.quan_yi,
        jing_li_run: record.jing_li_run,
        wei_fen_pei: record.wei_fen_pei,
    }
}

fn find_fin_record<'a>(
    records: &'a [netzipapi_rust_demo::TdxFinRecord],
    symbol: &str,
) -> Option<&'a netzipapi_rust_demo::TdxFinRecord> {
    records
        .iter()
        .find(|record| record.symbol.eq_ignore_ascii_case(symbol))
        .or_else(|| {
            records.iter().find(|record| {
                record
                    .code()
                    .map(|code| code.eq_ignore_ascii_case(symbol))
                    .unwrap_or(false)
            })
        })
}

fn load_quote_0547_sources(
    path: &std::path::Path,
) -> Result<(String, Vec<Quote0547LoadedSource>), ApiError> {
    let (source_mode, paths) = collect_quote_0547_input_paths(path)?;
    let mut sources = Vec::with_capacity(paths.len());
    for source_path in paths {
        let bytes = std::fs::read(&source_path)
            .map_err(|err| ApiError::bad_request(format!("read quote 0547 body failed: {err}")))?;
        let source_name = source_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| source_path.display().to_string());
        sources.push(Quote0547LoadedSource {
            source_path: source_path.display().to_string(),
            source_name,
            size: bytes.len(),
            parsed: parse_tdx_0547_body(&bytes),
        });
    }
    Ok((source_mode, sources))
}

fn collect_quote_0547_input_paths(
    path: &std::path::Path,
) -> Result<(String, Vec<PathBuf>), ApiError> {
    if path.is_file() {
        return Ok(("single_path".to_string(), vec![path.to_path_buf()]));
    }
    if !path.is_dir() {
        return Err(ApiError::bad_request(format!(
            "quote 0547 path is neither a file nor a directory: {}",
            path.display()
        )));
    }

    let mut preferred = Vec::new();
    let mut fallback = Vec::new();
    for entry in std::fs::read_dir(path)
        .map_err(|err| ApiError::bad_request(format!("read quote 0547 directory failed: {err}")))?
    {
        let entry = entry.map_err(|err| {
            ApiError::bad_request(format!("read quote 0547 directory entry failed: {err}"))
        })?;
        let entry_path = entry.path();
        if !entry_path.is_file() {
            continue;
        }
        let Some(name) = entry_path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.ends_with("_inflated.bin") {
            preferred.push(entry_path);
        } else if name.ends_with(".bin") {
            fallback.push(entry_path);
        }
    }

    preferred.sort();
    fallback.sort();
    let paths = if preferred.is_empty() {
        fallback
    } else {
        preferred
    };
    if paths.is_empty() {
        return Err(ApiError::bad_request(format!(
            "quote 0547 directory contains no .bin files: {}",
            path.display()
        )));
    }
    Ok(("directory".to_string(), paths))
}

fn quote_0547_record_preview(
    record: &netzipapi_rust_demo::Tdx0547Record,
    decimal_point_used: Option<u8>,
    code_table_info: Option<&Quote0547CodeTableInfo>,
    source_name: Option<&str>,
    source_path: Option<&str>,
) -> Quote0547RecordPreview {
    let code_table_name_keyword_tags = code_table_info
        .map(|info| infer_name_keyword_tags(&info.name))
        .unwrap_or_default();
    let (time_fields_match, time_fields_delta_seconds) = quote_0547_time_alignment(record);
    let code_table_lookup = code_table_info
        .map(|info| {
            let mut lookup = BTreeMap::new();
            lookup.insert(record.code.clone(), info.clone());
            lookup
        })
        .unwrap_or_default();
    let normalized_quote_head = record.quote_head.as_ref().and_then(|quote_head| {
        decimal_point_used
            .and_then(|decimal_point| tdx_0547_normalize_quote_head(quote_head, decimal_point))
            .map(|normalized| quote_0547_head_preview(&normalized))
    });
    let normalized_change_value = normalized_quote_head
        .as_ref()
        .map(|quote_head| round_decimal(quote_head.price - quote_head.last_close, 6));
    let normalized_change_percent = normalized_quote_head.as_ref().and_then(|quote_head| {
        if quote_head.last_close.abs() <= f64::EPSILON {
            None
        } else {
            Some(round_decimal(
                (quote_head.price - quote_head.last_close) / quote_head.last_close * 100.0,
                6,
            ))
        }
    });
    let normalized_amplitude_percent = normalized_quote_head.as_ref().and_then(|quote_head| {
        if quote_head.last_close.abs() <= f64::EPSILON {
            None
        } else {
            Some(round_decimal(
                (quote_head.high - quote_head.low) / quote_head.last_close * 100.0,
                6,
            ))
        }
    });
    let normalized_open_gap_value = normalized_quote_head
        .as_ref()
        .map(|quote_head| round_decimal(quote_head.open - quote_head.last_close, 6));
    let normalized_open_gap_percent = normalized_quote_head.as_ref().and_then(|quote_head| {
        if quote_head.last_close.abs() <= f64::EPSILON {
            None
        } else {
            Some(round_decimal(
                (quote_head.open - quote_head.last_close) / quote_head.last_close * 100.0,
                6,
            ))
        }
    });
    let normalized_intraday_range_value = normalized_quote_head
        .as_ref()
        .map(|quote_head| round_decimal(quote_head.high - quote_head.low, 6));
    let normalized_return_from_open_percent =
        normalized_quote_head.as_ref().and_then(|quote_head| {
            if quote_head.open.abs() <= f64::EPSILON {
                None
            } else {
                Some(round_decimal(
                    (quote_head.price - quote_head.open) / quote_head.open * 100.0,
                    6,
                ))
            }
        });
    let normalized_drawdown_from_high_percent =
        normalized_quote_head.as_ref().and_then(|quote_head| {
            if quote_head.high.abs() <= f64::EPSILON {
                None
            } else {
                Some(round_decimal(
                    (quote_head.price - quote_head.high) / quote_head.high * 100.0,
                    6,
                ))
            }
        });
    let code_table_pre_close_delta = normalized_quote_head.as_ref().and_then(|quote_head| {
        code_table_info
            .map(|info| round_decimal(quote_head.last_close - f64::from(info.pre_close), 6))
    });
    let code_table_pre_close_matches =
        code_table_pre_close_delta.map(|delta| delta.abs() <= 0.000_001);
    let pattern_bucket = quote_0547_pattern_bucket(record, &code_table_lookup).to_string();
    let pattern_subbucket = quote_0547_pattern_subbucket(record, &code_table_lookup).to_string();
    Quote0547RecordPreview {
        source_name: source_name.map(str::to_string),
        source_path: source_path.map(str::to_string),
        market: record.market,
        market_name: tdx_0547_market_name(record.market).map(str::to_string),
        code: record.code.clone(),
        start: record.start,
        len: record.len,
        decimal_point_used,
        code_table_name: code_table_info.map(|info| info.name.clone()),
        code_table_name_keyword_tag: code_table_name_keyword_tags.first().cloned(),
        code_table_name_keyword_tags,
        code_table_pre_close: code_table_info.map(|info| info.pre_close),
        code_table_pre_close_delta,
        code_table_pre_close_matches,
        active1_raw: record.active1_raw,
        time_hhmmss_raw: record.time_hhmmss_raw,
        time_hhmmss: record.time_hhmmss_raw.and_then(tdx_0547_format_hhmmss_raw),
        time_fields_match,
        time_fields_delta_seconds,
        extra0_raw: record.extra0_raw,
        extra0_time_hhmmss: record.extra0_time_hhmmss.clone(),
        extra1_raw: record.extra1_raw,
        extra2_raw: record.extra2_raw,
        extra3_raw: record.extra3_raw,
        volume: record.volume,
        current_volume: record.current_volume,
        amount: record.amount,
        amount_raw: record.amount_raw,
        extra3_positive: record.extra3_raw.map(|value| value > 0),
        special_zero_bucket: is_special_zero_bucket(record, &code_table_lookup),
        pattern_bucket,
        pattern_subbucket,
        quote_head: record.quote_head.as_ref().map(quote_0547_head_preview),
        normalized_quote_head,
        normalized_change_value,
        normalized_change_percent,
        normalized_amplitude_percent,
        normalized_open_gap_value,
        normalized_open_gap_percent,
        normalized_intraday_range_value,
        normalized_return_from_open_percent,
        normalized_drawdown_from_high_percent,
    }
}

fn build_quote_0547_code_table_lookup(
    records: &[netzipapi_rust_demo::Tdx7709CodeTableRecord],
) -> BTreeMap<String, Quote0547CodeTableInfo> {
    let mut lookup = BTreeMap::new();
    let mut code_counts = BTreeMap::<String, usize>::new();
    for record in records {
        *code_counts
            .entry(normalize_numeric_code_key(&record.code))
            .or_default() += 1;
    }
    for record in records {
        let code_key = normalize_numeric_code_key(&record.code);
        if !code_key.is_empty() {
            let info = Quote0547CodeTableInfo {
                name: record.name.clone(),
                decimal_point: record.decimal_point(),
                pre_close: record.pre_close(),
            };
            if let Some(market) = tdx_0547_market_name(record.market) {
                lookup.insert(format!("{market}{code_key}"), info.clone());
            }
            if code_counts.get(&code_key) == Some(&1) {
                lookup.insert(code_key, info);
            }
        }
    }
    lookup
}

fn normalize_numeric_code_key(code: &str) -> String {
    code.chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>()
}

fn resolve_quote_0547_decimal_point(
    record: &netzipapi_rust_demo::Tdx0547Record,
    manual_decimal_point: Option<u8>,
    code_table_lookup: &BTreeMap<String, Quote0547CodeTableInfo>,
) -> Option<u8> {
    manual_decimal_point
        .or_else(|| {
            lookup_quote_0547_code_table_info(record, code_table_lookup)
                .map(|info| info.decimal_point)
        })
        .or_else(|| quote_decimal_point_fallback(record.market, &record.code))
}

fn quote_decimal_point_fallback(market: u8, code: &str) -> Option<u8> {
    match market {
        0 if ["00", "20", "30"]
            .iter()
            .any(|prefix| code.starts_with(prefix)) =>
        {
            Some(2)
        }
        1 if ["60", "68", "90"]
            .iter()
            .any(|prefix| code.starts_with(prefix)) =>
        {
            Some(2)
        }
        2 => Some(2),
        _ => None,
    }
}

fn lookup_quote_0547_code_table_info<'a>(
    record: &netzipapi_rust_demo::Tdx0547Record,
    code_table_lookup: &'a BTreeMap<String, Quote0547CodeTableInfo>,
) -> Option<&'a Quote0547CodeTableInfo> {
    tdx_0547_record_symbol(record)
        .as_ref()
        .and_then(|symbol| code_table_lookup.get(symbol))
        .or_else(|| code_table_lookup.get(&record.code))
}

fn quote_0547_head_preview(
    quote_head: &netzipapi_rust_demo::Tdx0547QuoteHead,
) -> Quote0547HeadPreview {
    Quote0547HeadPreview {
        active1: quote_head.active1,
        price: quote_head.price,
        last_close: quote_head.last_close,
        open: quote_head.open,
        high: quote_head.high,
        low: quote_head.low,
    }
}

fn quote_0547_time_alignment(
    record: &netzipapi_rust_demo::Tdx0547Record,
) -> (Option<bool>, Option<i32>) {
    let left = record
        .time_hhmmss_raw
        .and_then(tdx_0547_hhmmss_raw_to_seconds);
    let right = record
        .extra0_raw
        .and_then(tdx_0547_extra0_time_hint_seconds);
    match (left, right) {
        (Some(left), Some(right)) => {
            let delta = left as i32 - right as i32;
            (Some(delta == 0), Some(delta))
        }
        _ => (None, None),
    }
}

fn quote_0547_quote_head_state_label(record: &netzipapi_rust_demo::Tdx0547Record) -> &'static str {
    if record.quote_head.is_some() {
        "with_quote_head"
    } else {
        "without_quote_head"
    }
}

fn quote_0547_time_presence_label(record: &netzipapi_rust_demo::Tdx0547Record) -> &'static str {
    if record.time_hhmmss_raw.is_some() {
        "time_present"
    } else {
        "time_absent"
    }
}

fn quote_0547_state_matrix_label(record: &netzipapi_rust_demo::Tdx0547Record) -> &'static str {
    match (
        quote_0547_quote_head_state_label(record),
        quote_0547_time_presence_label(record),
    ) {
        ("with_quote_head", "time_present") => "with_quote_head_time_present",
        ("with_quote_head", "time_absent") => "with_quote_head_time_absent",
        ("without_quote_head", "time_present") => "without_quote_head_time_present",
        ("without_quote_head", "time_absent") => "without_quote_head_time_absent",
        _ => "unknown",
    }
}

fn round_decimal(value: f64, places: u32) -> f64 {
    let scale = 10_f64.powi(places as i32);
    (value * scale).round() / scale
}

fn is_default_extra_pattern(record: &netzipapi_rust_demo::Tdx0547Record) -> bool {
    matches!(
        (record.extra1_raw, record.extra2_raw, record.extra3_raw),
        (Some(2), Some(0), Some(0))
    )
}

fn is_special_zero_bucket(
    record: &netzipapi_rust_demo::Tdx0547Record,
    code_table_lookup: &BTreeMap<String, Quote0547CodeTableInfo>,
) -> bool {
    matches!(
        (record.extra1_raw, record.extra2_raw, record.extra3_raw),
        (Some(0), Some(0), Some(0))
    ) && record.extra0_raw == Some(1)
        && record.active1_raw == Some(0)
        && record.time_hhmmss_raw == Some(0)
        && record.quote_head.is_none()
        && lookup_quote_0547_code_table_info(record, code_table_lookup)
            .map(|info| info.pre_close.abs() <= f32::EPSILON)
            .unwrap_or(false)
}

fn quote_0547_pattern_bucket(
    record: &netzipapi_rust_demo::Tdx0547Record,
    code_table_lookup: &BTreeMap<String, Quote0547CodeTableInfo>,
) -> &'static str {
    if is_special_zero_bucket(record, code_table_lookup) {
        return "special_zero_bucket";
    }
    match (record.extra1_raw, record.extra2_raw, record.extra3_raw) {
        (Some(2), Some(0), Some(0)) => "default_2_0_0",
        (Some(_), Some(2), Some(0)) => "extra2_2_zero_extra3",
        (_, _, Some(extra3)) if extra3 > 0 => "extra3_positive",
        _ => "other_anomaly",
    }
}

fn quote_0547_pattern_subbucket(
    record: &netzipapi_rust_demo::Tdx0547Record,
    code_table_lookup: &BTreeMap<String, Quote0547CodeTableInfo>,
) -> &'static str {
    if is_special_zero_bucket(record, code_table_lookup) {
        return "special_zero_bucket";
    }

    match (record.extra1_raw, record.extra2_raw, record.extra3_raw) {
        (Some(extra1), Some(2), Some(0)) => match extra1 {
            -10 => "extra2_2_zero_extra3_neg10",
            -21 => "extra2_2_zero_extra3_neg21",
            _ => "extra2_2_zero_extra3_other",
        },
        (_, _, Some(extra3)) if extra3 > 0 => match record.extra0_time_hhmmss.as_deref() {
            Some("15:30:00") => "extra3_positive_hint_153000",
            Some("15:30:12") => "extra3_positive_hint_153012",
            Some(_) => "extra3_positive_hint_other",
            None => "extra3_positive_hint_none",
        },
        (Some(2), Some(0), Some(0)) => {
            let quote_head_part = if record.quote_head.is_some() {
                "quote_head"
            } else {
                "no_quote_head"
            };
            let hint_part = match record.extra0_time_hhmmss.as_deref() {
                Some(value) if value.starts_with("15:00:") => "hint_1500",
                Some(value) if value.starts_with("15:30:") => "hint_1530",
                Some(_) => "hint_other",
                None => "hint_none",
            };
            let time_part = if record.time_hhmmss_raw.is_some() {
                "time_present"
            } else {
                "time_absent"
            };
            match (quote_head_part, hint_part, time_part) {
                ("quote_head", "hint_1500", "time_present") => {
                    "default_2_0_0_quote_head_hint_1500_time_present"
                }
                ("quote_head", "hint_1500", "time_absent") => {
                    "default_2_0_0_quote_head_hint_1500_time_absent"
                }
                ("quote_head", "hint_1530", "time_present") => {
                    "default_2_0_0_quote_head_hint_1530_time_present"
                }
                ("quote_head", "hint_1530", "time_absent") => {
                    "default_2_0_0_quote_head_hint_1530_time_absent"
                }
                ("quote_head", "hint_other", _) => "default_2_0_0_quote_head_hint_other",
                ("quote_head", "hint_none", _) => "default_2_0_0_quote_head_hint_none",
                ("no_quote_head", "hint_1500", _) => "default_2_0_0_no_quote_head_hint_1500",
                ("no_quote_head", "hint_1530", _) => "default_2_0_0_no_quote_head_hint_1530",
                ("no_quote_head", "hint_other", _) => "default_2_0_0_no_quote_head_hint_other",
                ("no_quote_head", "hint_none", _) => "default_2_0_0_no_quote_head_hint_none",
                _ => "default_2_0_0",
            }
        }
        _ => "other_anomaly",
    }
}

fn top_i32_values(values: impl Iterator<Item = i32>, limit: usize) -> Vec<Quote0547ValueCount> {
    let mut counts = BTreeMap::<i32, usize>::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    let mut entries = counts
        .into_iter()
        .map(|(value, count)| Quote0547ValueCount { value, count })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.value.cmp(&right.value))
    });
    entries.truncate(limit);
    entries
}

fn top_extra_patterns(
    records: &[&netzipapi_rust_demo::Tdx0547Record],
    limit: usize,
) -> Vec<Quote0547ExtraPatternSummary> {
    let mut counts = BTreeMap::<(i32, i32, i32), (usize, Vec<String>)>::new();
    for record in records {
        let key = match (record.extra1_raw, record.extra2_raw, record.extra3_raw) {
            (Some(extra1), Some(extra2), Some(extra3)) => (extra1, extra2, extra3),
            _ => continue,
        };
        let entry = counts.entry(key).or_insert_with(|| (0, Vec::new()));
        entry.0 += 1;
        if entry.1.len() < 5 {
            let symbol = tdx_0547_market_name(record.market)
                .map(|prefix| format!("{prefix}{}", record.code))
                .unwrap_or_else(|| record.code.clone());
            if !entry.1.iter().any(|value| value == &symbol) {
                entry.1.push(symbol);
            }
        }
    }

    let mut entries = counts
        .into_iter()
        .map(
            |((extra1_raw, extra2_raw, extra3_raw), (count, example_codes))| {
                Quote0547ExtraPatternSummary {
                    extra1_raw,
                    extra2_raw,
                    extra3_raw,
                    count,
                    example_codes,
                }
            },
        )
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.extra1_raw.cmp(&right.extra1_raw))
            .then_with(|| left.extra2_raw.cmp(&right.extra2_raw))
            .then_with(|| left.extra3_raw.cmp(&right.extra3_raw))
    });
    entries.truncate(limit);
    entries
}

fn top_pattern_correlations(
    records: &[&netzipapi_rust_demo::Tdx0547Record],
    code_table_lookup: &BTreeMap<String, Quote0547CodeTableInfo>,
    limit: usize,
) -> Vec<Quote0547PatternCorrelationSummary> {
    top_extra_patterns(records, limit)
        .into_iter()
        .map(|pattern| {
            let matching = records
                .iter()
                .copied()
                .filter(|record| {
                    matches!(
                        (record.extra1_raw, record.extra2_raw, record.extra3_raw),
                        (Some(extra1), Some(extra2), Some(extra3))
                            if extra1 == pattern.extra1_raw
                                && extra2 == pattern.extra2_raw
                                && extra3 == pattern.extra3_raw
                    )
                })
                .collect::<Vec<_>>();
            Quote0547PatternCorrelationSummary {
                extra1_raw: pattern.extra1_raw,
                extra2_raw: pattern.extra2_raw,
                extra3_raw: pattern.extra3_raw,
                count: pattern.count,
                prefix3_top: top_label_groups(
                    &matching,
                    code_table_lookup,
                    |record, _| normalize_code_prefix(&record.code, 3),
                    6,
                ),
                market_top: top_label_groups(
                    &matching,
                    code_table_lookup,
                    |record, _| {
                        Some(
                            tdx_0547_market_name(record.market)
                                .unwrap_or("?")
                                .to_string(),
                        )
                    },
                    6,
                ),
                decimal_point_top: top_label_groups(
                    &matching,
                    code_table_lookup,
                    |record, lookup| {
                        lookup
                            .get(&record.code)
                            .map(|info| info.decimal_point.to_string())
                    },
                    6,
                ),
                name_keyword_top: top_label_groups(
                    &matching,
                    code_table_lookup,
                    |record, lookup| {
                        lookup
                            .get(&record.code)
                            .and_then(|info| infer_name_keyword_tag(&info.name))
                    },
                    6,
                ),
                time_hhmmss_top: top_label_groups(
                    &matching,
                    code_table_lookup,
                    |record, _| record.time_hhmmss_raw.and_then(tdx_0547_format_hhmmss_raw),
                    6,
                ),
                extra0_time_hint_top: top_label_groups(
                    &matching,
                    code_table_lookup,
                    |record, _| record.extra0_time_hhmmss.clone(),
                    6,
                ),
            }
        })
        .collect()
}

fn top_label_correlations(
    records: &[&netzipapi_rust_demo::Tdx0547Record],
    code_table_lookup: &BTreeMap<String, Quote0547CodeTableInfo>,
    label_fn: impl Fn(
        &netzipapi_rust_demo::Tdx0547Record,
        &BTreeMap<String, Quote0547CodeTableInfo>,
    ) -> Option<String>,
    limit: usize,
) -> Vec<Quote0547LabelCorrelationSummary> {
    top_label_groups(records, code_table_lookup, &label_fn, limit)
        .into_iter()
        .map(|group| {
            let matching = records
                .iter()
                .copied()
                .filter(|record| {
                    label_fn(record, code_table_lookup).as_deref() == Some(group.label.as_str())
                })
                .collect::<Vec<_>>();
            Quote0547LabelCorrelationSummary {
                label: group.label,
                count: group.count,
                prefix3_top: top_label_groups(
                    &matching,
                    code_table_lookup,
                    |record, _| normalize_code_prefix(&record.code, 3),
                    6,
                ),
                market_top: top_label_groups(
                    &matching,
                    code_table_lookup,
                    |record, _| {
                        Some(
                            tdx_0547_market_name(record.market)
                                .unwrap_or("?")
                                .to_string(),
                        )
                    },
                    6,
                ),
                decimal_point_top: top_label_groups(
                    &matching,
                    code_table_lookup,
                    |record, lookup| {
                        lookup
                            .get(&record.code)
                            .map(|info| info.decimal_point.to_string())
                    },
                    6,
                ),
                name_keyword_top: top_label_groups(
                    &matching,
                    code_table_lookup,
                    |record, lookup| {
                        lookup
                            .get(&record.code)
                            .and_then(|info| infer_name_keyword_tag(&info.name))
                    },
                    6,
                ),
                time_hhmmss_top: top_label_groups(
                    &matching,
                    code_table_lookup,
                    |record, _| record.time_hhmmss_raw.and_then(tdx_0547_format_hhmmss_raw),
                    6,
                ),
                extra0_time_hint_top: top_label_groups(
                    &matching,
                    code_table_lookup,
                    |record, _| record.extra0_time_hhmmss.clone(),
                    6,
                ),
            }
        })
        .collect()
}

fn top_label_groups(
    records: &[&netzipapi_rust_demo::Tdx0547Record],
    code_table_lookup: &BTreeMap<String, Quote0547CodeTableInfo>,
    label_fn: impl Fn(
        &netzipapi_rust_demo::Tdx0547Record,
        &BTreeMap<String, Quote0547CodeTableInfo>,
    ) -> Option<String>,
    limit: usize,
) -> Vec<Quote0547LabelSummary> {
    let mut counts = BTreeMap::<String, (usize, Vec<String>)>::new();
    for record in records {
        let Some(label) = label_fn(record, code_table_lookup) else {
            continue;
        };
        let entry = counts.entry(label).or_insert_with(|| (0, Vec::new()));
        entry.0 += 1;
        if entry.1.len() < 5 {
            let symbol = tdx_0547_market_name(record.market)
                .map(|prefix| format!("{prefix}{}", record.code))
                .unwrap_or_else(|| record.code.clone());
            if !entry.1.iter().any(|value| value == &symbol) {
                entry.1.push(symbol);
            }
        }
    }

    let mut entries = counts
        .into_iter()
        .map(|(label, (count, example_codes))| Quote0547LabelSummary {
            label,
            count,
            example_codes,
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.label.cmp(&right.label))
    });
    entries.truncate(limit);
    entries
}

fn top_scoped_source_groups(
    records: &[Quote0547ScopedRecord<'_>],
    limit: usize,
) -> Vec<Quote0547LabelSummary> {
    let mut counts = BTreeMap::<String, (usize, Vec<String>)>::new();
    for scoped in records {
        let entry = counts
            .entry(scoped.source_name.to_string())
            .or_insert_with(|| (0, Vec::new()));
        entry.0 += 1;
        if entry.1.len() < 5 {
            let symbol = tdx_0547_market_name(scoped.record.market)
                .map(|prefix| format!("{prefix}{}", scoped.record.code))
                .unwrap_or_else(|| scoped.record.code.clone());
            if !entry.1.iter().any(|value| value == &symbol) {
                entry.1.push(symbol);
            }
        }
    }

    let mut entries = counts
        .into_iter()
        .map(|(label, (count, example_codes))| Quote0547LabelSummary {
            label,
            count,
            example_codes,
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.label.cmp(&right.label))
    });
    entries.truncate(limit);
    entries
}

fn normalize_code_prefix(code: &str, width: usize) -> Option<String> {
    let digits = normalize_numeric_code_key(code);
    if digits.len() < width {
        None
    } else {
        Some(digits[..width].to_string())
    }
}

fn infer_name_keyword_tag(name: &str) -> Option<String> {
    infer_name_keyword_tags(name).into_iter().next()
}

fn infer_name_keyword_tags(name: &str) -> Vec<String> {
    let mut tags = Vec::new();
    if name.contains("ETF") {
        tags.push("ETF".to_string());
    }
    if name.contains("转债") {
        tags.push("转债".to_string());
    }
    let upper = name.to_ascii_uppercase();
    if upper.contains("REIT") {
        tags.push("REIT".to_string());
    }
    if upper.contains("LOF") {
        tags.push("LOF".to_string());
    }
    if name.contains("指数") {
        tags.push("指数".to_string());
    }
    tags
}

fn augment_live_quote_symbols(
    requested: &[NormalizedLiveQuoteSymbol],
) -> Vec<NormalizedLiveQuoteSymbol> {
    const LIVE_QUOTE_SEEDS: &[&str] = &["SH600000", "SZ300948", "SH113638"];

    let mut out = requested.to_vec();
    let mut seen = out
        .iter()
        .map(|item| item.symbol.clone())
        .collect::<BTreeSet<_>>();

    for seed in LIVE_QUOTE_SEEDS {
        if out.len() >= 3 {
            break;
        }
        let Ok(normalized) = normalize_live_quote_symbol(seed) else {
            continue;
        };
        if seen.insert(normalized.symbol.clone()) {
            out.push(normalized);
        }
    }

    out
}

fn normalize_live_quote_symbol(input: &str) -> Result<NormalizedLiveQuoteSymbol, String> {
    let trimmed = input.trim().to_ascii_uppercase();
    if trimmed.is_empty() {
        return Err("symbols must not contain empty entries".to_string());
    }

    let (market, code) = if let Some(code) = trimmed.strip_prefix("SH") {
        (1u8, code.to_string())
    } else if let Some(code) = trimmed.strip_prefix("SZ") {
        (0u8, code.to_string())
    } else if let Some(code) = trimmed.strip_prefix("BJ") {
        (2u8, code.to_string())
    } else if trimmed.len() == 6 && trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        (
            infer_live_quote_market(&trimmed).ok_or_else(|| {
                format!("cannot infer market for bare code {trimmed}; use SH/SZ/BJ prefix")
            })?,
            trimmed.clone(),
        )
    } else {
        return Err(format!(
            "invalid symbol {trimmed}; expected SH600000/SZ300948/BJ430047 or 6-digit code"
        ));
    };

    if code.len() != 6 || !code.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("invalid 6-digit code in symbol {trimmed}"));
    }

    let market_name = tdx_0547_market_name(market).ok_or_else(|| {
        format!(
            "unsupported market flag {market} for symbol {}",
            trimmed.clone()
        )
    })?;
    Ok(NormalizedLiveQuoteSymbol {
        market,
        symbol: format!("{market_name}{code}"),
        code,
    })
}

fn parse_compact_quote_codes(input: &str) -> Result<Vec<NormalizedLiveQuoteSymbol>, String> {
    let values = input.split(',').map(str::trim).collect::<Vec<_>>();
    if values.is_empty() || values.iter().all(|value| value.is_empty()) {
        return Err("codes must contain at least one stock symbol".to_string());
    }
    if values.len() > 100 {
        return Err(format!(
            "codes supports at most 100 items, got {}",
            values.len()
        ));
    }

    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for value in values {
        let item = normalize_live_quote_symbol(value)?;
        if seen.insert(item.symbol.clone()) {
            normalized.push(item);
        }
    }
    Ok(normalized)
}

fn normalize_trade_date(input: &str) -> Result<String, String> {
    let value = input.trim();
    let bytes = value.as_bytes();
    if bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return Ok(value.to_string());
    }
    Err("trade_date must use YYYY-MM-DD format".to_string())
}

fn parse_gateway_worklist(value: &serde_json::Value) -> Result<GatewayWorklist, ApiError> {
    let payload = value
        .get("payload")
        .ok_or_else(|| ApiError::internal("quote-gateway worklist has no payload"))?;
    let trade_date = payload
        .pointer("/freshness/required_quote_trade_date")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ApiError::internal("quote-gateway worklist has no required trade date"))?;
    let trade_date = normalize_trade_date(trade_date).map_err(ApiError::internal)?;
    let as_of_date = payload
        .get("as_of_date")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ApiError::internal("quote-gateway worklist has no as_of_date"))?;
    let fallback_quote_time = if trade_date == as_of_date {
        let refreshed_at = payload
            .get("last_refreshed_at")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ApiError::internal("quote-gateway worklist has no last_refreshed_at"))?;
        normalize_clock_time(
            refreshed_at
                .rsplit_once(' ')
                .map_or(refreshed_at, |(_, time)| time),
        )
        .map_err(ApiError::internal)?
    } else {
        "15:00:00".to_string()
    };
    let rows = payload
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ApiError::internal("quote-gateway worklist data is not an array"))?;
    let mut symbols = Vec::with_capacity(rows.len());
    let mut names = BTreeMap::new();
    for row in rows {
        let market = row
            .get("market")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| ApiError::internal("quote-gateway worklist row has no market"))?;
        let code = row
            .get("code")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ApiError::internal("quote-gateway worklist row has no code"))?;
        let prefix = match market {
            0 => "SZ",
            1 => "SH",
            2 => "BJ",
            other => {
                return Err(ApiError::internal(format!(
                    "unsupported quote-gateway worklist market: {other}"
                )));
            }
        };
        let symbol = format!("{prefix}{code}");
        if let Some(name) = row
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            names.insert(symbol.clone(), name.to_string());
        }
        symbols.push(symbol);
    }
    if symbols.is_empty() {
        return Err(ApiError::internal("quote-gateway worklist is empty"));
    }
    Ok(GatewayWorklist {
        trade_date,
        fallback_quote_time,
        symbols,
        names,
    })
}

fn normalize_clock_time(input: &str) -> Result<String, String> {
    let parts = input.split(':').collect::<Vec<_>>();
    if parts.len() != 3 || parts.iter().any(|part| part.len() != 2) {
        return Err("time must use HH:MM:SS format".to_string());
    }
    let hour = parts[0]
        .parse::<u8>()
        .map_err(|_| "invalid hour".to_string())?;
    let minute = parts[1]
        .parse::<u8>()
        .map_err(|_| "invalid minute".to_string())?;
    let second = parts[2]
        .parse::<u8>()
        .map_err(|_| "invalid second".to_string())?;
    if hour >= 24 || minute >= 60 || second >= 60 {
        return Err("time is outside HH:MM:SS range".to_string());
    }
    Ok(format!("{hour:02}:{minute:02}:{second:02}"))
}

fn split_full_push_batches(
    symbols: &[String],
    batch_size: usize,
) -> Result<Vec<Vec<String>>, ApiError> {
    if !(1..=100).contains(&batch_size) {
        return Err(ApiError::bad_request(
            "batch_size must be between 1 and 100",
        ));
    }
    Ok(symbols.chunks(batch_size).map(<[String]>::to_vec).collect())
}

fn validated_full_push_worker_count(
    requested: usize,
    batch_count: usize,
) -> Result<usize, ApiError> {
    if !(1..=16).contains(&requested) {
        return Err(ApiError::bad_request(
            "worker_count must be between 1 and 16",
        ));
    }
    Ok(requested.min(batch_count))
}

fn shard_full_push_batches(
    batches: Vec<Vec<String>>,
    worker_count: usize,
) -> Vec<Vec<(usize, Vec<String>)>> {
    if batches.is_empty() || worker_count == 0 {
        return Vec::new();
    }
    let mut shards = (0..worker_count)
        .map(|_| Vec::new())
        .collect::<Vec<Vec<(usize, Vec<String>)>>>();
    for (index, batch) in batches.into_iter().enumerate() {
        shards[index % worker_count].push((index, batch));
    }
    shards
}

fn retry_full_push_batch<T, E>(
    max_attempts: usize,
    mut request: impl FnMut() -> Result<T, E>,
) -> Result<T, E> {
    let attempts = max_attempts.max(1);
    for attempt in 1..attempts {
        match request() {
            Ok(value) => return Ok(value),
            Err(_) if attempt < attempts => continue,
            Err(err) => return Err(err),
        }
    }
    request()
}

fn retry_full_push_with_reopen<S, T, E>(
    max_attempts: usize,
    session: &mut Option<S>,
    mut open: impl FnMut() -> Result<S, E>,
    mut request: impl FnMut(&mut S) -> Result<T, E>,
) -> Result<T, E> {
    let attempts = max_attempts.max(1);
    for attempt in 1..=attempts {
        if session.is_none() {
            *session = Some(open()?);
        }
        let result = request(session.as_mut().expect("session opened above"));
        match result {
            Ok(value) => return Ok(value),
            Err(err) if attempt == attempts => {
                *session = None;
                return Err(err);
            }
            Err(_) => *session = None,
        }
    }
    unreachable!("at least one retry attempt is always executed")
}

fn take_full_push_session<S>(cache: &Mutex<BTreeMap<String, S>>, key: &str) -> Option<S> {
    cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(key)
}

fn cache_full_push_session<S>(cache: &Mutex<BTreeMap<String, S>>, key: &str, session: S) {
    cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(key.to_string(), session);
}

fn return_full_push_session<S>(
    cache: &Mutex<BTreeMap<String, S>>,
    key: &str,
    session: Option<S>,
    reuse_session: bool,
) {
    if reuse_session && let Some(session) = session {
        cache_full_push_session(cache, key, session);
    }
}

fn full_push_session_cache() -> &'static Mutex<BTreeMap<String, Tdx7709Session>> {
    FULL_PUSH_SESSION_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn full_push_last_published_at() -> &'static Mutex<BTreeMap<String, String>> {
    FULL_PUSH_LAST_PUBLISHED_AT.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn full_push_quote_version(trade_date: &str, quote: &CompactQuote) -> Option<String> {
    let datetime = quote
        .quote_datetime
        .as_deref()
        .or(quote.datetime.as_deref())?
        .trim();
    if datetime.is_empty() {
        return None;
    }
    if datetime.len() >= 10 && datetime.as_bytes().get(4) == Some(&b'-') {
        Some(datetime.to_string())
    } else {
        Some(format!("{trade_date} {datetime}"))
    }
}

fn select_new_full_push_quotes(
    cache: &Mutex<BTreeMap<String, String>>,
    trade_date: &str,
    quotes: Vec<CompactQuote>,
) -> (Vec<CompactQuote>, usize) {
    let published_at = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut selected = Vec::with_capacity(quotes.len());
    let mut unchanged = 0usize;
    for quote in quotes {
        let is_newer = full_push_quote_version(trade_date, &quote).is_some_and(|version| {
            published_at
                .get(&quote.symbol)
                .is_none_or(|previous| version > *previous)
        });
        if is_newer {
            selected.push(quote);
        } else {
            unchanged = unchanged.saturating_add(1);
        }
    }
    (selected, unchanged)
}

fn record_published_full_push_quotes(
    cache: &Mutex<BTreeMap<String, String>>,
    trade_date: &str,
    quotes: &[CompactQuote],
) {
    let mut published_at = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for quote in quotes {
        if let Some(version) = full_push_quote_version(trade_date, quote) {
            published_at.insert(quote.symbol.clone(), version);
        }
    }
}

fn split_full_push_upstreams(symbols: &[String]) -> (Vec<String>, Vec<String>) {
    symbols
        .iter()
        .cloned()
        .partition(|symbol| !symbol.starts_with("BJ"))
}

fn build_bj_poll_plan(symbols: &[String], interval: Duration) -> Result<BjPollPlan, ApiError> {
    if interval < Duration::from_secs(1) || interval > Duration::from_secs(30) {
        return Err(ApiError::bad_request(
            "bj_poll_interval_secs must be between 1 and 30",
        ));
    }
    let batch_count = symbols.len().div_ceil(100);
    Ok(BjPollPlan {
        symbols: symbols.to_vec(),
        batch_count,
        worker_count: batch_count.min(4),
        interval,
    })
}

#[allow(clippy::too_many_arguments)]
fn execute_bj_poll_loop(
    plan: &BjPollPlan,
    deadline: Instant,
    trade_date: &str,
    fallback_quote_time: &str,
    worklist_names: &BTreeMap<String, String>,
    gateway_addr: &str,
    current_token: Option<&str>,
) -> BjPollSummary {
    let fallback_host =
        std::env::var("NETZIP_TDX7709_FALLBACK_HOST").unwrap_or_else(|_| "139.9.43.31".to_string());
    let config = Tdx7709Config {
        host: fallback_host,
        ..Tdx7709Config::default()
    };
    let session_cache = Mutex::new(BTreeMap::new());
    let mut protocol = GatewayPublishProtocol::Auto;
    let mut summary = BjPollSummary::default();
    while Instant::now() < deadline {
        match publish_full_push_stage(
            "bj-resident",
            &config,
            &plan.symbols,
            100,
            trade_date,
            fallback_quote_time,
            worklist_names,
            gateway_addr,
            current_token,
            plan.worker_count,
            &session_cache,
            true,
            &mut protocol,
            GatewayPublishLane::Bj,
        ) {
            Ok(result) => {
                summary.runs = summary.runs.saturating_add(1);
                summary.published_records = summary
                    .published_records
                    .saturating_add(result.published_count);
                summary.unchanged_records = summary
                    .unchanged_records
                    .saturating_add(result.unchanged_count);
            }
            Err(error) => {
                summary.failures = summary.failures.saturating_add(1);
                eprintln!("native BJ polling failed: {error:?}");
            }
        }
        let next_poll_at = Instant::now() + plan.interval;
        let sleep_for = deadline
            .saturating_duration_since(Instant::now())
            .min(next_poll_at.saturating_duration_since(Instant::now()));
        if !sleep_for.is_zero() {
            std::thread::sleep(sleep_for);
        }
    }
    summary
}

fn partition_hqw_quotes(
    quotes: Vec<CompactQuote>,
    mut missing_codes: Vec<String>,
) -> (Vec<CompactQuote>, Vec<String>) {
    let mut publishable = Vec::with_capacity(quotes.len());
    for quote in quotes {
        if quote.volume.is_some() && quote.amount.is_some() && quote.quote_datetime.is_some() {
            publishable.push(quote);
        } else {
            missing_codes.push(quote.symbol);
        }
    }
    (publishable, missing_codes)
}

enum NativePushReaderMessage {
    Record {
        shard: usize,
        record: netzipapi_rust_demo::Tdx0547Record,
    },
    Healthy {
        shard: usize,
    },
    Failed {
        shard: usize,
        error: String,
    },
    RenewalSent,
}

fn compact_quote_from_push_record(
    record: &netzipapi_rust_demo::Tdx0547Record,
    code_table_lookup: &BTreeMap<String, Quote0547CodeTableInfo>,
    worklist_names: &BTreeMap<String, String>,
) -> Option<CompactQuote> {
    let symbol = tdx_0547_record_symbol(record)?;
    let decimal_point = resolve_quote_0547_decimal_point(record, None, code_table_lookup)?;
    let head = record
        .quote_head
        .as_ref()
        .and_then(|head| tdx_0547_normalize_quote_head(head, decimal_point))?;
    let market = tdx_0547_market_name(record.market)?;
    let quote_datetime = tdx_0547_public_time_hhmmss(record);
    Some(CompactQuote {
        code: record.code.clone(),
        symbol: symbol.clone(),
        market: market.to_string(),
        name: worklist_names.get(&symbol).cloned().or_else(|| {
            lookup_quote_0547_code_table_info(record, code_table_lookup)
                .map(|info| info.name.clone())
        }),
        price: head.price,
        last_close: head.last_close,
        open: head.open,
        high: head.high,
        low: head.low,
        volume: record.volume,
        amount: oem_public_amount(record, decimal_point),
        datetime: quote_datetime.clone(),
        quote_datetime,
        source: "netzip-rust-7709-push",
    })
}

fn flush_native_push_quotes(
    coalescer: &mut TdxPushCoalescer<CompactQuote>,
    now_ms: u64,
    force: bool,
    trade_date: &str,
    gateway_addr: &str,
    current_token: Option<&str>,
    publish: bool,
    protocol: &mut GatewayPublishProtocol,
    lane: GatewayPublishLane,
) -> Result<(usize, usize, usize, usize), ApiError> {
    let batches = if force {
        coalescer.drain()
    } else {
        coalescer.drain_ready(Duration::from_millis(now_ms))
    };
    let mut published = 0usize;
    let mut unchanged = 0usize;
    let mut publish_batches = 0usize;
    let mut publish_failures = 0usize;
    for batch in batches {
        let quotes = batch
            .into_iter()
            .map(|event| event.value)
            .collect::<Vec<_>>();
        let (quotes, batch_unchanged) =
            select_new_full_push_quotes(full_push_last_published_at(), trade_date, quotes);
        unchanged = unchanged.saturating_add(batch_unchanged);
        if quotes.is_empty() || !publish {
            continue;
        }
        let payload = build_netzip_rust_7709_quote_batch(&quotes, trade_date)?;
        match post_quote_gateway_batch_auto(gateway_addr, current_token, &payload, protocol, lane) {
            Ok(_) => {
                record_published_full_push_quotes(
                    full_push_last_published_at(),
                    trade_date,
                    &quotes,
                );
                published = published.saturating_add(quotes.len());
                publish_batches = publish_batches.saturating_add(1);
            }
            Err(error) => {
                publish_failures = publish_failures.saturating_add(1);
                eprintln!(
                    "full push publish lane={} batch_quotes={} failed after bounded transport fallback: {error:?}",
                    lane.label(),
                    quotes.len()
                );
            }
        }
    }
    Ok((published, unchanged, publish_batches, publish_failures))
}

fn execute_hqw_push_worklist(
    gateway_addr: String,
    current_token: Option<String>,
    duration: Duration,
    audit_interval: Duration,
    bj_poll_interval: Duration,
    publish: bool,
) -> Result<HqwPushWorklistResponse, ApiError> {
    let overall_started_at = Instant::now();
    let worklist_payload = get_gateway_worklist(&gateway_addr, 6_000)?;
    let worklist = parse_gateway_worklist(&worklist_payload)?;
    let (primary_symbols, bj_symbols) = split_full_push_upstreams(&worklist.symbols);
    let bj_poll_plan = build_bj_poll_plan(&bj_symbols, bj_poll_interval)?;
    let batches = split_full_push_batches(&primary_symbols, 100)?;
    let code_table_lookup = if let Some(lookup) = NATIVE_PUSH_CODE_TABLE_LOOKUP.get() {
        lookup.clone()
    } else {
        let code_table = Tdx7709Session::open(&Tdx7709Config::default())
            .map_err(|err| {
                ApiError::internal(format!("open push code-table session failed: {err}"))
            })?
            .sync_result();
        let lookup = build_quote_0547_code_table_lookup(&code_table.records);
        let _ = NATIVE_PUSH_CODE_TABLE_LOOKUP.set(lookup.clone());
        lookup
    };
    let started_at = Instant::now();
    let deadline = started_at + duration;
    let (sender, receiver) = mpsc::channel::<NativePushReaderMessage>();
    let shard_count = batches.len();
    let renewal_rate_gate = Arc::new(Mutex::new(RenewalRateGate::per_second(160)));

    std::thread::scope(|scope| -> Result<HqwPushWorklistResponse, ApiError> {
        let bj_poll_handle = if publish && !bj_poll_plan.symbols.is_empty() {
            let plan = bj_poll_plan.clone();
            let trade_date = worklist.trade_date.clone();
            let fallback_quote_time = worklist.fallback_quote_time.clone();
            let worklist_names = worklist.names.clone();
            let bj_gateway_addr = gateway_addr.clone();
            let bj_current_token = current_token.clone();
            Some(scope.spawn(move || {
                execute_bj_poll_loop(
                    &plan,
                    deadline,
                    &trade_date,
                    &fallback_quote_time,
                    &worklist_names,
                    &bj_gateway_addr,
                    bj_current_token.as_deref(),
                )
            }))
        } else {
            None
        };
        for (shard, symbols) in batches.into_iter().enumerate() {
            let sender = sender.clone();
            let renewal_rate_gate = Arc::clone(&renewal_rate_gate);
            scope.spawn(move || {
                let request_items = symbols
                    .iter()
                    .filter_map(|symbol| normalize_live_quote_symbol(symbol).ok())
                    .map(|item| Tdx7709QuoteRequestItem {
                        market: item.market,
                        code: item.code,
                        token: 0,
                    })
                    .collect::<Vec<_>>();
                while Instant::now() < deadline {
                    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                        let session_started_at = Instant::now();
                        let mut session =
                            Tdx7709Session::open_quote_only(&Tdx7709Config::default())?;
                        let initial = session.request_live_quotes(&request_items)?;
                        let mut renewal_scheduler = QuoteRenewalScheduler::default();
                        let _ = sender.send(NativePushReaderMessage::Healthy { shard });
                        for record in initial
                            .quote_bodies
                            .into_iter()
                            .flat_map(|body| body.records)
                        {
                            if let Some(token) = record.renewal_token_raw {
                                renewal_scheduler.record_response(
                                    record.market,
                                    &record.code,
                                    token,
                                    session_started_at.elapsed().as_millis() as u64,
                                );
                            }
                            if sender
                                .send(NativePushReaderMessage::Record { shard, record })
                                .is_err()
                            {
                                return Ok(());
                            }
                        }
                        while Instant::now() < deadline {
                            let remaining = deadline.saturating_duration_since(Instant::now());
                            let observation = session.collect_quote_delivery_observation(
                                remaining.min(Duration::from_millis(250)),
                            )?;
                            for record in observation
                                .deliveries
                                .into_iter()
                                .flat_map(|timed| timed.delivery.body.records)
                            {
                                if let Some(token) = record.renewal_token_raw {
                                    renewal_scheduler.record_response(
                                        record.market,
                                        &record.code,
                                        token,
                                        session_started_at.elapsed().as_millis() as u64,
                                    );
                                }
                                if sender
                                    .send(NativePushReaderMessage::Record { shard, record })
                                    .is_err()
                                {
                                    return Ok(());
                                }
                            }
                            let session_now_ms = session_started_at.elapsed().as_millis() as u64;
                            let global_now_ms = started_at.elapsed().as_millis() as u64;
                            if renewal_scheduler.has_due(session_now_ms)
                                && renewal_rate_gate
                                    .lock()
                                    .expect("renewal rate gate poisoned")
                                    .try_take(global_now_ms)
                            {
                                let due = renewal_scheduler.take_due(session_now_ms, 100);
                                let renewals = due
                                    .into_iter()
                                    .map(|item| Tdx7709QuoteRequestItem {
                                        market: item.market,
                                        code: item.code,
                                        token: item.token,
                                    })
                                    .collect::<Vec<_>>();
                                if !renewals.is_empty() {
                                    session.send_live_quote_renewal(&renewals)?;
                                    let _ = sender.send(NativePushReaderMessage::RenewalSent);
                                }
                            }
                        }
                        Ok(())
                    })();
                    if let Err(error) = result {
                        let _ = sender.send(NativePushReaderMessage::Failed {
                            shard,
                            error: error.to_string(),
                        });
                        std::thread::sleep(
                            deadline
                                .saturating_duration_since(Instant::now())
                                .min(Duration::from_secs(1)),
                        );
                    }
                }
            });
        }
        drop(sender);

        let mut coalescer =
            TdxPushCoalescer::new(Duration::from_millis(100), 100).map_err(ApiError::internal)?;
        let mut protocol = GatewayPublishProtocol::Auto;
        let mut received_records = 0usize;
        let mut converted_records = 0usize;
        let mut unconverted_symbols = BTreeSet::new();
        let mut published_records = 0usize;
        let mut unchanged_records = 0usize;
        let mut publish_batches = 0usize;
        let mut publish_failures = 0usize;
        let mut reader_failures = 0usize;
        let mut reader_recoveries = 0usize;
        let mut renewal_requests = 0usize;
        let mut failed_shards = BTreeSet::new();
        let mut audit_runs = 0usize;
        let mut audit_failures = 0usize;
        let mut next_audit_at = started_at + audit_interval;
        let mut audit_handle = None;

        while Instant::now() < deadline {
            let now = Instant::now();
            let wait = deadline
                .saturating_duration_since(now)
                .min(Duration::from_millis(25));
            match receiver.recv_timeout(wait) {
                Ok(NativePushReaderMessage::Record { shard, record }) => {
                    let _ = shard;
                    received_records = received_records.saturating_add(1);
                    if let Some(quote) =
                        compact_quote_from_push_record(&record, &code_table_lookup, &worklist.names)
                    {
                        let source_time = record.time_hhmmss_raw.unwrap_or_default();
                        coalescer.push(
                            started_at.elapsed(),
                            TdxPushEvent {
                                symbol: quote.symbol.clone(),
                                source_time,
                                value: quote,
                            },
                        );
                        converted_records = converted_records.saturating_add(1);
                    } else if let Some(symbol) = tdx_0547_record_symbol(&record) {
                        unconverted_symbols.insert(symbol);
                    }
                }
                Ok(NativePushReaderMessage::Healthy { shard }) => {
                    if failed_shards.remove(&shard) {
                        reader_recoveries = reader_recoveries.saturating_add(1);
                    }
                }
                Ok(NativePushReaderMessage::Failed { shard, error }) => {
                    eprintln!("native push shard {shard} failed: {error}");
                    failed_shards.insert(shard);
                    reader_failures = reader_failures.saturating_add(1);
                }
                Ok(NativePushReaderMessage::RenewalSent) => {
                    renewal_requests = renewal_requests.saturating_add(1);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }

            let (published, unchanged, batches, failures) = flush_native_push_quotes(
                &mut coalescer,
                started_at.elapsed().as_millis() as u64,
                false,
                &worklist.trade_date,
                &gateway_addr,
                current_token.as_deref(),
                publish,
                &mut protocol,
                GatewayPublishLane::Main,
            )?;
            published_records = published_records.saturating_add(published);
            unchanged_records = unchanged_records.saturating_add(unchanged);
            publish_batches = publish_batches.saturating_add(batches);
            publish_failures = publish_failures.saturating_add(failures);

            if audit_handle
                .as_ref()
                .is_some_and(std::thread::JoinHandle::is_finished)
            {
                let result = audit_handle.take().expect("finished audit handle").join();
                audit_runs = audit_runs.saturating_add(1);
                if !matches!(result, Ok(Ok(_))) {
                    audit_failures = audit_failures.saturating_add(1);
                }
            }
            if publish && Instant::now() >= next_audit_at && audit_handle.is_none() {
                let audit_gateway = gateway_addr.clone();
                let audit_token = current_token.clone();
                audit_handle = Some(std::thread::spawn(move || {
                    execute_hqw_publish_worklist(
                        audit_gateway,
                        audit_token.as_deref(),
                        100,
                        6_000,
                        8,
                        false,
                    )
                }));
                next_audit_at = Instant::now() + audit_interval;
            }
        }

        let (published, unchanged, batches, failures) = flush_native_push_quotes(
            &mut coalescer,
            started_at.elapsed().as_millis() as u64,
            true,
            &worklist.trade_date,
            &gateway_addr,
            current_token.as_deref(),
            publish,
            &mut protocol,
            GatewayPublishLane::Main,
        )?;
        published_records = published_records.saturating_add(published);
        unchanged_records = unchanged_records.saturating_add(unchanged);
        publish_batches = publish_batches.saturating_add(batches);
        publish_failures = publish_failures.saturating_add(failures);
        if let Some(handle) = audit_handle {
            audit_runs = audit_runs.saturating_add(1);
            if !matches!(handle.join(), Ok(Ok(_))) {
                audit_failures = audit_failures.saturating_add(1);
            }
        }
        let bj_poll_summary = match bj_poll_handle {
            Some(handle) => handle.join().unwrap_or_else(|_| BjPollSummary {
                failures: 1,
                ..BjPollSummary::default()
            }),
            None => BjPollSummary::default(),
        };

        Ok(HqwPushWorklistResponse {
            success: true,
            publish,
            trade_date: worklist.trade_date,
            worklist_count: worklist.symbols.len(),
            subscribed_count: primary_symbols.len(),
            shard_count,
            received_records,
            converted_records,
            unconverted_symbols: unconverted_symbols.len(),
            unconverted_symbol_sample: unconverted_symbols.into_iter().take(30).collect(),
            published_records,
            unchanged_records,
            publish_batches,
            publish_failures,
            reader_failures,
            reader_recoveries,
            renewal_requests,
            audit_runs,
            audit_failures,
            bj_poll_symbols: bj_poll_plan.symbols.len(),
            bj_poll_runs: bj_poll_summary.runs,
            bj_poll_failures: bj_poll_summary.failures,
            bj_published_records: bj_poll_summary.published_records,
            bj_unchanged_records: bj_poll_summary.unchanged_records,
            gateway_publish_metrics: gateway_publish_metrics_snapshot(),
            elapsed_ms: overall_started_at.elapsed().as_millis(),
        })
    })
}

fn execute_hqw_publish_worklist(
    gateway_addr: String,
    current_token: Option<&str>,
    batch_size: usize,
    limit: usize,
    worker_count: usize,
    reuse_sessions: bool,
) -> Result<HqwPublishWorklistResponse, ApiError> {
    let started_at = Instant::now();
    let worklist_payload = get_gateway_worklist(&gateway_addr, limit)?;
    let worklist = parse_gateway_worklist(&worklist_payload)?;
    let mut gateway_protocol = GatewayPublishProtocol::Auto;
    let session_cache = full_push_session_cache();
    let (primary_symbols, mut fallback_symbols) = split_full_push_upstreams(&worklist.symbols);
    let primary_config = Tdx7709Config::default();
    let primary = publish_full_push_stage(
        "primary",
        &primary_config,
        &primary_symbols,
        batch_size,
        &worklist.trade_date,
        &worklist.fallback_quote_time,
        &worklist.names,
        &gateway_addr,
        current_token,
        worker_count,
        session_cache,
        reuse_sessions,
        &mut gateway_protocol,
        GatewayPublishLane::Manual,
    )?;
    fallback_symbols.splice(0..0, primary.missing_codes);
    let mut seen = BTreeSet::new();
    fallback_symbols.retain(|symbol| seen.insert(symbol.clone()));

    let fallback_host =
        std::env::var("NETZIP_TDX7709_FALLBACK_HOST").unwrap_or_else(|_| "139.9.43.31".to_string());
    let fallback_config = Tdx7709Config {
        host: fallback_host.clone(),
        ..Tdx7709Config::default()
    };
    let fallback = publish_full_push_stage(
        "fallback",
        &fallback_config,
        &fallback_symbols,
        batch_size,
        &worklist.trade_date,
        &worklist.fallback_quote_time,
        &worklist.names,
        &gateway_addr,
        current_token,
        worker_count,
        session_cache,
        reuse_sessions,
        &mut gateway_protocol,
        GatewayPublishLane::Manual,
    )?;
    let last_gateway_response = fallback
        .last_gateway_response
        .or(primary.last_gateway_response);

    Ok(HqwPublishWorklistResponse {
        success: true,
        gateway_addr,
        gateway_protocol: gateway_protocol.label(),
        trade_date: worklist.trade_date,
        worklist_count: worklist.symbols.len(),
        batch_size,
        worker_count,
        batch_count: primary.batch_count + fallback.batch_count,
        primary_batch_count: primary.batch_count,
        primary_worker_count: primary.worker_count,
        primary_elapsed_ms: primary.elapsed_ms,
        primary_slowest_batch_ms: primary.slowest_batch_ms,
        fallback_batch_count: fallback.batch_count,
        fallback_worker_count: fallback.worker_count,
        fallback_elapsed_ms: fallback.elapsed_ms,
        fallback_slowest_batch_ms: fallback.slowest_batch_ms,
        elapsed_ms: started_at.elapsed().as_millis(),
        fallback_host,
        fallback_published_count: fallback.published_count,
        published_count: primary.published_count + fallback.published_count,
        unchanged_count: primary.unchanged_count + fallback.unchanged_count,
        no_current_quote_count: fallback.missing_codes.len(),
        no_current_quote_codes: fallback.missing_codes,
        last_gateway_response,
        gateway_publish_metrics: gateway_publish_metrics_snapshot(),
    })
}

#[allow(clippy::too_many_arguments)]
fn publish_full_push_stage(
    stage: &str,
    config: &Tdx7709Config,
    symbols: &[String],
    batch_size: usize,
    trade_date: &str,
    fallback_quote_time: &str,
    worklist_names: &BTreeMap<String, String>,
    gateway_addr: &str,
    current_token: Option<&str>,
    requested_worker_count: usize,
    session_cache: &Mutex<BTreeMap<String, Tdx7709Session>>,
    reuse_sessions: bool,
    gateway_protocol: &mut GatewayPublishProtocol,
    lane: GatewayPublishLane,
) -> Result<FullPushStageResult, ApiError> {
    let started_at = Instant::now();
    let batches = split_full_push_batches(symbols, batch_size)?;
    if batches.is_empty() {
        return Ok(FullPushStageResult {
            batch_count: 0,
            worker_count: 0,
            elapsed_ms: started_at.elapsed().as_millis(),
            slowest_batch_ms: 0,
            total_batch_elapsed_ms: 0,
            published_count: 0,
            unchanged_count: 0,
            missing_codes: Vec::new(),
            last_gateway_response: None,
        });
    }
    let batch_count = batches.len();
    let worker_count = validated_full_push_worker_count(requested_worker_count, batch_count)?;
    let shards = shard_full_push_batches(batches, worker_count);
    let worker_results = std::thread::scope(|scope| {
        let handles = shards
            .into_iter()
            .enumerate()
            .map(|(worker_index, shard)| {
                scope.spawn(move || {
                    publish_full_push_worker(
                        stage,
                        worker_index,
                        config,
                        shard,
                        batch_count,
                        trade_date,
                        fallback_quote_time,
                        worklist_names,
                        gateway_addr,
                        current_token,
                        session_cache,
                        reuse_sessions,
                        lane,
                    )
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| {
                handle.join().map_err(|_| {
                    ApiError::internal(format!("7709 {stage} full-push worker panicked"))
                })?
            })
            .collect::<Result<Vec<_>, ApiError>>()
    })?;

    let mut result = FullPushStageResult {
        batch_count,
        worker_count,
        elapsed_ms: started_at.elapsed().as_millis(),
        slowest_batch_ms: 0,
        total_batch_elapsed_ms: 0,
        published_count: 0,
        unchanged_count: 0,
        missing_codes: Vec::new(),
        last_gateway_response: None,
    };
    for (worker, protocol) in worker_results {
        result.published_count = result
            .published_count
            .saturating_add(worker.published_count);
        result.unchanged_count = result
            .unchanged_count
            .saturating_add(worker.unchanged_count);
        result.total_batch_elapsed_ms = result
            .total_batch_elapsed_ms
            .saturating_add(worker.total_batch_elapsed_ms);
        result.slowest_batch_ms = result.slowest_batch_ms.max(worker.slowest_batch_ms);
        result.missing_codes.extend(worker.missing_codes);
        if worker.last_gateway_response.is_some() {
            result.last_gateway_response = worker.last_gateway_response;
        }
        merge_gateway_protocol(gateway_protocol, protocol);
    }
    result.elapsed_ms = started_at.elapsed().as_millis();
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn publish_full_push_worker(
    stage: &str,
    worker_index: usize,
    config: &Tdx7709Config,
    batches: Vec<(usize, Vec<String>)>,
    total_batch_count: usize,
    trade_date: &str,
    fallback_quote_time: &str,
    worklist_names: &BTreeMap<String, String>,
    gateway_addr: &str,
    current_token: Option<&str>,
    cache: &Mutex<BTreeMap<String, Tdx7709Session>>,
    reuse_session: bool,
    lane: GatewayPublishLane,
) -> Result<(FullPushStageResult, GatewayPublishProtocol), ApiError> {
    let started_at = Instant::now();
    let session_key = format!(
        "{stage}:{}:{}:worker-{worker_index}",
        config.host, config.port
    );
    let mut session = take_full_push_session(cache, &session_key);
    if session.is_none() {
        session = Some(Tdx7709Session::open(config).map_err(|err| {
            ApiError::internal(format!(
                "open 7709 {stage} full-push worker {worker_index} session {} failed: {err}",
                config.host
            ))
        })?);
    }

    let mut protocol = GatewayPublishProtocol::Auto;
    let worker_result = (|| -> Result<FullPushStageResult, ApiError> {
        let mut result = FullPushStageResult {
            batch_count: batches.len(),
            worker_count: 1,
            elapsed_ms: 0,
            slowest_batch_ms: 0,
            total_batch_elapsed_ms: 0,
            published_count: 0,
            unchanged_count: 0,
            missing_codes: Vec::new(),
            last_gateway_response: None,
        };

        for (index, symbols) in &batches {
            let batch_started_at = Instant::now();
            let normalized = symbols
                .iter()
                .map(|symbol| normalize_live_quote_symbol(symbol))
                .collect::<Result<Vec<_>, _>>()
                .map_err(ApiError::internal)?;
            let request_items = normalized
                .iter()
                .map(|item| Tdx7709QuoteRequestItem {
                    market: item.market,
                    code: item.code.clone(),
                    token: 0,
                })
                .collect::<Vec<_>>();
            let quote_result = retry_full_push_with_reopen(
                3,
                &mut session,
                || Tdx7709Session::open(config),
                |session| session.request_live_quotes(&request_items),
            )
            .map_err(|err| {
                ApiError::internal(format!(
                    "7709 {stage} full-push batch {}/{} failed after 3 fresh-session attempts: {err}",
                    index + 1,
                    total_batch_count
                ))
            })?;
            let mut quotes = compact_quotes_response_from_result(&normalized, &quote_result);
            for quote in &mut quotes.data {
                if quote.quote_datetime.is_none() {
                    quote.quote_datetime = Some(fallback_quote_time.to_string());
                    quote.datetime = quote.quote_datetime.clone();
                }
            }
            apply_worklist_names(&mut quotes.data, worklist_names);
            let (publishable, batch_missing) =
                partition_hqw_quotes(quotes.data, quotes.missing_codes);
            result.missing_codes.extend(batch_missing);
            let (publishable, batch_unchanged) =
                select_new_full_push_quotes(full_push_last_published_at(), trade_date, publishable);
            result.unchanged_count = result.unchanged_count.saturating_add(batch_unchanged);
            if publishable.is_empty() {
                let batch_elapsed_ms = batch_started_at.elapsed().as_millis();
                result.total_batch_elapsed_ms = result
                    .total_batch_elapsed_ms
                    .saturating_add(batch_elapsed_ms);
                result.slowest_batch_ms = result.slowest_batch_ms.max(batch_elapsed_ms);
                continue;
            }
            let current_payload = build_netzip_rust_7709_quote_batch(&publishable, trade_date)?;
            result.last_gateway_response = Some(post_quote_gateway_batch_auto(
                gateway_addr,
                current_token,
                &current_payload,
                &mut protocol,
                lane,
            )?);
            record_published_full_push_quotes(
                full_push_last_published_at(),
                trade_date,
                &publishable,
            );
            result.published_count += publishable.len();
            let batch_elapsed_ms = batch_started_at.elapsed().as_millis();
            result.total_batch_elapsed_ms = result
                .total_batch_elapsed_ms
                .saturating_add(batch_elapsed_ms);
            result.slowest_batch_ms = result.slowest_batch_ms.max(batch_elapsed_ms);
        }
        result.elapsed_ms = started_at.elapsed().as_millis();
        Ok(result)
    })();

    return_full_push_session(cache, &session_key, session, reuse_session);
    worker_result.map(|result| (result, protocol))
}

fn merge_gateway_protocol(current: &mut GatewayPublishProtocol, candidate: GatewayPublishProtocol) {
    if matches!(candidate, GatewayPublishProtocol::NetzipRust7709Tcp)
        || matches!(current, GatewayPublishProtocol::Auto)
    {
        *current = candidate;
    }
}

fn apply_worklist_names(quotes: &mut [CompactQuote], worklist_names: &BTreeMap<String, String>) {
    for quote in quotes {
        if let Some(name) = worklist_names.get(&quote.symbol) {
            quote.name = Some(name.clone());
        }
    }
}

fn get_gateway_worklist(gateway_addr: &str, limit: usize) -> Result<serde_json::Value, ApiError> {
    let path =
        format!("/api/codes/worklist?include_unknown=true&include_halted=false&limit={limit}");
    let mut stream = TcpStream::connect(gateway_addr).map_err(|err| {
        ApiError::internal(format!(
            "connect quote-gateway {gateway_addr} for worklist failed: {err}"
        ))
    })?;
    stream
        .set_read_timeout(Some(netzip_rust_tcp_ack_timeout()))
        .map_err(|err| ApiError::internal(format!("set gateway read timeout failed: {err}")))?;
    write!(
        stream,
        "GET {path} HTTP/1.0\r\nHost: {gateway_addr}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|err| ApiError::internal(format!("write gateway worklist request failed: {err}")))?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|err| ApiError::internal(format!("read gateway worklist failed: {err}")))?;
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| ApiError::internal("invalid quote-gateway worklist HTTP response"))?;
    let head = String::from_utf8_lossy(&response[..split]);
    if !head
        .lines()
        .next()
        .is_some_and(|line| line.contains(" 200 "))
    {
        return Err(ApiError::internal(format!(
            "quote-gateway worklist failed: {}",
            head.lines().next().unwrap_or("unknown status")
        )));
    }
    serde_json::from_slice(&response[split + 4..])
        .map_err(|err| ApiError::internal(format!("parse quote-gateway worklist failed: {err}")))
}

fn build_netzip_rust_7709_quote_batch(
    quotes: &[CompactQuote],
    trade_date: &str,
) -> Result<serde_json::Value, ApiError> {
    let mut payload = build_quote_gateway_quote_batch(
        quotes,
        trade_date,
        "netzipRust7709.quote_batch.v1",
        "netzipRust7709",
    )?;
    let first = quotes
        .first()
        .map(|quote| quote.symbol.as_str())
        .unwrap_or("empty");
    let last = quotes
        .last()
        .map(|quote| quote.symbol.as_str())
        .unwrap_or("empty");
    let time = quotes
        .first()
        .and_then(|quote| quote.quote_datetime.as_deref())
        .unwrap_or("unknown");
    payload["batch_id"] =
        serde_json::json!(format!("netzip-rust-{trade_date}-{first}-{last}-{time}"));
    Ok(payload)
}

fn build_quote_gateway_quote_batch(
    quotes: &[CompactQuote],
    trade_date: &str,
    schema: &str,
    source: &str,
) -> Result<serde_json::Value, ApiError> {
    let mut rows = Vec::with_capacity(quotes.len());
    for quote in quotes {
        let volume = quote.volume.ok_or_else(|| {
            ApiError::internal(format!("0547 quote {} has no decoded volume", quote.symbol))
        })?;
        let amount = quote.amount.ok_or_else(|| {
            ApiError::internal(format!("0547 quote {} has no decoded amount", quote.symbol))
        })?;
        let quote_time = quote.quote_datetime.as_deref().ok_or_else(|| {
            ApiError::internal(format!("0547 quote {} has no decoded time", quote.symbol))
        })?;
        rows.push(serde_json::json!({
            "market": quote.market,
            "code": quote.code,
            "name": quote.name.as_deref().unwrap_or(""),
            "datetime": format!("{trade_date} {quote_time}"),
            "price": quote.price,
            "last_close": quote.last_close,
            "open": quote.open,
            "high": quote.high,
            "low": quote.low,
            "volume": volume,
            "amount": amount,
            "source_protocol": "netzip-rust-7709-0547.v3"
        }));
    }
    Ok(serde_json::json!({
        "schema": schema,
        "event": "quote_batch",
        "source": source,
        "quotes": rows
    }))
}

struct GatewayHttpResponse {
    status: u16,
    status_line: String,
    body: Vec<u8>,
}

fn post_quote_gateway_batch_auto(
    gateway_addr: &str,
    current_token: Option<&str>,
    current_payload: &serde_json::Value,
    protocol: &mut GatewayPublishProtocol,
    lane: GatewayPublishLane,
) -> Result<serde_json::Value, ApiError> {
    let transport =
        std::env::var("NETZIP_QUOTE_GATEWAY_TRANSPORT").unwrap_or_else(|_| "tcp".to_string());
    let tcp_addr = std::env::var("NETZIP_QUOTE_GATEWAY_NETZIP_RUST_7709_TCP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:16889".to_string());
    post_quote_gateway_batch_with_tcp(
        gateway_addr,
        current_token,
        current_payload,
        protocol,
        !transport.eq_ignore_ascii_case("http"),
        &tcp_addr,
        lane,
    )
}

fn post_quote_gateway_batch_with_tcp(
    gateway_addr: &str,
    current_token: Option<&str>,
    current_payload: &serde_json::Value,
    protocol: &mut GatewayPublishProtocol,
    prefer_tcp: bool,
    tcp_addr: &str,
    lane: GatewayPublishLane,
) -> Result<serde_json::Value, ApiError> {
    record_gateway_publish_metric(lane, |metrics| metrics.attempts += 1);
    if prefer_tcp {
        let client_slot = netzip_rust_tcp_client_slot(lane, tcp_addr);
        let tcp_result = {
            let mut cached = client_slot.lock().unwrap_or_else(|err| err.into_inner());
            if cached.as_ref().is_none_or(|client| client.addr != tcp_addr) {
                *cached = match NetzipRustTcpClient::connect(tcp_addr) {
                    Ok(client) => Some(client),
                    Err(error) => {
                        eprintln!(
                            "gateway publish lane={} transport=tcp_msgpack connect failed: {error:?}",
                            lane.label()
                        );
                        record_gateway_publish_metric(lane, |metrics| {
                            metrics.tcp_failures += 1;
                            metrics.last_error = Some(format!("{error:?}"));
                        });
                        None
                    }
                };
            }
            cached.as_mut().map(|client| client.send(current_payload))
        };
        if let Some(result) = tcp_result {
            match result {
                Ok(result) => {
                    record_gateway_publish_metric(lane, |metrics| {
                        metrics.tcp_successes += 1;
                        metrics.successes += 1;
                        metrics.consecutive_failures = 0;
                        metrics.last_transport = Some("tcp_msgpack".to_string());
                        metrics.last_error = None;
                    });
                    *protocol = GatewayPublishProtocol::NetzipRust7709Tcp;
                    let saved_bytes = result.json_bytes.saturating_sub(result.wire_bytes);
                    let saved_percent = if result.json_bytes == 0 {
                        0.0
                    } else {
                        saved_bytes as f64 * 100.0 / result.json_bytes as f64
                    };
                    return Ok(serde_json::json!({
                        "success": true,
                        "source": "netzipRust7709",
                        "transport": "tcp_msgpack",
                        "sequence": result.ack.sequence,
                        "market_quotes_applied": result.ack.applied,
                        "cached_quotes": result.ack.cached_quotes,
                        "wire_bytes": result.wire_bytes,
                        "json_bytes": result.json_bytes,
                        "saved_bytes": saved_bytes,
                        "saved_percent": saved_percent
                    }));
                }
                Err(error) => {
                    eprintln!(
                        "gateway publish lane={} transport=tcp_msgpack failed: {error:?}",
                        lane.label()
                    );
                    *client_slot.lock().unwrap_or_else(|err| err.into_inner()) = None;
                    record_gateway_publish_metric(lane, |metrics| {
                        metrics.tcp_failures += 1;
                        metrics.last_error = Some(format!("{error:?}"));
                    });
                }
            }
        }
    }
    record_gateway_publish_metric(lane, |metrics| {
        metrics.http_fallbacks += 1;
        metrics.last_transport = Some("http_json".to_string());
    });
    let response = post_quote_gateway_json(
        gateway_addr,
        current_token,
        "/api/source/netzipRust7709/ingest",
        current_payload,
    );
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            record_gateway_publish_metric(lane, |metrics| {
                metrics.failures += 1;
                metrics.consecutive_failures += 1;
                metrics.last_error = Some(format!("{error:?}"));
            });
            return Err(error);
        }
    };
    if !(200..300).contains(&response.status) {
        let error = quote_gateway_http_error("netzipRust7709", &response);
        record_gateway_publish_metric(lane, |metrics| {
            metrics.failures += 1;
            metrics.consecutive_failures += 1;
            metrics.last_error = Some(format!("{error:?}"));
        });
        return Err(error);
    }
    *protocol = GatewayPublishProtocol::NetzipRust7709;
    let parsed = parse_quote_gateway_success(response)?;
    record_gateway_publish_metric(lane, |metrics| {
        metrics.successes += 1;
        metrics.consecutive_failures = 0;
        metrics.last_error = None;
    });
    Ok(parsed)
}

fn post_quote_gateway_json(
    gateway_addr: &str,
    token: Option<&str>,
    path: &str,
    payload: &serde_json::Value,
) -> Result<GatewayHttpResponse, ApiError> {
    let body = serde_json::to_vec(payload)
        .map_err(|err| ApiError::internal(format!("serialize quote batch failed: {err}")))?;
    let mut stream = TcpStream::connect(gateway_addr).map_err(|err| {
        ApiError::internal(format!(
            "connect quote-gateway {gateway_addr} failed: {err}"
        ))
    })?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|err| ApiError::internal(format!("set gateway read timeout failed: {err}")))?;
    let authorization = token
        .map(|value| format!("Authorization: Bearer {value}\r\n"))
        .unwrap_or_default();
    write!(
        stream,
        "POST {path} HTTP/1.0\r\nHost: {gateway_addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{authorization}Connection: close\r\n\r\n",
        body.len()
    )
    .and_then(|_| stream.write_all(&body))
    .map_err(|err| ApiError::internal(format!("write hqw batch failed: {err}")))?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|err| ApiError::internal(format!("read hqw ingest response failed: {err}")))?;
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| ApiError::internal("invalid quote-gateway HTTP response"))?;
    let head = String::from_utf8_lossy(&response[..split]);
    let status_line = head.lines().next().unwrap_or("unknown status").to_string();
    let status = status_line
        .split_ascii_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| {
            ApiError::internal(format!("invalid quote-gateway status: {status_line}"))
        })?;
    Ok(GatewayHttpResponse {
        status,
        status_line,
        body: response[split + 4..].to_vec(),
    })
}

fn parse_quote_gateway_success(
    response: GatewayHttpResponse,
) -> Result<serde_json::Value, ApiError> {
    serde_json::from_slice(&response.body).map_err(|err| {
        ApiError::internal(format!("parse quote-gateway ingest response failed: {err}"))
    })
}

fn quote_gateway_http_error(protocol: &str, response: &GatewayHttpResponse) -> ApiError {
    ApiError::internal(format!(
        "quote-gateway {protocol} ingest failed: {}",
        response.status_line
    ))
}

fn resolve_kline_category(category: Option<u16>, kline_type: Option<&str>) -> Result<u16, String> {
    if let Some(category) = category {
        return match category {
            0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 => Ok(category),
            _ => Err(format!("unsupported kline category {category}")),
        };
    }

    let normalized = kline_type.unwrap_or("1d").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "5m" => Ok(0),
        "15m" => Ok(1),
        "30m" => Ok(2),
        "60m" | "1h" => Ok(3),
        "1d" | "day" | "daily" => Ok(4),
        "1w" | "week" | "weekly" => Ok(5),
        "1mo" | "month" | "monthly" => Ok(6),
        "1m" | "1min" | "minute" => Ok(7),
        "1m-alt" => Ok(8),
        "1d-alt" => Ok(9),
        "1q" | "quarter" | "quarterly" => Ok(10),
        "1y" | "year" | "yearly" => Ok(11),
        _ => Err(format!("unsupported kline_type {normalized}")),
    }
}

fn describe_kline_category(category: u16) -> &'static str {
    match category {
        0 => "5m",
        1 => "15m",
        2 => "30m",
        3 => "60m",
        4 => "1d",
        5 => "1w",
        6 => "1mo",
        7 => "1m",
        8 => "1m-alt",
        9 => "1d-alt",
        10 => "1q",
        11 => "1y",
        _ => "unknown",
    }
}

fn tdx7709_kline_bar_preview(bar: &netzipapi_rust_demo::Tdx7709KlineBar) -> Tdx7709KlineBarPreview {
    Tdx7709KlineBarPreview {
        symbol: format!(
            "{}{}",
            tdx_0547_market_name(bar.market).unwrap_or("NA"),
            bar.code
        ),
        market: bar.market,
        market_name: tdx_0547_market_name(bar.market).unwrap_or("NA").to_string(),
        code: bar.code.clone(),
        category: bar.category,
        kline_type: describe_kline_category(bar.category).to_string(),
        datetime: bar.datetime.clone(),
        open: bar.open,
        high: bar.high,
        low: bar.low,
        close: bar.close,
        volume: bar.volume,
        amount: bar.amount,
    }
}

fn tdx7709_f10_category_preview(
    category: &netzipapi_rust_demo::Tdx7709F10Category,
) -> Tdx7709F10CategoryPreview {
    Tdx7709F10CategoryPreview {
        name: category.name.clone(),
        filename: category.filename.clone(),
        start: category.start,
        length: category.length,
    }
}

fn infer_live_quote_market(code: &str) -> Option<u8> {
    if code.len() != 6 || !code.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    if code.starts_with('4')
        || code.starts_with('8')
        || code.starts_with("920")
        || code.starts_with("430")
    {
        return Some(2);
    }
    if code.starts_with("00")
        || code.starts_with("12")
        || code.starts_with("15")
        || code.starts_with("16")
        || code.starts_with("18")
        || code.starts_with("20")
        || code.starts_with("30")
    {
        return Some(0);
    }
    if code.starts_with("11")
        || code.starts_with("13")
        || code.starts_with("50")
        || code.starts_with("51")
        || code.starts_with("52")
        || code.starts_with("56")
        || code.starts_with("58")
        || code.starts_with("60")
        || code.starts_with("68")
        || code.starts_with('5')
        || code.starts_with('6')
        || code.starts_with('9')
    {
        return Some(1);
    }
    None
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        Auth7100PathFilterRequest, CompactQuote, GatewayPublishLane, GatewayPublishProtocol,
        HqwPublishWorklistResponse, Quote0547CodeTableInfo, Quote0547ScopedRecord,
        apply_worklist_names, augment_live_quote_symbols, build_bj_poll_plan,
        build_netzip_rust_7709_quote_batch, build_quote_0547_code_table_lookup,
        cache_full_push_session, collect_quote_0547_input_paths,
        compact_quotes_response_from_result, filter_auth_7100_flow_matrix,
        filter_auth_7100_shell_correlation, infer_live_quote_market, infer_name_keyword_tag,
        infer_name_keyword_tags, linux_native_endpoints, linux_phase_skipped,
        normalize_live_quote_symbol, parse_compact_quote_codes, parse_gateway_worklist,
        partition_hqw_quotes, post_quote_gateway_batch_with_tcp, pure_rust_linux_hard_blockers,
        quote_0547_pattern_subbucket, quote_0547_quote_head_state_label,
        quote_0547_state_matrix_label, quote_0547_time_presence_label,
        quote_decimal_point_fallback, quote_frame_scan_summary, recommended_delivery_tracks,
        record_published_full_push_quotes, retry_full_push_batch, retry_full_push_with_reopen,
        return_full_push_session, select_new_full_push_quotes, shard_full_push_batches,
        split_full_push_batches, split_full_push_upstreams, stable_endpoints,
        take_full_push_session, tdx_0547_public_time_hhmmss, top_scoped_source_groups,
        validated_full_push_worker_count,
    };

    #[test]
    fn full_push_workers_are_bounded_by_batches_and_safety_limit() {
        assert_eq!(validated_full_push_worker_count(4, 58).unwrap(), 4);
        assert_eq!(validated_full_push_worker_count(4, 2).unwrap(), 2);
        assert_eq!(validated_full_push_worker_count(4, 0).unwrap(), 0);
        assert!(validated_full_push_worker_count(0, 58).is_err());
        assert!(validated_full_push_worker_count(17, 58).is_err());
    }

    #[test]
    fn full_push_batches_are_balanced_across_workers() {
        let batches = (0..10)
            .map(|index| vec![format!("SH{index:06}")])
            .collect::<Vec<_>>();

        let shards = shard_full_push_batches(batches, 4);

        assert_eq!(
            shards.iter().map(Vec::len).collect::<Vec<_>>(),
            [3, 3, 2, 2]
        );
        assert_eq!(
            shards[0]
                .iter()
                .map(|(index, _)| *index)
                .collect::<Vec<_>>(),
            [0, 4, 8]
        );
        assert_eq!(
            shards[1]
                .iter()
                .map(|(index, _)| *index)
                .collect::<Vec<_>>(),
            [1, 5, 9]
        );
    }

    #[test]
    fn full_push_result_calls_unreturned_quotes_no_current_quote() {
        let response = HqwPublishWorklistResponse {
            success: true,
            gateway_addr: "127.0.0.1:16886".to_string(),
            gateway_protocol: "netzipRust7709.tcp.msgpack.zstd.v2",
            trade_date: "2026-07-27".to_string(),
            worklist_count: 2,
            batch_size: 100,
            worker_count: 4,
            batch_count: 1,
            primary_batch_count: 1,
            primary_worker_count: 1,
            primary_elapsed_ms: 125,
            primary_slowest_batch_ms: 120,
            fallback_batch_count: 0,
            fallback_worker_count: 0,
            fallback_elapsed_ms: 0,
            fallback_slowest_batch_ms: 0,
            elapsed_ms: 126,
            fallback_host: "127.0.0.1".to_string(),
            fallback_published_count: 0,
            published_count: 1,
            unchanged_count: 0,
            no_current_quote_count: 1,
            no_current_quote_codes: vec!["SZ002036".to_string()],
            last_gateway_response: None,
            gateway_publish_metrics: BTreeMap::new(),
        };

        let json = serde_json::to_value(response).expect("serialize full-push response");
        assert_eq!(json["no_current_quote_count"], 1);
        assert_eq!(
            json["no_current_quote_codes"],
            serde_json::json!(["SZ002036"])
        );
        assert!(json.get("missing_count").is_none());
        assert!(json.get("missing_codes").is_none());
        assert_eq!(json["worker_count"], 4);
        assert_eq!(json["primary_worker_count"], 1);
        assert_eq!(json["elapsed_ms"], 126);
    }

    #[test]
    fn full_push_sends_only_quotes_with_a_newer_source_time() {
        let cache = std::sync::Mutex::new(BTreeMap::new());
        let make_quote = |datetime: &str| CompactQuote {
            code: "600000".to_string(),
            symbol: "SH600000".to_string(),
            market: "SH".to_string(),
            name: Some("浦发银行".to_string()),
            price: 9.13,
            last_close: 9.05,
            open: 9.12,
            high: 9.20,
            low: 9.04,
            volume: Some(439_348.0),
            amount: Some(400_946_048.0),
            datetime: Some(datetime.to_string()),
            quote_datetime: Some(datetime.to_string()),
            source: "netzip-rust-7709",
        };

        let (first, unchanged) =
            select_new_full_push_quotes(&cache, "2026-07-28", vec![make_quote("11:30:00")]);
        assert_eq!(first.len(), 1);
        assert_eq!(unchanged, 0);
        record_published_full_push_quotes(&cache, "2026-07-28", &first);

        let (duplicate, unchanged) =
            select_new_full_push_quotes(&cache, "2026-07-28", vec![make_quote("11:30:00")]);
        assert!(duplicate.is_empty());
        assert_eq!(unchanged, 1);

        let (newer, unchanged) =
            select_new_full_push_quotes(&cache, "2026-07-28", vec![make_quote("13:00:03")]);
        assert_eq!(newer.len(), 1);
        assert_eq!(unchanged, 0);

        let (next_day, unchanged) =
            select_new_full_push_quotes(&cache, "2026-07-29", vec![make_quote("09:25:00")]);
        assert_eq!(next_day.len(), 1);
        assert_eq!(unchanged, 0);
    }

    #[test]
    fn gateway_worklist_uses_market_qualified_symbols_and_required_trade_date() {
        let payload = serde_json::json!({
            "success": true,
            "payload": {
                "as_of_date": "2026-07-27",
                "last_refreshed_at": "2026-07-27 00:27:15",
                "freshness": { "required_quote_trade_date": "2026-07-24" },
                "data": [
                    { "market": 0, "code": "000001", "name": "平安银行" },
                    { "market": 1, "code": "600000", "name": "浦发银行" },
                    { "market": 2, "code": "920000", "name": "安徽凤凰" }
                ]
            }
        });

        let worklist = parse_gateway_worklist(&payload).expect("worklist");

        assert_eq!(worklist.trade_date, "2026-07-24");
        assert_eq!(worklist.fallback_quote_time, "15:00:00");
        assert_eq!(worklist.symbols, vec!["SZ000001", "SH600000", "BJ920000"]);
        assert_eq!(
            worklist.names.get("SZ000001").map(String::as_str),
            Some("平安银行")
        );
        assert_eq!(
            worklist.names.get("SH600000").map(String::as_str),
            Some("浦发银行")
        );
        assert_eq!(
            worklist.names.get("BJ920000").map(String::as_str),
            Some("安徽凤凰")
        );
    }

    #[test]
    fn full_push_batches_respect_the_0547_hundred_symbol_limit() {
        let symbols = (0..205)
            .map(|index| format!("SZ{index:06}"))
            .collect::<Vec<_>>();

        let batches = split_full_push_batches(&symbols, 100).expect("batches");

        assert_eq!(
            batches.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![100, 100, 5]
        );
        assert_eq!(batches.concat(), symbols);
    }

    #[test]
    fn full_push_keeps_valid_quotes_when_one_record_has_no_time() {
        let make_quote = |symbol: &str, quote_datetime: Option<&str>| CompactQuote {
            code: symbol[2..].to_string(),
            symbol: symbol.to_string(),
            market: symbol[..2].to_string(),
            name: None,
            price: 10.0,
            last_close: 9.9,
            open: 9.9,
            high: 10.1,
            low: 9.8,
            volume: Some(1000.0),
            amount: Some(10_000.0),
            datetime: quote_datetime.map(str::to_string),
            quote_datetime: quote_datetime.map(str::to_string),
            source: "netzip-rust-7709",
        };
        let quotes = vec![
            make_quote("SH600000", Some("15:00:00")),
            make_quote("SH600021", None),
        ];

        let (publishable, missing) = partition_hqw_quotes(quotes, Vec::new());

        assert_eq!(publishable.len(), 1);
        assert_eq!(publishable[0].symbol, "SH600000");
        assert_eq!(missing, vec!["SH600021"]);
    }

    #[test]
    fn stock_ranges_default_to_two_decimal_places_without_a_code_table_row() {
        assert_eq!(quote_decimal_point_fallback(2, "920001"), Some(2));
        assert_eq!(quote_decimal_point_fallback(1, "600238"), Some(2));
        assert_eq!(quote_decimal_point_fallback(1, "688680"), Some(2));
        assert_eq!(quote_decimal_point_fallback(0, "000001"), Some(2));
        assert_eq!(quote_decimal_point_fallback(0, "300750"), Some(2));
        assert_eq!(quote_decimal_point_fallback(1, "511010"), None);
        assert_eq!(quote_decimal_point_fallback(0, "159919"), None);
    }

    #[test]
    fn full_push_routes_beijing_symbols_to_the_fallback_upstream() {
        let symbols = vec![
            "SZ000001".to_string(),
            "SH600000".to_string(),
            "BJ920000".to_string(),
            "BJ920001".to_string(),
        ];

        let (primary, fallback) = split_full_push_upstreams(&symbols);

        assert_eq!(primary, vec!["SZ000001", "SH600000"]);
        assert_eq!(fallback, vec!["BJ920000", "BJ920001"]);
    }

    #[test]
    fn native_push_plans_beijing_polling_as_four_three_second_workers() {
        let symbols = (0..331)
            .map(|index| format!("BJ{index:06}"))
            .collect::<Vec<_>>();

        let plan = build_bj_poll_plan(&symbols, std::time::Duration::from_secs(3))
            .expect("beijing poll plan");

        assert_eq!(plan.symbols, symbols);
        assert_eq!(plan.batch_count, 4);
        assert_eq!(plan.worker_count, 4);
        assert_eq!(plan.interval, std::time::Duration::from_secs(3));
    }

    #[test]
    fn full_push_uses_authoritative_worklist_name_when_upstream_name_is_missing() {
        let mut quotes = vec![CompactQuote {
            code: "920000".to_string(),
            symbol: "BJ920000".to_string(),
            market: "BJ".to_string(),
            name: None,
            price: 14.18,
            last_close: 13.20,
            open: 13.20,
            high: 14.20,
            low: 13.10,
            volume: Some(49_996.0),
            amount: Some(69_078_304.0),
            datetime: Some("15:00:02".to_string()),
            quote_datetime: Some("15:00:02".to_string()),
            source: "netzip-rust-7709",
        }];
        let names = BTreeMap::from([("BJ920000".to_string(), "安徽凤凰".to_string())]);

        apply_worklist_names(&mut quotes, &names);
        let payload = build_netzip_rust_7709_quote_batch(&quotes, "2026-07-24").expect("payload");

        assert_eq!(payload["quotes"][0]["name"], "安徽凤凰");
    }

    #[test]
    fn quote_gateway_publisher_uses_only_the_netzip_rust_7709_route() {
        fn read_request(stream: &mut std::net::TcpStream) -> String {
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let count = stream.read(&mut chunk).unwrap();
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..count]);
                let Some(split) = bytes.windows(4).position(|part| part == b"\r\n\r\n") else {
                    continue;
                };
                let head = String::from_utf8_lossy(&bytes[..split]);
                let content_length = head
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length: "))
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0);
                if bytes.len() >= split + 4 + content_length {
                    break;
                }
            }
            String::from_utf8(bytes).unwrap()
        }

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            assert!(request.starts_with("POST /api/source/netzipRust7709/ingest HTTP/1.0"));
            assert!(request.contains("netzipRust7709.quote_batch.v1"));
            assert!(!request.contains("quoteNetzipWine.quote_batch.v1"));
            assert!(!request.contains("hqw.quote_batch.v1"));
            let body = r#"{"success":true}"#;
            write!(
                stream,
                "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let current = serde_json::json!({
            "schema": "netzipRust7709.quote_batch.v1",
            "event": "quote_batch",
            "source": "netzipRust7709",
            "batch_id": "test-batch",
            "quotes": []
        });
        let mut protocol = GatewayPublishProtocol::Auto;
        let response = post_quote_gateway_batch_with_tcp(
            &addr,
            None,
            &current,
            &mut protocol,
            true,
            "127.0.0.1:1",
            GatewayPublishLane::Manual,
        )
        .expect("netzipRust7709 HTTP fallback publish");
        assert_eq!(protocol, GatewayPublishProtocol::NetzipRust7709);
        assert_eq!(response["success"], true);
        server.join().unwrap();
    }

    use netzipapi_rust_demo::{
        Auth7100FlowMatrix, Auth7100FlowSession, Auth7100ShellCorrelationAnalysis,
        Auth7100ShellDirectionAnalysis, Auth7100ShellSessionAnalysis, Client10FrameSummary,
        QuoteFrameScanResult, Tdx0547Body, Tdx0547QuoteHead, Tdx0547Record, Tdx7709CodeTableRecord,
        Tdx7709LiveQuoteResult,
    };

    fn sample_auth_7100_path_filter_request() -> Auth7100PathFilterRequest {
        Auth7100PathFilterRequest {
            path: "/tmp/ignored.pcap".to_string(),
            local_endpoint: Some("192.168.3.38:14717".to_string()),
            session_role: Some("auth_login".to_string()),
            source_endpoint: Some("39.108.103.69:7100".to_string()),
            destination_endpoint: Some("192.168.3.38:14717".to_string()),
        }
    }

    #[test]
    fn filter_auth_7100_shell_correlation_keeps_requested_direction_only() {
        let analysis = Auth7100ShellCorrelationAnalysis {
            pcap_path: "/tmp/sample.pcap".to_string(),
            server_endpoint: "39.108.103.69:7100".to_string(),
            sessions: vec![
                Auth7100ShellSessionAnalysis {
                    local_endpoint: "192.168.3.38:14717".to_string(),
                    session_role: "auth_login".to_string(),
                    started_with_handshake: true,
                    directions: vec![
                        Auth7100ShellDirectionAnalysis {
                            source_endpoint: "192.168.3.38:14717".to_string(),
                            destination_endpoint: "39.108.103.69:7100".to_string(),
                            tcp_payload_bytes: 1434,
                            packets: Vec::new(),
                            packet_matches: Vec::new(),
                            findings: vec!["client".to_string()],
                        },
                        Auth7100ShellDirectionAnalysis {
                            source_endpoint: "39.108.103.69:7100".to_string(),
                            destination_endpoint: "192.168.3.38:14717".to_string(),
                            tcp_payload_bytes: 2649,
                            packets: Vec::new(),
                            packet_matches: Vec::new(),
                            findings: vec!["server".to_string()],
                        },
                    ],
                },
                Auth7100ShellSessionAnalysis {
                    local_endpoint: "192.168.3.38:2697".to_string(),
                    session_role: "auth_login".to_string(),
                    started_with_handshake: true,
                    directions: Vec::new(),
                },
            ],
            findings: Vec::new(),
        };

        let filtered =
            filter_auth_7100_shell_correlation(analysis, &sample_auth_7100_path_filter_request());
        assert_eq!(filtered.sessions.len(), 1);
        assert_eq!(filtered.sessions[0].local_endpoint, "192.168.3.38:14717");
        assert_eq!(filtered.sessions[0].directions.len(), 1);
        assert_eq!(
            filtered.sessions[0].directions[0].source_endpoint,
            "39.108.103.69:7100"
        );
        assert_eq!(filtered.findings, vec!["server".to_string()]);
    }

    #[test]
    fn filter_auth_7100_flow_matrix_keeps_requested_session_only() {
        let analysis = Auth7100FlowMatrix {
            pcap_path: "/tmp/sample.pcap".to_string(),
            server_endpoint: "39.108.103.69:7100".to_string(),
            sessions: vec![
                Auth7100FlowSession {
                    local_endpoint: "192.168.3.38:14717".to_string(),
                    server_endpoint: "39.108.103.69:7100".to_string(),
                    started_with_handshake: true,
                    client_unique_packets: 4,
                    server_unique_packets: 4,
                    client_tcp_payload_bytes: 1434,
                    server_tcp_payload_bytes: 2649,
                    session_role: "auth_login".to_string(),
                    client_packets: Vec::new(),
                    server_packets: Vec::new(),
                },
                Auth7100FlowSession {
                    local_endpoint: "192.168.3.38:2695".to_string(),
                    server_endpoint: "39.108.103.69:7100".to_string(),
                    started_with_handshake: true,
                    client_unique_packets: 1,
                    server_unique_packets: 1,
                    client_tcp_payload_bytes: 419,
                    server_tcp_payload_bytes: 345,
                    session_role: "auth_probe".to_string(),
                    client_packets: Vec::new(),
                    server_packets: Vec::new(),
                },
            ],
        };

        let filtered =
            filter_auth_7100_flow_matrix(analysis, &sample_auth_7100_path_filter_request());
        assert_eq!(filtered.sessions.len(), 1);
        assert_eq!(filtered.sessions[0].local_endpoint, "192.168.3.38:14717");
        assert_eq!(filtered.sessions[0].session_role, "auth_login");
    }

    #[test]
    fn infer_name_keyword_tags_returns_multiple_safe_tags() {
        assert_eq!(
            infer_name_keyword_tags("上证指数ETF"),
            vec!["ETF".to_string(), "指数".to_string()]
        );
        assert_eq!(
            infer_name_keyword_tags("台21转债"),
            vec!["转债".to_string()]
        );
        assert_eq!(
            infer_name_keyword_tags("华夏华润商业REIT"),
            vec!["REIT".to_string()]
        );
    }

    #[test]
    fn infer_name_keyword_tag_keeps_backward_compatible_first_tag() {
        assert_eq!(
            infer_name_keyword_tag("上证指数ETF").as_deref(),
            Some("ETF")
        );
        assert_eq!(infer_name_keyword_tag("普通股票"), None);
    }

    #[test]
    fn capabilities_include_linux_first_snapshot_and_mvp_endpoints() {
        assert!(stable_endpoints().contains(&"GET /api/quotes"));
        assert!(linux_native_endpoints().contains(&"GET /api/quotes"));
        assert!(stable_endpoints().contains(&"POST /api/hqw/publish"));
        assert!(linux_native_endpoints().contains(&"POST /api/hqw/publish"));
        assert!(stable_endpoints().contains(&"POST /api/hqw/publish-worklist"));
        assert!(linux_native_endpoints().contains(&"POST /api/hqw/publish-worklist"));
        assert!(stable_endpoints().contains(&"POST /api/hqw/push-worklist"));
        assert!(linux_native_endpoints().contains(&"POST /api/hqw/push-worklist"));
        assert!(stable_endpoints().contains(&"POST /api/tdx7709/snapshot"));
        assert!(linux_native_endpoints().contains(&"POST /api/tdx7709/snapshot"));
        assert!(stable_endpoints().contains(&"POST /api/linux/pure-rust-mvp"));
        assert!(linux_native_endpoints().contains(&"POST /api/linux/pure-rust-mvp"));
    }

    #[test]
    fn legacy_webgui_uses_documented_authentication_account() {
        let html = include_str!("../../webgui/index.html");

        assert!(html.contains("id=\"legacyAccount\" value=\"168\""));
        assert!(html.contains("id=\"legacyPassword\" type=\"password\" value=\"168\""));
        assert!(!html.contains("id=\"legacyPassword\" type=\"password\" value=\"xxx\""));
    }

    #[test]
    fn compact_quote_codes_accept_market_qualified_and_unambiguous_bare_codes() {
        let codes = parse_compact_quote_codes("SH600000,SZ000001,430047")
            .expect("valid quote-gateway symbols");
        assert_eq!(
            codes
                .iter()
                .map(|item| item.symbol.as_str())
                .collect::<Vec<_>>(),
            vec!["SH600000", "SZ000001", "BJ430047"]
        );
        assert!(parse_compact_quote_codes("700000").is_err());
        assert!(parse_compact_quote_codes("SH600000,").is_err());
    }

    #[test]
    fn compact_quote_response_normalizes_prices_and_reports_missing_codes() {
        let requested = parse_compact_quote_codes("SH600000,SZ000001").unwrap();
        let mut meta = [0u8; 13];
        meta[4] = 4;
        let result = Tdx7709LiveQuoteResult {
            code_table_reply: Vec::new(),
            code_table_frames: Vec::new(),
            code_table_records: vec![Tdx7709CodeTableRecord {
                market: 1,
                code: "600000".to_string(),
                name: "浦发银行".to_string(),
                meta,
            }],
            quote_reply: Vec::new(),
            quote_frames: Vec::new(),
            quote_bodies: vec![Tdx0547Body {
                xor93_count: Some(1),
                printable_ratio: 0.0,
                records: vec![Tdx0547Record {
                    start: 0,
                    len: 0,
                    market: 1,
                    code: "600000".to_string(),
                    active1_raw: Some(1),
                    time_hhmmss_raw: Some(153050),
                    renewal_token_raw: None,
                    extra0_time_hhmmss: None,
                    extra0_raw: None,
                    extra1_raw: None,
                    extra2_raw: None,
                    extra3_raw: None,
                    volume: Some(506_751.0),
                    current_volume: Some(2_600.0),
                    amount: Some(459_285_312.0),
                    amount_raw: Some(0),
                    quote_head: Some(Tdx0547QuoteHead {
                        active1: 1,
                        price: 901.0,
                        last_close: 895.0,
                        open: 899.0,
                        high: 905.0,
                        low: 890.0,
                    }),
                }],
            }],
        };

        let response = compact_quotes_response_from_result(&requested, &result);
        assert!(response.success);
        assert!(response.partial);
        assert_eq!(response.missing_codes, vec!["SZ000001"]);
        assert_eq!(response.data[0].price, 9.01);
        assert_eq!(response.data[0].quote_datetime.as_deref(), Some("15:00:00"));
        assert_eq!(
            result.quote_bodies[0].records[0].time_hhmmss_raw,
            Some(153050)
        );

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["source"], "netzip-rust-7709");
        assert_eq!(json["data"][0]["volume"], 506_751.0);
        assert_eq!(json["data"][0]["amount"], 459_285_344.0);

        let current_batch =
            build_netzip_rust_7709_quote_batch(&response.data, "2026-07-24").unwrap();
        assert_eq!(current_batch["schema"], "netzipRust7709.quote_batch.v1");
        assert_eq!(current_batch["source"], "netzipRust7709");
        assert!(
            current_batch["batch_id"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
        );
        assert_eq!(
            current_batch["quotes"][0]["datetime"],
            "2026-07-24 15:00:00"
        );
        assert_eq!(
            current_batch["quotes"][0]["source_protocol"],
            "netzip-rust-7709-0547.v3"
        );
    }

    #[test]
    fn compact_quote_response_matches_wine_oem_public_amount() {
        let requested = parse_compact_quote_codes("SH510300").unwrap();
        let mut meta = [0u8; 13];
        meta[4] = 3;
        let result = Tdx7709LiveQuoteResult {
            code_table_reply: Vec::new(),
            code_table_frames: Vec::new(),
            code_table_records: vec![Tdx7709CodeTableRecord {
                market: 1,
                code: "510300".to_string(),
                name: "300ETF".to_string(),
                meta,
            }],
            quote_reply: Vec::new(),
            quote_frames: Vec::new(),
            quote_bodies: vec![Tdx0547Body {
                xor93_count: Some(1),
                printable_ratio: 0.0,
                records: vec![Tdx0547Record {
                    start: 0,
                    len: 0,
                    market: 1,
                    code: "510300".to_string(),
                    active1_raw: Some(5_078),
                    time_hhmmss_raw: Some(153045),
                    renewal_token_raw: None,
                    extra0_time_hhmmss: None,
                    extra0_raw: Some(-5_461),
                    extra1_raw: Some(2),
                    extra2_raw: Some(0),
                    extra3_raw: Some(930_500),
                    volume: Some(15_092_028.0),
                    current_volume: Some(175_499.0),
                    amount: Some(6_987_965_952.0),
                    amount_raw: Some(1_339_048_435),
                    quote_head: Some(Tdx0547QuoteHead {
                        active1: 5_078,
                        price: 46.57,
                        last_close: 46.27,
                        open: 46.24,
                        high: 46.86,
                        low: 45.74,
                    }),
                }],
            }],
        };

        let response = compact_quotes_response_from_result(&requested, &result);

        assert_eq!(response.data[0].amount, Some(6_988_011_008.0));
        assert_eq!(
            result.quote_bodies[0].records[0].amount,
            Some(6_987_965_952.0)
        );
        assert_eq!(
            result.quote_bodies[0].records[0].amount_raw,
            Some(1_339_048_435)
        );
    }

    #[test]
    fn compact_quote_response_matches_observed_oem_amount_modes() {
        assert_eq!(
            compact_amount_for_observed_record(1, "511010", 3, 1_408.27, 74_227.0, 1_045_378_432.0),
            Some(1_045_378_368.0)
        );
        assert_eq!(
            compact_amount_for_observed_record(0, "159919", 3, 48.57, 2_092_755.0, 1_010_141_888.0),
            Some(1_010_151_936.0)
        );
        assert_eq!(
            compact_amount_for_observed_record(1, "688001", 2, 46.16, 107_005.0, 482_548_544.0),
            Some(482_553_312.0)
        );
        assert_eq!(
            compact_amount_for_observed_record(
                0,
                "399001",
                2,
                13_658.44,
                675_056_732.0,
                1_209_171_312_640.0,
            ),
            Some(1_209_168_297_984.0)
        );
        assert_eq!(
            compact_amount_for_observed_record(0, "123064", 3, 1_101.19, 122_303.0, 134_716_736.0,),
            Some(134_716_736.0)
        );
    }

    fn compact_amount_for_observed_record(
        market: u8,
        code: &str,
        decimal_point: u8,
        price: f64,
        volume: f64,
        amount: f64,
    ) -> Option<f64> {
        let market_name = netzipapi_rust_demo::tdx_0547_market_name(market).expect("known market");
        let requested = parse_compact_quote_codes(&format!("{market_name}{code}")).unwrap();
        let mut meta = [0u8; 13];
        meta[4] = decimal_point;
        let result = Tdx7709LiveQuoteResult {
            code_table_reply: Vec::new(),
            code_table_frames: Vec::new(),
            code_table_records: vec![Tdx7709CodeTableRecord {
                market,
                code: code.to_string(),
                name: "observed".to_string(),
                meta,
            }],
            quote_reply: Vec::new(),
            quote_frames: Vec::new(),
            quote_bodies: vec![Tdx0547Body {
                xor93_count: Some(1),
                printable_ratio: 0.0,
                records: vec![Tdx0547Record {
                    start: 0,
                    len: 0,
                    market,
                    code: code.to_string(),
                    active1_raw: Some(1),
                    time_hhmmss_raw: Some(150000),
                    renewal_token_raw: None,
                    extra0_time_hhmmss: None,
                    extra0_raw: None,
                    extra1_raw: None,
                    extra2_raw: None,
                    extra3_raw: None,
                    volume: Some(volume),
                    current_volume: Some(0.0),
                    amount: Some(amount),
                    amount_raw: None,
                    quote_head: Some(Tdx0547QuoteHead {
                        active1: 1,
                        price,
                        last_close: price,
                        open: price,
                        high: price,
                        low: price,
                    }),
                }],
            }],
        };

        compact_quotes_response_from_result(&requested, &result).data[0].amount
    }

    #[test]
    fn full_push_batch_retries_transient_quote_frame_failures() {
        let mut attempts = 0usize;
        let value = retry_full_push_batch(3, || {
            attempts += 1;
            if attempts < 3 {
                Err("no zlib-decodable quote frames")
            } else {
                Ok(42)
            }
        })
        .expect("third batch attempt succeeds");

        assert_eq!(value, 42);
        assert_eq!(attempts, 3);
    }

    #[test]
    fn full_push_batch_reopens_a_failed_session_before_retrying() {
        let mut opened = 0usize;
        let mut session = None;
        let value = retry_full_push_with_reopen(
            3,
            &mut session,
            || {
                opened += 1;
                Ok::<_, &'static str>(opened)
            },
            |session| {
                if *session < 3 {
                    Err("desynchronized stream")
                } else {
                    Ok(42)
                }
            },
        )
        .expect("third fresh session succeeds");

        assert_eq!(value, 42);
        assert_eq!(opened, 3);
    }

    #[test]
    fn full_push_session_cache_reuses_only_returned_healthy_sessions() {
        let cache = std::sync::Mutex::new(BTreeMap::new());
        cache_full_push_session(&cache, "primary", 41usize);
        assert_eq!(take_full_push_session(&cache, "primary"), Some(41));
        assert_eq!(take_full_push_session(&cache, "primary"), None);

        cache_full_push_session(&cache, "primary", 42usize);
        assert_eq!(take_full_push_session(&cache, "primary"), Some(42));
    }

    #[test]
    fn audit_session_policy_does_not_return_idle_sessions_to_cache() {
        let cache = std::sync::Mutex::new(BTreeMap::new());

        return_full_push_session(&cache, "primary", Some(41usize), false);
        assert_eq!(take_full_push_session(&cache, "primary"), None);

        return_full_push_session(&cache, "primary", Some(42usize), true);
        assert_eq!(take_full_push_session(&cache, "primary"), Some(42));
    }

    #[test]
    fn code_table_lookup_keeps_same_code_markets_separate() {
        let make_record = |market, name: &str, decimal_point| {
            let mut meta = [0u8; 13];
            meta[4] = decimal_point;
            Tdx7709CodeTableRecord {
                market,
                code: "000001".to_string(),
                name: name.to_string(),
                meta,
            }
        };
        let lookup = build_quote_0547_code_table_lookup(&[
            make_record(0, "平安银行", 2),
            make_record(1, "上证指数", 3),
        ]);

        assert_eq!(lookup.get("SZ000001").unwrap().name, "平安银行");
        assert_eq!(lookup.get("SH000001").unwrap().name, "上证指数");
        assert!(!lookup.contains_key("000001"));
    }

    #[test]
    fn delivery_tracks_keep_pure_rust_linux_as_highest_priority() {
        let tracks = recommended_delivery_tracks();
        assert_eq!(
            tracks.first().map(|item| item.name),
            Some("Pure Rust + Linux partial delivery")
        );
        assert_eq!(tracks.first().map(|item| item.priority), Some("highest"));
        assert!(
            pure_rust_linux_hard_blockers()
                .iter()
                .any(|item| item.contains("7100 登录后"))
        );
    }

    #[test]
    fn linux_phase_skipped_marks_phase_as_disabled() {
        let skipped = linux_phase_skipped::<super::Tdx7709SyncResponse>("skipped by request");
        assert!(!skipped.enabled);
        assert!(skipped.skipped);
        assert!(!skipped.ok);
        assert_eq!(skipped.error.as_deref(), Some("skipped by request"));
        assert!(skipped.data.is_none());
    }

    fn sample_record() -> Tdx0547Record {
        Tdx0547Record {
            start: 0,
            len: 0,
            market: 1,
            code: "600000".to_string(),
            active1_raw: Some(1),
            time_hhmmss_raw: None,
            renewal_token_raw: None,
            extra0_raw: Some(-4721),
            extra0_time_hhmmss: Some("15:00:01".to_string()),
            extra1_raw: Some(2),
            extra2_raw: Some(0),
            extra3_raw: Some(0),
            volume: Some(392_439.0),
            current_volume: Some(120.0),
            amount: Some(393_447_296.0),
            amount_raw: Some(0),
            quote_head: Some(netzipapi_rust_demo::Tdx0547QuoteHead {
                active1: 1,
                price: 1.0,
                last_close: 1.0,
                open: 1.0,
                high: 1.0,
                low: 1.0,
            }),
        }
    }

    fn sample_lookup() -> BTreeMap<String, Quote0547CodeTableInfo> {
        BTreeMap::from([(
            "600000".to_string(),
            Quote0547CodeTableInfo {
                name: "样本股".to_string(),
                decimal_point: 2,
                pre_close: 1.0,
            },
        )])
    }

    #[test]
    fn public_time_caps_extra0_fallback_without_changing_diagnostics() {
        let mut record = sample_record();
        record.extra0_raw = Some(5_492);
        record.extra0_time_hhmmss = Some("15:30:12".to_string());

        assert_eq!(
            tdx_0547_public_time_hhmmss(&record).as_deref(),
            Some("15:00:00")
        );
        assert_eq!(record.extra0_time_hhmmss.as_deref(), Some("15:30:12"));
    }

    #[test]
    fn default_quote_head_hint_1500_splits_by_time_presence() {
        let lookup = sample_lookup();
        let mut with_time = sample_record();
        with_time.time_hhmmss_raw = Some(150001);
        assert_eq!(
            quote_0547_pattern_subbucket(&with_time, &lookup),
            "default_2_0_0_quote_head_hint_1500_time_present"
        );

        let without_time = sample_record();
        assert_eq!(
            quote_0547_pattern_subbucket(&without_time, &lookup),
            "default_2_0_0_quote_head_hint_1500_time_absent"
        );
    }

    #[test]
    fn default_quote_head_hint_1530_splits_by_time_presence() {
        let lookup = sample_lookup();
        let mut with_time = sample_record();
        with_time.time_hhmmss_raw = Some(153012);
        with_time.extra0_raw = Some(5492);
        with_time.extra0_time_hhmmss = Some("15:30:12".to_string());
        assert_eq!(
            quote_0547_pattern_subbucket(&with_time, &lookup),
            "default_2_0_0_quote_head_hint_1530_time_present"
        );

        let mut without_time = sample_record();
        without_time.extra0_raw = Some(5480);
        without_time.extra0_time_hhmmss = Some("15:30:00".to_string());
        assert_eq!(
            quote_0547_pattern_subbucket(&without_time, &lookup),
            "default_2_0_0_quote_head_hint_1530_time_absent"
        );
    }

    #[test]
    fn quote_head_and_time_presence_labels_follow_record_shape() {
        let mut with_time = sample_record();
        with_time.time_hhmmss_raw = Some(150001);
        assert_eq!(
            quote_0547_quote_head_state_label(&with_time),
            "with_quote_head"
        );
        assert_eq!(quote_0547_time_presence_label(&with_time), "time_present");
        assert_eq!(
            quote_0547_state_matrix_label(&with_time),
            "with_quote_head_time_present"
        );

        let mut without_head = sample_record();
        without_head.quote_head = None;
        assert_eq!(
            quote_0547_quote_head_state_label(&without_head),
            "without_quote_head"
        );
        assert_eq!(quote_0547_time_presence_label(&without_head), "time_absent");
        assert_eq!(
            quote_0547_state_matrix_label(&without_head),
            "without_quote_head_time_absent"
        );
    }

    #[test]
    fn infer_live_quote_market_keeps_known_prefixes_conservative() {
        assert_eq!(infer_live_quote_market("600000"), Some(1));
        assert_eq!(infer_live_quote_market("113638"), Some(1));
        assert_eq!(infer_live_quote_market("300948"), Some(0));
        assert_eq!(infer_live_quote_market("159001"), Some(0));
        assert_eq!(infer_live_quote_market("430047"), Some(2));
        assert_eq!(infer_live_quote_market("999999"), Some(1));
        assert_eq!(infer_live_quote_market("700000"), None);
    }

    #[test]
    fn normalize_live_quote_symbol_accepts_prefixed_and_inferred_inputs() {
        let sh = normalize_live_quote_symbol("600000").expect("infer sh");
        assert_eq!(sh.market, 1);
        assert_eq!(sh.code, "600000");
        assert_eq!(sh.symbol, "SH600000");

        let sz = normalize_live_quote_symbol("sz300948").expect("explicit sz");
        assert_eq!(sz.market, 0);
        assert_eq!(sz.code, "300948");
        assert_eq!(sz.symbol, "SZ300948");

        let bj = normalize_live_quote_symbol("BJ430047").expect("explicit bj");
        assert_eq!(bj.market, 2);
        assert_eq!(bj.symbol, "BJ430047");

        assert!(normalize_live_quote_symbol("700000").is_err());
        assert!(normalize_live_quote_symbol("SHABCDE1").is_err());
    }

    #[test]
    fn augment_live_quote_symbols_pads_with_seed_basket() {
        let requested = vec![normalize_live_quote_symbol("SH600004").expect("normalize")];
        let transport = augment_live_quote_symbols(&requested);
        let symbols = transport
            .iter()
            .map(|item| item.symbol.as_str())
            .collect::<Vec<_>>();
        assert_eq!(symbols[0], "SH600004");
        assert!(symbols.contains(&"SH600000"));
        assert!(symbols.len() >= 3);
    }

    #[test]
    fn collect_quote_0547_input_paths_prefers_inflated_files_in_directory() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("netzip_quote0547_test_{unique}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        let inflated = dir.join("frame001_inflated.bin");
        let other_bin = dir.join("other.bin");
        let ignored = dir.join("readme.txt");
        fs::write(&inflated, [1, 2, 3]).expect("write inflated");
        fs::write(&other_bin, [4, 5, 6]).expect("write other bin");
        fs::write(&ignored, b"x").expect("write ignored");

        let (mode, paths) = collect_quote_0547_input_paths(&dir).expect("collect paths");
        assert_eq!(mode, "directory");
        assert_eq!(paths, vec![inflated.clone()]);

        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }

    #[test]
    fn top_scoped_source_groups_summarizes_by_source_name() {
        let record_a = sample_record();
        let mut record_b = sample_record();
        record_b.code = "600004".to_string();
        let mut record_c = sample_record();
        record_c.code = "300948".to_string();
        record_c.market = 0;

        let scoped = vec![
            Quote0547ScopedRecord {
                source_path: "/tmp/a.bin",
                source_name: "a.bin",
                record: &record_a,
            },
            Quote0547ScopedRecord {
                source_path: "/tmp/a.bin",
                source_name: "a.bin",
                record: &record_b,
            },
            Quote0547ScopedRecord {
                source_path: "/tmp/b.bin",
                source_name: "b.bin",
                record: &record_c,
            },
        ];

        let top = top_scoped_source_groups(&scoped, 8);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].label, "a.bin");
        assert_eq!(top[0].count, 2);
        assert_eq!(top[0].example_codes, vec!["SH600000", "SH600004"]);
        assert_eq!(top[1].label, "b.bin");
        assert_eq!(top[1].count, 1);
        assert_eq!(top[1].example_codes, vec!["SZ300948"]);
    }

    #[test]
    fn quote_frame_scan_summary_collects_phase_counts_in_order() {
        let scanned = QuoteFrameScanResult {
            input: "/tmp/flow_7171.bin".to_string(),
            size: 0,
            mode: "client10".to_string(),
            server_frames: Vec::new(),
            client_frames: vec![
                Client10FrameSummary {
                    index: 0,
                    offset: 0,
                    label: Some("bootstrap.main-site-validate".to_string()),
                    op: 0,
                    sub: 0,
                    flags: 0,
                    payload_len: 0,
                    inner_tag: None,
                    inner_kind: None,
                    code_table_request: None,
                    body_head_hex: String::new(),
                    block8: None,
                    body_after_tag_block8: None,
                    utf16_strings: Vec::new(),
                    ascii_strings: Vec::new(),
                    numeric_tokens: Vec::new(),
                },
                Client10FrameSummary {
                    index: 1,
                    offset: 0,
                    label: Some("post-login.bulk-record-29b-request".to_string()),
                    op: 0,
                    sub: 0,
                    flags: 0,
                    payload_len: 0,
                    inner_tag: None,
                    inner_kind: None,
                    code_table_request: None,
                    body_head_hex: String::new(),
                    block8: None,
                    body_after_tag_block8: None,
                    utf16_strings: Vec::new(),
                    ascii_strings: Vec::new(),
                    numeric_tokens: Vec::new(),
                },
                Client10FrameSummary {
                    index: 2,
                    offset: 0,
                    label: Some("post-login.bulk-record-29b-request".to_string()),
                    op: 0,
                    sub: 0,
                    flags: 0,
                    payload_len: 0,
                    inner_tag: None,
                    inner_kind: None,
                    code_table_request: None,
                    body_head_hex: String::new(),
                    block8: None,
                    body_after_tag_block8: None,
                    utf16_strings: Vec::new(),
                    ascii_strings: Vec::new(),
                    numeric_tokens: Vec::new(),
                },
            ],
            head_hex: String::new(),
        };

        let summary = quote_frame_scan_summary(&scanned);
        assert_eq!(
            summary.phase_order,
            vec![
                "bootstrap.main-site-validate".to_string(),
                "post-login.bulk-record-29b-request".to_string(),
            ]
        );
        assert_eq!(
            summary
                .phase_counts
                .get("post-login.bulk-record-29b-request"),
            Some(&2)
        );
    }
}

fn code_table_preview(
    record: &netzipapi_rust_demo::Tdx7709CodeTableRecord,
) -> CodeTableRecordPreview {
    CodeTableRecordPreview {
        market: tdx_0547_market_name(record.market)
            .unwrap_or("NA")
            .to_string(),
        code: record.code.clone(),
        name: record.name.clone(),
        decimal_point: record.decimal_point(),
        pre_close: record.pre_close(),
        meta_hex: record.meta_hex(),
    }
}

fn matches_code_query(record: &netzipapi_rust_demo::Tdx7709CodeTableRecord, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }

    let query_ascii = query.to_ascii_lowercase();
    record.code.to_ascii_lowercase().contains(&query_ascii)
        || record.name.contains(query)
        || record.name.to_ascii_lowercase().contains(&query_ascii)
}

fn quote_frame_scan_summary(
    scanned: &netzipapi_rust_demo::QuoteFrameScanResult,
) -> QuoteFrameScanSummary {
    let labeled_server_frames = scanned
        .server_frames
        .iter()
        .filter(|frame| frame.label.is_some())
        .count();
    let labeled_client_frames = scanned
        .client_frames
        .iter()
        .filter(|frame| frame.label.is_some())
        .count();
    let bootstrap_labels = scanned
        .client_frames
        .iter()
        .filter_map(|frame| frame.label.as_ref())
        .take(6)
        .cloned()
        .collect::<Vec<_>>();
    let code_table_requests = scanned
        .client_frames
        .iter()
        .filter(|frame| {
            frame
                .label
                .as_deref()
                .is_some_and(|label| label.starts_with("bootstrap.code-table-chunk"))
        })
        .count();
    let mut phase_order = Vec::new();
    let mut phase_counts = BTreeMap::<String, usize>::new();
    for label in scanned
        .client_frames
        .iter()
        .filter_map(|frame| frame.label.as_deref())
        .chain(
            scanned
                .server_frames
                .iter()
                .filter_map(|frame| frame.label.as_deref()),
        )
    {
        if !phase_counts.contains_key(label) {
            phase_order.push(label.to_string());
        }
        *phase_counts.entry(label.to_string()).or_default() += 1;
    }
    let main_site_validate_after_tag_block8 = scanned
        .client_frames
        .iter()
        .find(|frame| frame.label.as_deref() == Some("bootstrap.main-site-validate"))
        .and_then(|frame| frame.body_after_tag_block8.clone());

    QuoteFrameScanSummary {
        server_frames: scanned.server_frames.len(),
        client_frames: scanned.client_frames.len(),
        labeled_server_frames,
        labeled_client_frames,
        code_table_requests,
        bootstrap_labels,
        phase_order,
        phase_counts,
        main_site_validate_after_tag_block8,
    }
}

fn humanize_snake(name: &str) -> String {
    let mut out = String::new();
    let mut capitalize = true;
    for ch in name.chars() {
        if ch == '_' {
            out.push(' ');
            capitalize = true;
            continue;
        }
        if capitalize {
            out.extend(ch.to_uppercase());
            capitalize = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn spaced_hex(input: &str) -> String {
    input
        .as_bytes()
        .chunks(2)
        .filter_map(|chunk| std::str::from_utf8(chunk).ok())
        .map(str::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

fn hex_line(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn packet_kind_name(packet: netzipapi_rust_demo::Packet) -> String {
    match packet {
        netzipapi_rust_demo::Packet::InvalidRequest { .. } => "InvalidRequest",
        netzipapi_rust_demo::Packet::CodeTable { .. } => "CodeTable",
        netzipapi_rust_demo::Packet::Realtime { .. } => "Realtime",
        netzipapi_rust_demo::Packet::Tick { .. } => "Tick",
        netzipapi_rust_demo::Packet::Trend { .. } => "Trend",
        netzipapi_rust_demo::Packet::Kline { .. } => "Kline",
        netzipapi_rust_demo::Packet::Split { .. } => "Split",
        netzipapi_rust_demo::Packet::Finance { .. } => "Finance",
        netzipapi_rust_demo::Packet::F10 { .. } => "F10",
        netzipapi_rust_demo::Packet::BuySell610 { .. } => "BuySell610",
        netzipapi_rust_demo::Packet::Unknown { .. } => "Unknown",
    }
    .to_string()
}

fn parse_probe_encoding(value: Option<&str>) -> Result<ProbeEncoding, ApiError> {
    match value.unwrap_or("utf8").trim().to_ascii_lowercase().as_str() {
        "utf8" | "utf-8" => Ok(ProbeEncoding::Utf8),
        "utf16le" | "utf-16le" | "utf16" => Ok(ProbeEncoding::Utf16Le),
        "hex" => Ok(ProbeEncoding::Hex),
        other => Err(ApiError::bad_request(format!(
            "unsupported encoding: {other}. expected utf8, utf16le, or hex"
        ))),
    }
}

fn parse_listen() -> Result<SocketAddr, Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut listen = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--listen" => listen = Some(args.next().ok_or("missing value after --listen")?),
            other => return Err(format!("unknown arg: {other}").into()),
        }
    }

    if let Some(value) = listen.or_else(|| std::env::var("NETZIP_SERVICE_LISTEN").ok()) {
        return Ok(value.parse()?);
    }

    Ok("0.0.0.0:16893".parse()?)
}

fn print_help() {
    println!("netzip_service [options]");
    println!("options:");
    println!("  --listen <ip:port>           default 0.0.0.0:16893");
    println!("env:");
    println!("  NETZIP_SERVICE_LISTEN        override listen address");
    println!("example:");
    println!("  cargo run --bin netzip_service -- --listen 0.0.0.0:16893");
}
