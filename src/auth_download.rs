use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::Path;

const NET_PACKET_PREFIX_LEN: usize = 74;
const SAMPLE_SERVER_FLOW_REL: &str = "tmp/flow_7100_2697_server.bin";
const DOWNLOAD_TOP_LEVEL_LABELS: [&str; 9] = [
    "请求",
    "来源",
    "应答编号",
    "名称",
    "长度",
    "原长度",
    "压缩",
    "数据",
    "扩展",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadedServerEntry {
    pub group_name: Option<String>,
    pub name: String,
    pub host: String,
    pub main_port: u16,
    pub secondary_port: u16,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadedServerConfig {
    pub source_path: String,
    pub packet_offset: usize,
    pub packet_len: usize,
    pub field_52_hint: Option<u32>,
    pub top_level_child_labels: Vec<String>,
    pub top_level_child_count: usize,
    pub top_level_child_count_matches_field_52: Option<bool>,
    pub request_name: Option<String>,
    pub response_name: Option<String>,
    pub source_name: Option<String>,
    pub account: Option<String>,
    pub password: Option<String>,
    pub main_port: Option<u16>,
    pub secondary_port: Option<u16>,
    pub client_id: Option<u32>,
    pub version: Option<u32>,
    pub f10_serial: Option<u32>,
    pub markets: Vec<String>,
    pub active_servers: Vec<DownloadedServerEntry>,
    pub disabled_servers: Vec<DownloadedServerEntry>,
    pub raw_text: String,
}

#[derive(Clone, Debug)]
struct NetPacketPrefix {
    object_name: String,
    packet_len: u32,
    field_52: u32,
    tail: String,
}

pub fn load_downloaded_server_config_sample() -> Result<DownloadedServerConfig, Box<dyn Error>> {
    let path = crate::repository_fixture_path(SAMPLE_SERVER_FLOW_REL);
    parse_downloaded_server_config_from_path(path)
}

pub fn parse_downloaded_server_config_from_path(
    path: impl AsRef<Path>,
) -> Result<DownloadedServerConfig, Box<dyn Error>> {
    let path = path.as_ref();
    let bytes = fs::read(path)?;
    let (prefix, packet_offset, packet_bytes) = locate_download_packet(&bytes)
        .ok_or_else(|| format!("no 下载文件 packet found in {}", path.display()))?;
    let raw_utf16_strings = scan_utf16_strings(packet_bytes, 2);
    let top_level_child_labels = extract_download_top_level_child_labels(&raw_utf16_strings);
    let request_name = value_after(&raw_utf16_strings, "请求");
    let response_name = value_after(&raw_utf16_strings, "名称");
    let source_name = value_after(&raw_utf16_strings, "来源");
    let raw_text = extract_embedded_utf16_text(packet_bytes)
        .ok_or_else(|| format!("no embedded UTF-16 config text found in {}", path.display()))?;
    let parsed = parse_server_config_text(&raw_text);

    Ok(DownloadedServerConfig {
        source_path: path.display().to_string(),
        packet_offset,
        packet_len: packet_bytes.len(),
        field_52_hint: Some(prefix.field_52),
        top_level_child_count: top_level_child_labels.len(),
        top_level_child_count_matches_field_52: Some(
            top_level_child_labels.len() as u32 == prefix.field_52,
        ),
        top_level_child_labels,
        request_name,
        response_name,
        source_name,
        account: parsed.account,
        password: parsed.password,
        main_port: parsed.main_port,
        secondary_port: parsed.secondary_port,
        client_id: parsed.client_id,
        version: parsed.version,
        f10_serial: parsed.f10_serial,
        markets: parsed.markets,
        active_servers: parsed.active_servers,
        disabled_servers: parsed.disabled_servers,
        raw_text,
    })
}

#[derive(Default)]
struct ParsedConfigText {
    account: Option<String>,
    password: Option<String>,
    main_port: Option<u16>,
    secondary_port: Option<u16>,
    client_id: Option<u32>,
    version: Option<u32>,
    f10_serial: Option<u32>,
    markets: Vec<String>,
    active_servers: Vec<DownloadedServerEntry>,
    disabled_servers: Vec<DownloadedServerEntry>,
}

fn parse_server_config_text(text: &str) -> ParsedConfigText {
    let mut out = ParsedConfigText::default();
    let mut current_group = None::<String>;

    for raw_line in text.lines() {
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || (line.starts_with('[') && line.ends_with(']')) {
            continue;
        }
        if let Some(group_name) = line.strip_prefix("//") {
            let group_name = group_name.trim();
            if let Some(entry) = parse_server_entry(line, current_group.clone(), false) {
                out.disabled_servers.push(entry);
                continue;
            }
            if group_name.contains(',') {
                continue;
            }
            if !group_name.is_empty() {
                current_group = Some(group_name.to_string());
            }
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "账号" => out.account = Some(value.to_string()),
                "密码" => out.password = Some(value.to_string()),
                "主端口" => out.main_port = value.parse::<u16>().ok(),
                "次端口" => out.secondary_port = value.parse::<u16>().ok(),
                "客户ID" => out.client_id = value.parse::<u32>().ok(),
                "版本" => out.version = value.parse::<u32>().ok(),
                "F10序号" => out.f10_serial = value.parse::<u32>().ok(),
                "市场" => {
                    out.markets = value
                        .split(';')
                        .map(str::trim)
                        .filter(|item| !item.is_empty())
                        .map(str::to_string)
                        .collect();
                }
                _ => {}
            }
            continue;
        }

        if let Some(entry) = parse_server_entry(line, current_group.clone(), true) {
            out.active_servers.push(entry);
            continue;
        }

        if let Some(entry) = parse_server_entry(line, current_group.clone(), false) {
            out.disabled_servers.push(entry);
        }
    }

    out
}

fn parse_server_entry(
    line: &str,
    group_name: Option<String>,
    enabled: bool,
) -> Option<DownloadedServerEntry> {
    let line = if enabled {
        line
    } else {
        line.strip_prefix("//")?.trim()
    };
    let parts = line
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    if parts.len() != 4 {
        return None;
    }

    Some(DownloadedServerEntry {
        group_name,
        name: parts[0].to_string(),
        host: parts[1].to_string(),
        main_port: parts[2].parse::<u16>().ok()?,
        secondary_port: parts[3].parse::<u16>().ok()?,
        enabled,
    })
}

fn locate_download_packet(bytes: &[u8]) -> Option<(NetPacketPrefix, usize, &[u8])> {
    let mut offset = 0usize;
    while offset + NET_PACKET_PREFIX_LEN <= bytes.len() {
        let Some(prefix) = parse_netpacket_prefix(&bytes[offset..offset + NET_PACKET_PREFIX_LEN])
        else {
            offset += 1;
            continue;
        };
        let packet_len = prefix.packet_len as usize;
        if packet_len < NET_PACKET_PREFIX_LEN || offset + packet_len > bytes.len() {
            offset += 1;
            continue;
        }

        let packet_bytes = &bytes[offset..offset + packet_len];
        if prefix.object_name == "网络包"
            && prefix.tail.contains("下载文件")
            && packet_bytes
                .windows(4)
                .any(|window| window == [0xff, 0xfe, 0x5b, 0x00])
        {
            return Some((prefix, offset, packet_bytes));
        }

        offset += packet_len;
    }
    None
}

fn parse_netpacket_prefix(bytes: &[u8]) -> Option<NetPacketPrefix> {
    if bytes.len() < NET_PACKET_PREFIX_LEN {
        return None;
    }
    let object_name = decode_utf16_z(&bytes[..20])?;
    let packet_len = u32::from_le_bytes(bytes[32..36].try_into().ok()?);
    let field_52 = u32::from_le_bytes(bytes[52..56].try_into().ok()?);
    let tail = decode_utf16_tail(&bytes[60..74]);
    Some(NetPacketPrefix {
        object_name,
        packet_len,
        field_52,
        tail,
    })
}

fn extract_download_top_level_child_labels(values: &[String]) -> Vec<String> {
    DOWNLOAD_TOP_LEVEL_LABELS
        .iter()
        .filter(|label| values.iter().any(|value| value == **label))
        .map(|label| (*label).to_string())
        .collect()
}

fn extract_embedded_utf16_text(bytes: &[u8]) -> Option<String> {
    let start = bytes
        .windows(4)
        .position(|window| window == [0xff, 0xfe, 0x5b, 0x00])?;
    let remainder = &bytes[start..];
    if remainder.len() < 4 {
        return None;
    }

    let mut words = remainder[2..]
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    while matches!(words.last(), Some(0)) {
        words.pop();
    }
    let decoded = String::from_utf16_lossy(&words);

    let mut lines = Vec::new();
    let mut started = false;
    for raw_line in decoded.lines() {
        let line = raw_line.trim_end_matches('\u{0}');
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if started {
                lines.push(String::new());
            }
            continue;
        }
        let valid = is_config_line(trimmed);
        if valid {
            started = true;
            lines.push(trimmed.to_string());
            continue;
        }
        if started {
            break;
        }
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn is_config_line(line: &str) -> bool {
    if (line.starts_with('[') && line.ends_with(']')) || line.starts_with("//") {
        return true;
    }
    if line.contains('=') || line.matches(',').count() >= 3 {
        return true;
    }
    false
}

fn scan_utf16_strings(bytes: &[u8], min_chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut words = Vec::<u16>::new();
    for chunk in bytes.chunks_exact(2) {
        let word = u16::from_le_bytes([chunk[0], chunk[1]]);
        if is_visible_utf16(word) {
            words.push(word);
            continue;
        }

        if words.len() >= min_chars {
            let text = String::from_utf16_lossy(&words).trim().to_string();
            if !text.is_empty() {
                out.push(text);
            }
        }
        words.clear();
    }
    if words.len() >= min_chars {
        let text = String::from_utf16_lossy(&words).trim().to_string();
        if !text.is_empty() {
            out.push(text);
        }
    }
    out
}

fn is_visible_utf16(word: u16) -> bool {
    matches!(word, 0x0009 | 0x000a | 0x000d | 0x0020..=0x007e) || word >= 0x4e00
}

fn value_after(values: &[String], key: &str) -> Option<String> {
    values
        .iter()
        .position(|value| value == key)
        .and_then(|index| values.get(index + 1))
        .cloned()
}

fn decode_utf16_z(bytes: &[u8]) -> Option<String> {
    let mut words = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        let word = u16::from_le_bytes([chunk[0], chunk[1]]);
        if word == 0 {
            break;
        }
        words.push(word);
    }
    if words.is_empty() {
        return None;
    }
    let text = String::from_utf16_lossy(&words);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn decode_utf16_tail(bytes: &[u8]) -> String {
    let mut words = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        let word = u16::from_le_bytes([chunk[0], chunk[1]]);
        if word != 0 {
            words.push(word);
        }
    }
    String::from_utf16_lossy(&words)
}

#[cfg(test)]
mod tests {
    use super::{is_config_line, load_downloaded_server_config_sample, parse_server_config_text};

    #[test]
    fn parses_downloaded_server_text() {
        let text = "[通达信服务器]\n\n账号   = NetCardMac\n密码   = l123321\n主端口 = 7709\n次端口 = 7712\n客户ID = 1031\n版本   = 1\nF10序号 = 0\n市场   = SH;SZ\n\n//华泰\n南京移动, 120.195.71.160, 7709, 7709\n//通达信官方\n//上海腾讯云1, 122.51.120.217, 7709, 7709\n";
        let parsed = parse_server_config_text(text);
        assert_eq!(parsed.account.as_deref(), Some("NetCardMac"));
        assert_eq!(parsed.password.as_deref(), Some("l123321"));
        assert_eq!(parsed.main_port, Some(7709));
        assert_eq!(parsed.secondary_port, Some(7712));
        assert_eq!(parsed.client_id, Some(1031));
        assert_eq!(parsed.markets, vec!["SH".to_string(), "SZ".to_string()]);
        assert_eq!(parsed.active_servers.len(), 1);
        assert_eq!(parsed.disabled_servers.len(), 1);
        assert_eq!(parsed.active_servers[0].name, "南京移动");
        assert_eq!(parsed.disabled_servers[0].name, "上海腾讯云1");
    }

    #[test]
    fn recognizes_config_lines() {
        assert!(is_config_line("[通达信服务器]"));
        assert!(is_config_line("//华泰"));
        assert!(is_config_line("账号 = NetCardMac"));
        assert!(is_config_line("南京移动, 120.195.71.160, 7709, 7709"));
        assert!(!is_config_line("garbled binary"));
    }

    #[test]
    fn sample_download_packet_top_level_child_count_matches_field_52() {
        let config = load_downloaded_server_config_sample().expect("download config");
        assert_eq!(config.field_52_hint, Some(9));
        assert_eq!(config.top_level_child_count, 9);
        assert_eq!(
            config.top_level_child_labels,
            vec![
                "请求".to_string(),
                "来源".to_string(),
                "应答编号".to_string(),
                "名称".to_string(),
                "长度".to_string(),
                "原长度".to_string(),
                "压缩".to_string(),
                "数据".to_string(),
                "扩展".to_string(),
            ]
        );
        assert_eq!(config.top_level_child_count_matches_field_52, Some(true));
    }
}
