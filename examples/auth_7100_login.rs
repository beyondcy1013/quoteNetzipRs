use netzipapi_rust_demo::auth_7100_client::{login_auth_7100, Auth7100ClientConfig};
use netzipapi_rust_demo::auth_credentials;
use std::error::Error;
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
        std::env::var("NETZIP_TDX_AUTH_HOST").unwrap_or_else(|_| "121.41.70.217".to_string());
    let port = std::env::var("NETZIP_TDX_AUTH_PORT")
        .ok()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(7100);

    let result = login_auth_7100(&Auth7100ClientConfig {
        host,
        port,
        account: credentials.account,
        password,
        timeout: Duration::from_secs(5),
    })?;
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
