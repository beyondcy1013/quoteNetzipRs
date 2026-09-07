use netzip_fullpull::{Official5188InitializationStage, official_5188_ack_source_prefix};
use netzipapi_rust_demo::auth_7100_client::{
    Auth7100AbkControlFields, Auth7100AckControlFields, Auth7100ClientConfig,
    Auth7100LoginControlFields, DEFAULT_AUTH_HOST, DEFAULT_LOGIN_PORT, build_ack_manifest_text,
    build_candidate_official_5188_abk_control_packet,
    build_candidate_official_5188_ack_control_packet,
    build_candidate_official_5188_login_control_packet,
    connect_auth_control_with_verified_dictionary,
};
use netzipapi_rust_demo::auth_credentials;
use std::error::Error;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn utf16_bytes(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn local_ipv4(host: &str, port: u16) -> [u8; 4] {
    UdpSocket::bind("0.0.0.0:0")
        .ok()
        .and_then(|socket| {
            socket.connect((host, port)).ok()?;
            socket.local_addr().ok()
        })
        .and_then(|address| match address.ip() {
            std::net::IpAddr::V4(address) => Some(address.octets()),
            std::net::IpAddr::V6(address) => {
                address.to_ipv4_mapped().map(|address| address.octets())
            }
        })
        .unwrap_or([0, 0, 0, 0])
}

fn file_inventory(root: &Path) -> Result<Vec<(String, u32)>, Box<dyn Error>> {
    let mut rows = Vec::new();
    let mut push = |relative: String, path: &Path| -> Result<(), Box<dyn Error>> {
        if path.is_file() {
            let bytes = std::fs::read(path)?;
            rows.push((format!(".//{relative}"), crc32fast::hash(&bytes)));
        }
        Ok(())
    };
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_file() {
            push(name, &entry.path())?;
        }
    }
    let vendor_dir = root.join("大智慧");
    if vendor_dir.is_dir() {
        for entry in std::fs::read_dir(vendor_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_file() {
                push(format!("大智慧//{name}"), &entry.path())?;
            }
        }
    }
    rows.truncate(40);
    Ok(rows)
}

fn main() {
    if let Err(error) = run() {
        eprintln!("PHASE_S FAILED {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let credentials = auth_credentials::load(None, None);
    if credentials.password.is_none() {
        return Err("formal-account password is not configured".into());
    }
    let host =
        std::env::var("NETZIP_TDX_AUTH_HOST").unwrap_or_else(|_| DEFAULT_AUTH_HOST.to_string());
    let login_port = std::env::var("NETZIP_TDX_LOGIN_PORT")
        .ok()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(DEFAULT_LOGIN_PORT);
    let config = Auth7100ClientConfig {
        host: host.clone(),
        port: login_port,
        account: credentials.account,
        password: credentials.password.unwrap_or_default(),
        timeout: Duration::from_secs(8),
    };

    let started = Instant::now();
    let mut control = connect_auth_control_with_verified_dictionary(&config)?;
    let login = control.login_result().clone();
    let route_count_5188 = login
        .quote_servers
        .iter()
        .filter(|entry| entry.main_port == 5188 || entry.secondary_port == 5188)
        .count();
    println!(
        "LOGIN PASS roles={:?} packets={:?} routes={} routes_5188={}",
        login.response_roles,
        login.response_packet_lengths,
        login.quote_servers.len(),
        route_count_5188
    );
    let endpoint = login
        .selected_quote_endpoint
        .clone()
        .ok_or("L1 response did not select a 5188 endpoint")?;
    let mut data = control.connect_selected_official_5188(Duration::from_secs(8))?;
    println!("CONNECT PASS endpoint_port=5188 endpoint={endpoint}");

    let mut permissions = [0u8; 6];
    permissions.copy_from_slice(&utf16_bytes(
        &std::env::var("NETZIP_PHASE_S_PERMISSION").unwrap_or_else(|_| "点播版".to_string()),
    ));
    let mut broker = [0u8; 4];
    broker.copy_from_slice(&utf16_bytes(
        &std::env::var("NETZIP_PHASE_S_BROKER").unwrap_or_else(|_| "华创".to_string()),
    ));
    let login_fields = Auth7100LoginControlFields {
        local_ip: local_ipv4(&host, login_port),
        mac_ascii: [0; 12],
        account_permissions: permissions,
        broker,
        encryption_version: 0,
        user_id: 0,
        interface_version: 858,
    };
    let login_request = build_candidate_official_5188_login_control_packet(&login_fields, 2)?;
    let login_frame = control.exchange_official_5188_login_init(&login_request)?;
    let login_response =
        data.exchange_initialization_stage(Official5188InitializationStage::Login, &login_frame)?;
    println!(
        "STAGE1 PASS client=3610:{}B server=0x{:04x}:{}B",
        login_frame.payload.len(),
        login_response.kind.0,
        login_response.payload.len()
    );

    let abk_fields = Auth7100AbkControlFields {
        is_64_bit: 0x20,
        major_version: 8,
        minor_version: 0x32,
        build_number: 0x5956,
        total_memory: [0x00, 0x40, 0x6a, 0xfb, 0x0f, 0x00, 0x00, 0x00],
        user_id: 0,
        interface_version: 858,
    };
    let abk_request = build_candidate_official_5188_abk_control_packet(&abk_fields, 12)?;
    let abk_frame = control.exchange_official_5188_abk_init(&abk_request)?;
    let abk_response =
        data.exchange_initialization_stage(Official5188InitializationStage::Abk, &abk_frame)?;
    let first_nul = abk_response.payload.iter().position(|byte| *byte == 0);
    println!(
        "STAGE2 PASS client=3610:{}B server=0x{:04x}:{}B first_nul={first_nul:?}",
        abk_frame.payload.len(),
        abk_response.kind.0,
        abk_response.payload.len()
    );
    let _ack_source = official_5188_ack_source_prefix(&abk_response)?;
    let root = std::env::var("NETZIP_PHASE_S_FILE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../quoteNetzipWine"));
    let files = file_inventory(&root)?;
    let manifest = build_ack_manifest_text(&files, &[], &[]);
    let ack_fields = Auth7100AckControlFields {
        ack: u32::from_le_bytes(*b"penc"),
        user_id: 0,
        interface_version: 858,
        data: manifest.into_bytes(),
    };
    println!(
        "ACK_MANIFEST INFO files={} bytes={}",
        files.len(),
        ack_fields.data.len()
    );
    let ack_request = build_candidate_official_5188_ack_control_packet(&ack_fields, 14)?;
    let ack_frame = control.exchange_official_5188_ack_init(&ack_request)?;
    let ack_response =
        data.exchange_initialization_stage(Official5188InitializationStage::Ack, &ack_frame)?;
    println!(
        "STAGE3 PASS client=3610:{}B server=0x{:04x}:{}B",
        ack_frame.payload.len(),
        ack_response.kind.0,
        ack_response.payload.len()
    );

    for index in 0..8 {
        match data.read_frames() {
            Ok(frames) => {
                for frame in frames {
                    println!(
                        "POST_FRAME {index} kind=0x{:04x} wire={} payload={}B",
                        frame.kind.0,
                        frame.kind.wire_hex(),
                        frame.payload.len()
                    );
                }
            }
            Err(error) => {
                println!("POST_READ {index} ERROR {error}");
                break;
            }
        }
    }
    println!("DONE elapsed_ms={}", started.elapsed().as_millis());
    Ok(())
}
