pub mod api;
pub mod auth_7100_client;
pub mod auth_7100_flow_matrix;
pub mod auth_7100_prefix;
pub mod auth_client_shell;
pub mod auth_credentials;
pub mod auth_download;
pub mod auth_flow_sample;
pub mod capture_input;
pub mod debug_blob_compare;
pub mod debug_pcap;
pub mod debug_quote;
pub mod debug_quote_scan;
pub mod debug_stream;
pub mod legacy_panel;
pub mod local_2000;
pub mod local_2000_vs_auth7100;
pub mod packet;
pub mod stock_message;
pub mod tdx7709;
pub mod tdx_0547;
pub mod tdx_0547_delivery;
pub mod tdx_0547_scheduler;
pub mod tdx_fin;
pub mod tdx_push_coalescer;
pub mod tdx_push_poll_policy;
pub mod tdx_wire_finance;

pub(crate) fn repository_fixture_path(relative: &str) -> std::path::PathBuf {
    let compiled = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    if compiled.exists() {
        return compiled;
    }
    std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join(relative))
        .filter(|path| path.exists())
        .unwrap_or(compiled)
}

pub use api::{StockAnswer, StockApi};
pub use auth_7100_client::{
    Auth7100ClientConfig, Auth7100LoginResult, Auth7100ProbeResult, DEFAULT_AUTH_HOST,
    DEFAULT_LOGIN_PORT, DEFAULT_PROBE_PORTS, STOCK_DICTIONARY_SHA256, build_auth_7100_login_packet,
    build_auth_7100_probe_packet, login_auth_6100, login_auth_7100, probe_auth_server,
};
pub use auth_7100_flow_matrix::{
    Auth7100FlowMatrix, Auth7100FlowPacket, Auth7100FlowSession, analyze_auth_7100_flow_matrix,
    analyze_auth_7100_flow_matrix_sample,
};
pub use auth_7100_prefix::{Auth7100PrefixHints, summarize_auth_7100_prefix_hints};
pub use auth_client_shell::{
    Auth7100ClientPacketAnalysis, Auth7100ClientPacketMatches, Auth7100ClientShellAnalysis,
    Auth7100DumpCandidateMatch, Auth7100ShellCorrelationAnalysis, Auth7100ShellDirectionAnalysis,
    Auth7100ShellSessionAnalysis, Auth7100ZoneCompareSummary,
    analyze_auth_7100_client_shell_from_pcap, analyze_auth_7100_client_shell_sample,
    analyze_auth_7100_shell_correlation_from_pcap, analyze_auth_7100_shell_correlation_sample,
};
pub use auth_download::{
    DownloadedServerConfig, DownloadedServerEntry, load_downloaded_server_config_sample,
    parse_downloaded_server_config_from_path, parse_server_entries_from_packet_bytes,
};
pub use auth_flow_sample::{
    Auth7100ServerPacketAnalysis, Auth7100ServerSampleAnalysis, Auth7100ZstdFrameSummary,
    analyze_auth_7100_server_flow, analyze_auth_7100_server_sample,
};
pub use debug_blob_compare::{
    BlobCompareResult, BlockCompareSummary, DiffRun, compare_blob_bytes, compare_blob_files,
};
pub use debug_pcap::{PcapFlowSummary, PcapPacketSample, PcapSummary, summarize_pcap_file};
pub use debug_quote::{
    ProtoProbeConfig, ProtoProbeEncoding, ProtoProbeResult, QuoteReplayConfig, QuoteReplayResult,
    QuoteReplaySegment, parse_probe_encoding, parse_quote_segments, probe_proto, replay_quote_file,
};
pub use debug_quote_scan::{
    BlockPatternSummary, Client10FrameSummary, CodeTableRequestSummary, CodeTableRowSummary,
    CodeTableSummary, QuoteFrameScanResult, Server16FrameSummary, scan_quote_frame_bytes,
    scan_quote_frame_file,
};
pub use debug_stream::{
    ByteProfile, CandidatePacket, NetPacketPrefix, NetPacketRun, NetPacketSliceSummary,
    StreamAnalyzeResult, ZlibHit, analyze_stream_file, parse_hex_like,
};
pub use legacy_panel::{
    DEFAULT_DATA_HOST as LEGACY_PANEL_DEFAULT_DATA_HOST,
    DEFAULT_DATA_PORT as LEGACY_PANEL_DEFAULT_DATA_PORT, LegacyAuthServer, LegacyAuthServerProbe,
    LegacyDataServer, LegacyDataServerProbe, LegacyPanelBootstrap, LegacyPanelConfig,
    LegacyPanelConnectResponse, LegacyPanelDisconnectResponse, LegacyPanelProbeResponse,
    LegacyPanelSaveResponse, LegacyProtocolProbe, LegacyTcpProbe, connect as connect_legacy_panel,
    disconnect as disconnect_legacy_panel, load_bootstrap as load_legacy_panel_bootstrap,
    password_info_url as legacy_panel_password_info_url, probe_servers as probe_legacy_servers,
    save_config as save_legacy_panel_config,
};
pub use local_2000::{
    Local2000CodeHit, Local2000FieldTupleCount, Local2000LargeObject, Local2000LargeSegment,
    Local2000LogAnalysis, Local2000PacketHit, Local2000RecvEntry, analyze_local_2000_log_file,
    analyze_local_2000_log_text,
};
pub use local_2000_vs_auth7100::{
    Auth7100ComparablePrefix, BridgeLayoutMatch, Local2000ComparablePrefix,
    Local2000LargeComparablePrefix, Local2000VsAuth7100Analysis, analyze_local_2000_vs_auth7100,
    analyze_local_2000_vs_auth7100_sample,
};
pub use packet::{
    DecodedBuySell610, DecodedF10Section, DecodedFinance, DecodedHead, DecodedKline, DecodedMarket,
    DecodedReport, DecodedSplit, DecodedSplitGroup, DecodedStkInfo, DecodedTick, Packet,
    parse_answer_buffer, summarize_callback, summarize_from_answer_buffer, to_wide_null,
    utf16_ptr_to_string,
};
pub use stock_message::{
    StockMessage, StockMessageChannel, StockMessageKind, StockTextFormat,
    interpret_callback_form_data, interpret_callback_ptr, interpret_sync_answer,
};
pub use tdx_0547::{
    Tdx0547Body, Tdx0547QuoteHead, Tdx0547Record,
    extra0_time_hint_seconds as tdx_0547_extra0_time_hint_seconds,
    format_extra0_time_hint as tdx_0547_format_extra0_time_hint,
    format_hhmmss_raw as tdx_0547_format_hhmmss_raw,
    format_public_hhmmss_raw as tdx_0547_format_public_hhmmss_raw,
    hhmmss_raw_to_seconds as tdx_0547_hhmmss_raw_to_seconds,
    is_valid_hhmmss_raw as tdx_0547_is_valid_hhmmss_raw, market_name as tdx_0547_market_name,
    matches_tdx_0547_query, normalize_quote_head_by_decimal_point as tdx_0547_normalize_quote_head,
    parse_tdx_0547_body, public_time_hhmmss as tdx_0547_public_time_hhmmss, query_tdx_0547_records,
    record_symbol as tdx_0547_record_symbol, xor93_decode as tdx_0547_xor93_decode,
};
pub use tdx_fin::{
    FIN_EXPORTED_PAYLOAD_LEN, FIN_EXPORTED_PAYLOAD_OFFSET, FIN_FILE_MAGIC, FIN_GETTER_GAP_SPECS,
    FIN_GETTER_SPECS, FIN_GETTER_TAIL_SPECS, FIN_GETTER_UNRESOLVED_IDS, FIN_KEY_SIZE,
    FIN_MIN_RECORD_SIZE, FinGetterSpec, SH_FIN_URL, SZ_FIN_URL, TdxFinFile, TdxFinRecord,
    fin_getter_gap_specs, fin_getter_specs, fin_getter_unresolved_ids, parse_fin_bytes,
    parse_fin_file, quarter_from_bao_gao, write_fin_csv,
};
pub use tdx_wire_finance::{
    TdxWireFinanceRecord, WIRE_FINANCE_FLOAT_FIELDS, WIRE_FINANCE_RECORD_SIZE,
    parse_wire_finance_batch_body, parse_wire_finance_record,
};
pub use tdx7709::{
    BOOTSTRAP0_BODY_HEX as TDX7709_BOOTSTRAP0_BODY_HEX,
    BOOTSTRAP2_BODY_HEX as TDX7709_BOOTSTRAP2_BODY_HEX, DEFAULT_HOST as TDX7709_DEFAULT_HOST,
    DEFAULT_PORT as TDX7709_DEFAULT_PORT, PROBE_HELLO_HEX as TDX7709_PROBE_HELLO_HEX,
    Tdx7709BootstrapPacket, Tdx7709CodeTableRecord, Tdx7709Config, Tdx7709F10CategoriesResult,
    Tdx7709F10Category, Tdx7709F10ContentResult, Tdx7709KlineBar, Tdx7709KlineResult,
    Tdx7709LiveQuoteResult, Tdx7709QuoteRequestItem, Tdx7709ServerFrame, Tdx7709Session,
    Tdx7709SyncResult, ascii_preview_from_zlib as tdx7709_ascii_preview_from_zlib,
    build_bootstrap_packets, build_probe_hello, fetch_f10_categories, fetch_f10_content,
    fetch_kline, fetch_live_quotes, open_tdx7709_session, sync_code_table, write_code_table_csv,
};
