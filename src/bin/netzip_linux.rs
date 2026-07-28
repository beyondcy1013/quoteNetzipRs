use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;

use netzipapi_rust_demo::{
    TDX7709_DEFAULT_HOST, TDX7709_DEFAULT_PORT, Tdx7709Config, Tdx7709QuoteRequestItem,
    Tdx7709Session, fetch_f10_categories, fetch_kline, fetch_live_quotes, sync_code_table,
    tdx_0547_format_hhmmss_raw, tdx_0547_market_name, tdx_0547_record_symbol, write_code_table_csv,
};
use serde::Serialize;

#[derive(Debug)]
enum Command {
    Snapshot(SnapshotArgs),
    LiveQuote(LiveQuoteArgs),
    Kline(KlineArgs),
    F10Categories(F10CategoriesArgs),
    F10Content(F10ContentArgs),
    SyncCodeTable(SyncCodeTableArgs),
}

#[derive(Debug)]
struct Cli {
    config: Tdx7709Config,
    command: Command,
}

#[derive(Debug)]
struct SnapshotArgs {
    symbol: String,
    kline_type: String,
    kline_count: u16,
    f10_limit: usize,
    include_live_quote: bool,
    include_kline: bool,
    include_f10: bool,
}

#[derive(Debug)]
struct LiveQuoteArgs {
    symbols: Vec<String>,
    limit: usize,
}

#[derive(Debug)]
struct KlineArgs {
    symbol: String,
    kline_type: String,
    start: u16,
    count: u16,
    limit: usize,
}

#[derive(Debug)]
struct F10CategoriesArgs {
    symbol: String,
    limit: usize,
}

#[derive(Debug)]
struct F10ContentArgs {
    symbol: String,
    filename: Option<String>,
    category_name: Option<String>,
    start: u32,
    length: u32,
    preview_chars: usize,
}

#[derive(Debug)]
struct SyncCodeTableArgs {
    out: Option<PathBuf>,
}

#[derive(Clone, Debug)]
struct NormalizedSymbol {
    market: u8,
    code: String,
    symbol: String,
}

#[derive(Serialize)]
struct SnapshotResponse {
    target: String,
    symbol: String,
    kline_type: String,
    kline_count: u16,
    shared_session_attempted: bool,
    shared_session_established: bool,
    shared_session_fallback_phases: Vec<String>,
    include_live_quote: bool,
    include_kline: bool,
    include_f10: bool,
    live_quote_reply_frames: Option<usize>,
    live_quote_records: Option<usize>,
    live_quote_error: Option<String>,
    matched_live_quote: Option<LiveQuotePreview>,
    kline_reply_frames: Option<usize>,
    kline_error: Option<String>,
    kline_bars: Vec<KlinePreview>,
    f10_categories_total: Option<usize>,
    f10_categories_error: Option<String>,
    f10_categories_preview: Vec<F10CategoryPreview>,
}

#[derive(Serialize)]
struct LiveQuoteResponse {
    target: String,
    requested_symbols: Vec<String>,
    transport_symbols: Vec<String>,
    unmatched_symbols: Vec<String>,
    code_table_reply_frames: usize,
    quote_reply_frames: usize,
    parsed_quote_records: usize,
    matched_records: usize,
    preview: Vec<LiveQuotePreview>,
    first_match: Option<LiveQuotePreview>,
    last_match: Option<LiveQuotePreview>,
}

#[derive(Serialize)]
struct KlineResponse {
    target: String,
    symbol: String,
    kline_type: String,
    start: u16,
    requested_count: u16,
    code_table_reply_frames: usize,
    kline_reply_frames: usize,
    bars_total: usize,
    preview: Vec<KlinePreview>,
    first_bar: Option<KlinePreview>,
    last_bar: Option<KlinePreview>,
}

#[derive(Serialize)]
struct F10CategoriesResponse {
    target: String,
    symbol: String,
    code_table_reply_frames: usize,
    category_reply_frames: usize,
    categories_total: usize,
    preview: Vec<F10CategoryPreview>,
    first_category: Option<F10CategoryPreview>,
    last_category: Option<F10CategoryPreview>,
}

#[derive(Serialize)]
struct F10ContentResponse {
    target: String,
    symbol: String,
    resolved_category_name: Option<String>,
    filename: String,
    start: u32,
    length: u32,
    preview_chars: usize,
    code_table_reply_frames: usize,
    content_reply_frames: usize,
    content_chars_total: usize,
    content_preview: String,
}

#[derive(Serialize)]
struct LiveQuotePreview {
    symbol: String,
    market: u8,
    market_name: String,
    code: String,
    start: usize,
    len: usize,
    active1_raw: Option<u16>,
    time_hhmmss_raw: Option<u32>,
    time_hhmmss: Option<String>,
    extra0_raw: Option<i32>,
    extra0_time_hhmmss: Option<String>,
    extra1_raw: Option<i32>,
    extra2_raw: Option<i32>,
    extra3_raw: Option<i32>,
    price: Option<f64>,
    last_close: Option<f64>,
    open: Option<f64>,
    high: Option<f64>,
    low: Option<f64>,
}

#[derive(Serialize)]
struct KlinePreview {
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
struct F10CategoryPreview {
    name: String,
    filename: String,
    start: u32,
    length: u32,
}

#[derive(Serialize)]
struct SyncCodeTableResponse {
    target: String,
    reply_bytes: usize,
    reply_frames: usize,
    records_total: usize,
    out_csv_path: Option<String>,
    first_record: Option<CodeTablePreview>,
    last_record: Option<CodeTablePreview>,
}

#[derive(Serialize)]
struct CodeTablePreview {
    code: String,
    name: String,
    decimal_point: u8,
    pre_close: f32,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let cli = parse_args()?;
    match cli.command {
        Command::Snapshot(args) => run_snapshot(&cli.config, args),
        Command::LiveQuote(args) => run_live_quote(&cli.config, args),
        Command::Kline(args) => run_kline(&cli.config, args),
        Command::F10Categories(args) => run_f10_categories(&cli.config, args),
        Command::F10Content(args) => run_f10_content(&cli.config, args),
        Command::SyncCodeTable(args) => run_sync_code_table(&cli.config, args),
    }
}

fn run_snapshot(config: &Tdx7709Config, args: SnapshotArgs) -> Result<(), Box<dyn Error>> {
    let normalized = normalize_symbol(&args.symbol)?;
    let category = resolve_kline_category(&args.kline_type)?;
    let quote_request_items = augment_live_quote_symbols(&[normalized.clone()])
        .into_iter()
        .map(|item| Tdx7709QuoteRequestItem {
            market: item.market,
            code: item.code,
            token: 0,
        })
        .collect::<Vec<_>>();
    let shared_session_attempted =
        args.include_live_quote || args.include_kline || args.include_f10;
    let mut shared_session = if shared_session_attempted {
        Tdx7709Session::open(config).ok()
    } else {
        None
    };
    let shared_session_established = shared_session.is_some();
    let mut shared_session_fallback_phases = Vec::new();

    let (kline_reply_frames, kline_error, kline_bars) = if args.include_kline {
        if shared_session.is_some() {
            match shared_session.as_mut().unwrap().request_kline(
                normalized.market,
                &normalized.code,
                category,
                0,
                args.kline_count,
            ) {
                Ok(kline_result) => (
                    Some(kline_result.kline_frames.len()),
                    None,
                    kline_result
                        .bars
                        .iter()
                        .map(kline_preview)
                        .collect::<Vec<_>>(),
                ),
                Err(_) => {
                    shared_session = None;
                    shared_session_fallback_phases.push("kline".to_string());
                    match fetch_kline(
                        config,
                        normalized.market,
                        &normalized.code,
                        category,
                        0,
                        args.kline_count,
                    ) {
                        Ok(kline_result) => (
                            Some(kline_result.kline_frames.len()),
                            None,
                            kline_result
                                .bars
                                .iter()
                                .map(kline_preview)
                                .collect::<Vec<_>>(),
                        ),
                        Err(err) => (None, Some(err.to_string()), Vec::new()),
                    }
                }
            }
        } else {
            match fetch_kline(
                config,
                normalized.market,
                &normalized.code,
                category,
                0,
                args.kline_count,
            ) {
                Ok(kline_result) => (
                    Some(kline_result.kline_frames.len()),
                    None,
                    kline_result
                        .bars
                        .iter()
                        .map(kline_preview)
                        .collect::<Vec<_>>(),
                ),
                Err(err) => (None, Some(err.to_string()), Vec::new()),
            }
        }
    } else {
        (None, Some("skipped by request".to_string()), Vec::new())
    };

    let (f10_categories_total, f10_categories_error, f10_categories_preview) = if args.include_f10 {
        if shared_session.is_some() {
            match shared_session
                .as_mut()
                .unwrap()
                .request_f10_categories(normalized.market, &normalized.code)
            {
                Ok(f10_result) => (
                    Some(f10_result.categories.len()),
                    None,
                    f10_result
                        .categories
                        .iter()
                        .take(args.f10_limit)
                        .map(f10_category_preview)
                        .collect::<Vec<_>>(),
                ),
                Err(_) => {
                    shared_session = None;
                    shared_session_fallback_phases.push("f10-categories".to_string());
                    match fetch_f10_categories(config, normalized.market, &normalized.code) {
                        Ok(f10_result) => (
                            Some(f10_result.categories.len()),
                            None,
                            f10_result
                                .categories
                                .iter()
                                .take(args.f10_limit)
                                .map(f10_category_preview)
                                .collect::<Vec<_>>(),
                        ),
                        Err(err) => (None, Some(err.to_string()), Vec::new()),
                    }
                }
            }
        } else {
            match fetch_f10_categories(config, normalized.market, &normalized.code) {
                Ok(f10_result) => (
                    Some(f10_result.categories.len()),
                    None,
                    f10_result
                        .categories
                        .iter()
                        .take(args.f10_limit)
                        .map(f10_category_preview)
                        .collect::<Vec<_>>(),
                ),
                Err(err) => (None, Some(err.to_string()), Vec::new()),
            }
        }
    } else {
        (None, Some("skipped by request".to_string()), Vec::new())
    };

    let (live_quote_reply_frames, live_quote_records, live_quote_error, matched_live_quote) =
        if args.include_live_quote {
            if shared_session.is_some() {
                match shared_session
                    .as_mut()
                    .unwrap()
                    .request_live_quotes(&quote_request_items)
                {
                    Ok(quote_result) => {
                        let matched_live_quote = quote_result
                            .quote_bodies
                            .iter()
                            .flat_map(|body| body.records.iter())
                            .find(|record| {
                                tdx_0547_record_symbol(record).as_deref()
                                    == Some(normalized.symbol.as_str())
                            })
                            .map(live_quote_preview);
                        let live_quote_records = quote_result
                            .quote_bodies
                            .iter()
                            .map(|body| body.records.len())
                            .sum();
                        (
                            Some(quote_result.quote_frames.len()),
                            Some(live_quote_records),
                            None,
                            matched_live_quote,
                        )
                    }
                    Err(_) => {
                        shared_session_fallback_phases.push("live-quote".to_string());
                        match fetch_live_quotes(config, &quote_request_items) {
                            Ok(quote_result) => {
                                let matched_live_quote = quote_result
                                    .quote_bodies
                                    .iter()
                                    .flat_map(|body| body.records.iter())
                                    .find(|record| {
                                        tdx_0547_record_symbol(record).as_deref()
                                            == Some(normalized.symbol.as_str())
                                    })
                                    .map(live_quote_preview);
                                let live_quote_records = quote_result
                                    .quote_bodies
                                    .iter()
                                    .map(|body| body.records.len())
                                    .sum();
                                (
                                    Some(quote_result.quote_frames.len()),
                                    Some(live_quote_records),
                                    None,
                                    matched_live_quote,
                                )
                            }
                            Err(err) => (None, None, Some(err.to_string()), None),
                        }
                    }
                }
            } else {
                match fetch_live_quotes(config, &quote_request_items) {
                    Ok(quote_result) => {
                        let matched_live_quote = quote_result
                            .quote_bodies
                            .iter()
                            .flat_map(|body| body.records.iter())
                            .find(|record| {
                                tdx_0547_record_symbol(record).as_deref()
                                    == Some(normalized.symbol.as_str())
                            })
                            .map(live_quote_preview);
                        let live_quote_records = quote_result
                            .quote_bodies
                            .iter()
                            .map(|body| body.records.len())
                            .sum();
                        (
                            Some(quote_result.quote_frames.len()),
                            Some(live_quote_records),
                            None,
                            matched_live_quote,
                        )
                    }
                    Err(err) => (None, None, Some(err.to_string()), None),
                }
            }
        } else {
            (None, None, Some("skipped by request".to_string()), None)
        };

    let response = SnapshotResponse {
        target: format!("{}:{}", config.host, config.port),
        symbol: normalized.symbol,
        kline_type: describe_kline_category(category).to_string(),
        kline_count: args.kline_count,
        shared_session_attempted,
        shared_session_established,
        shared_session_fallback_phases,
        include_live_quote: args.include_live_quote,
        include_kline: args.include_kline,
        include_f10: args.include_f10,
        live_quote_reply_frames,
        live_quote_records,
        live_quote_error,
        matched_live_quote,
        kline_reply_frames,
        kline_error,
        kline_bars,
        f10_categories_total,
        f10_categories_error,
        f10_categories_preview,
    };

    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

fn run_live_quote(config: &Tdx7709Config, args: LiveQuoteArgs) -> Result<(), Box<dyn Error>> {
    let normalized_symbols = args
        .symbols
        .iter()
        .map(|value| normalize_symbol(value))
        .collect::<Result<Vec<_>, _>>()?;
    let transport_symbols = augment_live_quote_symbols(&normalized_symbols);
    let request_items = transport_symbols
        .iter()
        .map(|item| Tdx7709QuoteRequestItem {
            market: item.market,
            code: item.code.clone(),
            token: 0,
        })
        .collect::<Vec<_>>();
    let result = fetch_live_quotes(config, &request_items)?;

    let requested_set = normalized_symbols
        .iter()
        .map(|item| item.symbol.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut seen_symbols = std::collections::BTreeSet::new();
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

    let unmatched_symbols = normalized_symbols
        .iter()
        .filter(|item| !seen_symbols.contains(&item.symbol))
        .map(|item| item.symbol.clone())
        .collect::<Vec<_>>();
    let preview = matched_records
        .iter()
        .take(args.limit)
        .map(|record| live_quote_preview(record))
        .collect::<Vec<_>>();

    let response = LiveQuoteResponse {
        target: format!("{}:{}", config.host, config.port),
        requested_symbols: normalized_symbols
            .iter()
            .map(|item| item.symbol.clone())
            .collect(),
        transport_symbols: transport_symbols
            .iter()
            .map(|item| item.symbol.clone())
            .collect(),
        unmatched_symbols,
        code_table_reply_frames: result.code_table_frames.len(),
        quote_reply_frames: result.quote_frames.len(),
        parsed_quote_records,
        matched_records: matched_records.len(),
        preview,
        first_match: matched_records
            .first()
            .map(|record| live_quote_preview(record)),
        last_match: matched_records
            .last()
            .map(|record| live_quote_preview(record)),
    };

    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

fn run_kline(config: &Tdx7709Config, args: KlineArgs) -> Result<(), Box<dyn Error>> {
    let normalized = normalize_symbol(&args.symbol)?;
    let category = resolve_kline_category(&args.kline_type)?;
    let result = fetch_kline(
        config,
        normalized.market,
        &normalized.code,
        category,
        args.start,
        args.count,
    )?;

    let response = KlineResponse {
        target: format!("{}:{}", config.host, config.port),
        symbol: normalized.symbol,
        kline_type: describe_kline_category(category).to_string(),
        start: args.start,
        requested_count: args.count,
        code_table_reply_frames: result.code_table_frames.len(),
        kline_reply_frames: result.kline_frames.len(),
        bars_total: result.bars.len(),
        preview: result
            .bars
            .iter()
            .take(args.limit)
            .map(kline_preview)
            .collect(),
        first_bar: result.bars.first().map(kline_preview),
        last_bar: result.bars.last().map(kline_preview),
    };

    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

fn run_f10_categories(
    config: &Tdx7709Config,
    args: F10CategoriesArgs,
) -> Result<(), Box<dyn Error>> {
    let normalized = normalize_symbol(&args.symbol)?;
    let result = fetch_f10_categories(config, normalized.market, &normalized.code)?;

    let response = F10CategoriesResponse {
        target: format!("{}:{}", config.host, config.port),
        symbol: normalized.symbol,
        code_table_reply_frames: result.code_table_frames.len(),
        category_reply_frames: result.category_frames.len(),
        categories_total: result.categories.len(),
        preview: result
            .categories
            .iter()
            .take(args.limit)
            .map(f10_category_preview)
            .collect(),
        first_category: result.categories.first().map(f10_category_preview),
        last_category: result.categories.last().map(f10_category_preview),
    };

    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

fn run_f10_content(config: &Tdx7709Config, args: F10ContentArgs) -> Result<(), Box<dyn Error>> {
    let normalized = normalize_symbol(&args.symbol)?;

    let (resolved_category_name, resolved_filename, resolved_start, resolved_length, result) =
        if let Some(filename) = args
            .filename
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            (
                None,
                filename.to_string(),
                args.start,
                args.length.max(1),
                netzipapi_rust_demo::fetch_f10_content(
                    config,
                    normalized.market,
                    &normalized.code,
                    filename,
                    args.start,
                    args.length.max(1),
                )?,
            )
        } else if let Some(category_name) = args
            .category_name
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            let mut session = Tdx7709Session::open(config)?;
            let categories = session.request_f10_categories(normalized.market, &normalized.code)?;
            let matched = categories
                .categories
                .iter()
                .find(|item| item.name == category_name)
                .ok_or_else(|| {
                    format!(
                        "category_name not found for {}: {}",
                        normalized.symbol, category_name
                    )
                })?;
            let result = session.request_f10_content(
                normalized.market,
                &normalized.code,
                &matched.filename,
                matched.start,
                matched.length.max(1),
            )?;
            (
                Some(matched.name.clone()),
                matched.filename.clone(),
                matched.start,
                matched.length.max(1),
                result,
            )
        } else {
            return Err("f10-content requires --filename or --category-name".into());
        };

    let content_preview = result
        .content
        .chars()
        .take(args.preview_chars)
        .collect::<String>();

    let response = F10ContentResponse {
        target: format!("{}:{}", config.host, config.port),
        symbol: normalized.symbol,
        resolved_category_name,
        filename: resolved_filename,
        start: resolved_start,
        length: resolved_length,
        preview_chars: args.preview_chars,
        code_table_reply_frames: result.code_table_frames.len(),
        content_reply_frames: result.content_frames.len(),
        content_chars_total: result.content.chars().count(),
        content_preview,
    };

    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

fn run_sync_code_table(
    config: &Tdx7709Config,
    args: SyncCodeTableArgs,
) -> Result<(), Box<dyn Error>> {
    let result = sync_code_table(config)?;
    if let Some(path) = &args.out {
        write_code_table_csv(path, &result.records)?;
    }

    let response = SyncCodeTableResponse {
        target: format!("{}:{}", config.host, config.port),
        reply_bytes: result.raw_reply.len(),
        reply_frames: result.frames.len(),
        records_total: result.records.len(),
        out_csv_path: args.out.as_ref().map(|path| path.display().to_string()),
        first_record: result.records.first().map(code_table_preview),
        last_record: result.records.last().map(code_table_preview),
    };

    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

fn parse_args() -> Result<Cli, Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut host = TDX7709_DEFAULT_HOST.to_string();
    let mut port = TDX7709_DEFAULT_PORT;
    let mut read_timeout_ms = 1_000u64;
    let mut connect_timeout_ms = 5_000u64;
    let mut settle_ms = 250u64;
    let mut command_name = None::<String>;
    let mut tail = Vec::<String>::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--host" => host = args.next().ok_or("missing value after --host")?,
            "--port" => port = args.next().ok_or("missing value after --port")?.parse()?,
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
            other if command_name.is_none() => command_name = Some(other.to_string()),
            other => tail.push(other.to_string()),
        }
    }

    let config = Tdx7709Config {
        host,
        port,
        read_timeout: Duration::from_millis(read_timeout_ms),
        connect_timeout: Duration::from_millis(connect_timeout_ms),
        settle_delay: Duration::from_millis(settle_ms),
    };

    let Some(command_name) = command_name else {
        print_help();
        return Err("missing command".into());
    };

    let command = match command_name.as_str() {
        "snapshot" => parse_snapshot_args(tail)?,
        "live-quote" => parse_live_quote_args(tail)?,
        "kline" => parse_kline_args(tail)?,
        "f10-categories" => parse_f10_categories_args(tail)?,
        "f10-content" => parse_f10_content_args(tail)?,
        "sync-code-table" => parse_sync_code_table_args(tail)?,
        other => return Err(format!("unknown command: {other}").into()),
    };

    Ok(Cli { config, command })
}

fn parse_snapshot_args(args: Vec<String>) -> Result<Command, Box<dyn Error>> {
    let mut symbol = None::<String>;
    let mut kline_type = "1d".to_string();
    let mut kline_count = 3u16;
    let mut f10_limit = 8usize;
    let mut include_live_quote = true;
    let mut include_kline = true;
    let mut include_f10 = true;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--symbol" => symbol = Some(iter.next().ok_or("missing value after --symbol")?),
            "--kline-type" => kline_type = iter.next().ok_or("missing value after --kline-type")?,
            "--kline-count" => {
                kline_count = iter
                    .next()
                    .ok_or("missing value after --kline-count")?
                    .parse()?
            }
            "--f10-limit" => {
                f10_limit = iter
                    .next()
                    .ok_or("missing value after --f10-limit")?
                    .parse()?
            }
            "--skip-live-quote" => include_live_quote = false,
            "--skip-kline" => include_kline = false,
            "--skip-f10" => include_f10 = false,
            other if symbol.is_none() => symbol = Some(other.to_string()),
            other => return Err(format!("unknown snapshot arg: {other}").into()),
        }
    }

    Ok(Command::Snapshot(SnapshotArgs {
        symbol: symbol.ok_or("snapshot requires a symbol like SH600000")?,
        kline_type,
        kline_count,
        f10_limit,
        include_live_quote,
        include_kline,
        include_f10,
    }))
}

fn parse_live_quote_args(args: Vec<String>) -> Result<Command, Box<dyn Error>> {
    let mut symbols = Vec::<String>::new();
    let mut limit = 20usize;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--symbol" => symbols.push(iter.next().ok_or("missing value after --symbol")?),
            "--limit" => limit = iter.next().ok_or("missing value after --limit")?.parse()?,
            other => symbols.push(other.to_string()),
        }
    }

    if symbols.is_empty() {
        return Err("live-quote requires at least one symbol".into());
    }

    Ok(Command::LiveQuote(LiveQuoteArgs {
        symbols,
        limit: limit.clamp(1, 200),
    }))
}

fn parse_kline_args(args: Vec<String>) -> Result<Command, Box<dyn Error>> {
    let mut symbol = None::<String>;
    let mut kline_type = "1d".to_string();
    let mut start = 0u16;
    let mut count = 32u16;
    let mut limit = 20usize;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--symbol" => symbol = Some(iter.next().ok_or("missing value after --symbol")?),
            "--kline-type" => kline_type = iter.next().ok_or("missing value after --kline-type")?,
            "--start" => start = iter.next().ok_or("missing value after --start")?.parse()?,
            "--count" => count = iter.next().ok_or("missing value after --count")?.parse()?,
            "--limit" => limit = iter.next().ok_or("missing value after --limit")?.parse()?,
            other if symbol.is_none() => symbol = Some(other.to_string()),
            other => return Err(format!("unknown kline arg: {other}").into()),
        }
    }

    Ok(Command::Kline(KlineArgs {
        symbol: symbol.ok_or("kline requires a symbol like SH600000")?,
        kline_type,
        start,
        count: count.max(1),
        limit: limit.clamp(1, 200),
    }))
}

fn parse_f10_categories_args(args: Vec<String>) -> Result<Command, Box<dyn Error>> {
    let mut symbol = None::<String>;
    let mut limit = 20usize;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--symbol" => symbol = Some(iter.next().ok_or("missing value after --symbol")?),
            "--limit" => limit = iter.next().ok_or("missing value after --limit")?.parse()?,
            other if symbol.is_none() => symbol = Some(other.to_string()),
            other => return Err(format!("unknown f10-categories arg: {other}").into()),
        }
    }

    Ok(Command::F10Categories(F10CategoriesArgs {
        symbol: symbol.ok_or("f10-categories requires a symbol like SH600000")?,
        limit: limit.clamp(1, 200),
    }))
}

fn parse_f10_content_args(args: Vec<String>) -> Result<Command, Box<dyn Error>> {
    let mut symbol = None::<String>;
    let mut filename = None::<String>;
    let mut category_name = None::<String>;
    let mut start = 0u32;
    let mut length = 30_000u32;
    let mut preview_chars = 600usize;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--symbol" => symbol = Some(iter.next().ok_or("missing value after --symbol")?),
            "--filename" => filename = Some(iter.next().ok_or("missing value after --filename")?),
            "--category-name" => {
                category_name = Some(iter.next().ok_or("missing value after --category-name")?)
            }
            "--start" => start = iter.next().ok_or("missing value after --start")?.parse()?,
            "--length" => length = iter.next().ok_or("missing value after --length")?.parse()?,
            "--preview-chars" => {
                preview_chars = iter
                    .next()
                    .ok_or("missing value after --preview-chars")?
                    .parse()?
            }
            other if symbol.is_none() => symbol = Some(other.to_string()),
            other => return Err(format!("unknown f10-content arg: {other}").into()),
        }
    }

    if filename.is_none() && category_name.is_none() {
        return Err("f10-content requires --filename or --category-name".into());
    }

    Ok(Command::F10Content(F10ContentArgs {
        symbol: symbol.ok_or("f10-content requires a symbol like SH600000")?,
        filename,
        category_name,
        start,
        length: length.max(1),
        preview_chars: preview_chars.min(20_000),
    }))
}

fn parse_sync_code_table_args(args: Vec<String>) -> Result<Command, Box<dyn Error>> {
    let mut out = None::<PathBuf>;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" => {
                out = Some(PathBuf::from(
                    iter.next().ok_or("missing value after --out")?,
                ))
            }
            other => return Err(format!("unknown sync-code-table arg: {other}").into()),
        }
    }
    Ok(Command::SyncCodeTable(SyncCodeTableArgs { out }))
}

fn normalize_symbol(input: &str) -> Result<NormalizedSymbol, Box<dyn Error>> {
    let trimmed = input.trim().to_ascii_uppercase();
    if trimmed.is_empty() {
        return Err("symbol must not be empty".into());
    }

    let (market, code) = if let Some(code) = trimmed.strip_prefix("SH") {
        (1u8, code.to_string())
    } else if let Some(code) = trimmed.strip_prefix("SZ") {
        (0u8, code.to_string())
    } else if let Some(code) = trimmed.strip_prefix("BJ") {
        (2u8, code.to_string())
    } else if trimmed.len() == 6 && trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        (
            infer_market(&trimmed).ok_or_else(|| {
                format!("cannot infer market for bare code {trimmed}; use SH/SZ/BJ prefix")
            })?,
            trimmed.clone(),
        )
    } else {
        return Err(format!(
            "invalid symbol {trimmed}; expected SH600000/SZ300948/BJ430047 or 6-digit code"
        )
        .into());
    };

    if code.len() != 6 || !code.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("invalid 6-digit code in symbol {trimmed}").into());
    }

    let prefix = tdx_0547_market_name(market)
        .ok_or_else(|| format!("unsupported market {market} for symbol {trimmed}"))?;
    Ok(NormalizedSymbol {
        market,
        code: code.clone(),
        symbol: format!("{prefix}{code}"),
    })
}

fn augment_live_quote_symbols(requested: &[NormalizedSymbol]) -> Vec<NormalizedSymbol> {
    const LIVE_QUOTE_SEEDS: &[&str] = &["SH600000", "SZ300948", "SH113638"];

    let mut out = requested.to_vec();
    let mut seen = out
        .iter()
        .map(|item| item.symbol.clone())
        .collect::<std::collections::BTreeSet<_>>();

    for seed in LIVE_QUOTE_SEEDS {
        if out.len() >= 3 {
            break;
        }
        let Ok(normalized) = normalize_symbol(seed) else {
            continue;
        };
        if seen.insert(normalized.symbol.clone()) {
            out.push(normalized);
        }
    }

    out
}

fn infer_market(code: &str) -> Option<u8> {
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

fn resolve_kline_category(input: &str) -> Result<u16, Box<dyn Error>> {
    let normalized = input.trim().to_ascii_lowercase();
    let category = match normalized.as_str() {
        "5m" => 0,
        "15m" => 1,
        "30m" => 2,
        "60m" | "1h" => 3,
        "1d" | "day" | "daily" => 4,
        "1w" | "week" | "weekly" => 5,
        "1mo" | "month" | "monthly" => 6,
        "1m" | "1min" | "minute" => 7,
        "1m-alt" => 8,
        "1d-alt" => 9,
        "1q" | "quarter" | "quarterly" => 10,
        "1y" | "year" | "yearly" => 11,
        _ => return Err(format!("unsupported kline type {normalized}").into()),
    };
    Ok(category)
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

fn live_quote_preview(record: &netzipapi_rust_demo::Tdx0547Record) -> LiveQuotePreview {
    LiveQuotePreview {
        symbol: tdx_0547_record_symbol(record).unwrap_or_else(|| format!("NA{}", record.code)),
        market: record.market,
        market_name: tdx_0547_market_name(record.market)
            .unwrap_or("NA")
            .to_string(),
        code: record.code.clone(),
        start: record.start,
        len: record.len,
        active1_raw: record.active1_raw,
        time_hhmmss_raw: record.time_hhmmss_raw,
        time_hhmmss: record.time_hhmmss_raw.and_then(tdx_0547_format_hhmmss_raw),
        extra0_raw: record.extra0_raw,
        extra0_time_hhmmss: record.extra0_time_hhmmss.clone(),
        extra1_raw: record.extra1_raw,
        extra2_raw: record.extra2_raw,
        extra3_raw: record.extra3_raw,
        price: record.quote_head.as_ref().map(|value| value.price),
        last_close: record.quote_head.as_ref().map(|value| value.last_close),
        open: record.quote_head.as_ref().map(|value| value.open),
        high: record.quote_head.as_ref().map(|value| value.high),
        low: record.quote_head.as_ref().map(|value| value.low),
    }
}

fn kline_preview(bar: &netzipapi_rust_demo::Tdx7709KlineBar) -> KlinePreview {
    KlinePreview {
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

fn f10_category_preview(category: &netzipapi_rust_demo::Tdx7709F10Category) -> F10CategoryPreview {
    F10CategoryPreview {
        name: category.name.clone(),
        filename: category.filename.clone(),
        start: category.start,
        length: category.length,
    }
}

fn code_table_preview(record: &netzipapi_rust_demo::Tdx7709CodeTableRecord) -> CodeTablePreview {
    CodeTablePreview {
        code: record.code.clone(),
        name: record.name.clone(),
        decimal_point: record.decimal_point(),
        pre_close: record.pre_close(),
    }
}

fn print_help() {
    println!("netzip_linux [global options] <command> [command options]");
    println!();
    println!("global options:");
    println!("  --host <ip>                   default {TDX7709_DEFAULT_HOST}");
    println!("  --port <port>                 default {TDX7709_DEFAULT_PORT}");
    println!("  --read-timeout-ms <n>         default 1000");
    println!("  --connect-timeout-ms <n>      default 5000");
    println!("  --settle-ms <n>               default 250");
    println!();
    println!("commands:");
    println!(
        "  snapshot <symbol> [--kline-type 1d] [--kline-count 3] [--f10-limit 8] [--skip-live-quote] [--skip-kline] [--skip-f10]"
    );
    println!("  live-quote <symbol> [symbol...] [--limit 20]");
    println!("  kline <symbol> [--kline-type 1d] [--start 0] [--count 32] [--limit 20]");
    println!("  f10-categories <symbol> [--limit 20]");
    println!(
        "  f10-content <symbol> (--category-name 公司概况 | --filename 000001.txt --start 0 --length 30000) [--preview-chars 600]"
    );
    println!("  sync-code-table [--out /tmp/tdx7709_codes.csv]");
    println!();
    println!("examples:");
    println!("  cargo run --bin netzip_linux -- snapshot SH600000");
    println!("  cargo run --bin netzip_linux -- live-quote SH600000 SZ000001");
    println!("  cargo run --bin netzip_linux -- snapshot 600000 --kline-type 1m --kline-count 5");
    println!("  cargo run --bin netzip_linux -- kline SH600000 --kline-type 1d --count 10");
    println!("  cargo run --bin netzip_linux -- f10-categories SH600000 --limit 5");
    println!("  cargo run --bin netzip_linux -- f10-content SH600000 --category-name 公司概况");
    println!("  cargo run --bin netzip_linux -- sync-code-table --out /tmp/tdx7709_codes.csv");
}

#[cfg(test)]
mod tests {
    use super::{
        Command, describe_kline_category, normalize_symbol, parse_f10_content_args,
        parse_live_quote_args, parse_snapshot_args, resolve_kline_category,
    };

    #[test]
    fn normalize_symbol_accepts_prefixed_and_inferred_inputs() {
        let sh = normalize_symbol("600000").expect("infer sh");
        assert_eq!(sh.market, 1);
        assert_eq!(sh.symbol, "SH600000");

        let sz = normalize_symbol("sz300948").expect("explicit sz");
        assert_eq!(sz.market, 0);
        assert_eq!(sz.symbol, "SZ300948");

        let bj = normalize_symbol("BJ430047").expect("explicit bj");
        assert_eq!(bj.market, 2);
        assert_eq!(bj.symbol, "BJ430047");
    }

    #[test]
    fn resolve_kline_category_maps_aliases() {
        assert_eq!(resolve_kline_category("1min").expect("1min"), 7);
        assert_eq!(resolve_kline_category("daily").expect("daily"), 4);
        assert_eq!(describe_kline_category(10), "1q");
        assert!(resolve_kline_category("2d").is_err());
    }

    #[test]
    fn parse_snapshot_args_accepts_phase_skip_flags() {
        let command = parse_snapshot_args(vec![
            "SH600000".to_string(),
            "--skip-live-quote".to_string(),
            "--skip-f10".to_string(),
        ])
        .expect("parse snapshot args");

        match command {
            Command::Snapshot(args) => {
                assert_eq!(args.symbol, "SH600000");
                assert!(!args.include_live_quote);
                assert!(args.include_kline);
                assert!(!args.include_f10);
            }
            other => panic!("expected snapshot command, got {other:?}"),
        }
    }

    #[test]
    fn parse_live_quote_args_accepts_multiple_symbols() {
        let command = parse_live_quote_args(vec![
            "SH600000".to_string(),
            "SZ000001".to_string(),
            "--limit".to_string(),
            "5".to_string(),
        ])
        .expect("parse live-quote args");

        match command {
            Command::LiveQuote(args) => {
                assert_eq!(args.symbols, vec!["SH600000", "SZ000001"]);
                assert_eq!(args.limit, 5);
            }
            other => panic!("expected live-quote command, got {other:?}"),
        }
    }

    #[test]
    fn parse_f10_content_args_accepts_category_lookup() {
        let command = parse_f10_content_args(vec![
            "SH600000".to_string(),
            "--category-name".to_string(),
            "公司概况".to_string(),
            "--preview-chars".to_string(),
            "1200".to_string(),
        ])
        .expect("parse f10-content args");

        match command {
            Command::F10Content(args) => {
                assert_eq!(args.symbol, "SH600000");
                assert_eq!(args.category_name.as_deref(), Some("公司概况"));
                assert_eq!(args.preview_chars, 1200);
            }
            other => panic!("expected f10-content command, got {other:?}"),
        }
    }
}
