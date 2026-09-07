//! Runtime ownership boundary for the authenticated 5188 full-push lane.
//!
//! This module does not decode opaque business payloads.  It makes the
//! production wiring explicit: a 5188 connection can only be planned from a
//! completed 7100 login and a current-session Wine-shaped initialization.

use netzip_fullpull::{
    Official5188CodeTable, Official5188CodeTableRecord, Official5188DeltaEnvelope,
    Official5188Frame, Official5188Kind, Official5188MapBaselineResolver, Official5188OemState,
    Official5188PublicQuote, Official5188Session, decode_official_5188_delta_indexes,
    decode_official_5188_values_partial_with_fresh_fallback,
};
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Bounded raw-frame evidence retained by the official 5188 lane.
///
/// Frames are kept verbatim for later decoder/callback correlation; this
/// container deliberately does not interpret business payload bytes.
#[derive(Debug)]
pub struct Official5188FrameSink {
    frames: VecDeque<Official5188Frame>,
    max_frames: usize,
    max_bytes: usize,
    bytes: usize,
    dropped_frames: u64,
}

/// Read-only metrics for the bounded official 5188 evidence buffer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Official5188FrameSinkSnapshot {
    pub frame_count: usize,
    pub byte_count: usize,
    pub dropped_frames: u64,
    pub wire_kind_counts: BTreeMap<String, u64>,
    pub latest_wire_kind: Option<String>,
    pub latest_payload_len: Option<usize>,
    pub subscriptions: Vec<Official5188SubscriptionSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Official5188SubscriptionSummary {
    pub payload_len: usize,
    pub declared_entry_count: u32,
    pub decoded_entry_count: usize,
    pub consecutive_range_count: usize,
    pub market_entry_counts: BTreeMap<String, usize>,
    pub first_index_by_market: BTreeMap<String, u32>,
    pub last_index_by_market: BTreeMap<String, u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Official5188ShadowSnapshot {
    pub running: bool,
    pub endpoint: String,
    pub frames_received: u64,
    pub application_bytes_received: u64,
    pub delta_2704_frames: u64,
    pub bulk_3e04_frames: u64,
    pub receive_terminations: u64,
    pub decoder_seed_records: usize,
    pub decoder_attempted_frames: u64,
    pub decoder_decoded_frames: u64,
    pub decoder_partial_frames: u64,
    pub decoder_failed_frames: u64,
    pub decoder_error_kinds: BTreeMap<String, u64>,
    pub decoder_baseline_modes: BTreeMap<String, u64>,
    pub decoder_error_samples: BTreeMap<String, String>,
    pub decoder_decoded_records: u64,
    pub decoder_omitted_tail_records: u64,
    pub decoder_oem_state_symbols: usize,
    pub decoder_oem_state_updates: u64,
    pub decoder_missing_previous_close_seeds: u64,
    pub decoder_rejected_public_quotes: u64,
    pub decoder_last_error: Option<String>,
    pub last_error: Option<String>,
    pub retained: Official5188FrameSinkSnapshot,
}

#[derive(Debug)]
struct Official5188ShadowState {
    running: bool,
    endpoint: String,
    frames_received: u64,
    application_bytes_received: u64,
    delta_2704_frames: u64,
    bulk_3e04_frames: u64,
    receive_terminations: u64,
    decoder: Official5188ShadowDecoder,
    last_error: Option<String>,
    sink: Official5188FrameSink,
}

#[derive(Debug)]
struct Official5188ShadowDecoder {
    resolver: Official5188MapBaselineResolver,
    seed_records: usize,
    symbol_codes: BTreeMap<([u8; 2], u16), String>,
    previous_close_seeds: BTreeMap<([u8; 2], String), Official5188CodeTableRecord>,
    missing_previous_close_seeds: u64,
    attempted_frames: u64,
    decoded_frames: u64,
    partial_frames: u64,
    failed_frames: u64,
    decoded_records: u64,
    omitted_tail_records: u64,
    oem_states: BTreeMap<([u8; 2], u16), Official5188OemState>,
    public_quotes: BTreeMap<([u8; 2], u16), Official5188PublicQuote>,
    oem_state_updates: u64,
    rejected_public_quotes: u64,
    last_error: Option<String>,
    error_kinds: BTreeMap<String, u64>,
    baseline_modes: BTreeMap<String, u64>,
    error_samples: BTreeMap<String, String>,
}

fn decoder_error_kind(error: &str) -> String {
    let category = if error.contains("bitstream exhausted") {
        "bitstream_exhausted"
    } else if error.contains("token prefix did not match") {
        "token_prefix_mismatch"
    } else {
        "other"
    };
    let stage = error
        .split("stage=")
        .nth(1)
        .and_then(|value| value.split_whitespace().next())
        .unwrap_or("unknown");
    format!("{stage}:{category}")
}

fn decoder_baseline_mode(error: &str) -> &str {
    error
        .split("baseline_mode=")
        .nth(1)
        .and_then(|value| value.split_whitespace().next())
        .unwrap_or("unknown")
}

fn china_day(timestamp: u32) -> u64 {
    (u64::from(timestamp) + 8 * 60 * 60) / (24 * 60 * 60)
}

fn validate_public_quote(quote: &Official5188PublicQuote, now: SystemTime) -> Result<(), String> {
    const MAX_PRICE_WITHOUT_REFERENCE: f64 = 1_000_000.0;
    const MAX_PRICE_TO_PREVIOUS_CLOSE: f64 = 10.0;
    const MAX_CUMULATIVE_VOLUME: f64 = 100_000_000_000.0;
    const MAX_CUMULATIVE_AMOUNT: f64 = 1_000_000_000_000_000.0;
    const MAX_BOOK_VOLUME: f64 = 1_000_000_000.0;
    let now = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock precedes Unix epoch".to_string())?
        .as_secs();
    if china_day(quote.timestamp) != (now + 8 * 60 * 60) / (24 * 60 * 60) {
        return Err(format!(
            "public quote timestamp {} is outside the current China date",
            quote.timestamp
        ));
    }
    let scalars = [
        quote.price,
        quote.last_close,
        quote.open,
        quote.high,
        quote.low,
        quote.volume,
        quote.amount,
    ];
    if scalars
        .into_iter()
        .chain(quote.ask_prices)
        .chain(quote.ask_volumes)
        .chain(quote.bid_prices)
        .chain(quote.bid_volumes)
        .any(|value| !value.is_finite() || value < 0.0)
    {
        return Err("public quote contains a non-finite or negative value".to_string());
    }
    if quote.high != 0.0 && quote.low != 0.0 && quote.high < quote.low {
        return Err(format!(
            "public quote high {} is below low {}",
            quote.high, quote.low
        ));
    }
    if quote.high != 0.0
        && [quote.price, quote.open]
            .into_iter()
            .any(|value| value != 0.0 && value > quote.high)
    {
        return Err("public quote price/open exceeds high".to_string());
    }
    if quote.low != 0.0
        && [quote.price, quote.open]
            .into_iter()
            .any(|value| value != 0.0 && value < quote.low)
    {
        return Err("public quote price/open is below low".to_string());
    }
    let max_price = if quote.last_close > 0.0 {
        quote.last_close * MAX_PRICE_TO_PREVIOUS_CLOSE
    } else {
        MAX_PRICE_WITHOUT_REFERENCE
    };
    if [quote.price, quote.open, quote.high, quote.low]
        .into_iter()
        .chain(quote.ask_prices)
        .chain(quote.bid_prices)
        .any(|value| value > max_price)
    {
        return Err(format!(
            "public quote price exceeds broad reference bound {max_price}"
        ));
    }
    if quote.volume > MAX_CUMULATIVE_VOLUME
        || quote.amount > MAX_CUMULATIVE_AMOUNT
        || quote
            .ask_volumes
            .into_iter()
            .chain(quote.bid_volumes)
            .any(|value| value > MAX_BOOK_VOLUME)
    {
        return Err("public quote quantity exceeds broad market bound".to_string());
    }
    Ok(())
}

impl Official5188ShadowDecoder {
    fn new(code_tables: &[Official5188CodeTable]) -> Self {
        let mut resolver = Official5188MapBaselineResolver::new();
        let seed_records = resolver.seed_code_tables(code_tables);
        let mut symbol_codes = BTreeMap::new();
        let mut previous_close_seeds = BTreeMap::new();
        for table in code_tables {
            for record in &table.records {
                symbol_codes.insert((table.market, record.symbol_index), record.code.clone());
                if record.opaque_tail.get(11..15).is_some() {
                    previous_close_seeds
                        .insert((table.market, record.code.clone()), record.clone());
                }
            }
        }
        Self {
            resolver,
            seed_records,
            symbol_codes,
            previous_close_seeds,
            missing_previous_close_seeds: 0,
            attempted_frames: 0,
            decoded_frames: 0,
            partial_frames: 0,
            failed_frames: 0,
            decoded_records: 0,
            omitted_tail_records: 0,
            oem_states: BTreeMap::new(),
            public_quotes: BTreeMap::new(),
            oem_state_updates: 0,
            rejected_public_quotes: 0,
            last_error: None,
            error_kinds: BTreeMap::new(),
            baseline_modes: BTreeMap::new(),
            error_samples: BTreeMap::new(),
        }
    }

    fn observe(&mut self, frame: &Official5188Frame) {
        if frame.kind != Official5188Kind::SERVER_DELTA {
            return;
        }
        self.attempted_frames += 1;
        let result = (|| {
            let envelope = Official5188DeltaEnvelope::decode(frame)?;
            let streams = envelope.split_streams()?;
            let indexes = decode_official_5188_delta_indexes(streams)?;
            // The shadow reader starts at connect and therefore receives the
            // server's initial dump; symbols without a decoded baseline take
            // the fresh path exactly like the vendor client's empty slots.
            let outcome = decode_official_5188_values_partial_with_fresh_fallback(
                streams,
                &indexes,
                &mut self.resolver,
            );
            self.merge_oem_records(&outcome.records);
            self.resolver.update_from_decoded(&outcome.records);
            Ok::<_, String>(outcome)
        })();
        match result {
            Ok(outcome) => {
                self.decoded_records += outcome.records.len() as u64;
                self.omitted_tail_records += outcome.omitted_tail_records as u64;
                if let Some(error) = outcome.error {
                    if !outcome.records.is_empty() {
                        self.partial_frames += 1;
                    }
                    self.record_error(frame, error);
                } else {
                    self.decoded_frames += 1;
                }
            }
            Err(error) => {
                self.record_error(frame, error);
            }
        }
    }

    fn merge_oem_records(&mut self, records: &[netzip_fullpull::Official5188DecodedValueRecord]) {
        for record in records {
            let index_key = (record.index.market, record.index.symbol_index);
            let Some(code) = self.symbol_codes.get(&index_key) else {
                self.missing_previous_close_seeds += 1;
                continue;
            };
            let seed_key = (record.index.market, code.clone());
            let Some(metadata) = self.previous_close_seeds.get(&seed_key).cloned() else {
                self.missing_previous_close_seeds += 1;
                continue;
            };
            let seed_state = match Official5188OemState::from_code_table_metadata(
                record.index.market,
                &metadata,
            ) {
                Ok(state) => state,
                Err(_) => {
                    self.missing_previous_close_seeds += 1;
                    continue;
                }
            };
            let mut candidate = if let Some(state) = self.oem_states.get(&index_key) {
                state.clone()
            } else {
                seed_state
            };
            let quote =
                match candidate.merge_and_project_from_code_table_metadata(record, &metadata) {
                    Ok(quote) => quote,
                    Err(_) => {
                        self.missing_previous_close_seeds += 1;
                        continue;
                    }
                };
            if validate_public_quote(&quote, SystemTime::now()).is_err() {
                self.rejected_public_quotes += 1;
                continue;
            }
            self.oem_states.insert(index_key, candidate);
            self.public_quotes.insert(index_key, quote);
            self.oem_state_updates += 1;
        }
    }

    fn record_error(&mut self, frame: &Official5188Frame, error: String) {
        self.failed_frames += 1;
        let kind = decoder_error_kind(&error);
        *self.error_kinds.entry(kind.clone()).or_insert(0) += 1;
        *self
            .baseline_modes
            .entry(decoder_baseline_mode(&error).to_string())
            .or_insert(0) += 1;
        let contextual_error = format!(
            "payload_len={} metadata_len={} {error}",
            frame.payload.len(),
            frame.metadata.len()
        );
        if self.error_samples.len() < 32 {
            self.error_samples
                .entry(kind)
                .or_insert_with(|| contextual_error.clone());
        }
        self.last_error = Some(contextual_error);
    }
}

/// Receive-only observer for an authenticated 5188 socket.
///
/// Starting this reader never writes initialization, subscription, heartbeat,
/// or supplement traffic. The caller must provide a session whose current
/// lifecycle is already suitable for observation.
#[derive(Debug)]
pub struct Official5188ShadowReader {
    shared: Arc<Mutex<Official5188ShadowState>>,
    stop_stream: TcpStream,
    stop_requested: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Official5188ShadowReader {
    pub fn start(
        session: Official5188Session,
        max_frames: usize,
        max_bytes: usize,
    ) -> Result<Self, String> {
        Self::start_with_code_tables(session, max_frames, max_bytes, Vec::new())
    }

    pub fn start_with_code_tables(
        mut session: Official5188Session,
        max_frames: usize,
        max_bytes: usize,
        code_tables: Vec<Official5188CodeTable>,
    ) -> Result<Self, String> {
        let endpoint = session.endpoint().to_string();
        let stop_stream = session
            .try_clone_stream()
            .map_err(|error| format!("clone 5188 shadow stop stream: {error}"))?;
        let sink = Official5188FrameSink::new(max_frames, max_bytes)?;
        let shared = Arc::new(Mutex::new(Official5188ShadowState {
            running: true,
            endpoint,
            frames_received: 0,
            application_bytes_received: 0,
            delta_2704_frames: 0,
            bulk_3e04_frames: 0,
            receive_terminations: 0,
            decoder: Official5188ShadowDecoder::new(&code_tables),
            last_error: None,
            sink,
        }));
        let stop_requested = Arc::new(AtomicBool::new(false));
        let worker_shared = Arc::clone(&shared);
        let worker_stop = Arc::clone(&stop_requested);
        let worker = thread::Builder::new()
            .name("official-5188-shadow".to_string())
            .spawn(move || {
                let result = session.receive_until_disconnect(|frame| {
                    let mut state = worker_shared
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    state.frames_received += 1;
                    state.application_bytes_received +=
                        (frame.metadata.len() + frame.payload.len()) as u64;
                    if frame.kind == netzip_fullpull::Official5188Kind::SERVER_DELTA {
                        state.delta_2704_frames += 1;
                    }
                    if frame.kind == netzip_fullpull::Official5188Kind::SERVER_META {
                        state.bulk_3e04_frames += 1;
                    }
                    state.decoder.observe(&frame);
                    state.sink.push(frame);
                    Ok(())
                });
                let mut state = worker_shared
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                state.running = false;
                state.receive_terminations += 1;
                if !worker_stop.load(Ordering::Acquire) {
                    state.last_error = result.err();
                }
            })
            .map_err(|error| format!("start 5188 shadow reader: {error}"))?;
        Ok(Self {
            shared,
            stop_stream,
            stop_requested,
            worker: Some(worker),
        })
    }

    #[must_use]
    pub fn snapshot(&self) -> Official5188ShadowSnapshot {
        let state = self
            .shared
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        Official5188ShadowSnapshot {
            running: state.running,
            endpoint: state.endpoint.clone(),
            frames_received: state.frames_received,
            application_bytes_received: state.application_bytes_received,
            delta_2704_frames: state.delta_2704_frames,
            bulk_3e04_frames: state.bulk_3e04_frames,
            receive_terminations: state.receive_terminations,
            decoder_seed_records: state.decoder.seed_records,
            decoder_attempted_frames: state.decoder.attempted_frames,
            decoder_decoded_frames: state.decoder.decoded_frames,
            decoder_partial_frames: state.decoder.partial_frames,
            decoder_failed_frames: state.decoder.failed_frames,
            decoder_error_kinds: state.decoder.error_kinds.clone(),
            decoder_baseline_modes: state.decoder.baseline_modes.clone(),
            decoder_error_samples: state.decoder.error_samples.clone(),
            decoder_decoded_records: state.decoder.decoded_records,
            decoder_omitted_tail_records: state.decoder.omitted_tail_records,
            decoder_oem_state_symbols: state.decoder.oem_states.len(),
            decoder_oem_state_updates: state.decoder.oem_state_updates,
            decoder_missing_previous_close_seeds: state.decoder.missing_previous_close_seeds,
            decoder_rejected_public_quotes: state.decoder.rejected_public_quotes,
            decoder_last_error: state.decoder.last_error.clone(),
            last_error: state.last_error.clone(),
            retained: state.sink.snapshot(),
        }
    }

    /// Latest successfully projected quotes for diagnostics and parity checks.
    /// This does not publish to quoteGateway or change source routing.
    #[must_use]
    pub fn public_quotes(&self) -> Vec<Official5188PublicQuote> {
        self.shared
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .decoder
            .public_quotes
            .values()
            .cloned()
            .collect()
    }

    pub fn stop(&mut self) -> Result<(), String> {
        self.stop_requested.store(true, Ordering::Release);
        let shutdown_result = self.stop_stream.shutdown(Shutdown::Both);
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| "5188 shadow reader thread panicked".to_string())?;
        }
        match shutdown_result {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotConnected => Ok(()),
            Err(error) => Err(format!("stop 5188 shadow reader: {error}")),
        }
    }
}

impl Drop for Official5188ShadowReader {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl Official5188FrameSink {
    pub fn new(max_frames: usize, max_bytes: usize) -> Result<Self, String> {
        if max_frames == 0 || max_bytes == 0 {
            return Err("5188 frame sink limits must be non-zero".into());
        }
        Ok(Self {
            frames: VecDeque::new(),
            max_frames,
            max_bytes,
            bytes: 0,
            dropped_frames: 0,
        })
    }

    pub fn push(&mut self, frame: Official5188Frame) {
        let frame_bytes = frame.payload.len() + frame.metadata.len();
        while (!self.frames.is_empty())
            && (self.frames.len() >= self.max_frames || self.bytes + frame_bytes > self.max_bytes)
        {
            if let Some(old) = self.frames.pop_front() {
                self.bytes -= old.payload.len() + old.metadata.len();
                self.dropped_frames += 1;
            }
        }
        if frame_bytes > self.max_bytes {
            self.dropped_frames += 1;
            return;
        }
        self.bytes += frame_bytes;
        self.frames.push_back(frame);
    }

    pub fn frames(&self) -> impl Iterator<Item = &Official5188Frame> {
        self.frames.iter()
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }
    pub fn byte_count(&self) -> usize {
        self.bytes
    }
    pub fn dropped_frames(&self) -> u64 {
        self.dropped_frames
    }

    #[must_use]
    pub fn snapshot(&self) -> Official5188FrameSinkSnapshot {
        let mut wire_kind_counts = BTreeMap::new();
        for frame in &self.frames {
            *wire_kind_counts.entry(frame.kind.wire_hex()).or_insert(0) += 1;
        }
        let latest = self.frames.back();
        let subscriptions = self
            .frames
            .iter()
            .filter_map(subscription_summary)
            .collect();
        Official5188FrameSinkSnapshot {
            frame_count: self.frame_count(),
            byte_count: self.byte_count(),
            dropped_frames: self.dropped_frames(),
            wire_kind_counts,
            latest_wire_kind: latest.map(|frame| frame.kind.wire_hex()),
            latest_payload_len: latest.map(|frame| frame.payload.len()),
            subscriptions,
        }
    }
}

fn subscription_summary(frame: &Official5188Frame) -> Option<Official5188SubscriptionSummary> {
    let envelope = netzip_fullpull::Official5188SubscriptionEnvelope::decode(frame).ok()?;
    let mut market_entry_counts = BTreeMap::new();
    let mut first_index_by_market = BTreeMap::new();
    let mut last_index_by_market = BTreeMap::new();
    for index in 0..envelope.entries.len() {
        let (market, symbol_index) = envelope.opaque_code_value(index)?;
        let market = String::from_utf8_lossy(&market).into_owned();
        *market_entry_counts.entry(market.clone()).or_insert(0) += 1;
        first_index_by_market
            .entry(market.clone())
            .or_insert(symbol_index);
        last_index_by_market.insert(market, symbol_index);
    }
    Some(Official5188SubscriptionSummary {
        payload_len: frame.payload.len(),
        declared_entry_count: envelope.declared_entry_count(),
        decoded_entry_count: envelope.entries.len(),
        consecutive_range_count: envelope.consecutive_ranges().len(),
        market_entry_counts,
        first_index_by_market,
        last_index_by_market,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Official5188ConnectionPlan {
    pub endpoint: String,
    pub authenticated: bool,
    pub initialization: Vec<Official5188Frame>,
}

impl Official5188ConnectionPlan {
    pub fn new(
        endpoint: impl Into<String>,
        authenticated: bool,
        initialization: Vec<Official5188Frame>,
    ) -> Result<Self, String> {
        let endpoint = endpoint.into();
        if !authenticated {
            return Err("official 5188 plan requires authenticated 7100 session".into());
        }
        if initialization.len() < 6 {
            return Err("official 5188 plan requires Wine-shaped initialization".into());
        }
        let mut handshake = netzip_fullpull::Official5188Handshake::new();
        for frame in &initialization {
            handshake.observe_client(frame)?;
        }
        if !handshake.matches_wine_shape() {
            return Err("official 5188 initialization does not match Wine shape".into());
        }
        Ok(Self {
            endpoint,
            authenticated,
            initialization,
        })
    }

    pub fn connect(&self, timeout: Duration) -> Result<Official5188Session, String> {
        Official5188Session::connect_authenticated(
            self.endpoint.clone(),
            timeout,
            self.authenticated,
        )
    }

    /// Runs the official 5188 framed receive loop for an already initialized
    /// session. Callers receive complete frames and decide how to persist raw
    /// evidence; this method never falls back to a supplement endpoint.
    pub fn receive_until_disconnect<F>(
        &self,
        session: &mut Official5188Session,
        on_frame: F,
    ) -> Result<(), String>
    where
        F: FnMut(netzip_fullpull::Official5188Frame) -> Result<(), String>,
    {
        if session.endpoint() != self.endpoint {
            return Err("5188 session endpoint does not match connection plan".into());
        }
        session.receive_until_disconnect(on_frame)
    }

    /// Drains the authenticated session into a bounded evidence sink. No
    /// payload interpretation or supplement fallback occurs in this path.
    pub fn receive_into_sink(
        &self,
        session: &mut Official5188Session,
        sink: &mut Official5188FrameSink,
    ) -> Result<(), String> {
        self.receive_until_disconnect(session, |frame| {
            sink.push(frame);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use netzip_fullpull::{Official5188CodeTableRecord, Official5188Frame, Official5188Kind};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    fn current_timestamp() -> u32 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .try_into()
            .unwrap()
    }

    #[test]
    fn decoder_errors_are_grouped_by_stage_and_category() {
        assert_eq!(
            decoder_error_kind(
                "5188 value record 1 mask=0x04 header=0x01 stage=ladder_volumes bit=275: 5188 bitstream exhausted"
            ),
            "ladder_volumes:bitstream_exhausted"
        );
        assert_eq!(
            decoder_error_kind("5188 value record 2 stage=ohlc bit=9: token prefix did not match"),
            "ohlc:token_prefix_mismatch"
        );
        assert_eq!(
            decoder_baseline_mode(
                "5188 value record 2 baseline_mode=relative stage=ohlc bit=9: failed"
            ),
            "relative"
        );
        assert_eq!(decoder_baseline_mode("5188 bitstream exhausted"), "unknown");
    }

    #[test]
    fn shadow_decoder_seeds_previous_close_by_market_and_index() {
        let mut tail = [0_u8; 23];
        tail[11..15].copy_from_slice(&1_234_i32.to_le_bytes());
        let decoder = Official5188ShadowDecoder::new(&[Official5188CodeTable {
            market: *b"SH",
            records: vec![Official5188CodeTableRecord {
                symbol_index: 7,
                code: "600000".into(),
                name: "fixture".into(),
                amount_mode: 0,
                opaque_tail: tail,
            }],
        }]);
        let metadata = decoder
            .previous_close_seeds
            .get(&(*b"SH", "600000".to_string()))
            .expect("full 0104 metadata is retained");
        assert_eq!(
            i32::from_le_bytes(metadata.opaque_tail[11..15].try_into().unwrap()),
            1_234
        );
        assert_eq!(metadata.name, "fixture");
        assert_eq!(
            decoder.symbol_codes.get(&(*b"SH", 7)),
            Some(&"600000".to_string())
        );
        assert!(
            !decoder
                .previous_close_seeds
                .contains_key(&(*b"SZ", "600007".to_string()))
        );
    }

    #[test]
    fn shadow_decoder_keeps_missing_seed_records_not_ready() {
        let mut decoder = Official5188ShadowDecoder::new(&[]);
        let mut decoded_bytes = [0_u8; 311];
        decoded_bytes[..4].copy_from_slice(&current_timestamp().to_le_bytes());
        decoded_bytes[0xdf..0xe1].copy_from_slice(&7_u16.to_le_bytes());
        decoded_bytes[0xe1..0xe3].copy_from_slice(b"SH");
        let decoded = netzip_fullpull::Official5188DecodedValueRecord {
            index: netzip_fullpull::Official5188DeltaIndexState {
                market: *b"SH",
                symbol_index: 7,
                timestamp: current_timestamp(),
                uses_baseline: false,
            },
            mask: 0,
            header: netzip_fullpull::Official5188ValueRecordHeader::decode(0).unwrap(),
            bit_start: 0,
            bit_end: 0,
            record: netzip_fullpull::Official5188InternalRecord::decode(&decoded_bytes).unwrap(),
        };
        decoder.merge_oem_records(&[decoded]);
        assert_eq!(decoder.missing_previous_close_seeds, 1);
        assert!(decoder.oem_states.is_empty());
        assert!(decoder.public_quotes.is_empty());
        assert_eq!(decoder.oem_state_updates, 0);
    }

    #[test]
    fn shadow_decoder_retains_latest_successful_public_projection() {
        let mut tail = [0_u8; 23];
        tail[0] = 2;
        tail[11..15].copy_from_slice(&1_234_i32.to_le_bytes());
        let mut decoder = Official5188ShadowDecoder::new(&[Official5188CodeTable {
            market: *b"SH",
            records: vec![Official5188CodeTableRecord {
                symbol_index: 7,
                code: "600000".into(),
                name: "fixture".into(),
                amount_mode: 0,
                opaque_tail: tail,
            }],
        }]);
        let mut decoded_bytes = [0_u8; 311];
        decoded_bytes[..4].copy_from_slice(&current_timestamp().to_le_bytes());
        decoded_bytes[0xdf..0xe1].copy_from_slice(&7_u16.to_le_bytes());
        decoded_bytes[0xe1..0xe3].copy_from_slice(b"SH");
        let decoded = netzip_fullpull::Official5188DecodedValueRecord {
            index: netzip_fullpull::Official5188DeltaIndexState {
                market: *b"SH",
                symbol_index: 7,
                timestamp: current_timestamp(),
                uses_baseline: false,
            },
            mask: 0,
            header: netzip_fullpull::Official5188ValueRecordHeader::decode(0).unwrap(),
            bit_start: 0,
            bit_end: 0,
            record: netzip_fullpull::Official5188InternalRecord::decode(&decoded_bytes).unwrap(),
        };

        decoder.merge_oem_records(&[decoded]);

        assert_eq!(decoder.oem_state_updates, 1);
        let quote = decoder.public_quotes.get(&(*b"SH", 7)).unwrap();
        assert_eq!(quote.code, "600000");
        assert_eq!(quote.name, "fixture");
        assert_eq!(quote.last_close, f64::from(12.34_f32));
    }

    #[test]
    fn semantic_validation_rejects_wrong_day_negative_and_incoherent_quotes() {
        let now = UNIX_EPOCH + Duration::from_secs(1_788_746_400);
        let valid = Official5188PublicQuote {
            market: "SH".into(),
            code: "600000".into(),
            name: "fixture".into(),
            timestamp: 1_788_746_400,
            price: 10.0,
            last_close: 9.9,
            open: 9.8,
            high: 10.1,
            low: 9.7,
            volume: 100.0,
            amount: 1_000.0,
            ask_prices: [0.0; 10],
            ask_volumes: [0.0; 10],
            bid_prices: [0.0; 10],
            bid_volumes: [0.0; 10],
            source_protocol: "fixture",
        };
        assert!(validate_public_quote(&valid, now).is_ok());
        let mut wrong_day = valid.clone();
        wrong_day.timestamp = 1;
        assert!(validate_public_quote(&wrong_day, now).is_err());
        let mut negative = valid.clone();
        negative.bid_volumes[0] = -1.0;
        assert!(validate_public_quote(&negative, now).is_err());
        let mut incoherent = valid;
        incoherent.high = 9.0;
        incoherent.low = 10.0;
        assert!(validate_public_quote(&incoherent, now).is_err());
        let mut price_outside_ohlc = wrong_day;
        price_outside_ohlc.timestamp = 1_788_746_400;
        price_outside_ohlc.price = 11.0;
        assert!(validate_public_quote(&price_outside_ohlc, now).is_err());
        let mut implausible_price = negative;
        implausible_price.bid_volumes[0] = 0.0;
        implausible_price.ask_prices[0] = 1_000.0;
        assert!(validate_public_quote(&implausible_price, now).is_err());
    }

    #[test]
    fn shadow_decoder_does_not_commit_semantically_invalid_candidate() {
        let mut tail = [0_u8; 23];
        tail[0] = 2;
        tail[11..15].copy_from_slice(&1_234_i32.to_le_bytes());
        let mut decoder = Official5188ShadowDecoder::new(&[Official5188CodeTable {
            market: *b"SH",
            records: vec![Official5188CodeTableRecord {
                symbol_index: 7,
                code: "600000".into(),
                name: "fixture".into(),
                amount_mode: 0,
                opaque_tail: tail,
            }],
        }]);
        let timestamp = current_timestamp();
        let make_record = |price: i32| {
            let mut bytes = [0_u8; 311];
            bytes[..4].copy_from_slice(&timestamp.to_le_bytes());
            bytes[0x10..0x14].copy_from_slice(&price.to_le_bytes());
            bytes[0xdf..0xe1].copy_from_slice(&7_u16.to_le_bytes());
            bytes[0xe1..0xe3].copy_from_slice(b"SH");
            netzip_fullpull::Official5188DecodedValueRecord {
                index: netzip_fullpull::Official5188DeltaIndexState {
                    market: *b"SH",
                    symbol_index: 7,
                    timestamp,
                    uses_baseline: false,
                },
                mask: 0,
                header: netzip_fullpull::Official5188ValueRecordHeader::decode(0).unwrap(),
                bit_start: 0,
                bit_end: 0,
                record: netzip_fullpull::Official5188InternalRecord::decode(&bytes).unwrap(),
            }
        };

        decoder.merge_oem_records(&[make_record(1_000)]);
        let accepted = decoder.public_quotes.get(&(*b"SH", 7)).unwrap().clone();
        decoder.merge_oem_records(&[make_record(-1)]);

        assert_eq!(decoder.oem_state_updates, 1);
        assert_eq!(decoder.rejected_public_quotes, 1);
        assert_eq!(decoder.public_quotes.get(&(*b"SH", 7)), Some(&accepted));
        assert_eq!(accepted.price, 10.0);
    }

    #[test]
    fn shadow_decoder_reconstruction_clears_stale_session_metadata() {
        let mut first_tail = [0_u8; 23];
        first_tail[11..15].copy_from_slice(&1_234_i32.to_le_bytes());
        let mut decoder = Official5188ShadowDecoder::new(&[Official5188CodeTable {
            market: *b"SH",
            records: vec![Official5188CodeTableRecord {
                symbol_index: 7,
                code: "600000".into(),
                name: "old".into(),
                amount_mode: 0,
                opaque_tail: first_tail,
            }],
        }]);
        assert!(
            decoder
                .previous_close_seeds
                .contains_key(&(*b"SH", "600000".to_string()))
        );

        let mut second_tail = [0_u8; 23];
        second_tail[11..15].copy_from_slice(&5_678_i32.to_le_bytes());
        decoder = Official5188ShadowDecoder::new(&[Official5188CodeTable {
            market: *b"SZ",
            records: vec![Official5188CodeTableRecord {
                symbol_index: 9,
                code: "000001".into(),
                name: "new".into(),
                amount_mode: 0,
                opaque_tail: second_tail,
            }],
        }]);

        assert!(
            !decoder
                .previous_close_seeds
                .contains_key(&(*b"SH", "600000".to_string()))
        );
        assert!(
            decoder
                .previous_close_seeds
                .contains_key(&(*b"SZ", "000001".to_string()))
        );
        assert_eq!(decoder.seed_records, 1);
        assert!(decoder.oem_states.is_empty());
        assert!(decoder.public_quotes.is_empty());
        assert_eq!(decoder.missing_previous_close_seeds, 0);
    }

    fn frame(kind: Official5188Kind, len: usize) -> Official5188Frame {
        Official5188Frame {
            kind,
            metadata: [0; 4],
            payload: vec![0; len],
        }
    }

    fn subscription_frame(entries: &[([u8; 2], u32)]) -> Official5188Frame {
        let mut payload = vec![0; 10];
        payload[4..8].copy_from_slice(&(entries.len() as u32).to_le_bytes());
        for (market, symbol_index) in entries {
            payload.extend_from_slice(market);
            payload.extend_from_slice(&symbol_index.to_le_bytes());
        }
        Official5188Frame {
            kind: Official5188Kind::CLIENT_SUBSCRIBE,
            metadata: [0; 4],
            payload,
        }
    }

    #[test]
    fn rejects_unauthenticated_plan() {
        let err = Official5188ConnectionPlan::new("TARGET:5188", false, Vec::new())
            .expect_err("unauthenticated plan must be rejected");
        assert!(err.contains("authenticated"));
    }

    #[test]
    fn accepts_wine_shaped_current_session_plan() {
        let mut frames = vec![frame(Official5188Kind::CLIENT_INIT, 95); 3];
        frames.extend((0..3).map(|_| frame(Official5188Kind::CLIENT_SESSION, 32)));
        let plan =
            Official5188ConnectionPlan::new("TARGET:5188", true, frames).expect("Wine-shaped plan");
        assert_eq!(plan.initialization.len(), 6);
    }

    #[test]
    fn frame_sink_is_bounded_and_retains_latest_evidence() {
        let mut sink = Official5188FrameSink::new(2, 64).unwrap();
        sink.push(frame(Official5188Kind::SERVER_INIT_CONTINUE, 20));
        sink.push(frame(Official5188Kind::SERVER_INIT_CONTINUE, 20));
        sink.push(frame(Official5188Kind::SERVER_INIT_CONTINUE, 20));
        assert_eq!(sink.frame_count(), 2);
        assert_eq!(sink.byte_count(), 48);
        assert_eq!(sink.dropped_frames(), 1);
    }

    #[test]
    fn frame_sink_rejects_single_frame_over_byte_limit() {
        let mut sink = Official5188FrameSink::new(2, 8).unwrap();
        sink.push(frame(Official5188Kind::SERVER_INIT_CONTINUE, 20));
        assert_eq!(sink.frame_count(), 0);
        assert_eq!(sink.dropped_frames(), 1);
    }

    #[test]
    fn frame_sink_snapshot_reports_retained_evidence_only() {
        let mut sink = Official5188FrameSink::new(2, 128).unwrap();
        sink.push(frame(Official5188Kind::SERVER_INIT_CONTROL, 20));
        sink.push(frame(Official5188Kind::SERVER_INIT_CONTINUE, 24));
        sink.push(frame(Official5188Kind::SERVER_INIT_CONTINUE, 28));

        let snapshot = sink.snapshot();
        assert_eq!(snapshot.frame_count, 2);
        assert_eq!(snapshot.byte_count, 60);
        assert_eq!(snapshot.dropped_frames, 1);
        assert_eq!(snapshot.wire_kind_counts.get("3210"), Some(&2));
        assert_eq!(snapshot.latest_wire_kind.as_deref(), Some("3210"));
        assert_eq!(snapshot.latest_payload_len, Some(28));
    }

    #[test]
    fn frame_sink_snapshot_summarizes_subscription_indexes_without_raw_entries() {
        let mut sink = Official5188FrameSink::new(4, 1024).unwrap();
        sink.push(subscription_frame(&[
            (*b"SH", 89_173),
            (*b"SH", 89_174),
            (*b"SZ", 65_536),
        ]));

        let snapshot = sink.snapshot();
        let summary = snapshot.subscriptions.first().unwrap();
        assert_eq!(summary.payload_len, 28);
        assert_eq!(summary.declared_entry_count, 3);
        assert_eq!(summary.decoded_entry_count, 3);
        assert_eq!(summary.consecutive_range_count, 2);
        assert_eq!(summary.market_entry_counts.get("SH"), Some(&2));
        assert_eq!(summary.market_entry_counts.get("SZ"), Some(&1));
        assert_eq!(summary.first_index_by_market.get("SH"), Some(&89_173));
        assert_eq!(summary.last_index_by_market.get("SH"), Some(&89_174));
        assert_eq!(summary.first_index_by_market.get("SZ"), Some(&65_536));
        assert_eq!(summary.last_index_by_market.get("SZ"), Some(&65_536));
    }

    #[test]
    fn shadow_reader_receives_split_frames_and_counts_wire_kinds() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = frame(Official5188Kind::SERVER_DELTA, 20).encode().unwrap();
            bytes.extend(frame(Official5188Kind::SERVER_META, 24).encode().unwrap());
            stream.write_all(&bytes[..5]).unwrap();
            stream.write_all(&bytes[5..]).unwrap();
            stream.shutdown(Shutdown::Write).unwrap();
        });
        let session = Official5188Session::connect(endpoint, Duration::from_secs(1)).unwrap();
        let mut reader = Official5188ShadowReader::start(session, 8, 1024).unwrap();
        server.join().unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        while reader.snapshot().running && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let snapshot = reader.snapshot();
        assert!(!snapshot.running);
        assert_eq!(snapshot.frames_received, 2);
        assert_eq!(snapshot.application_bytes_received, 52);
        assert_eq!(snapshot.delta_2704_frames, 1);
        assert_eq!(snapshot.bulk_3e04_frames, 1);
        assert_eq!(snapshot.retained.wire_kind_counts.get("2704"), Some(&1));
        assert_eq!(snapshot.retained.wire_kind_counts.get("3e04"), Some(&1));
        assert_eq!(snapshot.receive_terminations, 1);
        assert!(
            snapshot
                .last_error
                .as_deref()
                .unwrap_or_default()
                .contains("closed")
        );
        reader.stop().unwrap();
    }

    #[test]
    fn shadow_reader_never_writes_to_server_and_stop_joins() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        let (result_tx, result_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_millis(150)))
                .unwrap();
            let mut byte = [0_u8; 1];
            result_tx.send(stream.read(&mut byte)).unwrap();
            release_rx.recv().unwrap();
        });
        let session = Official5188Session::connect(endpoint, Duration::from_secs(1)).unwrap();
        let mut reader = Official5188ShadowReader::start(session, 8, 1024).unwrap();
        let read_result = result_rx.recv().unwrap();
        assert!(matches!(
            read_result,
            Err(ref error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                )
        ));
        reader.stop().unwrap();
        release_tx.send(()).unwrap();
        server.join().unwrap();
        let snapshot = reader.snapshot();
        assert!(!snapshot.running);
        assert_eq!(snapshot.frames_received, 0);
        assert_eq!(snapshot.receive_terminations, 1);
        assert_eq!(snapshot.last_error, None);
    }
}
