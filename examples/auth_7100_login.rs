use netzipapi_rust_demo::auth_7100_client::{
    Auth7100ClientConfig, Auth7100LoginResult, Auth7100ProbeResult, DEFAULT_AUTH_HOST,
    DEFAULT_LOGIN_PORT, DEFAULT_PROBE_PORTS, login_auth_6100, probe_auth_server,
};
use netzipapi_rust_demo::auth_credentials;
use serde::Serialize;
use std::error::Error;
use std::fs;
use std::time::Duration;

fn main() {
    if let Err(err) = run() {
        eprintln!("7100 authentication failed: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let credentials = auth_credentials::load(None, None);
    let password = credentials
        .password
        .ok_or("NETZIP_TDX_PASSWORD is required")?;
    let host =
        std::env::var("NETZIP_TDX_AUTH_HOST").unwrap_or_else(|_| DEFAULT_AUTH_HOST.to_string());
    let login_port = std::env::var("NETZIP_TDX_LOGIN_PORT")
        .ok()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(DEFAULT_LOGIN_PORT);
    let probe_ports = std::env::var("NETZIP_TDX_PROBE_PORTS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .filter(|item| !item.trim().is_empty())
                .map(|item| item.trim().parse::<u16>())
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_else(|| DEFAULT_PROBE_PORTS.to_vec());
    let dictionary_path = std::env::var("NETZIP_TDX_DICTIONARY_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from("docs/netzip_api_bin/NetzipAPI/StockC++/Stock.字典")
        });
    let dictionary = fs::read(dictionary_path)?;

    let mut probes = Vec::new();
    for port in probe_ports {
        let config = Auth7100ClientConfig {
            host: host.clone(),
            port,
            account: credentials.account.clone(),
            password: password.clone(),
            timeout: Duration::from_secs(5),
        };
        probes.push(match probe_auth_server(&config) {
            Ok(result) => ProbeAttempt {
                endpoint: result.endpoint.clone(),
                result: Some(result),
                error: None,
            },
            Err(error) => ProbeAttempt {
                endpoint: format!("{host}:{port}"),
                result: None,
                error: Some(error.to_string()),
            },
        });
    }

    let login = login_auth_6100(
        &Auth7100ClientConfig {
            host,
            port: login_port,
            account: credentials.account,
            password,
            timeout: Duration::from_secs(5),
        },
        &dictionary,
    )?;
    println!(
        "{}",
        serde_json::to_string(&AuthRunResult { probes, login })?
    );
    Ok(())
}

#[derive(Serialize)]
struct ProbeAttempt {
    endpoint: String,
    result: Option<Auth7100ProbeResult>,
    error: Option<String>,
}

#[derive(Serialize)]
struct AuthRunResult {
    probes: Vec<ProbeAttempt>,
    login: Auth7100LoginResult,
}
