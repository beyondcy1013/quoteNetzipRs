use netzipapi_rust_demo::analyze_auth_7100_flow_matrix;
use std::error::Error;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: auth_7100_flow_matrix <capture.pcap|capture.pcapng>")?;
    let matrix = analyze_auth_7100_flow_matrix(path)?;
    serde_json::to_writer_pretty(std::io::stdout().lock(), &matrix)?;
    println!();
    Ok(())
}
