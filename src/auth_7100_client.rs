use crate::auth_download::{DownloadedServerEntry, parse_server_entries_from_packet_bytes};
use serde::Serialize;
use std::error::Error;
use std::io::{Cursor, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::thread;
use std::time::Duration;

const MAX_PACKET_LEN: usize = 2 * 1024 * 1024;
pub const DEFAULT_AUTH_HOST: &str = "121.41.70.217";
pub const DEFAULT_PROBE_PORTS: [u16; 2] = [6100, 7100];
pub const DEFAULT_LOGIN_PORT: u16 = 6100;
pub const STOCK_DICTIONARY_SHA256: &str =
    "8f44f49cbf10c8d203d9cabbda256da37c1f7d43e7f99b60c08d077c47b2bc68";
const STOCK_DICTIONARY_BYTES: &[u8] =
    include_bytes!("../netzip_api_bin/NetzipAPI/StockC++/Stock.字典");
const INNER_PREFIX_HEX: &str = "a48bc18b0000000000000000000000000000000013000000000000000000000052030000520300000200000004000000000000000000000020000000f78b426c00007b76555f0000";
const INNER_SUFFIX_HEX: &str = "0400000006000000000000000000000026000000065290676f8ff64e0000ea819a5b494e0000050000000800000000000000000000002a000000d08f258446550d54f079000058005800fb79a8520000020000000a000000000000000000000026000000216a57570000a1806879a25b3762ef7a00000c00000004000000000000000200000034000000530074006f0063006b002e006500780065005f00e5651f670000b3f9030000000c00000004000000000000000200000034000000530074006f0063006b002e006500780065005f0048722c6700006a01000000000400000004000000000000000200000024000000cd645c4ffb7cdf7e0000e803000000000400000004000000000000000300000024000000a25b37620768c68b0000609350160000070000000400000000000000030000002a000000517f4596ce982e006500780065000000ec503f630000090000000400000000000000030000002e000000530074006f0063006b002e0064006c006c000000b622072a0000080000000400000000000000030000002c000000530074006f0063006b002e00575b78510000666148240000080000000400000000000000030000002c0000004753a77e4d916e7f2e0069006e0069000000330feb9100000c00000004000000000000000300000034000000287537625c000d67a1526856175268882e0069006e006900000099a51cb500000f0000000400000000000000030000003a000000fb7cdf7e5c00530074006f0063006b006400720076002e0064006c006c000000533e358b00000c00000004000000000000000300000034000000fb7cdf7e5c00496c575b807bfc6268882e007400780074000000cbd1cc8200000400000006000000000000000000000026000000ea81a8524753a77e0000337a9a5b4872000002000000040000000000000002000000200000001a9053900000000000000000";
const OUTER_PREFIX_HEX: &str = "517fdc7e055300000000000000000000000000000400000000000000000000006d0200006d02000002000000080000000000000000000000240000008b53297f00005a00530054004400000002000000040000000000000002000000200000007f95a65e0000c3010000000003000000040000000000000002000000220000009f537f95a65e000052030000000002000000c30100000000000009000000df01000070656e630000";
const PROBE_INNER_PREFIX_HEX: &str = "a48bc18b000000000000000000000000000000000a0000000000000000000000b2010000b20100000200000004000000000000000000000020000000f78b426c00004b6d1f900000";
const PROBE_INNER_SUFFIX_HEX: &str = "0400000006000000000000000000000026000000065290676f8ff64e0000ea819a5b494e0000050000000800000000000000000000002a000000d08f258446550d54f0790000c96cde5dfb79a8520000020000000a000000000000000000000026000000216a57570000a1806879a25b3762ef7a00000c00000004000000000000000200000034000000530074006f0063006b002e006500780065005f00e5651f6700005dd5030000000c00000004000000000000000200000034000000530074006f0063006b002e006500780065005f0048722c6700006901000000000400000004000000000000000200000024000000cd645c4ffb7cdf7e0000e803000000000400000004000000000000000300000024000000a25b37620768c68b0000eaadeaac0000";
const PROBE_OUTER_PREFIX_HEX: &str = "517fdc7e05530000000000000000000000000000040000000000000000000000a3010000a301000002000000080000000000000000000000240000008b53297f00005a00530054004400000002000000040000000000000002000000200000007f95a65e0000f9000000000003000000040000000000000002000000220000009f537f95a65e0000b2010000000002000000f900000000000000090000001501000070656e630000";
const FOLLOWUP_DOWNLOAD_HEX: &str = "517fdc7e055300000000000000000000000000000400000000000000000000000f0100000f010000020000000c0000000000000000000000280000008b53297f00005a00530054004400575b7851000002000000040000000000000002000000200000007f95a65e000061000000000003000000040000000000000002000000220000009f537f95a65e0000820000000000020000006100000000000000090000007d00000070656e63000028b52ffd2082c50200e4030b4e7d8f8765f64e000200821e3a00000000fb7cdf7e5c001a90be8fe14fa18068792e0069006e0069040300000020000000167ff75300000100000000000a00208baed9285913628b73600d24dfb4b6cb0cec6600390000";
const FOLLOWUP_FINISH_HEX: &str = "517fdc7e055300000000000000000000000000000400000000000000000000004d0100004d010000020000000c0000000000000000000000280000008b53297f00005a00530054004400575b7851000002000000040000000000000002000000200000007f95a65e00009f000000000003000000040000000000000002000000220000009f537f95a65e0000ae0100000000020000009f0000000000000009000000bb00000070656e63000028b52ffd60ae00ad040052061722904d73b79f525cd366f054ffbf12df56d8766a58c6f8261fee2dcb64c91c211022538df171902c4209347827941969441bcff428ea4d9385ab1456a26ab8cbb963998472ab5e688723042d29312bb1d92be465e4e3df86031a6760160084139e9908e61e90149f9d6d020e24d5586e808118438d4e19d6336187822a1c00e44b5533628bfb7296b71b0dec8633b29a10400000";

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

/// Sends the same credential-bearing `认证|测速` packet to one authentication candidate.
pub fn probe_auth_server(
    config: &Auth7100ClientConfig,
) -> Result<Auth7100ProbeResult, Box<dyn Error>> {
    if config.account.trim().is_empty() || config.password.is_empty() {
        return Err("authentication credentials are incomplete".into());
    }

    let endpoint = format!("{}:{}", config.host, config.port);
    let socket = endpoint
        .to_socket_addrs()?
        .next()
        .ok_or("failed to resolve 7100 endpoint")?;
    let mut stream = TcpStream::connect_timeout(&socket, config.timeout)?;
    stream.set_read_timeout(Some(config.timeout))?;
    stream.set_write_timeout(Some(config.timeout))?;

    let probe = build_auth_7100_probe_packet(&config.account, &config.password)?;
    stream.write_all(&probe)?;
    stream.flush()?;
    let response = read_netpacket(&mut stream)?;
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
    if config.account.trim().is_empty() || config.password.is_empty() {
        return Err("authentication credentials are incomplete".into());
    }

    let dictionary_sha256 = dictionary.map(validate_stock_dictionary).transpose()?;
    let endpoint = format!("{}:{}", config.host, config.port);
    let socket = endpoint
        .to_socket_addrs()?
        .next()
        .ok_or("failed to resolve authentication endpoint")?;
    let mut stream = TcpStream::connect_timeout(&socket, config.timeout)?;
    stream.set_read_timeout(Some(config.timeout))?;
    stream.set_write_timeout(Some(config.timeout))?;

    let login = build_auth_7100_login_packet(&config.account, &config.password)?;
    stream.write_all(&login)?;
    stream.flush()?;
    let first = read_netpacket(&mut stream)?;

    thread::sleep(Duration::from_millis(250));
    stream.write_all(&decode_hex(FOLLOWUP_DOWNLOAD_HEX)?)?;
    stream.flush()?;
    let second = read_netpacket(&mut stream)?;

    thread::sleep(Duration::from_millis(100));
    stream.write_all(&decode_hex(FOLLOWUP_FINISH_HEX)?)?;
    stream.flush()?;
    let third = read_netpacket(&mut stream)?;

    let packets = [&first, &second, &third];
    let roles = packets
        .iter()
        .map(|packet| classify_response(packet))
        .collect::<Result<Vec<_>, _>>()?;
    let authenticated = roles == ["zstd_dictionary", "download_file", "zstd_dictionary"];
    if !authenticated {
        return Err(format!("unexpected authentication login response sequence: {roles:?}").into());
    }

    let (decoded_response_lengths, login_success_confirmed, quote_servers, selected_quote_endpoint) =
        if let Some(dictionary) = dictionary {
            let first_decoded = decode_dictionary_response(&first, dictionary)?;
            let third_decoded = decode_dictionary_response(&third, dictionary)?;
            let quote_servers = parse_server_entries_from_packet_bytes(&second)?;
            let selected_quote_endpoint = quote_servers.iter().find_map(|entry| {
                if entry.main_port == 5188 {
                    Some(format!("{}:{}", entry.host, entry.main_port))
                } else if entry.secondary_port == 5188 {
                    Some(format!("{}:{}", entry.host, entry.secondary_port))
                } else {
                    None
                }
            });
            let login_success_confirmed = contains_utf16(&first_decoded, "登录成功")
                || contains_utf16(&third_decoded, "登录成功");
            if !login_success_confirmed {
                return Err("decoded authentication response did not contain 登录成功".into());
            }
            if selected_quote_endpoint.is_none() {
                return Err("decoded download response contained no 5188 quote endpoint".into());
            }
            (
                vec![first_decoded.len(), third_decoded.len()],
                login_success_confirmed,
                quote_servers,
                selected_quote_endpoint,
            )
        } else {
            (Vec::new(), false, Vec::new(), None)
        };

    Ok(Auth7100LoginResult {
        endpoint,
        account: config.account.clone(),
        status: "登录成功".to_string(),
        authenticated,
        response_packet_lengths: packets.iter().map(|packet| packet.len()).collect(),
        response_roles: roles.into_iter().map(str::to_string).collect(),
        dictionary_length: dictionary.map(|bytes| bytes.len()),
        dictionary_sha256,
        decoded_response_lengths,
        login_success_confirmed,
        quote_servers,
        selected_quote_endpoint,
    })
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

fn contains_utf16(bytes: &[u8], value: &str) -> bool {
    let needle = value
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    bytes.windows(needle.len()).any(|window| window == needle)
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
    let mut header = vec![0u8; 74];
    stream.read_exact(&mut header)?;
    let packet_len = u32::from_le_bytes(header[32..36].try_into()?) as usize;
    if !(74..=MAX_PACKET_LEN).contains(&packet_len) {
        return Err(format!("invalid 7100 packet length: {packet_len}").into());
    }
    let mut packet = header;
    packet.resize(packet_len, 0);
    stream.read_exact(&mut packet[74..])?;
    Ok(packet)
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
    if field20 == 4 && field44 == 8 && tail.contains("ZSTD") {
        return Ok("auth_probe_response");
    }
    if field20 == 4 && field44 == 12 && tail.contains("ZSTD") {
        return Ok("zstd_dictionary");
    }
    Err(format!("unrecognized 7100 response: type={field20} payload={field44} tail={tail}").into())
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
        build_auth_7100_login_packet, build_auth_7100_probe_packet, classify_response,
        decode_dictionary_response, decode_hex,
    };
    use std::io::{Cursor, Write};

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
}
