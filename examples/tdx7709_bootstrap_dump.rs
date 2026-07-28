use netzipapi_rust_demo::{build_bootstrap_packets, build_probe_hello};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let probe = build_probe_hello()?;
    println!("probe.hello len={} hex={}", probe.len(), hex_line(&probe));

    for packet in build_bootstrap_packets()? {
        println!(
            "{} op=0x{:04x} sub=0x{:04x} flags=0x{:04x} body_len={} request_len={}",
            packet.label,
            packet.op,
            packet.sub,
            packet.flags,
            packet.body_len,
            packet.request.len()
        );
        println!("  request={}", hex_line(&packet.request));
    }

    Ok(())
}

fn hex_line(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}
