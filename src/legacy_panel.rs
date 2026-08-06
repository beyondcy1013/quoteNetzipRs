use crate::{
    auth_credentials,
    DownloadedServerConfig, ProtoProbeConfig, ProtoProbeEncoding,
    load_downloaded_server_config_sample, probe_proto,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const DEFAULT_DATA_HOST: &str = "120.195.71.160";
pub const DEFAULT_DATA_PORT: u16 = 7709;
const AUTH_PROTOCOL_VERSION: &str = "20221120";
const RUNTIME_STATE_REL: &str = "tmp/netzip_legacy_panel_state.json";
const REFERENCE_CONFIG_REL: &str = "netzip_api_bin/NetzipAPI/StockC++/用户/配置文件.ini";
const REFERENCE_SERVER_LIST_REL: &str = "netzip_api_bin/NetzipAPI/StockC++/用户/服务器列表.ini";
const PROTOCOL_NOTE: &str =
    "当前 Linux Web 服务可加载配置、测速和端口连通性验证；真实 DLL 登录仍待协议封装。";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyAuthServer {
    pub value: String,
    pub name: String,
    pub display_name: String,
    pub host: String,
    pub probe_port: u16,
    pub login_port: u16,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyDataServer {
    pub value: String,
    pub name: String,
    pub display_name: String,
    pub host: String,
    pub port: u16,
    pub role: String,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyPanelConfig {
    pub account: String,
    #[serde(skip_serializing)]
    pub password: String,
    pub account_expiry: String,
    pub analysis_software: String,
    pub analysis_version: String,
    pub auto_upgrade: String,
    pub selected_auth_server: String,
    pub selected_stock_main_server: String,
    pub selected_stock_backup_server: String,
    pub selected_reserved_server: String,
    pub selected_wenhua_server: String,
    pub selected_boyi_server: String,
    pub enable_stock_main: bool,
    pub enable_stock_backup: bool,
    pub enable_reserved: bool,
    pub enable_wenhua: bool,
    pub enable_boyi: bool,
    pub fill_daily: bool,
    pub fill_daily_count: u32,
    pub fill_5min: bool,
    pub fill_5min_count: u32,
    pub fill_1min: bool,
    pub fill_1min_count: u32,
    pub fill_ticks: bool,
    pub fill_ticks_count: u32,
    pub fill_f10: bool,
    pub fill_all_stocks: bool,
    pub network_time: bool,
    pub data_interval: u32,
    pub operator_enabled: bool,
    pub operator_name: String,
    pub connect_selected: bool,
    pub disconnect_selected: bool,
    pub login_status_text: String,
    pub status_bar_text: String,
    pub password_info_url_template: Option<String>,
    pub protocol_note: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct LegacyPanelBootstrap {
    pub config_source: String,
    pub servers_source: String,
    pub state_path: String,
    pub auth_login_supported: bool,
    pub data_login_supported: bool,
    pub auth_servers: Vec<LegacyAuthServer>,
    pub data_servers: Vec<LegacyDataServer>,
    pub downloaded_server_config: Option<DownloadedServerConfig>,
    pub config: LegacyPanelConfig,
}

#[derive(Clone, Debug, Serialize)]
pub struct LegacyPanelSaveResponse {
    pub saved: bool,
    pub state_path: String,
    pub status_text: String,
    pub config: LegacyPanelConfig,
}

#[derive(Clone, Debug, Serialize)]
pub struct LegacyTcpProbe {
    pub target: String,
    pub host: String,
    pub port: u16,
    pub reachable: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LegacyAuthServerProbe {
    pub value: String,
    pub display_name: String,
    pub host: String,
    pub probe_6100: LegacyTcpProbe,
    pub login_7100: LegacyTcpProbe,
}

#[derive(Clone, Debug, Serialize)]
pub struct LegacyDataServerProbe {
    pub value: String,
    pub display_name: String,
    pub host: String,
    pub port: u16,
    pub probe: LegacyTcpProbe,
}

#[derive(Clone, Debug, Serialize)]
pub struct LegacyPanelProbeResponse {
    pub timeout_ms: u64,
    pub recommended_auth_server: String,
    pub recommended_stock_backup_server: String,
    pub auth_servers: Vec<LegacyAuthServerProbe>,
    pub data_servers: Vec<LegacyDataServerProbe>,
    pub downloaded_server_config: Option<DownloadedServerConfig>,
    pub status_text: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct LegacyProtocolProbe {
    pub target: String,
    pub request_text: String,
    pub sent_bytes: usize,
    pub reply_bytes: usize,
    pub reply_head_hex: String,
    pub response_utf16le_preview: Option<String>,
    pub parsed_kind: Option<String>,
    pub parsed_summary: Option<String>,
    pub transport_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LegacyPanelConnectResponse {
    pub state_path: String,
    pub effective_auth_server: Option<LegacyAuthServer>,
    pub effective_stock_backup_server: Option<LegacyDataServer>,
    pub auth_probe: Option<LegacyTcpProbe>,
    pub auth_login_probe: Option<LegacyTcpProbe>,
    pub stock_backup_probe: Option<LegacyTcpProbe>,
    pub protocol_probe: Option<LegacyProtocolProbe>,
    pub protocol_login_supported: bool,
    pub protocol_login_state: String,
    pub downloaded_server_config: Option<DownloadedServerConfig>,
    pub login_status_text: String,
    pub status_text: String,
    pub config: LegacyPanelConfig,
}

#[derive(Clone, Debug, Serialize)]
pub struct LegacyPanelDisconnectResponse {
    pub saved: bool,
    pub state_path: String,
    pub login_status_text: String,
    pub status_text: String,
    pub config: LegacyPanelConfig,
}

pub fn load_bootstrap() -> Result<LegacyPanelBootstrap, Box<dyn Error>> {
    let auth_servers = load_auth_servers()?;
    let downloaded_server_config = load_downloaded_server_config_sample().ok();
    let config_source = manifest_path(REFERENCE_CONFIG_REL);
    let servers_source = manifest_path(REFERENCE_SERVER_LIST_REL);
    let state_path = manifest_path(RUNTIME_STATE_REL);
    let defaults = load_reference_config(&auth_servers)?;
    let config = load_runtime_state()?.unwrap_or(defaults);
    let config = normalize_config(config, &auth_servers);
    let data_servers = build_data_servers(&config, downloaded_server_config.as_ref());

    Ok(LegacyPanelBootstrap {
        config_source: config_source.display().to_string(),
        servers_source: servers_source.display().to_string(),
        state_path: state_path.display().to_string(),
        auth_login_supported: false,
        data_login_supported: false,
        auth_servers,
        data_servers,
        downloaded_server_config,
        config,
    })
}

pub fn save_config(config: LegacyPanelConfig) -> Result<LegacyPanelSaveResponse, Box<dyn Error>> {
    let auth_servers = load_auth_servers()?;
    let config = normalize_config(config, &auth_servers);
    persist_runtime_state(&config)?;
    let state_path = manifest_path(RUNTIME_STATE_REL);

    Ok(LegacyPanelSaveResponse {
        saved: true,
        state_path: state_path.display().to_string(),
        status_text: "已保存本地连接配置".to_string(),
        config,
    })
}

pub fn probe_servers(
    config: LegacyPanelConfig,
    timeout_ms: u64,
) -> Result<LegacyPanelProbeResponse, Box<dyn Error>> {
    let auth_servers = load_auth_servers()?;
    let downloaded_server_config = load_downloaded_server_config_sample().ok();
    let config = normalize_config(config, &auth_servers);
    let data_servers = build_data_servers(&config, downloaded_server_config.as_ref());

    let auth_probes = auth_servers
        .iter()
        .map(|server| LegacyAuthServerProbe {
            value: server.value.clone(),
            display_name: server.display_name.clone(),
            host: server.host.clone(),
            probe_6100: tcp_probe(&server.host, server.probe_port, timeout_ms),
            login_7100: tcp_probe(&server.host, server.login_port, timeout_ms),
        })
        .collect::<Vec<_>>();

    let data_probes = data_servers
        .iter()
        .map(|server| LegacyDataServerProbe {
            value: server.value.clone(),
            display_name: server.display_name.clone(),
            host: server.host.clone(),
            port: server.port,
            probe: tcp_probe(&server.host, server.port, timeout_ms),
        })
        .collect::<Vec<_>>();

    let recommended_auth_server = recommended_auth_server(&config, &auth_probes);
    let recommended_stock_backup_server = recommended_data_server(&config, &data_probes);
    let status_text = build_probe_status(
        &config,
        &auth_probes,
        &data_probes,
        &recommended_auth_server,
        &recommended_stock_backup_server,
    );

    Ok(LegacyPanelProbeResponse {
        timeout_ms,
        recommended_auth_server,
        recommended_stock_backup_server,
        auth_servers: auth_probes,
        data_servers: data_probes,
        downloaded_server_config,
        status_text,
    })
}

pub fn connect(
    config: LegacyPanelConfig,
    timeout_ms: u64,
) -> Result<LegacyPanelConnectResponse, Box<dyn Error>> {
    let probe = probe_servers(config.clone(), timeout_ms)?;
    let auth_servers = load_auth_servers()?;
    let downloaded_server_config = load_downloaded_server_config_sample().ok();
    let mut config = normalize_config(config, &auth_servers);
    config.selected_auth_server = probe.recommended_auth_server.clone();
    config.selected_stock_backup_server = probe.recommended_stock_backup_server.clone();

    let effective_auth_server = auth_servers
        .iter()
        .find(|server| server.value == config.selected_auth_server)
        .cloned();
    let data_servers = build_data_servers(&config, downloaded_server_config.as_ref());
    let effective_stock_backup_server = data_servers
        .iter()
        .find(|server| server.value == config.selected_stock_backup_server)
        .cloned();

    let auth_probe = effective_auth_server.as_ref().and_then(|server| {
        probe
            .auth_servers
            .iter()
            .find(|item| item.value == server.value)
            .map(|item| item.probe_6100.clone())
    });
    let auth_login_probe = effective_auth_server.as_ref().and_then(|server| {
        probe
            .auth_servers
            .iter()
            .find(|item| item.value == server.value)
            .map(|item| item.login_7100.clone())
    });
    let stock_backup_probe = effective_stock_backup_server.as_ref().and_then(|server| {
        probe
            .data_servers
            .iter()
            .find(|item| item.value == server.value)
            .map(|item| item.probe.clone())
    });

    let protocol_probe = effective_auth_server
        .as_ref()
        .filter(|_| auth_login_probe.as_ref().is_some_and(|item| item.reachable))
        .map(|server| -> Result<LegacyProtocolProbe, Box<dyn Error>> {
            let request_text = build_auth_login_request(&config);
            let result = probe_proto(&ProtoProbeConfig {
                host: server.host.clone(),
                port: server.login_port,
                payload: Some(request_text.clone()),
                encoding: ProtoProbeEncoding::Utf16Le,
                nul_terminate: true,
                prefix_u32le: false,
                read_secs: 2,
            })?;

            Ok(LegacyProtocolProbe {
                target: result.target,
                request_text,
                sent_bytes: result.sent_len,
                reply_bytes: result.reply_bytes,
                reply_head_hex: result.reply_head_hex,
                response_utf16le_preview: result.response_utf16le_preview,
                parsed_kind: result.parsed_kind,
                parsed_summary: result.parsed_summary,
                transport_error: result.transport_error,
            })
        })
        .transpose()?;

    let protocol_login_state = if let Some(protocol_probe) = &protocol_probe {
        if protocol_probe.transport_error.is_some() {
            "transport_error".to_string()
        } else if protocol_probe.reply_bytes > 0 {
            "reply_received".to_string()
        } else {
            "transport_connected_no_reply".to_string()
        }
    } else if auth_login_probe.as_ref().is_some_and(|item| item.reachable) {
        "login_port_reachable".to_string()
    } else {
        "login_port_unreachable".to_string()
    };

    let protocol_login_supported = matches!(protocol_login_state.as_str(), "reply_received");
    config.connect_selected = auth_probe.as_ref().is_some_and(|item| item.reachable)
        || auth_login_probe.as_ref().is_some_and(|item| item.reachable)
        || stock_backup_probe
            .as_ref()
            .is_some_and(|item| item.reachable);
    config.disconnect_selected = !config.connect_selected;
    config.login_status_text = if protocol_login_supported {
        "认证收到回复".to_string()
    } else if config.connect_selected {
        "端口已连通，协议无回复".to_string()
    } else {
        "网络断开".to_string()
    };
    config.status_bar_text = build_connect_status(
        &auth_probe,
        &auth_login_probe,
        &stock_backup_probe,
        &protocol_login_state,
    );
    persist_runtime_state(&config)?;

    Ok(LegacyPanelConnectResponse {
        state_path: manifest_path(RUNTIME_STATE_REL).display().to_string(),
        effective_auth_server,
        effective_stock_backup_server,
        auth_probe,
        auth_login_probe,
        stock_backup_probe,
        protocol_probe,
        protocol_login_supported,
        protocol_login_state,
        downloaded_server_config,
        login_status_text: config.login_status_text.clone(),
        status_text: config.status_bar_text.clone(),
        config,
    })
}

pub fn disconnect(
    config: LegacyPanelConfig,
) -> Result<LegacyPanelDisconnectResponse, Box<dyn Error>> {
    let auth_servers = load_auth_servers()?;
    let mut config = normalize_config(config, &auth_servers);
    config.connect_selected = false;
    config.disconnect_selected = true;
    config.login_status_text = "已断开（本地状态）".to_string();
    config.status_bar_text = "已断开；当前未保持持久连接。".to_string();
    persist_runtime_state(&config)?;

    Ok(LegacyPanelDisconnectResponse {
        saved: true,
        state_path: manifest_path(RUNTIME_STATE_REL).display().to_string(),
        login_status_text: config.login_status_text.clone(),
        status_text: config.status_bar_text.clone(),
        config,
    })
}

pub fn password_info_url(config: &LegacyPanelConfig) -> Option<String> {
    let template = config.password_info_url_template.as_ref()?;
    let first = template.replacen("%s", &config.account, 1);
    Some(first.replacen("%s", &config.password, 1))
}

fn load_auth_servers() -> Result<Vec<LegacyAuthServer>, Box<dyn Error>> {
    let path = manifest_path(REFERENCE_SERVER_LIST_REL);
    let text = read_utf16le_text(&path)?;
    let mut section = String::new();
    let mut grouped = BTreeMap::<(String, String), LegacyAuthServer>::new();

    for raw_line in text.lines() {
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line.trim_matches(['[', ']']).to_string();
            continue;
        }
        if section != "认证服务器" || line.starts_with("//") {
            continue;
        }

        let (payload, comment) = split_comment(line);
        let parts = payload
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>();
        if parts.len() != 3 {
            continue;
        }

        let raw_name = parts[0].to_string();
        let host = parts[1].to_string();
        let port = parts[2].parse::<u16>()?;
        let (base_name, hinted_port) = split_name_port(&raw_name);
        let key = (base_name.clone(), host.clone());
        let entry = grouped.entry(key).or_insert_with(|| LegacyAuthServer {
            value: raw_name.clone(),
            name: base_name.clone(),
            display_name: raw_name.clone(),
            host: host.clone(),
            probe_port: hinted_port.unwrap_or(port),
            login_port: hinted_port.unwrap_or(port),
            note: comment.map(str::to_string),
        });

        match hinted_port.unwrap_or(port) {
            6100 => {
                entry.value = raw_name.clone();
                entry.display_name = raw_name.clone();
                entry.probe_port = port;
            }
            7100 => {
                entry.login_port = port;
            }
            other => {
                entry.probe_port = other;
                entry.login_port = other;
            }
        }
        if entry.note.is_none() {
            entry.note = comment.map(str::to_string);
        }
    }

    let mut out = grouped.into_values().collect::<Vec<_>>();
    out.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    Ok(out)
}

fn load_reference_config(
    auth_servers: &[LegacyAuthServer],
) -> Result<LegacyPanelConfig, Box<dyn Error>> {
    let path = manifest_path(REFERENCE_CONFIG_REL);
    let text = read_utf16le_text(&path)?;
    let values = parse_key_values(&text);

    let config = LegacyPanelConfig {
        account: value_of(&values, "账号", "168"),
        password: value_of(&values, "密码", "168"),
        account_expiry: value_of(&values, "账号到期", ""),
        analysis_software: value_of(&values, "分析软件", "自定义"),
        analysis_version: value_of(&values, "分析软件版本", "510"),
        auto_upgrade: value_of(&values, "自动升级", "稳定版"),
        selected_auth_server: normalize_auth_choice(
            &value_of(&values, "认证服务器", "智能选择"),
            auth_servers,
        ),
        selected_stock_main_server: value_of(&values, "股票主站服务器", "智能选择"),
        selected_stock_backup_server: value_of(&values, "股票备用服务器", "南京移动"),
        selected_reserved_server: String::new(),
        selected_wenhua_server: String::new(),
        selected_boyi_server: String::new(),
        enable_stock_main: truthy(&values, "登录股票主站"),
        enable_stock_backup: truthy(&values, "登录股票备用"),
        enable_reserved: false,
        enable_wenhua: false,
        enable_boyi: false,
        fill_daily: truthy(&values, "补日线"),
        fill_daily_count: integer_of(&values, "补日线根数", 1000),
        fill_5min: truthy(&values, "补5分钟线"),
        fill_5min_count: integer_of(&values, "补5分钟根数", 3000),
        fill_1min: truthy(&values, "补1分钟线"),
        fill_1min_count: integer_of(&values, "补1分钟根数", 2000),
        fill_ticks: truthy(&values, "补分笔"),
        fill_ticks_count: integer_of(&values, "补分时根数", 1).max(integer_of(
            &values,
            "补分笔根数",
            1,
        )),
        fill_f10: truthy(&values, "补F10"),
        fill_all_stocks: truthy(&values, "补全部股票"),
        network_time: truthy(&values, "网络校时"),
        data_interval: integer_of(&values, "补数据间隔", 10),
        operator_enabled: truthy(&values, "网络运营商"),
        operator_name: value_of(&values, "运营商名称", "XX移动"),
        connect_selected: false,
        disconnect_selected: true,
        login_status_text: value_of(&values, "登录状态", "网络断开"),
        status_bar_text: value_of(&values, "提示信息", "点击连接重新登录"),
        password_info_url_template: values.get("账号信息").cloned(),
        protocol_note: PROTOCOL_NOTE.to_string(),
    };

    Ok(config)
}

fn load_runtime_state() -> Result<Option<LegacyPanelConfig>, Box<dyn Error>> {
    let path = manifest_path(RUNTIME_STATE_REL);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path)?;
    Ok(Some(serde_json::from_slice(&bytes)?))
}

fn persist_runtime_state(config: &LegacyPanelConfig) -> Result<(), Box<dyn Error>> {
    let path = manifest_path(RUNTIME_STATE_REL);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(config)?)?;
    Ok(())
}

fn build_data_servers(
    config: &LegacyPanelConfig,
    downloaded_server_config: Option<&DownloadedServerConfig>,
) -> Vec<LegacyDataServer> {
    if let Some(downloaded_server_config) = downloaded_server_config {
        let mut data_servers = downloaded_server_config
            .active_servers
            .iter()
            .map(|entry| LegacyDataServer {
                value: entry.name.clone(),
                name: entry.name.clone(),
                display_name: format!(
                    "{} ({}, {}, {})",
                    entry.name, entry.host, entry.main_port, entry.secondary_port
                ),
                host: entry.host.clone(),
                port: entry.main_port,
                role: "股票备用".to_string(),
                source: downloaded_server_config.source_path.clone(),
            })
            .collect::<Vec<_>>();
        if !data_servers.is_empty() {
            data_servers.sort_by(|left, right| left.name.cmp(&right.name));
            return data_servers;
        }
    }

    let label = if config.selected_stock_backup_server.trim().is_empty() {
        "南京移动"
    } else {
        config.selected_stock_backup_server.trim()
    };

    vec![LegacyDataServer {
        value: label.to_string(),
        name: "股票备用".to_string(),
        display_name: format!(
            "配置项: {} / 回退: {}:{}",
            label, DEFAULT_DATA_HOST, DEFAULT_DATA_PORT
        ),
        host: DEFAULT_DATA_HOST.to_string(),
        port: DEFAULT_DATA_PORT,
        role: "股票备用".to_string(),
        source: "tdx7709-default".to_string(),
    }]
}

fn build_auth_login_request(config: &LegacyPanelConfig) -> String {
    format!(
        "股票数据?请求=登录&模块=认证&账号={}&密码={}&自动升级={}&版本={}&等待=3000&编号=0",
        config.account, config.password, config.auto_upgrade, AUTH_PROTOCOL_VERSION
    )
}

fn normalize_config(
    mut config: LegacyPanelConfig,
    auth_servers: &[LegacyAuthServer],
) -> LegacyPanelConfig {
    // The Rust path uses the real production account. A deployed secret may override only
    // the password; an old panel-state account must not silently select account 168/168.
    let credentials = auth_credentials::load(None, Some(&config.password));
    config.account = credentials.account;
    if let Some(password) = credentials.password {
        config.password = password;
    }
    config.selected_auth_server = normalize_auth_choice(&config.selected_auth_server, auth_servers);
    if config.selected_stock_backup_server.trim().is_empty() {
        config.selected_stock_backup_server = "南京移动".to_string();
    }
    if config.protocol_note.trim().is_empty() {
        config.protocol_note = PROTOCOL_NOTE.to_string();
    }
    config
}

fn normalize_auth_choice(choice: &str, auth_servers: &[LegacyAuthServer]) -> String {
    let trimmed = choice.trim();
    if trimmed.is_empty() || trimmed == "智能选择" {
        return "智能选择".to_string();
    }
    if let Some(server) = auth_servers.iter().find(|server| {
        server.value == trimmed || server.display_name == trimmed || server.name == trimmed
    }) {
        return server.value.clone();
    }
    "智能选择".to_string()
}

fn recommended_auth_server(config: &LegacyPanelConfig, probes: &[LegacyAuthServerProbe]) -> String {
    if config.selected_auth_server != "智能选择"
        && probes
            .iter()
            .any(|item| item.value == config.selected_auth_server)
    {
        return config.selected_auth_server.clone();
    }

    probes
        .iter()
        .filter(|item| item.probe_6100.reachable && item.login_7100.reachable)
        .min_by_key(|item| item.probe_6100.latency_ms.unwrap_or(u64::MAX))
        .or_else(|| probes.iter().find(|item| item.probe_6100.reachable))
        .or_else(|| probes.first())
        .map(|item| item.value.clone())
        .unwrap_or_else(|| "智能选择".to_string())
}

fn recommended_data_server(config: &LegacyPanelConfig, probes: &[LegacyDataServerProbe]) -> String {
    if !config.selected_stock_backup_server.trim().is_empty()
        && probes
            .iter()
            .any(|item| item.value == config.selected_stock_backup_server)
    {
        return config.selected_stock_backup_server.clone();
    }

    probes
        .iter()
        .find(|item| item.probe.reachable)
        .or_else(|| probes.first())
        .map(|item| item.value.clone())
        .unwrap_or_else(|| "南京移动".to_string())
}

fn build_probe_status(
    config: &LegacyPanelConfig,
    auth_probes: &[LegacyAuthServerProbe],
    data_probes: &[LegacyDataServerProbe],
    auth_choice: &str,
    data_choice: &str,
) -> String {
    let auth = auth_probes.iter().find(|item| item.value == auth_choice);
    let data = data_probes.iter().find(|item| item.value == data_choice);
    let mut parts = Vec::new();
    if let Some(auth) = auth {
        parts.push(format!(
            "认证 {}: 6100={} 7100={}",
            auth.display_name,
            probe_label(&auth.probe_6100),
            probe_label(&auth.login_7100)
        ));
    }
    if let Some(data) = data {
        parts.push(format!(
            "{} {}:{} = {}",
            data.display_name,
            data.host,
            data.port,
            probe_label(&data.probe)
        ));
    }
    if parts.is_empty() {
        return config.protocol_note.clone();
    }
    format!("{}。{}", parts.join("；"), config.protocol_note)
}

fn build_connect_status(
    auth_probe: &Option<LegacyTcpProbe>,
    auth_login_probe: &Option<LegacyTcpProbe>,
    stock_backup_probe: &Option<LegacyTcpProbe>,
    protocol_state: &str,
) -> String {
    let mut parts = Vec::new();
    if let Some(probe) = auth_probe {
        parts.push(format!("认证6100={}", probe_label(probe)));
    }
    if let Some(probe) = auth_login_probe {
        parts.push(format!("认证7100={}", probe_label(probe)));
    }
    if let Some(probe) = stock_backup_probe {
        parts.push(format!("股票备用7709={}", probe_label(probe)));
    }
    let head = if parts.is_empty() {
        "未完成连接试探".to_string()
    } else {
        parts.join("；")
    };
    match protocol_state {
        "reply_received" => format!("{head}；认证请求已收到回复。"),
        "transport_connected_no_reply" => {
            format!("{head}；认证端口已连通，但当前环境尚未拿到协议级回复。")
        }
        "login_port_reachable" => format!("{head}；认证登录端口可达。"),
        "login_port_unreachable" => format!("{head}；认证登录端口不可达。"),
        "transport_error" => format!("{head}；认证请求遇到传输错误。"),
        _ => head,
    }
}

fn probe_label(probe: &LegacyTcpProbe) -> String {
    if probe.reachable {
        format!("可达 {}ms", probe.latency_ms.unwrap_or(0))
    } else {
        probe.error.clone().unwrap_or_else(|| "不可达".to_string())
    }
}

fn tcp_probe(host: &str, port: u16, timeout_ms: u64) -> LegacyTcpProbe {
    let target = format!("{host}:{port}");
    let timeout = Duration::from_millis(timeout_ms);
    let start = Instant::now();

    let result = (|| -> Result<u64, Box<dyn Error>> {
        let addr = target
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| std::io::Error::other("failed to resolve target address"))?;
        let stream = TcpStream::connect_timeout(&addr, timeout)?;
        let elapsed = start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        let _ = stream.shutdown(std::net::Shutdown::Both);
        Ok(elapsed)
    })();

    match result {
        Ok(latency_ms) => LegacyTcpProbe {
            target,
            host: host.to_string(),
            port,
            reachable: true,
            latency_ms: Some(latency_ms),
            error: None,
        },
        Err(err) => LegacyTcpProbe {
            target,
            host: host.to_string(),
            port,
            reachable: false,
            latency_ms: None,
            error: Some(err.to_string()),
        },
    }
}

fn parse_key_values(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for raw_line in text.lines() {
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('[') || line.starts_with("//") {
            continue;
        }
        let (payload, _) = split_comment(line);
        let Some((key, value)) = payload.split_once('=') else {
            continue;
        };
        out.insert(key.trim().to_string(), value.trim().to_string());
    }
    out
}

fn read_utf16le_text(path: &Path) -> Result<String, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    if bytes.len() % 2 != 0 {
        return Err(format!("utf16 file has odd byte length: {}", path.display()).into());
    }
    let mut words = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    if words.first() == Some(&0xfeff) {
        words.remove(0);
    }
    Ok(String::from_utf16(&words)?)
}

fn split_comment(line: &str) -> (&str, Option<&str>) {
    for (index, _) in line.match_indices("//") {
        if index > 0
            && line[..index]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace)
        {
            return (line[..index].trim(), Some(line[index + 2..].trim()));
        }
    }
    (line.trim(), None)
}

fn split_name_port(name: &str) -> (String, Option<u16>) {
    let Some((base, suffix)) = name.rsplit_once('_') else {
        return (name.to_string(), None);
    };
    let Some(port) = suffix.parse::<u16>().ok() else {
        return (name.to_string(), None);
    };
    (base.to_string(), Some(port))
}

fn truthy(values: &BTreeMap<String, String>, key: &str) -> bool {
    values
        .get(key)
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "True"))
        .unwrap_or(false)
}

fn integer_of(values: &BTreeMap<String, String>, key: &str, fallback: u32) -> u32 {
    values
        .get(key)
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(fallback)
}

fn value_of(values: &BTreeMap<String, String>, key: &str, fallback: &str) -> String {
    values
        .get(key)
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

fn manifest_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

#[cfg(test)]
mod tests {
    use super::{load_reference_config, normalize_config, parse_key_values, split_comment, split_name_port};

    #[test]
    fn parses_comment_and_name_port() {
        assert_eq!(
            split_comment("杭州阿里云VIP7_6100, 39.108.103.69, 6100 //VIP7"),
            ("杭州阿里云VIP7_6100, 39.108.103.69, 6100", Some("VIP7"))
        );
        assert_eq!(
            split_name_port("杭州阿里云VIP7_6100"),
            ("杭州阿里云VIP7".to_string(), Some(6100))
        );
    }

    #[test]
    fn parses_reference_style_key_values() {
        let values = parse_key_values(
            "[数据接口]\n账号 = 168\n密码 = 168\n自动升级 = 稳定版 //注释\n登录股票备用 = 1\n",
        );
        assert_eq!(values.get("账号").map(String::as_str), Some("168"));
        assert_eq!(values.get("自动升级").map(String::as_str), Some("稳定版"));
        assert_eq!(values.get("登录股票备用").map(String::as_str), Some("1"));
    }

    #[test]
    fn normalizes_legacy_account_to_production_account_and_redacts_password() {
        let config = load_reference_config(&[]).expect("reference config");
        let normalized = normalize_config(config, &[]);
        assert_eq!(normalized.account, "1522");
        let serialized = serde_json::to_string(&normalized).expect("serialize panel config");
        assert!(!serialized.contains("\"password\":"));
        assert!(!serialized.contains("\"account\":\"168\""));
    }
}
