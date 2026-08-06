use netzipapi_rust_demo::analyze_auth_7100_flow_matrix;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: auth_7100_matrix <capture.pcap>")?;
    let matrix = analyze_auth_7100_flow_matrix(path)?;
    println!("{}", serde_json::to_string_pretty(&matrix)?);
    Ok(())
}
