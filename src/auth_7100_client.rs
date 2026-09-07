use crate::auth_download::{DownloadedServerEntry, parse_server_entries_from_packet_bytes};
use netzip_fullpull::{
    Official5188EmbeddedClientFrame, Official5188Frame, Official5188InitialCodeTables,
    Official5188InitializationStage, Official5188Kind, Official5188Session,
    Official5188SubscriptionEntry, Official5188SubscriptionEnvelope,
    build_official_5188_client_session_triplet,
    build_official_5188_primary_subscription_partitions,
    build_official_5188_receive_list_partitions, embedded_client_frames,
    official_5188_ack_source_prefix,
};
use serde::Serialize;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::io::{self, Cursor, ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::thread;
use std::time::{Duration, Instant};

const MAX_PACKET_LEN: usize = 2 * 1024 * 1024;
pub const DEFAULT_AUTH_HOST: &str = "121.41.70.217";
pub const DEFAULT_PROBE_PORTS: [u16; 2] = [6100, 7100];
pub const DEFAULT_LOGIN_PORT: u16 = 7100;
pub const STOCK_DICTIONARY_SHA256: &str =
    "8f44f49cbf10c8d203d9cabbda256da37c1f7d43e7f99b60c08d077c47b2bc68";
const STOCK_DICTIONARY_BYTES: &[u8] =
    include_bytes!("../docs/netzip_api_bin/NetzipAPI/StockC++/Stock.字典");
const INNER_PREFIX_HEX: &str = "a48bc18b0000000000000000000000000000000013000000000000000000000052030000520300000200000004000000000000000000000020000000f78b426c00007b76555f0000";
const INNER_SUFFIX_HEX: &str = "0400000006000000000000000000000026000000065290676f8ff64e0000ea819a5b494e0000050000000800000000000000000000002a000000d08f258446550d54f079000058005800fb79a8520000020000000a000000000000000000000026000000216a57570000a1806879a25b3762ef7a00000c00000004000000000000000200000034000000530074006f0063006b002e006500780065005f00e5651f670000b3f9030000000c00000004000000000000000200000034000000530074006f0063006b002e006500780065005f0048722c6700006a01000000000400000004000000000000000200000024000000cd645c4ffb7cdf7e0000e803000000000400000004000000000000000300000024000000a25b37620768c68b0000609350160000070000000400000000000000030000002a000000517f4596ce982e006500780065000000ec503f630000090000000400000000000000030000002e000000530074006f0063006b002e0064006c006c000000b622072a0000080000000400000000000000030000002c000000530074006f0063006b002e00575b78510000666148240000080000000400000000000000030000002c0000004753a77e4d916e7f2e0069006e0069000000330feb9100000c00000004000000000000000300000034000000287537625c000d67a1526856175268882e0069006e006900000099a51cb500000f0000000400000000000000030000003a000000fb7cdf7e5c00530074006f0063006b006400720076002e0064006c006c000000533e358b00000c00000004000000000000000300000034000000fb7cdf7e5c00496c575b807bfc6268882e007400780074000000cbd1cc8200000400000006000000000000000000000026000000ea81a8524753a77e0000337a9a5b4872000002000000040000000000000002000000200000001a9053900000000000000000";
const OUTER_PREFIX_HEX: &str = "517fdc7e055300000000000000000000000000000400000000000000000000006d0200006d02000002000000080000000000000000000000240000008b53297f00005a00530054004400000002000000040000000000000002000000200000007f95a65e0000c3010000000003000000040000000000000002000000220000009f537f95a65e000052030000000002000000c30100000000000009000000df01000070656e630000";
const PROBE_INNER_PREFIX_HEX: &str = "a48bc18b000000000000000000000000000000000a0000000000000000000000b2010000b20100000200000004000000000000000000000020000000f78b426c00004b6d1f900000";
const PROBE_INNER_SUFFIX_HEX: &str = "0400000006000000000000000000000026000000065290676f8ff64e0000ea819a5b494e0000050000000800000000000000000000002a000000d08f258446550d54f0790000c96cde5dfb79a8520000020000000a000000000000000000000026000000216a57570000a1806879a25b3762ef7a00000c00000004000000000000000200000034000000530074006f0063006b002e006500780065005f00e5651f6700005dd5030000000c00000004000000000000000200000034000000530074006f0063006b002e006500780065005f0048722c6700006901000000000400000004000000000000000200000024000000cd645c4ffb7cdf7e0000e803000000000400000004000000000000000300000024000000a25b37620768c68b0000eaadeaac0000";
const PROBE_OUTER_PREFIX_HEX: &str = "517fdc7e05530000000000000000000000000000040000000000000000000000a3010000a301000002000000080000000000000000000000240000008b53297f00005a00530054004400000002000000040000000000000002000000200000007f95a65e0000f9000000000003000000040000000000000002000000220000009f537f95a65e0000b2010000000002000000f900000000000000090000001501000070656e630000";
const FOLLOWUP_DOWNLOAD_HEX: &str = "517fdc7e055300000000000000000000000000000400000000000000000000000f0100000f010000020000000c0000000000000000000000280000008b53297f00005a00530054004400575b7851000002000000040000000000000002000000200000007f95a65e000061000000000003000000040000000000000002000000220000009f537f95a65e0000820000000000020000006100000000000000090000007d00000070656e63000028b52ffd2082c50200e4030b4e7d8f8765f64e000200821e3a00000000fb7cdf7e5c001a90be8fe14fa18068792e0069006e0069040300000020000000167ff75300000100000000000a00208baed9285913628b73600d24dfb4b6cb0cec6600390000";
const FOLLOWUP_FINISH_HEX: &str = "517fdc7e055300000000000000000000000000000400000000000000000000004d0100004d010000020000000c0000000000000000000000280000008b53297f00005a00530054004400575b7851000002000000040000000000000002000000200000007f95a65e00009f000000000003000000040000000000000002000000220000009f537f95a65e0000ae0100000000020000009f0000000000000009000000bb00000070656e63000028b52ffd60ae00ad040052061722904d73b79f525cd366f054ffbf12df56d8766a58c6f8261fee2dcb64c91c211022538df171902c4209347827941969441bcff428ea4d9385ab1456a26ab8cbb963998472ab5e688723042d29312bb1d92be465e4e3df86031a6760160084139e9908e61e90149f9d6d020e24d5586e808118438d4e19d6336187822a1c00e44b5533628bfb7296b71b0dec8633b29a10400000";
const FOLLOWUP_DOWNLOAD_NUMBER_OFFSET: usize = 124;
const FOLLOWUP_FINISH_NUMBER_OFFSET: usize = 116;
const FOLLOWUP_OUTER_TAIL_LEN: usize = 2;
const AUTH_IO_RETRY_SLEEP: Duration = Duration::from_millis(20);
const OPTIONAL_DICTIONARY_DRAIN: Duration = Duration::from_millis(400);
/// Wine keeps ten initialized 5188 sockets even though only the first seven
/// carry a `2a10` subscription in the current formal-account capture.
const OFFICIAL_5188_WINE_SLOT_COUNT: usize = 10;
const OFFICIAL_5188_PRIMARY_HEARTBEAT_PAYLOAD: [u8; 12] = [
    b'S', b'H', 0x00, 0x00, 0x00, 0x81, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
];

#[derive(Clone, Debug)]
pub struct Auth7100ClientConfig {
    pub host: String,
    pub port: u16,
    pub account: String,
    pub password: String,
    pub timeout: Duration,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Auth7100ProbeResult {
    pub endpoint: String,
    pub request_packet_length: usize,
    pub response_packet_length: usize,
    pub response_role: String,
    pub reachable: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Auth7100LoginResult {
    pub endpoint: String,
    pub account: String,
    pub status: String,
    pub authenticated: bool,
    pub response_packet_lengths: Vec<usize>,
    pub response_roles: Vec<String>,
    pub dictionary_length: Option<usize>,
    pub dictionary_sha256: Option<String>,
    pub decoded_response_lengths: Vec<usize>,
    pub login_success_confirmed: bool,
    pub quote_servers: Vec<DownloadedServerEntry>,
    pub selected_quote_endpoint: Option<String>,
    pub selected_7709_endpoint: Option<String>,
}

/// Credential-free IO diagnostic for the authentication control chain.
///
/// The display never includes account, password, raw packets, or payload bytes.
#[derive(Debug)]
struct AuthIoError {
    stage: &'static str,
    op: &'static str,
    endpoint: String,
    kind: &'static str,
    os_code: Option<i32>,
    attempt: u32,
    elapsed_ms: u128,
}

impl AuthIoError {
    fn from_io(
        stage: &'static str,
        op: &'static str,
        endpoint: &str,
        error: &io::Error,
        attempt: u32,
        elapsed: Duration,
    ) -> Self {
        Self {
            stage,
            op,
            endpoint: endpoint.to_string(),
            kind: classify_io_kind(error),
            os_code: error.raw_os_error(),
            attempt,
            elapsed_ms: elapsed.as_millis(),
        }
    }

    fn is_timeout(&self) -> bool {
        matches!(self.kind, "would_block" | "timed_out")
    }
}

impl fmt::Display for AuthIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "auth io: stage={} endpoint={} op={} kind={} os={} attempt={} elapsed_ms={}",
            self.stage,
            self.endpoint,
            self.op,
            self.kind,
            self.os_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "none".to_string()),
            self.attempt,
            self.elapsed_ms
        )
    }
}

impl Error for AuthIoError {}

fn classify_io_kind(error: &io::Error) -> &'static str {
    match error.kind() {
        ErrorKind::WouldBlock => "would_block",
        ErrorKind::Interrupted => "interrupted",
        ErrorKind::TimedOut => "timed_out",
        ErrorKind::ConnectionRefused => "connection_refused",
        ErrorKind::ConnectionReset => "connection_reset",
        ErrorKind::ConnectionAborted => "connection_aborted",
        ErrorKind::NotConnected => "not_connected",
        ErrorKind::UnexpectedEof => "unexpected_eof",
        ErrorKind::BrokenPipe => "broken_pipe",
        _ if error.raw_os_error() == Some(11) => "would_block",
        _ if error.raw_os_error() == Some(4) => "interrupted",
        _ => "other",
    }
}

fn io_error_is_retryable(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::WouldBlock | ErrorKind::Interrupted | ErrorKind::TimedOut
    ) || matches!(error.raw_os_error(), Some(4 | 11))
}

/// Owns the authenticated control socket that supplied endpoint discovery.
///
/// The socket must remain bound to the same login lifecycle while requesting
/// per-connection 5188 initialization. It does not contain reusable captured
/// initialization bytes and does not fall back to a supplementation endpoint.
pub struct Auth7100ControlSession {
    stream: TcpStream,
    login: Auth7100LoginResult,
}

/// Current-session values required by the observed 448-byte `加密包` request.
///
/// This type deliberately has no `Debug` or serialization implementation.
pub struct Auth7100LoginControlFields {
    pub local_ip: [u8; 4],
    pub mac_ascii: [u8; 12],
    pub account_permissions: [u8; 6],
    pub broker: [u8; 4],
    pub encryption_version: u32,
    pub user_id: u32,
    pub interface_version: u32,
}

/// Current-session values required by the observed 452-byte ABK request.
/// This type deliberately has no `Debug` or serialization implementation.
pub struct Auth7100AbkControlFields {
    pub is_64_bit: u32,
    pub major_version: u32,
    pub minor_version: u32,
    pub build_number: u32,
    pub total_memory: [u8; 8],
    pub user_id: u32,
    pub interface_version: u32,
}

/// Current-session values required by the observed variable-length ACK request.
/// The opaque data must come from the same authenticated lifecycle.
pub struct Auth7100AckControlFields {
    pub ack: u32,
    pub user_id: u32,
    pub interface_version: u32,
    pub data: Vec<u8>,
}

/// Builds the runtime ACK manifest from local file, market and data-id state.
/// The returned text is caller-owned metadata and is never logged here.
pub fn build_ack_manifest_text(
    file_infos: &[(String, u32)],
    market_rows: &[String],
    sdid_rows: &[(u32, u32)],
) -> String {
    let crlf = "\r\n";
    let mut out = String::from("SFLogInPack=ACK");
    out.push_str(crlf);
    out.push_str("<MarketInfo>\r\n");
    out.push_str("m_wMarkeId|m_wMarketAttr|m_dwServIp|m_dwStaticVer|m_dwStaticDay|m_dwChangeNum|m_dwInitTime|m_dwStaticExVer|m_dwStaticExDay|\r\n");
    for row in market_rows {
        out.push_str(row);
        out.push_str(crlf);
    }
    out.push_str("</MarketInfo>\r\n<FileInfo>\r\n");
    out.push_str("m_charCDir|m_dwFileCrcCode|\r\n");
    for (path, crc) in file_infos {
        out.push_str(path);
        out.push('|');
        out.push_str(&crc.to_string());
        out.push_str("|\r\n");
    }
    out.push_str("</FileInfo>\r\n<SDidsCrc>\r\n");
    out.push_str("m_dwDid|m_dwCrc|\r\n");
    for (did, crc) in sdid_rows {
        out.push_str(&did.to_string());
        out.push('|');
        out.push_str(&crc.to_string());
        out.push_str("|\r\n");
    }
    out.push_str("</SDidsCrc>\r\n");
    out
}

/// Returns the nine structural market rows observed in the vendor ACK
/// manifest. Dynamic version/status columns intentionally remain zero until
/// supplied by the current authenticated session.
pub fn default_ack_market_rows() -> Vec<String> {
    netzip_fullpull::auth_manifest::default_ack_market_rows()
}

/// Builds an ACK manifest with the protocol's structural market rows.
/// Callers may still use `build_ack_manifest_text` when session-specific rows
/// are available.
pub fn build_default_ack_manifest_text(
    file_infos: &[(String, u32)],
    sdid_rows: &[(u32, u32)],
) -> String {
    build_ack_manifest_text(file_infos, &default_ack_market_rows(), sdid_rows)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Official5188InitTriplet {
    pub login: Official5188Frame,
    pub abk: Official5188Frame,
    pub ack: Official5188Frame,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Official5188InterleavedInitialization {
    pub login_response: Official5188Frame,
    pub abk_response: Official5188Frame,
    pub ack_response: Official5188Frame,
    pub post_initialization_frame_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Official5188InterleavedWithCodeTables {
    pub initialization: Official5188InterleavedInitialization,
    pub code_tables: Official5188InitialCodeTables,
}

pub struct Official5188SlotSession {
    pub slot: usize,
    pub login_number: u32,
    pub subscription_entries: usize,
    pub session: Official5188Session,
    pub initialization: Official5188InterleavedWithCodeTables,
}

#[derive(Clone, Copy)]
enum Official5188ControlStage {
    Login,
    Abk,
    Ack,
}

impl Official5188ControlStage {
    fn request_name(self) -> &'static str {
        match self {
            Self::Login => "大智慧C_登录包",
            Self::Abk => "大智慧C_ABK",
            Self::Ack => "大智慧C_ACK",
        }
    }

    fn payload_len(self) -> usize {
        match self {
            Self::Login => 95,
            Self::Abk => 94,
            Self::Ack => 67,
        }
    }

    /// The ACK-stage 3610 length scales with the manifest the client
    /// reported: 67B for the captured 1387B manifest, 44B observed live for
    /// a 592B generated manifest, 62B observed live with default ack rows.
    /// Login/ABK stay within their observed session-value variants.
    fn accepts_payload_len(self, payload_len: usize) -> bool {
        match self {
            Self::Ack => (40..=80).contains(&payload_len),
            Self::Abk => matches!(payload_len, 94 | 106),
            Self::Login => payload_len == 95,
        }
    }
}

struct ControlField<'a> {
    label: String,
    value: &'a [u8],
}

impl Auth7100ControlSession {
    pub fn login_result(&self) -> &Auth7100LoginResult {
        &self.login
    }

    pub fn into_login_result(self) -> Auth7100LoginResult {
        self.login
    }

    /// Opens the selected official 5188 data channel from this authenticated
    /// login lifecycle. The endpoint is never taken from the 7709 supplement
    /// configuration and authentication must have been confirmed first.
    pub fn connect_selected_official_5188(
        &self,
        timeout: Duration,
    ) -> Result<Official5188Session, Box<dyn Error>> {
        if !self.login.login_success_confirmed {
            return Err("cannot open 5188 before authenticated 7100 login".into());
        }
        let endpoint = self
            .login
            .selected_quote_endpoint
            .as_deref()
            .ok_or("authenticated login did not provide a quote endpoint")?;
        Official5188Session::connect_authenticated(endpoint, timeout, true).map_err(Into::into)
    }

    /// Builds the per-connection login control fields from this session's own
    /// L1 route entry. Returns `None` when the retained route set has no
    /// matching 5188 metadata, so callers fail closed instead of sending zeros.
    pub fn current_login_control_fields(
        &self,
        local_ip: [u8; 4],
        mac_ascii: [u8; 12],
    ) -> Option<Auth7100LoginControlFields> {
        login_control_fields_from_result(&self.login, local_ip, mac_ascii)
    }

    /// Exchanges one complete vendor `网络包` on the authenticated socket.
    pub fn exchange_current_session_packet(
        &mut self,
        request: &[u8],
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        validate_complete_netpacket(request)?;
        self.stream.write_all(request)?;
        self.stream.flush()?;
        read_netpacket(&mut self.stream)
    }

    /// Exchanges one control request and extracts complete known 5188 client
    /// frames without promoting them to an accepted handshake.
    pub fn exchange_official_5188_initialization_candidates(
        &mut self,
        request: &[u8],
    ) -> Result<Vec<Official5188EmbeddedClientFrame>, Box<dyn Error>> {
        let response = self.exchange_current_session_packet(request)?;
        Ok(embedded_client_frames(&response))
    }

    /// Exchanges one control request and requires exactly one Wine `3610`
    /// candidate with the expected payload length.
    pub fn exchange_expected_official_5188_init(
        &mut self,
        request: &[u8],
        expected_payload_len: usize,
    ) -> Result<Official5188Frame, Box<dyn Error>> {
        let candidates = self.exchange_official_5188_initialization_candidates(request)?;
        select_expected_official_5188_init(candidates, expected_payload_len)
    }

    /// Collects the three evidence-backed `3610` stages on this control socket.
    ///
    /// This compatibility helper is suitable for offline fixtures only. A live
    /// Wine-equivalent session must call the three stage methods separately and
    /// interleave them with their 5188 `3110`/`3210` responses.
    pub fn exchange_official_5188_init_triplet(
        &mut self,
        login_request: &[u8],
        abk_request: &[u8],
        ack_request: &[u8],
    ) -> Result<Official5188InitTriplet, Box<dyn Error>> {
        Ok(Official5188InitTriplet {
            login: self.exchange_official_5188_control_stage(
                login_request,
                Official5188ControlStage::Login,
            )?,
            abk: self
                .exchange_official_5188_control_stage(abk_request, Official5188ControlStage::Abk)?,
            ack: self
                .exchange_official_5188_control_stage(ack_request, Official5188ControlStage::Ack)?,
        })
    }

    /// Exchanges the login-packet control stage and returns its 95-byte
    /// `3610`. Send it immediately on the assigned 5188 connection.
    pub fn exchange_official_5188_login_init(
        &mut self,
        request: &[u8],
    ) -> Result<Official5188Frame, Box<dyn Error>> {
        self.exchange_official_5188_control_stage(request, Official5188ControlStage::Login)
    }

    /// Exchanges the ABK control stage and returns its 94-byte `3610`.
    pub fn exchange_official_5188_abk_init(
        &mut self,
        request: &[u8],
    ) -> Result<Official5188Frame, Box<dyn Error>> {
        self.exchange_official_5188_control_stage(request, Official5188ControlStage::Abk)
    }

    /// Exchanges the ACK control stage and returns its 67-byte `3610`.
    pub fn exchange_official_5188_ack_init(
        &mut self,
        request: &[u8],
    ) -> Result<Official5188Frame, Box<dyn Error>> {
        self.exchange_official_5188_control_stage(request, Official5188ControlStage::Ack)
    }

    /// Runs one Wine-shaped 7100/5188 initialization as an interleaved state
    /// machine without inventing the still-opaque ACK or `2d10` bytes.
    ///
    /// `build_ack_request` is invoked only after the two `3110` responses are
    /// available. `build_post_frames` is invoked only after the `3210`
    /// response. Both builders must derive bytes from the current lifecycle;
    /// captured blobs are not accepted or selected by this method.
    pub fn initialize_official_5188_interleaved<BuildAck, BuildPost>(
        &mut self,
        data_session: &mut Official5188Session,
        login_request: &[u8],
        abk_request: &[u8],
        build_ack_request: BuildAck,
        build_post_frames: BuildPost,
    ) -> Result<Official5188InterleavedInitialization, Box<dyn Error>>
    where
        BuildAck: FnOnce(&[u8]) -> Result<Vec<u8>, Box<dyn Error>>,
        BuildPost: FnOnce(
            &Official5188Frame,
            &Official5188Frame,
            &Official5188Frame,
        ) -> Result<Vec<Official5188Frame>, Box<dyn Error>>,
    {
        let login = self.exchange_official_5188_login_init(login_request)?;
        let login_response = data_session
            .exchange_initialization_stage(Official5188InitializationStage::Login, &login)
            .map_err(|error| format!("5188 login stage failed: {error}"))?;

        let abk = self.exchange_official_5188_abk_init(abk_request)?;
        let abk_response = data_session
            .exchange_initialization_stage(Official5188InitializationStage::Abk, &abk)
            .map_err(|error| format!("5188 ABK stage failed: {error}"))?;

        let ack_source = official_5188_ack_source_prefix(&abk_response)?;
        let ack_request = build_ack_request(ack_source)?;
        let ack = self.exchange_official_5188_ack_init(&ack_request)?;
        let ack_response = data_session
            .exchange_initialization_stage(Official5188InitializationStage::Ack, &ack)
            .map_err(|error| format!("5188 ACK stage failed: {error}"))?;

        let post_frames = build_post_frames(&login_response, &abk_response, &ack_response)?;
        data_session
            .send_wine_post_initialization(&post_frames)
            .map_err(|error| format!("5188 post-initialization failed: {error}"))?;

        Ok(Official5188InterleavedInitialization {
            login_response,
            abk_response,
            ack_response,
            post_initialization_frame_count: post_frames.len(),
        })
    }

    /// Interleaved lifecycle that consumes the current connection's four `0104`
    /// objects after `3210` and exposes decoded headers to the post-init builder.
    pub fn initialize_official_5188_interleaved_with_code_tables<BuildAck, BuildPost>(
        &mut self,
        data_session: &mut Official5188Session,
        login_request: &[u8],
        abk_request: &[u8],
        build_ack_request: BuildAck,
        build_post_frames: BuildPost,
    ) -> Result<Official5188InterleavedWithCodeTables, Box<dyn Error>>
    where
        BuildAck: FnOnce(&[u8]) -> Result<Vec<u8>, Box<dyn Error>>,
        BuildPost: FnOnce(
            &Official5188Frame,
            &Official5188Frame,
            &Official5188Frame,
            &Official5188InitialCodeTables,
        ) -> Result<Vec<Official5188Frame>, Box<dyn Error>>,
    {
        let login = self.exchange_official_5188_login_init(login_request)?;
        let login_response = data_session
            .exchange_initialization_stage(Official5188InitializationStage::Login, &login)
            .map_err(|error| format!("5188 login stage failed: {error}"))?;
        let abk = self.exchange_official_5188_abk_init(abk_request)?;
        let abk_response = data_session
            .exchange_initialization_stage(Official5188InitializationStage::Abk, &abk)
            .map_err(|error| format!("5188 ABK stage failed: {error}"))?;
        let ack_source = official_5188_ack_source_prefix(&abk_response)?;
        let ack_request = build_ack_request(ack_source)?;
        let ack = self.exchange_official_5188_ack_init(&ack_request)?;
        let ack_response = data_session
            .exchange_initialization_stage(Official5188InitializationStage::Ack, &ack)
            .map_err(|error| format!("5188 ACK stage failed: {error}"))?;
        let code_tables = data_session
            .receive_initial_code_tables()
            .map_err(|error| format!("5188 code-table collection failed: {error}"))?;
        let post_frames =
            build_post_frames(&login_response, &abk_response, &ack_response, &code_tables)?;
        data_session
            .send_wine_post_initialization(&post_frames)
            .map_err(|error| format!("5188 post-initialization failed: {error}"))?;
        Ok(Official5188InterleavedWithCodeTables {
            initialization: Official5188InterleavedInitialization {
                login_response,
                abk_response,
                ack_response,
                post_initialization_frame_count: post_frames.len(),
            },
            code_tables,
        })
    }

    /// Connects the ten 5188 sockets kept by Wine.
    ///
    /// Wine keeps about ten ESTAB sockets and sends one `2a10` on seven of
    /// them. The first seven slots carry the evidence-backed subscription
    /// partitions (five primary plus P6/P7); the final three still complete
    /// the 3610/2d10 initialization but send no fabricated subscription.
    /// Every slot gets unique 7100 control numbers.
    pub fn connect_and_initialize_official_5188_slots(
        &mut self,
        timeout: Duration,
        receive_codes: &BTreeSet<String>,
    ) -> Result<Vec<Official5188SlotSession>, Box<dyn Error>> {
        let candidates: Vec<String> = self
            .login
            .quote_servers
            .iter()
            .filter_map(|entry| {
                if entry.main_port == 5188 {
                    Some(format_endpoint(&entry.host, entry.main_port))
                } else if entry.secondary_port == 5188 {
                    Some(format_endpoint(&entry.host, entry.secondary_port))
                } else {
                    None
                }
            })
            .collect();
        let original = self.login.selected_quote_endpoint.clone();
        let mut errors = Vec::new();
        for endpoint in candidates {
            self.login.selected_quote_endpoint = Some(endpoint.clone());
            match self.connect_and_initialize_official_5188_slots_at_selected_endpoint(
                timeout,
                receive_codes,
            ) {
                Ok(slots) => return Ok(slots),
                Err(error) => errors.push(format!("{endpoint}: {error}")),
            }
        }
        self.login.selected_quote_endpoint = original;
        if errors.is_empty() {
            Err("authenticated login did not provide a 5188 quote endpoint".into())
        } else {
            Err(format!(
                "all authenticated 5188 endpoints failed: {}",
                errors.join("; ")
            )
            .into())
        }
    }

    fn connect_and_initialize_official_5188_slots_at_selected_endpoint(
        &mut self,
        timeout: Duration,
        receive_codes: &BTreeSet<String>,
    ) -> Result<Vec<Official5188SlotSession>, Box<dyn Error>> {
        let first = self.connect_and_initialize_official_5188_slot(timeout, receive_codes, 0)?;
        let plan = official_5188_subscription_partitions(
            &first.initialization.code_tables,
            receive_codes,
        )?;
        let slots = official_5188_wine_slot_indexes(plan.len())?;
        let mut opened = vec![first];
        for slot in slots.skip(1) {
            match self.connect_and_initialize_official_5188_slot(timeout, receive_codes, slot) {
                Ok(session) => opened.push(session),
                Err(error) => {
                    for session in &mut opened {
                        let _ = session.session.stop();
                    }
                    return Err(error);
                }
            }
        }
        Ok(opened)
    }

    fn connect_and_initialize_official_5188_slot(
        &mut self,
        timeout: Duration,
        receive_codes: &BTreeSet<String>,
        slot: usize,
    ) -> Result<Official5188SlotSession, Box<dyn Error>> {
        let mut data = self.connect_selected_official_5188(timeout)?;
        let local_ip = outbound_ipv4(&self.login.endpoint);
        let mac_ascii = netzip_fullpull::auth_7100::outbound_mac_ascii(local_ip);
        let login_fields = self
            .current_login_control_fields(local_ip, mac_ascii)
            .ok_or("authenticated login did not provide L1 5188 route metadata")?;
        let (login_number, abk_number, ack_number) = official_5188_slot_control_numbers(slot)?;
        let login_request =
            build_candidate_official_5188_login_control_packet(&login_fields, login_number)?;
        let abk_request = build_candidate_official_5188_abk_control_packet(
            &Auth7100AbkControlFields {
                is_64_bit: 0x20,
                major_version: 8,
                minor_version: 0x32,
                build_number: 0x5956,
                total_memory: [0x00, 0x40, 0x6a, 0xfb, 0x0f, 0x00, 0x00, 0x00],
                user_id: 0,
                interface_version: 858,
            },
            abk_number,
        )?;
        let receive_codes_for_frames = receive_codes.clone();
        let initialization = self.initialize_official_5188_interleaved_with_code_tables(
            &mut data,
            &login_request,
            &abk_request,
            move |_ack_source| {
                let file_infos = official_5188_ack_file_infos();
                let manifest = build_default_ack_manifest_text(&file_infos, &[]);
                let ack = Auth7100AckControlFields {
                    ack: u32::from_le_bytes(*b"penc"),
                    user_id: 0,
                    interface_version: 858,
                    data: manifest.into_bytes(),
                };
                build_candidate_official_5188_ack_control_packet(&ack, ack_number)
            },
            move |_, _, _, code_tables| {
                official_5188_slot_post_initialization_frames(
                    code_tables,
                    &receive_codes_for_frames,
                    slot,
                )
                .map(|(frames, _, _)| frames)
            },
        );
        match initialization {
            Ok(result) => {
                let subscription_entries =
                    official_5188_subscription_partitions(&result.code_tables, receive_codes)?
                        .get(slot)
                        .map(Vec::len)
                        .unwrap_or(0);
                Ok(Official5188SlotSession {
                    slot,
                    login_number,
                    subscription_entries,
                    session: data,
                    initialization: result,
                })
            }
            Err(error) => {
                let _ = data.stop();
                Err(format!("5188 slot {slot} initialization failed: {error}").into())
            }
        }
    }

    fn exchange_official_5188_control_stage(
        &mut self,
        request: &[u8],
        stage: Official5188ControlStage,
    ) -> Result<Official5188Frame, Box<dyn Error>> {
        let request_decoded = decode_dictionary_response(request, STOCK_DICTIONARY_BYTES)?;
        let request_fields = parse_control_object(&request_decoded)?;
        require_control_utf16(&request_fields, "请求", stage.request_name())?;
        let request_number = require_control_u32(&request_fields, "编号")?;

        let response = self.exchange_current_session_packet(request)?;
        let response_decoded = decode_dictionary_response(&response, STOCK_DICTIONARY_BYTES)?;
        let response_fields = parse_control_object(&response_decoded)?;
        if response_fields.len() != 4
            || response_fields
                .iter()
                .map(|field| field.label.as_str())
                .ne(["请求", "来源", "应答编号", "数据"])
        {
            return Err("5188 control response does not have the verified four-field shape".into());
        }
        require_control_utf16(&response_fields, "请求", stage.request_name())?;
        require_control_utf16(&response_fields, "来源", "认证服务器")?;
        let answer_number = require_control_u32(&response_fields, "应答编号")?;
        if answer_number != request_number {
            return Err(format!(
                "5188 control response number mismatch: request {request_number}, answer {answer_number}"
            )
            .into());
        }
        let data = require_control_field(&response_fields, "数据")?.value;
        let frame = Official5188Frame::decode(data)?;
        if frame.kind != Official5188Kind::CLIENT_INIT {
            return Err(format!(
                "{} control response has unexpected frame kind {}, payload {} bytes",
                stage.request_name(),
                frame.kind.wire_hex(),
                frame.payload.len()
            )
            .into());
        }
        if !stage.accepts_payload_len(frame.payload.len()) {
            return Err(format!(
                "{} control response requires one {}-byte 3610 payload, got {} with {} bytes",
                stage.request_name(),
                stage.payload_len(),
                frame.kind.wire_hex(),
                frame.payload.len()
            )
            .into());
        }
        Ok(frame)
    }
}

/// Wine login packets use `编号` 2..11 across ten sockets. ABK/ACK then continue
/// as 12..21 and 22..31.
pub fn official_5188_slot_control_numbers(slot: usize) -> Result<(u32, u32, u32), Box<dyn Error>> {
    if slot >= OFFICIAL_5188_WINE_SLOT_COUNT {
        return Err(format!("5188 slot {slot} exceeds the ten observed Wine sockets").into());
    }
    let slot = u32::try_from(slot).map_err(|_| "5188 slot index exceeds u32")?;
    Ok((2 + slot, 12 + slot, 22 + slot))
}

fn official_5188_wine_slot_indexes(
    subscription_partition_count: usize,
) -> Result<std::ops::Range<usize>, Box<dyn Error>> {
    if subscription_partition_count > OFFICIAL_5188_WINE_SLOT_COUNT {
        return Err(format!(
            "5188 subscription plan has {subscription_partition_count} partitions, exceeding the ten observed Wine sockets"
        )
        .into());
    }
    Ok(0..OFFICIAL_5188_WINE_SLOT_COUNT)
}

fn official_5188_subscription_partitions(
    code_tables: &Official5188InitialCodeTables,
    receive_codes: &BTreeSet<String>,
) -> Result<Vec<Vec<Official5188SubscriptionEntry>>, Box<dyn Error>> {
    let table = |market: [u8; 2]| {
        code_tables
            .code_tables
            .iter()
            .find(|table| table.market == market)
            .ok_or_else(|| {
                format!(
                    "missing {}{} code table",
                    market[0] as char, market[1] as char
                )
            })
    };
    if receive_codes.is_empty() {
        Ok(
            build_official_5188_primary_subscription_partitions(table(*b"SH")?, table(*b"SZ")?)?
                .into_iter()
                .collect(),
        )
    } else {
        Ok(build_official_5188_receive_list_partitions(
            table(*b"SH")?,
            table(*b"SZ")?,
            receive_codes,
        )?
        .partitions)
    }
}

fn official_5188_slot_post_initialization_frames(
    code_tables: &Official5188InitialCodeTables,
    receive_codes: &BTreeSet<String>,
    slot: usize,
) -> Result<(Vec<Official5188Frame>, usize, usize), Box<dyn Error>> {
    let triplet = build_official_5188_client_session_triplet(&code_tables.headers)
        .map_err(|error| -> Box<dyn Error> { error.into() })?;
    let partitions = official_5188_subscription_partitions(code_tables, receive_codes)?;
    official_5188_slot_tail(triplet.to_vec(), &partitions, slot)
}

fn official_5188_slot_tail(
    mut frames: Vec<Official5188Frame>,
    partitions: &[Vec<Official5188SubscriptionEntry>],
    slot: usize,
) -> Result<(Vec<Official5188Frame>, usize, usize), Box<dyn Error>> {
    official_5188_wine_slot_indexes(partitions.len())?;
    if slot >= OFFICIAL_5188_WINE_SLOT_COUNT {
        return Err(format!("5188 slot {slot} exceeds the ten observed Wine sockets").into());
    }
    let entries = partitions.get(slot).map(Vec::as_slice).unwrap_or_default();
    if slot == 0 {
        frames.push(Official5188Frame {
            kind: Official5188Kind::CLIENT_HEARTBEAT,
            metadata: [0; 4],
            payload: OFFICIAL_5188_PRIMARY_HEARTBEAT_PAYLOAD.to_vec(),
        });
    }
    if !entries.is_empty() {
        frames.push(
            Official5188SubscriptionEnvelope::from_current_entries(entries)
                .map_err(|error| -> Box<dyn Error> { error.into() })?,
        );
    }
    Ok((frames, entries.len(), partitions.len()))
}

fn parse_control_object(bytes: &[u8]) -> Result<Vec<ControlField<'_>>, Box<dyn Error>> {
    parse_control_object_with_root(bytes, "加密包")
}

fn parse_control_object_with_root<'a>(
    bytes: &'a [u8],
    expected_root: &str,
) -> Result<Vec<ControlField<'a>>, Box<dyn Error>> {
    if bytes.len() < 40 || decode_utf16_prefix(&bytes[..20]) != expected_root {
        return Err(format!("control object is not a complete {expected_root} header").into());
    }
    let field_count = u32::from_le_bytes(bytes[20..24].try_into()?) as usize;
    if bytes[24..32] != [0; 8]
        || u32::from_le_bytes(bytes[32..36].try_into()?) as usize != bytes.len()
        || u32::from_le_bytes(bytes[36..40].try_into()?) as usize != bytes.len()
    {
        return Err("control object header lengths do not close exactly".into());
    }

    let mut fields = Vec::with_capacity(field_count);
    let mut offset = 40usize;
    for _ in 0..field_count {
        let header = bytes
            .get(offset..offset + 20)
            .ok_or("control field header is truncated")?;
        let value_len = u32::from_le_bytes(header[4..8].try_into()?) as usize;
        let span_len = u32::from_le_bytes(header[16..20].try_into()?) as usize;
        let end = offset
            .checked_add(span_len)
            .filter(|end| *end <= bytes.len())
            .ok_or("control field span is invalid")?;
        let tail = bytes
            .get(offset + 20..end)
            .ok_or("control field body is truncated")?;
        let label_len = tail
            .chunks_exact(2)
            .position(|word| word == [0, 0])
            .map(|words| words * 2)
            .ok_or("control field label is not terminated")?;
        let label = decode_utf16_prefix(&tail[..label_len]);
        let value_start = offset + 20 + label_len + 2;
        let value_end = value_start
            .checked_add(value_len)
            .ok_or("control field value length overflow")?;
        if value_end.checked_add(2) != Some(end) || bytes[value_end..end] != [0, 0] {
            return Err("control field value does not close its declared span".into());
        }
        fields.push(ControlField {
            label,
            value: &bytes[value_start..value_end],
        });
        offset = end;
    }
    if offset != bytes.len() {
        return Err("control object has trailing bytes after declared fields".into());
    }
    Ok(fields)
}

fn require_control_field<'a>(
    fields: &'a [ControlField<'a>],
    label: &str,
) -> Result<&'a ControlField<'a>, Box<dyn Error>> {
    let mut matching = fields.iter().filter(|field| field.label == label);
    let field = matching
        .next()
        .ok_or_else(|| format!("control object is missing field {label}"))?;
    if matching.next().is_some() {
        return Err(format!("control object repeats field {label}").into());
    }
    Ok(field)
}

fn require_control_u32(fields: &[ControlField<'_>], label: &str) -> Result<u32, Box<dyn Error>> {
    let value = require_control_field(fields, label)?.value;
    Ok(u32::from_le_bytes(value.try_into().map_err(|_| {
        format!("control field {label} is not a u32")
    })?))
}

fn require_control_utf16(
    fields: &[ControlField<'_>],
    label: &str,
    expected: &str,
) -> Result<(), Box<dyn Error>> {
    let value = require_control_field(fields, label)?.value;
    if value != utf16_bytes(expected) {
        return Err(format!("control field {label} has an unexpected value").into());
    }
    Ok(())
}

fn select_expected_official_5188_init(
    candidates: Vec<Official5188EmbeddedClientFrame>,
    expected_payload_len: usize,
) -> Result<Official5188Frame, Box<dyn Error>> {
    let mut matching = candidates.into_iter().filter(|candidate| {
        candidate.frame.kind == Official5188Kind::CLIENT_INIT
            && candidate.frame.payload.len() == expected_payload_len
    });
    let frame = matching.next().ok_or_else(|| {
        format!("control response contained no {expected_payload_len}-byte 3610 candidate")
    })?;
    if matching.next().is_some() {
        return Err(format!(
            "control response contained multiple {expected_payload_len}-byte 3610 candidates"
        )
        .into());
    }
    Ok(frame.frame)
}

pub fn login_auth_7100(
    config: &Auth7100ClientConfig,
) -> Result<Auth7100LoginResult, Box<dyn Error>> {
    login_auth_sequence(config, None)
}

/// Performs the formal login flow on the configured authentication endpoint and decodes the
/// dictionary responses with the verified installed `Stock.字典` bytes.
pub fn login_auth_6100(
    config: &Auth7100ClientConfig,
    dictionary: &[u8],
) -> Result<Auth7100LoginResult, Box<dyn Error>> {
    login_auth_sequence(config, Some(dictionary))
}

/// Runs formal login with the verified repository dictionary without exposing its bytes.
pub fn login_auth_6100_with_verified_dictionary(
    config: &Auth7100ClientConfig,
) -> Result<Auth7100LoginResult, Box<dyn Error>> {
    login_auth_sequence(config, Some(STOCK_DICTIONARY_BYTES))
}

/// Performs verified login while retaining the selected control connection.
pub fn connect_auth_control_with_verified_dictionary(
    config: &Auth7100ClientConfig,
) -> Result<Auth7100ControlSession, Box<dyn Error>> {
    let (stream, login) = connect_auth_sequence(config, Some(STOCK_DICTIONARY_BYTES))?;
    Ok(Auth7100ControlSession { stream, login })
}

/// Sends the same credential-bearing `认证|测速` packet to one authentication candidate.
pub fn probe_auth_server(
    config: &Auth7100ClientConfig,
) -> Result<Auth7100ProbeResult, Box<dyn Error>> {
    if config.account.trim().is_empty() || config.password.is_empty() {
        return Err("authentication credentials are incomplete".into());
    }

    let endpoint = format!("{}:{}", config.host, config.port);
    let deadline = Instant::now() + config.timeout;
    let mut stream = connect_auth_stream(&endpoint, deadline)?;

    let probe = build_auth_7100_probe_packet(&config.account, &config.password)?;
    write_all_timed(&mut stream, &probe, deadline, "probe_write", &endpoint)?;
    let response = read_netpacket_timed(&mut stream, deadline, "probe_read", &endpoint)?;
    let response_role = classify_response(&response)?;
    if response_role != "auth_probe_response" {
        return Err(format!("unexpected authentication probe response: {response_role}").into());
    }

    Ok(Auth7100ProbeResult {
        endpoint,
        request_packet_length: probe.len(),
        response_packet_length: response.len(),
        response_role: response_role.to_string(),
        reachable: true,
    })
}

fn login_auth_sequence(
    config: &Auth7100ClientConfig,
    dictionary: Option<&[u8]>,
) -> Result<Auth7100LoginResult, Box<dyn Error>> {
    connect_auth_sequence(config, dictionary).map(|(_, login)| login)
}

fn connect_auth_sequence(
    config: &Auth7100ClientConfig,
    dictionary: Option<&[u8]>,
) -> Result<(TcpStream, Auth7100LoginResult), Box<dyn Error>> {
    if config.account.trim().is_empty() || config.password.is_empty() {
        return Err("authentication credentials are incomplete".into());
    }

    let dictionary = dictionary.unwrap_or(STOCK_DICTIONARY_BYTES);
    let dictionary_sha256 = Some(validate_stock_dictionary(dictionary)?);
    let endpoint = format!("{}:{}", config.host, config.port);
    let deadline = Instant::now() + config.timeout;
    let mut stream = connect_auth_stream(&endpoint, deadline)?;

    // Current live login is the 19-field `认证/请求登录` manifest with ordinary
    // level-3 ZSTD. The August hex template is stale and the 12-field dictionary
    // `加密包` belongs only to per-5188-connection initialization.
    let login =
        netzip_fullpull::auth_7100::build_real_login_packet(&config.account, &config.password)
            .map_err(|error| error.to_string())?;
    write_all_timed(&mut stream, &login, deadline, "login_write", &endpoint)?;

    let mut packets = Vec::new();
    let mut roles = Vec::new();
    let login_packet = read_until_role(
        &mut stream,
        deadline,
        "login_read",
        &endpoint,
        "auth_login",
        &mut packets,
        &mut roles,
    )?;

    thread::sleep(Duration::from_millis(250));
    let download_request =
        netzip_fullpull::auth_7100::build_download_file_request("系统\\大智慧服务器L1.ini", 1)
            .map_err(|error| error.to_string())?;
    write_all_timed(
        &mut stream,
        &download_request,
        deadline,
        "download_write",
        &endpoint,
    )?;
    let download_packet = read_until_role(
        &mut stream,
        deadline,
        "download_read",
        &endpoint,
        "download_file",
        &mut packets,
        &mut roles,
    )?;

    let mut dictionary_packet = drain_optional_dictionary(
        &mut stream,
        deadline,
        "dictionary_read",
        &endpoint,
        &mut packets,
        &mut roles,
    )?;
    if dictionary_packet.is_none() && roles.contains(&"auth_probe_response") {
        let finish = build_auth_7100_followup_finish_packet(2)?;
        write_all_timed(&mut stream, &finish, deadline, "finish_write", &endpoint)?;
        dictionary_packet = drain_optional_dictionary(
            &mut stream,
            deadline,
            "dictionary_read",
            &endpoint,
            &mut packets,
            &mut roles,
        )?;
    }

    if !login_core_sequence_is_valid(&roles) {
        return Err(format!("unexpected authentication login response sequence: {roles:?}").into());
    }

    let first_decoded = decode_login_object(&login_packet, dictionary)?;
    let first_fields = parse_control_object_with_root(&first_decoded, "认证")?;
    if first_fields.len() != 15 {
        return Err(format!(
            "initial login response requires the verified 15-field shape, got {} fields",
            first_fields.len()
        )
        .into());
    }
    require_control_utf16(&first_fields, "提示信息", "登录成功")?;

    let mut decoded_response_lengths = vec![first_decoded.len()];
    if let Some(dictionary_packet) = dictionary_packet.as_ref() {
        let third_decoded = decode_control_response(dictionary_packet, dictionary, "加密解密")?;
        let third_fields = parse_control_object_with_root(&third_decoded, "加密解密")?;
        if third_fields.len() != 4
            || third_fields.iter().map(|field| field.label.as_str()).ne([
                "请求",
                "来源",
                "应答编号",
                "数据",
            ])
        {
            return Err("Tdx_Encrypt response does not have the verified four-field shape".into());
        }
        require_control_utf16(&third_fields, "请求", "Tdx_Encrypt")?;
        if require_control_u32(&third_fields, "应答编号")? != 2 {
            return Err("Tdx_Encrypt response number mismatch".into());
        }
        decoded_response_lengths.push(third_decoded.len());
    }

    let quote_servers = parse_server_entries_from_packet_bytes(&download_packet)?;
    if quote_servers.is_empty() {
        return Err("decoded download response contained no active server entries".into());
    }
    let selected_quote_endpoint = select_server_endpoint(&quote_servers, 5188);
    let selected_7709_endpoint = select_server_endpoint(&quote_servers, 7709);
    let authenticated = roles.contains(&"auth_login") && roles.contains(&"download_file");

    let login = Auth7100LoginResult {
        endpoint,
        account: config.account.clone(),
        status: "登录成功".to_string(),
        authenticated,
        response_packet_lengths: packets.iter().map(Vec::len).collect(),
        response_roles: roles.into_iter().map(str::to_string).collect(),
        dictionary_length: Some(dictionary.len()),
        dictionary_sha256,
        decoded_response_lengths,
        login_success_confirmed: true,
        quote_servers,
        selected_quote_endpoint,
        selected_7709_endpoint,
    };
    Ok((stream, login))
}

fn login_core_sequence_is_valid(roles: &[&str]) -> bool {
    let core: Vec<_> = roles
        .iter()
        .copied()
        .filter(|role| *role != "auth_probe_response")
        .collect();
    matches!(
        core.as_slice(),
        ["auth_login", "download_file", "zstd_dictionary"] | ["auth_login", "download_file"]
    )
}

fn read_until_role(
    stream: &mut TcpStream,
    deadline: Instant,
    stage: &'static str,
    endpoint: &str,
    expected: &'static str,
    packets: &mut Vec<Vec<u8>>,
    roles: &mut Vec<&'static str>,
) -> Result<Vec<u8>, Box<dyn Error>> {
    loop {
        let packet = read_netpacket_timed(stream, deadline, stage, endpoint)?;
        let role = classify_response(&packet)?;
        packets.push(packet.clone());
        roles.push(role);
        if role == expected {
            return Ok(packet);
        }
        if role != "auth_probe_response" {
            return Err(format!(
                "unexpected {stage} response: {role}, expected {expected} or auth_probe_response"
            )
            .into());
        }
    }
}

fn drain_optional_dictionary(
    stream: &mut TcpStream,
    overall_deadline: Instant,
    stage: &'static str,
    endpoint: &str,
    packets: &mut Vec<Vec<u8>>,
    roles: &mut Vec<&'static str>,
) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    let remaining = overall_deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Ok(None);
    }
    let drain_timeout = remaining.min(OPTIONAL_DICTIONARY_DRAIN);
    let previous_timeout = stream.read_timeout().ok().flatten();
    stream.set_read_timeout(Some(drain_timeout))?;
    let drain_deadline = Instant::now() + drain_timeout;
    let result = (|| {
        loop {
            if Instant::now() >= drain_deadline {
                return Ok(None);
            }
            match read_netpacket_timed(stream, drain_deadline, stage, endpoint) {
                Ok(packet) => {
                    let role = classify_response(&packet)?;
                    packets.push(packet.clone());
                    roles.push(role);
                    match role {
                        "zstd_dictionary" => return Ok(Some(packet)),
                        "auth_probe_response" => continue,
                        other => {
                            return Err(format!("unexpected {stage} response: {other}").into());
                        }
                    }
                }
                Err(error) if error_is_idle_timeout(error.as_ref()) => return Ok(None),
                Err(error) => return Err(error),
            }
        }
    })();
    let restore = overall_deadline
        .saturating_duration_since(Instant::now())
        .max(Duration::from_millis(100));
    let _ = stream.set_read_timeout(previous_timeout.or(Some(restore)));
    result
}

fn error_is_idle_timeout(error: &(dyn Error + 'static)) -> bool {
    error
        .downcast_ref::<AuthIoError>()
        .is_some_and(AuthIoError::is_timeout)
}

fn decode_login_object(packet: &[u8], dictionary: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    if let Ok(decoded) = decode_control_response(packet, dictionary, "认证") {
        return Ok(decoded);
    }
    if let Ok(decoded) = decode_plain_control_response(packet, "认证") {
        return Ok(decoded);
    }
    if packet.len() > 74 {
        let body = &packet[74..];
        if body.len() >= 20 && decode_utf16_prefix(&body[..20.min(body.len())]) == "认证" {
            return Ok(body.to_vec());
        }
    }
    Err("login response is not a 认证 object".into())
}

fn select_server_endpoint(entries: &[DownloadedServerEntry], port: u16) -> Option<String> {
    entries.iter().find_map(|entry| {
        if entry.main_port == port {
            Some(format_endpoint(&entry.host, entry.main_port))
        } else if entry.secondary_port == port {
            Some(format_endpoint(&entry.host, entry.secondary_port))
        } else {
            None
        }
    })
}

fn format_endpoint(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn split_endpoint(endpoint: &str) -> Option<(&str, &str)> {
    if let Some(rest) = endpoint.strip_prefix('[') {
        rest.split_once("]:")
    } else {
        endpoint.rsplit_once(':')
    }
}

fn outbound_ipv4(auth_endpoint: &str) -> [u8; 4] {
    let Some((host, port_text)) = split_endpoint(auth_endpoint) else {
        return [0; 4];
    };
    let Ok(port) = port_text.parse::<u16>() else {
        return [0; 4];
    };
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect((host, port))?;
            socket.local_addr()
        })
        .ok()
        .and_then(|addr| match addr.ip() {
            std::net::IpAddr::V4(v4) => Some(v4.octets()),
            std::net::IpAddr::V6(v6) => v6.to_ipv4_mapped().map(|v4| v4.octets()),
        })
        .unwrap_or([0, 0, 0, 0])
}

fn login_control_fields_from_result(
    login: &Auth7100LoginResult,
    local_ip: [u8; 4],
    mac_ascii: [u8; 12],
) -> Option<Auth7100LoginControlFields> {
    let selected = login.selected_quote_endpoint.as_deref()?;
    let (host, port_text) = split_endpoint(selected)?;
    let port = port_text.parse::<u16>().ok()?;
    let entry = login.quote_servers.iter().find(|entry| {
        entry.host == host
            && (entry.main_port == port || entry.secondary_port == port)
            && entry.interface_version.is_some()
    })?;
    let mut account_permissions = [0u8; 6];
    let permission = utf16_bytes(entry.permission.as_deref()?);
    if permission.len() > account_permissions.len() {
        return None;
    }
    account_permissions[..permission.len()].copy_from_slice(&permission);
    let mut broker = [0u8; 4];
    let broker_bytes = utf16_bytes(entry.broker.as_deref()?);
    if broker_bytes.len() > broker.len() {
        return None;
    }
    broker[..broker_bytes.len()].copy_from_slice(&broker_bytes);
    Some(Auth7100LoginControlFields {
        local_ip,
        mac_ascii,
        account_permissions,
        broker,
        encryption_version: 0,
        user_id: 0,
        interface_version: entry.interface_version?,
    })
}

fn official_5188_ack_file_infos() -> Vec<(String, u32)> {
    let root = std::env::var("NETZIP_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("docs/netzip_api_bin/NetzipAPI/StockC++")
        });
    ["Stock.exe", "网际风.exe", "Stock.dll", "Stock.字典"]
        .iter()
        .map(|name| {
            let bytes = std::fs::read(root.join(name)).unwrap_or_default();
            ((*name).to_string(), crc32fast::hash(&bytes))
        })
        .collect()
}

fn validate_stock_dictionary(dictionary: &[u8]) -> Result<String, Box<dyn Error>> {
    if dictionary != STOCK_DICTIONARY_BYTES {
        return Err(format!(
            "unsupported Stock.字典 bytes: expected length {}, got {}",
            STOCK_DICTIONARY_BYTES.len(),
            dictionary.len()
        )
        .into());
    }
    Ok(STOCK_DICTIONARY_SHA256.to_string())
}

fn decode_dictionary_response(packet: &[u8], dictionary: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let magic = packet
        .windows(4)
        .position(|window| window == [0x28, 0xb5, 0x2f, 0xfd])
        .ok_or("dictionary response contains no ZSTD frame")?;
    let mut decoder =
        zstd::stream::read::Decoder::with_dictionary(Cursor::new(&packet[magic..]), dictionary)?
            .single_frame();
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded)?;
    Ok(decoded)
}

fn decode_control_response(
    packet: &[u8],
    dictionary: &[u8],
    expected_root: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    if let Ok(decoded) = decode_dictionary_response(packet, dictionary) {
        if decoded.len() >= 20 && decode_utf16_prefix(&decoded[..20]) == expected_root {
            return Ok(decoded);
        }
    }
    let magic = packet
        .windows(4)
        .position(|window| window == [0x28, 0xb5, 0x2f, 0xfd])
        .ok_or("control response contains no ZSTD frame")?;
    let decoded = zstd::stream::decode_all(Cursor::new(&packet[magic..]))?;
    if decoded.len() < 20 || decode_utf16_prefix(&decoded[..20]) != expected_root {
        return Err(format!("control response root is not {expected_root}").into());
    }
    Ok(decoded)
}

fn decode_plain_control_response(
    packet: &[u8],
    expected_root: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let magic = packet
        .windows(4)
        .position(|window| window == [0x28, 0xb5, 0x2f, 0xfd])
        .ok_or("control response contains no ZSTD frame")?;
    let mut decoder =
        zstd::stream::read::Decoder::new(Cursor::new(&packet[magic..]))?.single_frame();
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded)?;
    if decoded.len() < 20 || decode_utf16_prefix(&decoded[..20]) != expected_root {
        return Err(format!("control response root is not {expected_root}").into());
    }
    Ok(decoded)
}

pub fn build_auth_7100_probe_packet(
    account: &str,
    password: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    build_auth_packet(
        account,
        password,
        PROBE_INNER_PREFIX_HEX,
        PROBE_INNER_SUFFIX_HEX,
        PROBE_OUTER_PREFIX_HEX,
    )
}

pub fn build_auth_7100_login_packet(
    account: &str,
    password: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    build_auth_packet(
        account,
        password,
        INNER_PREFIX_HEX,
        INNER_SUFFIX_HEX,
        OUTER_PREFIX_HEX,
    )
}

/// Builds the per-5188-connection 12-field dictionary login control packet.
///
/// This is not the initial account login. Initial authentication uses
/// [`build_auth_7100_login_packet`].
pub fn build_current_auth_login_packet(
    account: &str,
    password: &str,
    request_number: u32,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut decoded = vec![0u8; 40];
    write_fixed_utf16(&mut decoded[..20], "加密包")?;
    append_control_field(&mut decoded, 2, 0, "请求", &utf16_bytes("大智慧C_登录包"))?;
    append_control_field(&mut decoded, 4, 3, "本地IP", &[0; 4])?;
    append_control_field(&mut decoded, 5, 1, "网卡MAC", &[])?;
    append_control_field(&mut decoded, 2, 0, "版本", &[])?;
    append_control_field(&mut decoded, 4, 0, "账号权限", &[0; 6])?;
    append_control_field(&mut decoded, 2, 0, "券商", &[0; 4])?;
    append_control_field(&mut decoded, 4, 2, "加密版本", &0u32.to_le_bytes())?;
    append_control_field(&mut decoded, 2, 0, "账号", &utf16_bytes(account))?;
    append_control_field(&mut decoded, 2, 0, "密码", &utf16_bytes(password))?;
    append_control_field(&mut decoded, 4, 2, "用户ID", &0u32.to_le_bytes())?;
    append_control_field(&mut decoded, 4, 2, "接口版本", &0u32.to_le_bytes())?;
    append_control_field(&mut decoded, 2, 3, "编号", &request_number.to_le_bytes())?;
    patch_u32(&mut decoded, 20, 12)?;
    let decoded_len = decoded.len();
    patch_u32(&mut decoded, 32, decoded_len)?;
    patch_u32(&mut decoded, 36, decoded_len)?;
    encode_dictionary_control_packet(&decoded, FOLLOWUP_DOWNLOAD_HEX)
}

/// Builds the evidence-backed shape of one per-connection 5188 login request.
///
/// Field sources and connection assignment are still under investigation, so
/// callers must supply values from the current authenticated lifecycle. This
/// function never reads captured field values or selects a 5188 endpoint.
pub fn build_candidate_official_5188_login_control_packet(
    fields: &Auth7100LoginControlFields,
    request_number: u32,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut decoded = vec![0u8; 40];
    write_fixed_utf16(&mut decoded[..20], "加密包")?;
    append_control_field(&mut decoded, 2, 0, "请求", &utf16_bytes("大智慧C_登录包"))?;
    append_control_field(&mut decoded, 4, 3, "本地IP", &fields.local_ip)?;
    append_control_field(&mut decoded, 5, 1, "网卡MAC", &fields.mac_ascii)?;
    append_control_field(&mut decoded, 2, 0, "版本", &[])?;
    append_control_field(&mut decoded, 4, 0, "账号权限", &fields.account_permissions)?;
    append_control_field(&mut decoded, 2, 0, "券商", &fields.broker)?;
    append_control_field(
        &mut decoded,
        4,
        2,
        "加密版本",
        &fields.encryption_version.to_le_bytes(),
    )?;
    append_control_field(&mut decoded, 2, 0, "账号", &[])?;
    append_control_field(&mut decoded, 2, 0, "密码", &[])?;
    append_control_field(&mut decoded, 4, 2, "用户ID", &fields.user_id.to_le_bytes())?;
    append_control_field(
        &mut decoded,
        4,
        2,
        "接口版本",
        &fields.interface_version.to_le_bytes(),
    )?;
    append_control_field(&mut decoded, 2, 3, "编号", &request_number.to_le_bytes())?;
    patch_u32(&mut decoded, 20, 12)?;
    let decoded_len = decoded.len();
    patch_u32(&mut decoded, 32, decoded_len)?;
    patch_u32(&mut decoded, 36, decoded_len)?;
    encode_dictionary_control_packet(&decoded, FOLLOWUP_DOWNLOAD_HEX)
}

/// Builds the evidence-backed shape of one ABK initialization request.
pub fn build_candidate_official_5188_abk_control_packet(
    fields: &Auth7100AbkControlFields,
    request_number: u32,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut decoded = vec![0u8; 40];
    write_fixed_utf16(&mut decoded[..20], "加密包")?;
    append_control_field(&mut decoded, 2, 0, "请求", &utf16_bytes("大智慧C_ABK"))?;
    append_control_field(&mut decoded, 3, 2, "64位", &fields.is_64_bit.to_le_bytes())?;
    append_control_field(
        &mut decoded,
        3,
        2,
        "主版本",
        &fields.major_version.to_le_bytes(),
    )?;
    append_control_field(
        &mut decoded,
        3,
        2,
        "次版本",
        &fields.minor_version.to_le_bytes(),
    )?;
    append_control_field(
        &mut decoded,
        3,
        2,
        "构建号",
        &fields.build_number.to_le_bytes(),
    )?;
    append_control_field(&mut decoded, 5, 1, "网卡MAC", &[])?;
    append_control_field(&mut decoded, 3, 6, "总内存", &fields.total_memory)?;
    append_control_field(&mut decoded, 2, 0, "账号", &[])?;
    append_control_field(&mut decoded, 2, 0, "密码", &[])?;
    append_control_field(&mut decoded, 4, 2, "用户ID", &fields.user_id.to_le_bytes())?;
    append_control_field(
        &mut decoded,
        4,
        2,
        "接口版本",
        &fields.interface_version.to_le_bytes(),
    )?;
    append_control_field(&mut decoded, 2, 3, "编号", &request_number.to_le_bytes())?;
    patch_u32(&mut decoded, 20, 12)?;
    let decoded_len = decoded.len();
    patch_u32(&mut decoded, 32, decoded_len)?;
    patch_u32(&mut decoded, 36, decoded_len)?;
    encode_dictionary_control_packet(&decoded, FOLLOWUP_DOWNLOAD_HEX)
}

/// Builds the evidence-backed shape of one ACK initialization request.
///
/// `data` is the runtime's own `SFLogInPack=ACK` manifest. Its length is not
/// a fixed protocol constant; captured and generated manifests are both valid
/// ASCII CRLF text bounded here for defensive packet construction.
pub fn build_candidate_official_5188_ack_control_packet(
    fields: &Auth7100AckControlFields,
    request_number: u32,
) -> Result<Vec<u8>, Box<dyn Error>> {
    if fields.data.is_empty() || fields.data.len() > 8192 {
        return Err(format!(
            "unverified ACK data length {}; expected a generated manifest between 1 and 8192 bytes",
            fields.data.len()
        )
        .into());
    }
    let mut decoded = vec![0u8; 40];
    write_fixed_utf16(&mut decoded[..20], "加密包")?;
    append_control_field(&mut decoded, 2, 0, "请求", &utf16_bytes("大智慧C_ACK"))?;
    append_control_field(&mut decoded, 3, 0, "ack", &fields.ack.to_le_bytes())?;
    append_control_field(&mut decoded, 2, 0, "账号", &[])?;
    append_control_field(&mut decoded, 2, 0, "密码", &[])?;
    append_control_field(&mut decoded, 4, 2, "用户ID", &fields.user_id.to_le_bytes())?;
    append_control_field(
        &mut decoded,
        4,
        2,
        "接口版本",
        &fields.interface_version.to_le_bytes(),
    )?;
    append_control_field(&mut decoded, 2, 3, "编号", &request_number.to_le_bytes())?;
    append_control_field(&mut decoded, 2, 9, "数据", &fields.data)?;
    patch_u32(&mut decoded, 20, 8)?;
    let decoded_len = decoded.len();
    patch_u32(&mut decoded, 32, decoded_len)?;
    patch_u32(&mut decoded, 36, decoded_len)?;
    encode_dictionary_control_packet(&decoded, FOLLOWUP_DOWNLOAD_HEX)
}

/// Builds the dictionary-backed C2 download request with the supplied request number.
///
/// The template is decoded before the number is changed, then recompressed with the
/// verified raw-content dictionary. This preserves the vendor's level-3 ZSTD output
/// for the baseline number while allowing later request pairs to advance their number.
pub fn build_auth_7100_followup_download_packet(
    request_number: u32,
) -> Result<Vec<u8>, Box<dyn Error>> {
    build_auth_7100_download_file_packet(request_number, "系统\\通达信股票服务器.ini")
}

/// Builds a dictionary-backed `下载文件` request for a specific vendor config.
///
/// The official full-push chain requests `系统\\大智慧服务器L1.ini`; the
/// historical 7709 compatibility route uses `系统\\通达信股票服务器.ini`.
pub fn build_auth_7100_download_file_packet(
    request_number: u32,
    file_name: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let template = decode_hex(FOLLOWUP_DOWNLOAD_HEX)?;
    let mut decoded = decode_dictionary_response(&template, STOCK_DICTIONARY_BYTES)?;
    let old_name = utf16_bytes("系统\\通达信股票服务器.ini");
    let new_name = utf16_bytes(file_name);
    if old_name.len() != new_name.len() {
        return Err("download file name must preserve the observed UTF-16 slot length".into());
    }
    let offset = decoded
        .windows(old_name.len())
        .position(|window| window == old_name.as_slice())
        .ok_or("download request file-name field not found")?;
    decoded[offset..offset + old_name.len()].copy_from_slice(&new_name);
    patch_u32(
        &mut decoded,
        FOLLOWUP_DOWNLOAD_NUMBER_OFFSET,
        request_number as usize,
    )?;
    encode_dictionary_control_packet(&decoded, FOLLOWUP_DOWNLOAD_HEX)
}

/// Builds the dictionary-backed C3 `Tdx_Encrypt` request with the supplied request number.
pub fn build_auth_7100_followup_finish_packet(
    request_number: u32,
) -> Result<Vec<u8>, Box<dyn Error>> {
    build_dictionary_followup_packet(
        FOLLOWUP_FINISH_HEX,
        FOLLOWUP_FINISH_NUMBER_OFFSET,
        request_number,
    )
}

fn build_dictionary_followup_packet(
    template_hex: &str,
    number_offset: usize,
    request_number: u32,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let template = decode_hex(template_hex)?;
    let mut decoded = decode_dictionary_response(&template, STOCK_DICTIONARY_BYTES)?;
    patch_u32(&mut decoded, number_offset, request_number as usize)?;

    encode_dictionary_control_packet(&decoded, template_hex)
}

fn encode_dictionary_control_packet(
    decoded: &[u8],
    template_hex: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut compressor = zstd::bulk::Compressor::with_dictionary(3, STOCK_DICTIONARY_BYTES)?;
    let compressed = compressor.compress(decoded)?;
    let template = decode_hex(template_hex)?;
    let magic = template
        .windows(4)
        .position(|window| window == [0x28, 0xb5, 0x2f, 0xfd])
        .ok_or("follow-up template contains no ZSTD frame")?;
    if template.len() < magic + FOLLOWUP_OUTER_TAIL_LEN {
        return Err("follow-up template is shorter than its outer tail".into());
    }
    let tail_start = template.len() - FOLLOWUP_OUTER_TAIL_LEN;
    let tail = &template[tail_start..];
    if tail != [0, 0] {
        return Err("follow-up template has an unexpected outer tail".into());
    }

    let packet_len = magic
        .checked_add(compressed.len())
        .and_then(|value| value.checked_add(tail.len()))
        .ok_or("follow-up packet length overflow")?;
    let mut packet = template[..magic].to_vec();
    patch_u32(&mut packet, 32, packet_len)?;
    patch_u32(&mut packet, 36, packet_len)?;
    patch_u32(&mut packet, 106, compressed.len())?;
    patch_u32(&mut packet, 140, decoded.len())?;
    patch_u32(&mut packet, 150, compressed.len())?;
    patch_u32(&mut packet, 162, compressed.len() + 28)?;
    packet.extend_from_slice(&compressed);
    packet.extend_from_slice(tail);
    Ok(packet)
}

fn append_control_field(
    bytes: &mut Vec<u8>,
    type_id: u32,
    field_12: u32,
    label: &str,
    value: &[u8],
) -> Result<(), Box<dyn Error>> {
    let label_bytes = utf16_bytes(label);
    let span_len = 20usize
        .checked_add(label_bytes.len())
        .and_then(|length| length.checked_add(2))
        .and_then(|length| length.checked_add(value.len()))
        .and_then(|length| length.checked_add(2))
        .ok_or("control field length overflow")?;
    bytes.extend_from_slice(&type_id.to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(value.len())?.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&field_12.to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(span_len)?.to_le_bytes());
    bytes.extend_from_slice(&label_bytes);
    bytes.extend_from_slice(&[0, 0]);
    bytes.extend_from_slice(value);
    bytes.extend_from_slice(&[0, 0]);
    Ok(())
}

fn utf16_bytes(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn write_fixed_utf16(target: &mut [u8], value: &str) -> Result<(), Box<dyn Error>> {
    let encoded = utf16_bytes(value);
    if encoded.len() + 2 > target.len() {
        return Err("fixed UTF-16 field is too long".into());
    }
    target.fill(0);
    target[..encoded.len()].copy_from_slice(&encoded);
    Ok(())
}

fn build_auth_packet(
    account: &str,
    password: &str,
    inner_prefix_hex: &str,
    inner_suffix_hex: &str,
    outer_prefix_hex: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut inner = decode_hex(inner_prefix_hex)?;
    append_string_field(&mut inner, "账号", account)?;
    append_string_field(&mut inner, "密码", password)?;
    inner.extend_from_slice(&decode_hex(inner_suffix_hex)?);
    let inner_len = inner.len();
    patch_u32(&mut inner, 32, inner_len)?;
    patch_u32(&mut inner, 36, inner_len)?;

    let compressed = zstd::bulk::compress(&inner, 3)?;
    let mut packet = decode_hex(outer_prefix_hex)?;
    let packet_len = packet.len() + compressed.len() + 2;
    patch_u32(&mut packet, 32, packet_len)?;
    patch_u32(&mut packet, 36, packet_len)?;
    patch_u32(&mut packet, 102, compressed.len())?;
    patch_u32(&mut packet, 136, inner.len())?;
    patch_u32(&mut packet, 146, compressed.len())?;
    patch_u32(&mut packet, 158, compressed.len() + 28)?;
    packet.extend_from_slice(&compressed);
    packet.extend_from_slice(&[0, 0]);
    Ok(packet)
}

fn append_string_field(
    bytes: &mut Vec<u8>,
    label: &str,
    value: &str,
) -> Result<(), Box<dyn Error>> {
    let value_words = value.encode_utf16().collect::<Vec<_>>();
    let value_len = value_words.len().checked_mul(2).ok_or("string too long")?;
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(value_len)?.to_le_bytes());
    bytes.extend_from_slice(&[0; 8]);
    bytes.extend_from_slice(&u32::try_from(28usize + value_len)?.to_le_bytes());
    append_utf16_z(bytes, label);
    for word in value_words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend_from_slice(&[0, 0]);
    Ok(())
}

fn append_utf16_z(bytes: &mut Vec<u8>, value: &str) {
    for word in value.encode_utf16() {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend_from_slice(&[0, 0]);
}

fn patch_u32(bytes: &mut [u8], offset: usize, value: usize) -> Result<(), Box<dyn Error>> {
    let target = bytes
        .get_mut(offset..offset + 4)
        .ok_or("packet patch offset out of range")?;
    target.copy_from_slice(&u32::try_from(value)?.to_le_bytes());
    Ok(())
}

fn read_netpacket(stream: &mut TcpStream) -> Result<Vec<u8>, Box<dyn Error>> {
    let timeout = stream
        .read_timeout()
        .ok()
        .flatten()
        .unwrap_or(Duration::from_secs(8));
    let endpoint = stream
        .peer_addr()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    read_netpacket_timed(
        stream,
        Instant::now() + timeout,
        "read_netpacket",
        &endpoint,
    )
}

fn read_netpacket_timed(
    stream: &mut TcpStream,
    deadline: Instant,
    stage: &'static str,
    endpoint: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut header = vec![0u8; 74];
    read_exact_with_deadline(stream, &mut header, deadline, stage, "read", endpoint)?;
    let packet_len = u32::from_le_bytes(header[32..36].try_into()?) as usize;
    if !(74..=MAX_PACKET_LEN).contains(&packet_len) {
        return Err(format!("invalid 7100 packet length: {packet_len}").into());
    }
    let mut packet = header;
    packet.resize(packet_len, 0);
    read_exact_with_deadline(stream, &mut packet[74..], deadline, stage, "read", endpoint)?;
    Ok(packet)
}

fn connect_auth_stream(endpoint: &str, deadline: Instant) -> Result<TcpStream, Box<dyn Error>> {
    let started = Instant::now();
    let socket = endpoint
        .to_socket_addrs()
        .map_err(|error| {
            AuthIoError::from_io("resolve", "dns", endpoint, &error, 1, started.elapsed())
        })?
        .next()
        .ok_or("failed to resolve authentication endpoint")?;
    let mut delay = Duration::from_millis(100);
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Box::new(AuthIoError {
                stage: "connect",
                op: "connect",
                endpoint: endpoint.to_string(),
                kind: "timed_out",
                os_code: None,
                attempt,
                elapsed_ms: started.elapsed().as_millis(),
            }));
        }
        match TcpStream::connect_timeout(&socket, remaining.min(Duration::from_secs(3))) {
            Ok(stream) => {
                if let Err(error) = stream.set_nonblocking(false) {
                    return Err(Box::new(AuthIoError::from_io(
                        "connect",
                        "set_blocking",
                        endpoint,
                        &error,
                        attempt,
                        started.elapsed(),
                    )));
                }
                if let Some(error) = stream.take_error().ok().flatten() {
                    if io_error_is_retryable(&error) && Instant::now() < deadline {
                        thread::sleep(delay);
                        delay = (delay * 2).min(Duration::from_secs(1));
                        continue;
                    }
                    return Err(Box::new(AuthIoError::from_io(
                        "connect",
                        "socket_error",
                        endpoint,
                        &error,
                        attempt,
                        started.elapsed(),
                    )));
                }
                let _ = stream.set_nodelay(true);
                let timeout = deadline.saturating_duration_since(Instant::now());
                let timeout = if timeout.is_zero() {
                    Duration::from_millis(100)
                } else {
                    timeout
                };
                stream.set_read_timeout(Some(timeout)).map_err(|error| {
                    AuthIoError::from_io(
                        "connect",
                        "set_read_timeout",
                        endpoint,
                        &error,
                        attempt,
                        started.elapsed(),
                    )
                })?;
                stream.set_write_timeout(Some(timeout)).map_err(|error| {
                    AuthIoError::from_io(
                        "connect",
                        "set_write_timeout",
                        endpoint,
                        &error,
                        attempt,
                        started.elapsed(),
                    )
                })?;
                return Ok(stream);
            }
            Err(error) if io_error_is_retryable(&error) && Instant::now() < deadline => {
                thread::sleep(delay);
                delay = (delay * 2).min(Duration::from_secs(1));
            }
            Err(error) => {
                return Err(Box::new(AuthIoError::from_io(
                    "connect",
                    "connect",
                    endpoint,
                    &error,
                    attempt,
                    started.elapsed(),
                )));
            }
        }
    }
}

fn write_all_timed(
    stream: &mut TcpStream,
    bytes: &[u8],
    deadline: Instant,
    stage: &'static str,
    endpoint: &str,
) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let mut written = 0usize;
    let mut attempt = 0u32;
    while written < bytes.len() {
        attempt += 1;
        if Instant::now() >= deadline {
            return Err(Box::new(AuthIoError {
                stage,
                op: "write",
                endpoint: endpoint.to_string(),
                kind: "timed_out",
                os_code: None,
                attempt,
                elapsed_ms: started.elapsed().as_millis(),
            }));
        }
        match stream.write(&bytes[written..]) {
            Ok(0) => {
                return Err(Box::new(AuthIoError {
                    stage,
                    op: "write",
                    endpoint: endpoint.to_string(),
                    kind: "unexpected_eof",
                    os_code: None,
                    attempt,
                    elapsed_ms: started.elapsed().as_millis(),
                }));
            }
            Ok(count) => written += count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) if io_error_is_retryable(&error) && Instant::now() < deadline => {
                thread::sleep(AUTH_IO_RETRY_SLEEP);
            }
            Err(error) => {
                return Err(Box::new(AuthIoError::from_io(
                    stage,
                    "write",
                    endpoint,
                    &error,
                    attempt,
                    started.elapsed(),
                )));
            }
        }
    }
    stream.flush().map_err(|error| {
        Box::new(AuthIoError::from_io(
            stage,
            "flush",
            endpoint,
            &error,
            attempt,
            started.elapsed(),
        )) as Box<dyn Error>
    })
}

fn read_exact_with_deadline<R: Read>(
    reader: &mut R,
    buf: &mut [u8],
    deadline: Instant,
    stage: &'static str,
    op: &'static str,
    endpoint: &str,
) -> Result<(), Box<dyn Error>> {
    if buf.is_empty() {
        return Ok(());
    }
    let started = Instant::now();
    let mut filled = 0usize;
    let mut attempt = 0u32;
    while filled < buf.len() {
        attempt += 1;
        if Instant::now() >= deadline {
            return Err(Box::new(AuthIoError {
                stage,
                op,
                endpoint: endpoint.to_string(),
                kind: "timed_out",
                os_code: None,
                attempt,
                elapsed_ms: started.elapsed().as_millis(),
            }));
        }
        match reader.read(&mut buf[filled..]) {
            Ok(0) => {
                return Err(Box::new(AuthIoError {
                    stage,
                    op,
                    endpoint: endpoint.to_string(),
                    kind: "unexpected_eof",
                    os_code: None,
                    attempt,
                    elapsed_ms: started.elapsed().as_millis(),
                }));
            }
            Ok(count) => filled += count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) if io_error_is_retryable(&error) && Instant::now() < deadline => {
                thread::sleep(AUTH_IO_RETRY_SLEEP);
            }
            Err(error) => {
                return Err(Box::new(AuthIoError::from_io(
                    stage,
                    op,
                    endpoint,
                    &error,
                    attempt,
                    started.elapsed(),
                )));
            }
        }
    }
    Ok(())
}

fn validate_complete_netpacket(packet: &[u8]) -> Result<(), Box<dyn Error>> {
    if packet.len() < 74 {
        return Err("control request is shorter than the network packet header".into());
    }
    if decode_utf16_prefix(&packet[..20]) != "网络包" {
        return Err("control request is not a 网络包 object".into());
    }
    let declared = u32::from_le_bytes(packet[32..36].try_into()?) as usize;
    if declared != packet.len() || declared > MAX_PACKET_LEN {
        return Err(format!(
            "control request length mismatch: declared {declared}, actual {}",
            packet.len()
        )
        .into());
    }
    Ok(())
}

fn decode_utf16_prefix(bytes: &[u8]) -> String {
    let words = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .take_while(|word| *word != 0)
        .collect::<Vec<_>>();
    String::from_utf16_lossy(&words)
}

fn classify_response(packet: &[u8]) -> Result<&'static str, Box<dyn Error>> {
    if packet.len() < 74 {
        return Err("short 7100 response packet".into());
    }
    let field20 = u32::from_le_bytes(packet[20..24].try_into()?);
    let field44 = u32::from_le_bytes(packet[44..48].try_into()?);
    let tail = String::from_utf16_lossy(
        &packet[60..74]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .filter(|word| *word != 0)
            .collect::<Vec<_>>(),
    );
    if field20 == 1 && tail.contains("下载文件") {
        return Ok("download_file");
    }
    // The production 19-field login manifest currently returns a plain
    // authentication object with the `数据认证` envelope marker.  Older
    // captures used a compressed type-4 response, so retain that path below.
    if field20 == 1 && field44 == 536 && tail.contains("数据认证") {
        return Ok("auth_login");
    }
    if field20 == 4 && field44 == 8 && tail.contains("ZSTD") {
        if let Some(magic) = packet
            .windows(4)
            .position(|window| window == [0x28, 0xb5, 0x2f, 0xfd])
        {
            if let Ok(mut decoder) = zstd::stream::read::Decoder::new(Cursor::new(&packet[magic..]))
                .map(|decoder| decoder.single_frame())
            {
                let mut decoded = Vec::new();
                if decoder.read_to_end(&mut decoded).is_ok()
                    && (contains_utf16(&decoded, "登录成功") || contains_utf16(&decoded, "登录"))
                {
                    return Ok("auth_login");
                }
            }
        }
        return Ok("auth_probe_response");
    }
    // Live 19-field login success is often a dictionary ZSTD envelope
    // (`field20=4`, `field44=12`). The crate validates that first packet as a
    // 认证 object; envelope-only classification would mislabel it as
    // `zstd_dictionary` and abort login_read. Tdx_Encrypt still falls through
    // because it does not contain 登录成功.
    if field20 == 4 && field44 == 12 && tail.contains("ZSTD") {
        if login_packet_contains_success(packet) {
            return Ok("auth_login");
        }
        return Ok("zstd_dictionary");
    }
    if login_packet_contains_success(packet) {
        return Ok("auth_login");
    }
    Err(format!("unrecognized 7100 response: type={field20} payload={field44} tail={tail}").into())
}

fn login_packet_contains_success(packet: &[u8]) -> bool {
    decode_login_object(packet, STOCK_DICTIONARY_BYTES)
        .ok()
        .is_some_and(|decoded| contains_utf16(&decoded, "登录成功"))
}

fn contains_utf16(bytes: &[u8], value: &str) -> bool {
    let needle = value
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    bytes.windows(needle.len()).any(|window| window == needle)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    if value.len() % 2 != 0 {
        return Err("hex constant must have even length".into());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair)?;
            Ok(u8::from_str_radix(text, 16)?)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        Auth7100AbkControlFields, Auth7100AckControlFields, Auth7100ClientConfig,
        Auth7100ControlSession, Auth7100LoginControlFields, Auth7100LoginResult, AuthIoError,
        Official5188ControlStage, append_control_field, build_ack_manifest_text,
        build_auth_7100_download_file_packet, build_auth_7100_followup_download_packet,
        build_auth_7100_followup_finish_packet, build_auth_7100_login_packet,
        build_auth_7100_probe_packet, build_candidate_official_5188_abk_control_packet,
        build_candidate_official_5188_ack_control_packet,
        build_candidate_official_5188_login_control_packet, build_current_auth_login_packet,
        build_default_ack_manifest_text, classify_io_kind, classify_response,
        connect_auth_sequence, decode_dictionary_response, decode_hex, default_ack_market_rows,
        encode_dictionary_control_packet, login_core_sequence_is_valid, patch_u32,
        read_exact_with_deadline, read_netpacket, select_expected_official_5188_init,
        select_server_endpoint, utf16_bytes, write_fixed_utf16,
    };
    use crate::auth_download::{DownloadedServerEntry, load_downloaded_server_config_sample};
    use netzip_fullpull::{
        Official5188ClientSessionEnvelope, Official5188Frame, Official5188Kind, Official5188Session,
    };
    use std::io::{self, Cursor, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn builds_probe_packet_with_probe_role_and_dynamic_credentials() {
        let packet = build_auth_7100_probe_packet("1522", "123456").expect("packet");
        let declared = u32::from_le_bytes(packet[32..36].try_into().unwrap()) as usize;
        assert_eq!(declared, packet.len());
        assert_eq!(&packet[162..166], b"penc");

        let inner = zstd::stream::decode_all(Cursor::new(&packet[168..packet.len() - 2]))
            .expect("inner zstd");
        assert_eq!(
            u32::from_le_bytes(inner[32..36].try_into().unwrap()) as usize,
            inner.len()
        );
        assert!(contains_utf16(&inner, "测速"));
        assert!(contains_utf16(&inner, "1522"));
        assert!(contains_utf16(&inner, "123456"));
    }

    #[test]
    fn builds_dynamic_login_packet_without_a_captured_credential_frame() {
        let packet = build_auth_7100_login_packet("1522", "fixture-secret").expect("packet");
        let declared = u32::from_le_bytes(packet[32..36].try_into().unwrap()) as usize;
        assert_eq!(declared, packet.len());
        assert_eq!(&packet[162..166], b"penc");
        assert_eq!(packet[172], 0x60);
        assert_eq!(
            u32::from_le_bytes(packet[146..150].try_into().unwrap()) as usize,
            packet.len() - 170
        );
        assert_eq!(u32::from_le_bytes(packet[150..154].try_into().unwrap()), 0);

        let inner = zstd::stream::decode_all(Cursor::new(&packet[168..packet.len() - 2]))
            .expect("inner zstd");
        assert_eq!(
            u32::from_le_bytes(inner[32..36].try_into().unwrap()) as usize,
            inner.len()
        );
        assert!(contains_utf16(&inner, "1522"));
        assert!(contains_utf16(&inner, "fixture-secret"));
    }

    #[test]
    fn builds_current_dictionary_login_with_exact_twelve_field_shape() {
        let packet = build_current_auth_login_packet("168", "fixture-secret", 7)
            .expect("current login packet");
        let decoded = decode_dictionary_response(&packet, super::STOCK_DICTIONARY_BYTES)
            .expect("dictionary login");
        let fields = super::parse_control_object(&decoded).expect("control fields");
        assert_eq!(fields.len(), 12);
        assert_eq!(fields[0].label, "请求");
        assert_eq!(
            super::decode_utf16_prefix(fields[0].value),
            "大智慧C_登录包"
        );
        assert_eq!(super::decode_utf16_prefix(fields[7].value), "168");
        assert_eq!(
            super::decode_utf16_prefix(fields[8].value),
            "fixture-secret"
        );
        assert_eq!(u32::from_le_bytes(fields[11].value.try_into().unwrap()), 7);
    }

    #[test]
    fn classifies_and_validates_fifteen_field_plain_zstd_login_success() {
        let mut decoded = vec![0u8; 40];
        write_fixed_utf16(&mut decoded[..20], "认证").expect("root");
        append_control_field(
            &mut decoded,
            2,
            0,
            "提示信息",
            &super::utf16_bytes("登录成功"),
        )
        .expect("success field");
        for index in 1..15 {
            append_control_field(
                &mut decoded,
                4,
                2,
                &format!("字段{index}"),
                &(index as u32).to_le_bytes(),
            )
            .expect("metadata field");
        }
        patch_u32(&mut decoded, 20, 15).expect("field count");
        let decoded_len = decoded.len();
        patch_u32(&mut decoded, 32, decoded_len).expect("length");
        patch_u32(&mut decoded, 36, decoded_len).expect("duplicate length");

        let compressed = zstd::bulk::compress(&decoded, 3).expect("plain zstd");
        let mut packet = decode_hex(super::OUTER_PREFIX_HEX).expect("outer prefix");
        let packet_len = packet.len() + compressed.len() + 2;
        patch_u32(&mut packet, 32, packet_len).expect("packet length");
        patch_u32(&mut packet, 36, packet_len).expect("duplicate packet length");
        patch_u32(&mut packet, 102, compressed.len()).expect("compressed length");
        patch_u32(&mut packet, 136, decoded.len()).expect("decoded length");
        patch_u32(&mut packet, 146, compressed.len()).expect("zstd length");
        patch_u32(&mut packet, 158, compressed.len() + 28).expect("object span");
        packet.extend_from_slice(&compressed);
        packet.extend_from_slice(&[0, 0]);

        assert_eq!(
            classify_response(&packet).expect("classification"),
            "auth_login"
        );
        let inflated =
            super::decode_plain_control_response(&packet, "认证").expect("plain response");
        let fields =
            super::parse_control_object_with_root(&inflated, "认证").expect("response fields");
        assert_eq!(fields.len(), 15);
        super::require_control_utf16(&fields, "提示信息", "登录成功").expect("success marker");
    }

    #[test]
    fn rebuilds_the_captured_followup_templates_byte_for_byte() {
        let download_template = decode_hex(super::FOLLOWUP_DOWNLOAD_HEX).expect("download");
        let finish_template = decode_hex(super::FOLLOWUP_FINISH_HEX).expect("finish");

        assert_eq!(
            build_auth_7100_followup_download_packet(1).expect("download packet"),
            download_template
        );
        assert_eq!(
            build_auth_7100_followup_finish_packet(2).expect("finish packet"),
            finish_template
        );
    }

    #[test]
    fn builds_official_fullpull_l1_download_request() {
        let packet = build_auth_7100_download_file_packet(1, "系统\\大智慧服务器L1.ini")
            .expect("L1 download packet");
        let decoded = decode_dictionary_response(&packet, super::STOCK_DICTIONARY_BYTES)
            .expect("decode L1 request");
        assert!(contains_utf16(&decoded, "系统\\大智慧服务器L1.ini"));
        assert!(!contains_utf16(&decoded, "系统\\通达信股票服务器.ini"));
    }

    #[test]
    fn advances_followup_number_without_exposing_template_payload() {
        let download = build_auth_7100_followup_download_packet(3).expect("download packet");
        let download_decoded =
            decode_dictionary_response(&download, super::STOCK_DICTIONARY_BYTES).expect("decode");
        assert_eq!(
            u32::from_le_bytes(download_decoded[124..128].try_into().unwrap()),
            3
        );
        assert_eq!(
            u32::from_le_bytes(download[32..36].try_into().unwrap()) as usize,
            download.len()
        );

        let finish = build_auth_7100_followup_finish_packet(4).expect("finish packet");
        let finish_decoded =
            decode_dictionary_response(&finish, super::STOCK_DICTIONARY_BYTES).expect("decode");
        assert_eq!(
            u32::from_le_bytes(finish_decoded[116..120].try_into().unwrap()),
            4
        );
        assert_eq!(
            u32::from_le_bytes(finish[32..36].try_into().unwrap()) as usize,
            finish.len()
        );
    }

    #[test]
    fn builds_candidate_5188_login_control_shape_without_captured_values() {
        let fields = Auth7100LoginControlFields {
            local_ip: [198, 18, 0, 1],
            mac_ascii: [0; 12],
            account_permissions: [1, 2, 3, 4, 5, 6],
            broker: [7, 8, 9, 10],
            encryption_version: 0,
            user_id: 42,
            interface_version: 858,
        };
        let packet = build_candidate_official_5188_login_control_packet(&fields, 2)
            .expect("candidate control packet");
        let decoded = decode_dictionary_response(&packet, super::STOCK_DICTIONARY_BYTES)
            .expect("decode candidate");
        assert_eq!(decoded.len(), 460);
        assert_eq!(super::decode_utf16_prefix(&decoded[..20]), "加密包");
        assert_eq!(
            [
                u32::from_le_bytes(decoded[20..24].try_into().unwrap()),
                u32::from_le_bytes(decoded[24..28].try_into().unwrap()),
                u32::from_le_bytes(decoded[28..32].try_into().unwrap()),
                u32::from_le_bytes(decoded[32..36].try_into().unwrap()),
                u32::from_le_bytes(decoded[36..40].try_into().unwrap()),
            ],
            [12, 0, 0, 460, 460]
        );
        assert!(contains_utf16(&decoded, "大智慧C_登录包"));
        assert!(contains_utf16(&decoded, "编号"));
        assert_eq!(&decoded[454..458], &2u32.to_le_bytes());
    }

    #[test]
    fn builds_candidate_5188_abk_control_shape_without_captured_values() {
        let fields = Auth7100AbkControlFields {
            is_64_bit: 1,
            major_version: 2,
            minor_version: 3,
            build_number: 4,
            total_memory: [5, 6, 7, 8, 9, 10, 11, 12],
            user_id: 42,
            interface_version: 858,
        };
        let packet = build_candidate_official_5188_abk_control_packet(&fields, 12)
            .expect("candidate ABK packet");
        let decoded = decode_dictionary_response(&packet, super::STOCK_DICTIONARY_BYTES)
            .expect("decode candidate ABK");
        assert_eq!(decoded.len(), 452);
        assert_eq!(super::decode_utf16_prefix(&decoded[..20]), "加密包");
        assert_eq!(
            [
                u32::from_le_bytes(decoded[20..24].try_into().unwrap()),
                u32::from_le_bytes(decoded[24..28].try_into().unwrap()),
                u32::from_le_bytes(decoded[28..32].try_into().unwrap()),
                u32::from_le_bytes(decoded[32..36].try_into().unwrap()),
                u32::from_le_bytes(decoded[36..40].try_into().unwrap()),
            ],
            [12, 0, 0, 452, 452]
        );
        assert!(contains_utf16(&decoded, "大智慧C_ABK"));
        assert_eq!(&decoded[446..450], &12u32.to_le_bytes());
    }

    #[test]
    fn builds_candidate_5188_ack_control_shape_with_current_opaque_data() {
        let fields = Auth7100AckControlFields {
            ack: 1,
            user_id: 42,
            interface_version: 858,
            data: vec![0x5a; 1379],
        };
        let packet = build_candidate_official_5188_ack_control_packet(&fields, 14)
            .expect("candidate ACK packet");
        let decoded = decode_dictionary_response(&packet, super::STOCK_DICTIONARY_BYTES)
            .expect("decode candidate ACK");
        assert_eq!(decoded.len(), 1685);
        assert_eq!(super::decode_utf16_prefix(&decoded[..20]), "加密包");
        assert_eq!(
            [
                u32::from_le_bytes(decoded[20..24].try_into().unwrap()),
                u32::from_le_bytes(decoded[24..28].try_into().unwrap()),
                u32::from_le_bytes(decoded[28..32].try_into().unwrap()),
                u32::from_le_bytes(decoded[32..36].try_into().unwrap()),
                u32::from_le_bytes(decoded[36..40].try_into().unwrap()),
            ],
            [8, 0, 0, 1685, 1685]
        );
        assert!(contains_utf16(&decoded, "大智慧C_ACK"));
        assert_eq!(&decoded[272..276], &14u32.to_le_bytes());
        assert_eq!(&decoded[304..1683], fields.data.as_slice());
    }

    #[test]
    fn candidate_5188_ack_rejects_unobserved_data_length() {
        let fields = Auth7100AckControlFields {
            ack: 1,
            user_id: 42,
            interface_version: 858,
            data: Vec::new(),
        };
        let error = build_candidate_official_5188_ack_control_packet(&fields, 14)
            .expect_err("empty ACK manifest must fail");
        assert!(error.to_string().contains("unverified ACK data length"));
    }

    #[test]
    fn authenticated_control_session_preserves_one_socket_across_exchanges() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind control fixture");
        let endpoint = listener.local_addr().expect("control fixture endpoint");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept control fixture");
            for _ in 0..2 {
                let packet = read_netpacket(&mut stream).expect("read control request");
                stream.write_all(&packet).expect("echo control response");
            }
        });

        let stream = TcpStream::connect(endpoint).expect("connect control fixture");
        let mut session = Auth7100ControlSession {
            stream,
            login: fixture_login_result(endpoint.to_string()),
        };
        let first = build_auth_7100_followup_download_packet(3).expect("first request");
        let second = build_auth_7100_followup_finish_packet(4).expect("second request");
        assert_eq!(
            session
                .exchange_current_session_packet(&first)
                .expect("first exchange"),
            first
        );
        assert_eq!(
            session
                .exchange_current_session_packet(&second)
                .expect("second exchange"),
            second
        );
        assert_eq!(session.login_result().endpoint, endpoint.to_string());
        server.join().expect("join control fixture");
    }

    #[test]
    fn authenticated_control_session_rejects_incomplete_network_packet() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind control fixture");
        let endpoint = listener.local_addr().expect("control fixture endpoint");
        let stream = TcpStream::connect(endpoint).expect("connect control fixture");
        let mut session = Auth7100ControlSession {
            stream,
            login: fixture_login_result(endpoint.to_string()),
        };
        let mut packet = build_auth_7100_followup_download_packet(3).expect("request");
        packet.pop();
        let error = session
            .exchange_current_session_packet(&packet)
            .expect_err("truncated packet must fail");
        assert!(error.to_string().contains("length mismatch"));
    }

    #[test]
    fn authenticated_control_session_extracts_current_response_5188_candidates() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind control fixture");
        let endpoint = listener.local_addr().expect("control fixture endpoint");
        let expected = Official5188Frame {
            kind: Official5188Kind::CLIENT_INIT,
            metadata: [1, 2, 3, 4],
            payload: vec![5, 6, 7, 8],
        };
        let encoded = expected.encode().expect("encode 5188 fixture");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept control fixture");
            let mut response = read_netpacket(&mut stream).expect("read control request");
            response.extend_from_slice(&encoded);
            let response_len = response.len();
            super::patch_u32(&mut response, 32, response_len).expect("patch response length");
            super::patch_u32(&mut response, 36, response_len).expect("patch response length");
            stream.write_all(&response).expect("write control response");
        });

        let stream = TcpStream::connect(endpoint).expect("connect control fixture");
        let mut session = Auth7100ControlSession {
            stream,
            login: fixture_login_result(endpoint.to_string()),
        };
        let request = build_auth_7100_followup_download_packet(3).expect("request");
        let candidates = session
            .exchange_official_5188_initialization_candidates(&request)
            .expect("extract candidates");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].offset, request.len());
        assert_eq!(candidates[0].frame, expected);
        server.join().expect("join control fixture");
    }

    #[test]
    fn expected_5188_init_selector_rejects_missing_or_ambiguous_candidates() {
        let expected = Official5188Frame {
            kind: Official5188Kind::CLIENT_INIT,
            metadata: [1, 2, 3, 4],
            payload: vec![0; 95],
        };
        let candidate = netzip_fullpull::Official5188EmbeddedClientFrame {
            offset: 40,
            frame: expected.clone(),
        };
        assert_eq!(
            select_expected_official_5188_init(vec![candidate.clone()], 95)
                .expect("one expected candidate"),
            expected
        );
        assert!(select_expected_official_5188_init(Vec::new(), 95).is_err());
        assert!(
            select_expected_official_5188_init(vec![candidate.clone(), candidate], 95).is_err()
        );
    }

    #[test]
    fn authenticated_control_session_collects_ordered_3610_triplet() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind control fixture");
        let endpoint = listener.local_addr().expect("control fixture endpoint");
        let expected = [95usize, 94, 67].map(|payload_len| Official5188Frame {
            kind: Official5188Kind::CLIENT_INIT,
            metadata: [1, 2, 3, 4],
            payload: vec![payload_len as u8; payload_len],
        });
        let server_frames = [
            (Official5188ControlStage::Login, 2, expected[0].clone()),
            (Official5188ControlStage::Abk, 12, expected[1].clone()),
            (Official5188ControlStage::Ack, 14, expected[2].clone()),
        ];
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept control fixture");
            for (stage, request_number, frame) in server_frames {
                read_netpacket(&mut stream).expect("read control request");
                let response = fixture_control_stage_response(stage, request_number, &frame);
                stream.write_all(&response).expect("write control response");
            }
        });

        let stream = TcpStream::connect(endpoint).expect("connect control fixture");
        let mut session = Auth7100ControlSession {
            stream,
            login: fixture_login_result(endpoint.to_string()),
        };
        let login_request = build_candidate_official_5188_login_control_packet(
            &Auth7100LoginControlFields {
                local_ip: [127, 0, 0, 1],
                mac_ascii: [0; 12],
                account_permissions: [1; 6],
                broker: [2; 4],
                encryption_version: 3,
                user_id: 4,
                interface_version: 5,
            },
            2,
        )
        .expect("login request");
        let abk_request = build_candidate_official_5188_abk_control_packet(
            &Auth7100AbkControlFields {
                is_64_bit: 0,
                major_version: 1,
                minor_version: 2,
                build_number: 3,
                total_memory: [4; 8],
                user_id: 5,
                interface_version: 6,
            },
            12,
        )
        .expect("ABK request");
        let ack_request = build_candidate_official_5188_ack_control_packet(
            &Auth7100AckControlFields {
                ack: 1,
                user_id: 5,
                interface_version: 6,
                data: vec![7; 1379],
            },
            14,
        )
        .expect("ACK request");
        let triplet = session
            .exchange_official_5188_init_triplet(&login_request, &abk_request, &ack_request)
            .expect("collect init triplet");
        assert_eq!(triplet.login, expected[0]);
        assert_eq!(triplet.abk, expected[1]);
        assert_eq!(triplet.ack, expected[2]);
        server.join().expect("join control fixture");
    }

    #[test]
    fn orchestrates_7100_and_5188_as_one_interleaved_lifecycle() {
        let control_listener = TcpListener::bind("127.0.0.1:0").expect("bind control fixture");
        let control_endpoint = control_listener.local_addr().expect("control endpoint");
        let data_listener = TcpListener::bind("127.0.0.1:0").expect("bind data fixture");
        let data_endpoint = data_listener.local_addr().expect("data endpoint");

        let control_server = thread::spawn(move || {
            let (mut stream, _) = control_listener.accept().expect("accept control fixture");
            for (stage, request_number, payload_len) in [
                (Official5188ControlStage::Login, 2, 95),
                (Official5188ControlStage::Abk, 12, 94),
                (Official5188ControlStage::Ack, 14, 67),
            ] {
                read_netpacket(&mut stream).expect("read control request");
                let frame = Official5188Frame {
                    kind: Official5188Kind::CLIENT_INIT,
                    metadata: [0; 4],
                    payload: vec![payload_len as u8; payload_len],
                };
                stream
                    .write_all(&fixture_control_stage_response(
                        stage,
                        request_number,
                        &frame,
                    ))
                    .expect("write control response");
            }
        });
        let data_server = thread::spawn(move || {
            let (mut stream, _) = data_listener.accept().expect("accept data fixture");
            for (client_len, server_kind, server_len) in [
                (95, Official5188Kind::SERVER_INIT_CONTROL, 48),
                (94, Official5188Kind::SERVER_INIT_CONTROL, 631),
                (67, Official5188Kind::SERVER_INIT_CONTINUE, 466),
            ] {
                let frame = read_5188_fixture_frame(&mut stream);
                assert_eq!(frame.kind, Official5188Kind::CLIENT_INIT);
                assert_eq!(frame.payload.len(), client_len);
                let mut server_payload = vec![server_len as u8; server_len];
                if server_len == 631 {
                    server_payload[64] = 0;
                } else if server_len == 636 {
                    server_payload[111] = 0;
                }
                stream
                    .write_all(
                        &Official5188Frame {
                            kind: server_kind,
                            metadata: [1, 2, 3, 4],
                            payload: server_payload,
                        }
                        .encode()
                        .expect("encode data response"),
                    )
                    .expect("write data response");
            }
            for _ in 0..3 {
                let frame = read_5188_fixture_frame(&mut stream);
                assert_eq!(frame.kind, Official5188Kind::CLIENT_SESSION);
                Official5188ClientSessionEnvelope::decode(&frame)
                    .expect("valid post-initialization frame");
            }
        });

        let control_stream = TcpStream::connect(control_endpoint).expect("connect control fixture");
        let mut control = Auth7100ControlSession {
            stream: control_stream,
            login: fixture_login_result(data_endpoint.to_string()),
        };
        let mut data =
            Official5188Session::connect(data_endpoint.to_string(), Duration::from_secs(1))
                .expect("connect data fixture");
        let login_request = fixture_login_control_request(2);
        let abk_request = fixture_abk_control_request(12);
        let result = control
            .initialize_official_5188_interleaved(
                &mut data,
                &login_request,
                &abk_request,
                |ack_source| {
                    assert_eq!(ack_source.len(), 64);
                    Ok(fixture_ack_control_request(14))
                },
                |login_response, abk_response, ack_response| {
                    assert_eq!(login_response.payload.len(), 48);
                    assert_eq!(abk_response.payload.len(), 631);
                    assert_eq!(ack_response.payload.len(), 466);
                    Ok((0..3)
                        .map(|index| {
                            Official5188ClientSessionEnvelope {
                                word0: index,
                                word1: 2,
                                word2: 3,
                                word3: 4,
                            }
                            .into_frame([0; 4])
                        })
                        .collect())
                },
            )
            .expect("interleaved initialization");
        assert_eq!(result.login_response.payload.len(), 48);
        assert_eq!(result.abk_response.payload.len(), 631);
        assert_eq!(result.ack_response.payload.len(), 466);
        assert_eq!(result.post_initialization_frame_count, 3);
        drop(data);
        control_server.join().expect("join control fixture");
        data_server.join().expect("join data fixture");
    }

    #[test]
    fn strict_control_stage_rejects_mismatched_answer_number() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind control fixture");
        let endpoint = listener.local_addr().expect("control fixture endpoint");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept control fixture");
            read_netpacket(&mut stream).expect("read login request");
            let frame = Official5188Frame {
                kind: Official5188Kind::CLIENT_INIT,
                metadata: [0; 4],
                payload: vec![0; 95],
            };
            let response =
                fixture_control_stage_response(Official5188ControlStage::Login, 3, &frame);
            stream.write_all(&response).expect("write control response");
        });
        let request = build_candidate_official_5188_login_control_packet(
            &Auth7100LoginControlFields {
                local_ip: [127, 0, 0, 1],
                mac_ascii: [0; 12],
                account_permissions: [1; 6],
                broker: [2; 4],
                encryption_version: 3,
                user_id: 4,
                interface_version: 5,
            },
            2,
        )
        .expect("login request");
        let stream = TcpStream::connect(endpoint).expect("connect control fixture");
        let mut session = Auth7100ControlSession {
            stream,
            login: fixture_login_result(endpoint.to_string()),
        };
        let error = session
            .exchange_official_5188_control_stage(&request, Official5188ControlStage::Login)
            .expect_err("mismatched answer number must fail");
        assert!(error.to_string().contains("number mismatch"));
        server.join().expect("join control fixture");
    }

    fn fixture_control_stage_response(
        stage: Official5188ControlStage,
        answer_number: u32,
        frame: &Official5188Frame,
    ) -> Vec<u8> {
        let mut decoded = vec![0u8; 40];
        write_fixed_utf16(&mut decoded[..20], "加密包").expect("write response root");
        append_control_field(
            &mut decoded,
            2,
            0,
            "请求",
            &super::utf16_bytes(stage.request_name()),
        )
        .expect("append response request");
        append_control_field(
            &mut decoded,
            2,
            0,
            "来源",
            &super::utf16_bytes("认证服务器"),
        )
        .expect("append response source");
        append_control_field(&mut decoded, 2, 3, "应答编号", &answer_number.to_le_bytes())
            .expect("append response number");
        append_control_field(
            &mut decoded,
            2,
            9,
            "数据",
            &frame.encode().expect("encode 5188 response frame"),
        )
        .expect("append response data");
        patch_u32(&mut decoded, 20, 4).expect("patch response field count");
        let decoded_len = decoded.len();
        patch_u32(&mut decoded, 32, decoded_len).expect("patch response length");
        patch_u32(&mut decoded, 36, decoded_len).expect("patch response length");
        encode_dictionary_control_packet(&decoded, super::FOLLOWUP_DOWNLOAD_HEX)
            .expect("encode control response")
    }

    fn fixture_login_control_request(request_number: u32) -> Vec<u8> {
        build_candidate_official_5188_login_control_packet(
            &Auth7100LoginControlFields {
                local_ip: [127, 0, 0, 1],
                mac_ascii: [0; 12],
                account_permissions: [1; 6],
                broker: [2; 4],
                encryption_version: 3,
                user_id: 4,
                interface_version: 5,
            },
            request_number,
        )
        .expect("login request")
    }

    fn fixture_abk_control_request(request_number: u32) -> Vec<u8> {
        build_candidate_official_5188_abk_control_packet(
            &Auth7100AbkControlFields {
                is_64_bit: 0,
                major_version: 1,
                minor_version: 2,
                build_number: 3,
                total_memory: [4; 8],
                user_id: 5,
                interface_version: 6,
            },
            request_number,
        )
        .expect("ABK request")
    }

    fn fixture_ack_control_request(request_number: u32) -> Vec<u8> {
        build_candidate_official_5188_ack_control_packet(
            &Auth7100AckControlFields {
                ack: 1,
                user_id: 5,
                interface_version: 6,
                data: vec![7; 1379],
            },
            request_number,
        )
        .expect("ACK request")
    }

    fn read_5188_fixture_frame(stream: &mut TcpStream) -> Official5188Frame {
        let mut header = [0u8; 8];
        stream.read_exact(&mut header).expect("read 5188 header");
        let payload_len = usize::from(u16::from_le_bytes([header[2], header[3]]));
        let mut encoded = Vec::with_capacity(8 + payload_len);
        encoded.extend_from_slice(&header);
        encoded.resize(8 + payload_len, 0);
        stream
            .read_exact(&mut encoded[8..])
            .expect("read 5188 payload");
        Official5188Frame::decode(&encoded).expect("decode fixture frame")
    }

    #[test]
    fn parses_the_complete_login_success_response_roles() {
        let first = decode_hex(super::FOLLOWUP_DOWNLOAD_HEX).expect("dict packet");
        let mut download = vec![0u8; 1540];
        download[..20].copy_from_slice(&first[..20]);
        download[20..24].copy_from_slice(&1u32.to_le_bytes());
        download[32..36].copy_from_slice(&1540u32.to_le_bytes());
        download[36..40].copy_from_slice(&1540u32.to_le_bytes());
        download[60..74].copy_from_slice(&utf16_tail("数据下载文件"));
        assert_eq!(classify_response(&first).unwrap(), "zstd_dictionary");
        assert_eq!(classify_response(&download).unwrap(), "download_file");
    }

    #[test]
    fn builds_ack_manifest_with_vendor_sections_and_crlf() {
        let text = build_ack_manifest_text(
            &[("System\\Stock.dat".to_string(), 0x1234_5678)],
            &["1|2|3|4|5|6|7|8|9|".to_string()],
            &[(7, 0x90ab_cdef)],
        );
        assert!(text.starts_with("SFLogInPack=ACK\r\n<MarketInfo>\r\n"));
        assert!(text.contains("System\\Stock.dat|305419896|\r\n"));
        assert!(text.contains("7|2427178479|\r\n"));
        assert!(text.ends_with("</SDidsCrc>\r\n"));
        assert!(
            !text.contains('\n')
                || text
                    .match_indices('\n')
                    .all(|(i, _)| i > 0 && text.as_bytes()[i - 1] == b'\r')
        );
    }

    #[test]
    fn default_ack_market_rows_include_all_structural_markets() {
        let rows = default_ack_market_rows();
        assert_eq!(rows.len(), 9);
        assert_eq!(rows[0], "18515|20|0|0|0|0|0|0|0|");
        assert_eq!(rows[8], "19010|4|0|0|0|0|0|0|0|");
    }

    #[test]
    fn default_ack_manifest_includes_market_section_rows() {
        let text = build_default_ack_manifest_text(&[], &[]);
        assert!(text.contains("<MarketInfo>\r\n"));
        assert!(text.contains("18515|20|0|0|0|0|0|0|0|\r\n"));
        assert!(text.contains("19010|4|0|0|0|0|0|0|0|\r\n"));
    }

    #[test]
    fn decodes_raw_content_dictionary_frame_without_exposing_payload() {
        let dictionary = b"network packet login success";
        let mut encoder =
            zstd::stream::Encoder::with_dictionary(Vec::new(), 3, dictionary).expect("encoder");
        encoder.write_all("登录成功".as_bytes()).expect("payload");
        let compressed = encoder.finish().expect("finish");

        let mut packet = vec![0u8; 172];
        packet[20..24].copy_from_slice(&4u32.to_le_bytes());
        packet[44..48].copy_from_slice(&12u32.to_le_bytes());
        packet.extend_from_slice(&compressed);
        packet.extend_from_slice(&[0, 0]);

        let decoded = decode_dictionary_response(&packet, dictionary).expect("dictionary decode");
        assert!(String::from_utf8_lossy(&decoded).contains("登录成功"));
    }

    #[test]
    fn decodes_the_captured_dictionary_request_with_the_verified_stock_dictionary() {
        let packet = decode_hex(super::FOLLOWUP_DOWNLOAD_HEX).expect("captured packet");
        let decoded = decode_dictionary_response(&packet, super::STOCK_DICTIONARY_BYTES)
            .expect("Stock.字典 decode");
        assert_eq!(decoded.len(), 130);
        assert!(contains_utf16(&decoded, "下载文件"));
        assert!(contains_utf16(&decoded, "通达信股票服务器.ini"));
    }

    #[test]
    fn classifies_standard_probe_response() {
        let mut response = vec![0u8; 74];
        response[20..24].copy_from_slice(&4u32.to_le_bytes());
        response[44..48].copy_from_slice(&8u32.to_le_bytes());
        response[60..74].copy_from_slice(&utf16_tail("压缩ZSTD"));
        assert_eq!(classify_response(&response).unwrap(), "auth_probe_response");
    }

    #[test]
    fn selects_5188_and_7709_routes_without_treating_either_as_authentication() {
        let entries = vec![
            DownloadedServerEntry {
                group_name: None,
                name: "quote".to_string(),
                host: "198.51.100.10".to_string(),
                main_port: 5188,
                secondary_port: 5188,
                enabled: true,
                broker: Some("华创".to_string()),
                permission: Some("点播版".to_string()),
                interface_version: Some(858),
            },
            DownloadedServerEntry {
                group_name: None,
                name: "bootstrap".to_string(),
                host: "198.51.100.11".to_string(),
                main_port: 7709,
                secondary_port: 7709,
                enabled: true,
                broker: None,
                permission: None,
                interface_version: None,
            },
        ];

        assert_eq!(
            select_server_endpoint(&entries, 5188).as_deref(),
            Some("198.51.100.10:5188")
        );
        assert_eq!(
            select_server_endpoint(&entries, 7709).as_deref(),
            Some("198.51.100.11:7709")
        );
        assert_eq!(select_server_endpoint(&entries, 14017), None);

        let ipv6_entry = DownloadedServerEntry {
            group_name: None,
            name: "ipv6".to_string(),
            host: "2001:db8::1".to_string(),
            main_port: 7709,
            secondary_port: 7709,
            enabled: true,
            broker: None,
            permission: None,
            interface_version: None,
        };
        assert_eq!(
            select_server_endpoint(&[ipv6_entry], 7709).as_deref(),
            Some("[2001:db8::1]:7709")
        );
    }

    #[test]
    fn current_login_control_fields_come_from_l1_route_metadata() {
        let login = Auth7100LoginResult {
            endpoint: "121.41.70.217:7100".to_string(),
            account: "fixture-account".to_string(),
            status: "登录成功".to_string(),
            authenticated: true,
            response_packet_lengths: vec![438, 1880],
            response_roles: vec!["auth_login".to_string(), "download_file".to_string()],
            dictionary_length: None,
            dictionary_sha256: None,
            decoded_response_lengths: Vec::new(),
            login_success_confirmed: true,
            quote_servers: vec![DownloadedServerEntry {
                group_name: None,
                name: "华创/点播版/北京联通".to_string(),
                host: "222.85.139.177".to_string(),
                main_port: 5188,
                secondary_port: 5188,
                enabled: true,
                broker: Some("华创".to_string()),
                permission: Some("点播版".to_string()),
                interface_version: Some(858),
            }],
            selected_quote_endpoint: Some("222.85.139.177:5188".to_string()),
            selected_7709_endpoint: None,
        };
        let fields = super::login_control_fields_from_result(&login, [192, 168, 3, 2], [b'A'; 12])
            .expect("L1 metadata");
        assert_eq!(fields.local_ip, [192, 168, 3, 2]);
        assert_eq!(fields.mac_ascii, [b'A'; 12]);
        assert_eq!(fields.interface_version, 858);
        assert_eq!(&fields.broker[..4], &utf16_bytes("华创")[..4]);
        assert_eq!(
            &fields.account_permissions[..6],
            &utf16_bytes("点播版")[..6]
        );
        assert!(
            super::login_control_fields_from_result(
                &Auth7100LoginResult {
                    selected_quote_endpoint: Some("198.51.100.10:5188".to_string()),
                    ..login
                },
                [192, 168, 3, 2],
                [0; 12],
            )
            .is_none()
        );
    }

    #[test]
    fn accepts_the_download_sample_as_a_7709_only_route_set() {
        let config = load_downloaded_server_config_sample().expect("download sample");
        assert_eq!(select_server_endpoint(&config.active_servers, 5188), None);
        assert!(
            select_server_endpoint(&config.active_servers, 7709)
                .expect("7709 route")
                .ends_with(":7709")
        );
    }

    #[test]
    fn login_sequences_keep_repeated_probes_and_both_response_orders() {
        assert!(login_core_sequence_is_valid(&[
            "auth_login",
            "auth_probe_response",
            "download_file",
            "auth_probe_response",
            "zstd_dictionary",
        ]));
        assert!(login_core_sequence_is_valid(&[
            "auth_login",
            "download_file",
            "auth_probe_response",
            "zstd_dictionary",
        ]));
        assert!(login_core_sequence_is_valid(&[
            "auth_login",
            "download_file"
        ]));
        assert!(!login_core_sequence_is_valid(&[
            "auth_login",
            "zstd_dictionary"
        ]));
    }

    #[test]
    fn dictionary_envelope_login_success_is_auth_login_not_zstd_dictionary() {
        let login = fixture_dictionary_wrapped_auth_login_packet();
        assert_eq!(classify_response(&login).unwrap(), "auth_login");
        assert_eq!(
            classify_response(&fixture_zstd_dictionary_packet()).unwrap(),
            "zstd_dictionary"
        );
    }

    #[test]
    fn login_accepts_dictionary_wrapped_login_then_download() {
        let login = login_against_fixture(vec![
            fixture_dictionary_wrapped_auth_login_packet(),
            fixture_download_file_packet(),
        ]);
        assert!(login.login_success_confirmed);
        assert_eq!(login.response_roles, vec!["auth_login", "download_file"]);
        assert_eq!(
            login.selected_quote_endpoint.as_deref(),
            Some("198.51.100.10:5188")
        );
    }

    #[test]
    fn structured_io_error_classifies_eagain_without_secrets() {
        let error = io::Error::from_raw_os_error(11);
        assert_eq!(classify_io_kind(&error), "would_block");
        let diagnostic = AuthIoError::from_io(
            "login_read",
            "read",
            "121.41.70.217:7100",
            &error,
            3,
            Duration::from_millis(812),
        );
        let text = diagnostic.to_string();
        assert!(text.contains("stage=login_read"));
        assert!(text.contains("endpoint=121.41.70.217:7100"));
        assert!(text.contains("kind=would_block"));
        assert!(text.contains("os=11"));
        assert!(text.contains("attempt=3"));
        assert!(text.contains("elapsed_ms=812"));
        assert!(!text.contains("password"));
        assert!(!text.contains("fixture-secret"));
        assert!(!text.contains("1522"));
    }

    #[test]
    fn read_exact_retries_would_block_and_interrupted() {
        let payload = b"retry-ok-packet";
        let mut reader = FlakyReader {
            blocks: 2,
            interrupts: 1,
            inner: Cursor::new(payload.to_vec()),
        };
        let mut buf = vec![0u8; payload.len()];
        read_exact_with_deadline(
            &mut reader,
            &mut buf,
            Instant::now() + Duration::from_secs(1),
            "login_read",
            "read",
            "127.0.0.1:7100",
        )
        .expect("retry until payload arrives");
        assert_eq!(buf, payload);
    }

    #[test]
    fn stalled_login_read_reports_structured_timeout_not_raw_os_error_11() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stalled fixture");
        let endpoint = listener.local_addr().expect("stalled fixture endpoint");
        let server = thread::spawn(move || {
            let (_stream, _) = listener.accept().expect("accept stalled fixture");
            thread::sleep(Duration::from_millis(400));
        });
        let error = connect_auth_sequence(
            &Auth7100ClientConfig {
                host: endpoint.ip().to_string(),
                port: endpoint.port(),
                account: "fixture-account".to_string(),
                password: "fixture-secret".to_string(),
                timeout: Duration::from_millis(250),
            },
            None,
        )
        .expect_err("stalled server must fail");
        let text = error.to_string();
        assert!(
            text.contains("stage=")
                && (text.contains("kind=would_block") || text.contains("kind=timed_out")),
            "expected structured timeout, got {text}"
        );
        assert!(!text.contains("fixture-secret"));
        assert!(!text.contains("password"));
        server.join().expect("join stalled fixture");
    }

    #[test]
    fn login_accepts_probe_then_download_then_dictionary() {
        let login = login_against_fixture(vec![
            fixture_auth_login_packet(),
            fixture_probe_packet(),
            fixture_download_file_packet(),
            fixture_probe_packet(),
            fixture_zstd_dictionary_packet(),
        ]);
        assert!(login.login_success_confirmed);
        assert_eq!(
            login.response_roles,
            vec![
                "auth_login",
                "auth_probe_response",
                "download_file",
                "auth_probe_response",
                "zstd_dictionary",
            ]
        );
        assert_eq!(
            login.selected_quote_endpoint.as_deref(),
            Some("198.51.100.10:5188")
        );
        assert!(
            !login
                .selected_quote_endpoint
                .as_deref()
                .unwrap()
                .ends_with(":7709")
        );
    }

    #[test]
    fn login_accepts_download_then_probe_then_dictionary() {
        let login = login_against_fixture(vec![
            fixture_auth_login_packet(),
            fixture_download_file_packet(),
            fixture_probe_packet(),
            fixture_zstd_dictionary_packet(),
        ]);
        assert!(login.login_success_confirmed);
        assert_eq!(
            login.response_roles,
            vec![
                "auth_login",
                "download_file",
                "auth_probe_response",
                "zstd_dictionary",
            ]
        );
        assert_eq!(login.response_packet_lengths.len(), 4);
    }

    fn login_against_fixture(responses: Vec<Vec<u8>>) -> Auth7100LoginResult {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind login fixture");
        let endpoint = listener.local_addr().expect("login fixture endpoint");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept login fixture");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("server read timeout");
            let _login = read_netpacket(&mut stream).expect("read login request");
            let auth_login = responses
                .iter()
                .position(|packet| classify_response(packet).unwrap() == "auth_login")
                .expect("fixture contains auth_login");
            for packet in &responses[..=auth_login] {
                stream.write_all(packet).expect("write pre-download");
            }
            let leftover = &responses[auth_login + 1..];
            let _download = read_netpacket(&mut stream).expect("read download request");
            for packet in leftover {
                stream.write_all(packet).expect("write post-download");
            }
            // Keep the control socket open so the optional dictionary drain
            // times out instead of seeing EOF from a dropped fixture peer.
            thread::sleep(Duration::from_millis(700));
        });
        let (_stream, login) = connect_auth_sequence(
            &Auth7100ClientConfig {
                host: endpoint.ip().to_string(),
                port: endpoint.port(),
                account: "fixture-account".to_string(),
                password: "fixture-secret".to_string(),
                timeout: Duration::from_secs(3),
            },
            None,
        )
        .expect("login fixture");
        let _ = server.join();
        login
    }

    fn fixture_login_success_object() -> Vec<u8> {
        let mut decoded = vec![0u8; 40];
        write_fixed_utf16(&mut decoded[..20], "认证").expect("root");
        append_control_field(&mut decoded, 2, 0, "提示信息", &utf16_bytes("登录成功"))
            .expect("success field");
        for index in 1..15 {
            append_control_field(
                &mut decoded,
                4,
                2,
                &format!("字段{index}"),
                &(index as u32).to_le_bytes(),
            )
            .expect("metadata field");
        }
        patch_u32(&mut decoded, 20, 15).expect("field count");
        let decoded_len = decoded.len();
        patch_u32(&mut decoded, 32, decoded_len).expect("length");
        patch_u32(&mut decoded, 36, decoded_len).expect("duplicate length");
        decoded
    }

    fn fixture_auth_login_packet() -> Vec<u8> {
        let decoded = fixture_login_success_object();
        let compressed = zstd::bulk::compress(&decoded, 3).expect("plain zstd");
        let mut packet = decode_hex(super::OUTER_PREFIX_HEX).expect("outer prefix");
        let packet_len = packet.len() + compressed.len() + 2;
        patch_u32(&mut packet, 32, packet_len).expect("packet length");
        patch_u32(&mut packet, 36, packet_len).expect("duplicate packet length");
        patch_u32(&mut packet, 102, compressed.len()).expect("compressed length");
        patch_u32(&mut packet, 136, decoded.len()).expect("decoded length");
        patch_u32(&mut packet, 146, compressed.len()).expect("zstd length");
        patch_u32(&mut packet, 158, compressed.len() + 28).expect("object span");
        packet.extend_from_slice(&compressed);
        packet.extend_from_slice(&[0, 0]);
        packet
    }

    fn fixture_probe_packet() -> Vec<u8> {
        let mut response = vec![0u8; 74];
        write_fixed_utf16(&mut response[..20], "网络包").expect("object");
        response[20..24].copy_from_slice(&4u32.to_le_bytes());
        response[32..36].copy_from_slice(&74u32.to_le_bytes());
        response[36..40].copy_from_slice(&74u32.to_le_bytes());
        response[44..48].copy_from_slice(&8u32.to_le_bytes());
        response[60..74].copy_from_slice(&utf16_tail("压缩ZSTD"));
        response
    }

    fn fixture_download_file_packet() -> Vec<u8> {
        let text = "\u{feff}[行情服务器]\n华创, 点播版, 北京联通, 198.51.100.10, 5188, 858, 8.50\n";
        let mut packet = vec![0u8; 74];
        write_fixed_utf16(&mut packet[..20], "网络包").expect("object");
        packet[20..24].copy_from_slice(&1u32.to_le_bytes());
        packet[60..74].copy_from_slice(&utf16_tail("数据下载文件"));
        for word in text.encode_utf16() {
            packet.extend_from_slice(&word.to_le_bytes());
        }
        let len = packet.len() as u32;
        packet[32..36].copy_from_slice(&len.to_le_bytes());
        packet[36..40].copy_from_slice(&len.to_le_bytes());
        packet
    }

    fn fixture_dictionary_wrapped_auth_login_packet() -> Vec<u8> {
        let decoded = fixture_login_success_object();
        let mut compressor =
            zstd::bulk::Compressor::with_dictionary(3, super::STOCK_DICTIONARY_BYTES)
                .expect("dictionary compressor");
        let compressed = compressor.compress(&decoded).expect("compress");
        let mut packet = vec![0u8; 74];
        write_fixed_utf16(&mut packet[..20], "网络包").expect("object");
        packet[20..24].copy_from_slice(&4u32.to_le_bytes());
        packet[44..48].copy_from_slice(&12u32.to_le_bytes());
        packet[60..74].copy_from_slice(&utf16_tail("压缩ZSTD"));
        packet.extend_from_slice(&compressed);
        packet.extend_from_slice(&[0, 0]);
        let len = packet.len() as u32;
        packet[32..36].copy_from_slice(&len.to_le_bytes());
        packet[36..40].copy_from_slice(&len.to_le_bytes());
        packet
    }

    fn fixture_zstd_dictionary_packet() -> Vec<u8> {
        let mut decoded = vec![0u8; 40];
        write_fixed_utf16(&mut decoded[..20], "加密解密").expect("root");
        append_control_field(&mut decoded, 2, 0, "请求", &utf16_bytes("Tdx_Encrypt"))
            .expect("request");
        append_control_field(&mut decoded, 2, 0, "来源", &utf16_bytes("认证服务器"))
            .expect("source");
        append_control_field(&mut decoded, 4, 2, "应答编号", &2u32.to_le_bytes()).expect("number");
        append_control_field(&mut decoded, 2, 0, "数据", &utf16_bytes("ok")).expect("data");
        patch_u32(&mut decoded, 20, 4).expect("field count");
        let decoded_len = decoded.len();
        patch_u32(&mut decoded, 32, decoded_len).expect("length");
        patch_u32(&mut decoded, 36, decoded_len).expect("duplicate length");
        let mut compressor =
            zstd::bulk::Compressor::with_dictionary(3, super::STOCK_DICTIONARY_BYTES)
                .expect("dictionary compressor");
        let compressed = compressor.compress(&decoded).expect("compress");
        let mut packet = vec![0u8; 74];
        write_fixed_utf16(&mut packet[..20], "网络包").expect("object");
        packet[20..24].copy_from_slice(&4u32.to_le_bytes());
        packet[44..48].copy_from_slice(&12u32.to_le_bytes());
        packet[60..74].copy_from_slice(&utf16_tail("压缩ZSTD"));
        packet.extend_from_slice(&compressed);
        packet.extend_from_slice(&[0, 0]);
        let len = packet.len() as u32;
        packet[32..36].copy_from_slice(&len.to_le_bytes());
        packet[36..40].copy_from_slice(&len.to_le_bytes());
        packet
    }

    struct FlakyReader {
        blocks: u32,
        interrupts: u32,
        inner: Cursor<Vec<u8>>,
    }

    impl Read for FlakyReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.blocks > 0 {
                self.blocks -= 1;
                return Err(io::Error::from_raw_os_error(11));
            }
            if self.interrupts > 0 {
                self.interrupts -= 1;
                return Err(io::Error::from_raw_os_error(4));
            }
            self.inner.read(buf)
        }
    }

    fn contains_utf16(bytes: &[u8], value: &str) -> bool {
        let needle = value
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        bytes.windows(needle.len()).any(|window| window == needle)
    }

    fn utf16_tail(value: &str) -> [u8; 14] {
        let mut out = [0u8; 14];
        for (index, word) in value.encode_utf16().take(7).enumerate() {
            out[index * 2..index * 2 + 2].copy_from_slice(&word.to_le_bytes());
        }
        out
    }

    fn fixture_login_result(endpoint: String) -> Auth7100LoginResult {
        Auth7100LoginResult {
            endpoint,
            account: "fixture-account".to_string(),
            status: "fixture-authenticated".to_string(),
            authenticated: true,
            response_packet_lengths: Vec::new(),
            response_roles: Vec::new(),
            dictionary_length: None,
            dictionary_sha256: None,
            decoded_response_lengths: Vec::new(),
            login_success_confirmed: true,
            quote_servers: Vec::new(),
            selected_quote_endpoint: Some("127.0.0.1:5188".to_string()),
            selected_7709_endpoint: None,
        }
    }

    #[test]
    fn slot_control_numbers_are_unique_for_ten_wine_sockets() {
        let mut seen = std::collections::BTreeSet::new();
        for slot in 0..10 {
            let (login, abk, ack) = super::official_5188_slot_control_numbers(slot).unwrap();
            assert!(seen.insert(login), "duplicate login number {login}");
            assert!(seen.insert(abk), "duplicate ABK number {abk}");
            assert!(seen.insert(ack), "duplicate ACK number {ack}");
        }
        assert_eq!(
            super::official_5188_slot_control_numbers(0).unwrap(),
            (2, 12, 22)
        );
        assert_eq!(
            super::official_5188_slot_control_numbers(9).unwrap(),
            (11, 21, 31)
        );
        assert!(super::official_5188_slot_control_numbers(10).is_err());
    }

    #[test]
    fn wine_slot_plan_always_opens_ten_connections() {
        assert_eq!(
            super::official_5188_wine_slot_indexes(5)
                .unwrap()
                .collect::<Vec<_>>(),
            (0..10).collect::<Vec<_>>()
        );
        assert_eq!(
            super::official_5188_wine_slot_indexes(7)
                .unwrap()
                .collect::<Vec<_>>(),
            (0..10).collect::<Vec<_>>()
        );
        assert!(super::official_5188_wine_slot_indexes(11).is_err());
    }

    #[test]
    fn final_three_wine_slots_initialize_without_fabricated_subscriptions() {
        let session_frame = || netzip_fullpull::Official5188Frame {
            kind: netzip_fullpull::Official5188Kind::CLIENT_SESSION,
            metadata: [0; 4],
            payload: Vec::new(),
        };
        let partitions = (0..7)
            .map(|slot| vec![(*b"SH", 0x1_0000 + slot)])
            .collect::<Vec<_>>();
        let mut entry_counts = Vec::new();
        for slot in 0..10 {
            let (frames, entries, partition_count) = super::official_5188_slot_tail(
                vec![session_frame(), session_frame(), session_frame()],
                &partitions,
                slot,
            )
            .unwrap();
            assert_eq!(partition_count, 7);
            assert_eq!(
                frames.len(),
                match slot {
                    0 => 5,
                    1..=6 => 4,
                    _ => 3,
                }
            );
            if slot == 0 {
                assert_eq!(
                    frames[3].kind,
                    netzip_fullpull::Official5188Kind::CLIENT_HEARTBEAT
                );
                assert_eq!(
                    frames[3].payload,
                    super::OFFICIAL_5188_PRIMARY_HEARTBEAT_PAYLOAD
                );
            }
            entry_counts.push(entries);
        }
        assert_eq!(entry_counts, vec![1, 1, 1, 1, 1, 1, 1, 0, 0, 0]);
    }
}
