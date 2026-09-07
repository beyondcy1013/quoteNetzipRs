use std::env;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const PCAPNG_MAGIC: [u8; 4] = [0x0a, 0x0d, 0x0d, 0x0a];
const CLASSIC_PCAP_MAGICS: [[u8; 4]; 4] = [
    [0xd4, 0xc3, 0xb2, 0xa1],
    [0xa1, 0xb2, 0xc3, 0xd4],
    [0x4d, 0x3c, 0xb2, 0xa1],
    [0xa1, 0xb2, 0x3c, 0x4d],
];

pub fn read_capture_file_as_pcap_bytes(path: impl AsRef<Path>) -> Result<Vec<u8>, Box<dyn Error>> {
    let path = path.as_ref();
    let bytes = fs::read(path)?;
    if bytes.starts_with(&PCAPNG_MAGIC) {
        return convert_pcapng_to_pcap_bytes(path);
    }
    Ok(bytes)
}

fn convert_pcapng_to_pcap_bytes(path: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("clock before unix epoch: {err}"))?
        .as_nanos();
    let temp_path = env::temp_dir().join(format!(
        "netzipapi-rust-demo-{}-{stamp}.pcap",
        std::process::id()
    ));

    let output = Command::new("tcpdump")
        .arg("-r")
        .arg(path)
        .arg("-w")
        .arg(&temp_path)
        .output()?;
    let converted = fs::read(&temp_path).unwrap_or_default();
    let _ = fs::remove_file(&temp_path);
    if !output.status.success() {
        // pktmon may leave one incomplete terminal pcapng block when capture is
        // stopped. tcpdump still emits every preceding complete packet. Keep
        // that bounded prefix only when it is independently a valid pcap file.
        if is_classic_pcap(&converted) {
            return Ok(converted);
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(format!(
            "convert pcapng to pcap with tcpdump failed for {}: {}",
            path.display(),
            detail
        )
        .into());
    }
    if !is_classic_pcap(&converted) {
        return Err(format!(
            "tcpdump produced no valid classic pcap for {}",
            path.display()
        )
        .into());
    }
    Ok(converted)
}

fn is_classic_pcap(bytes: &[u8]) -> bool {
    bytes.len() >= 24
        && CLASSIC_PCAP_MAGICS
            .iter()
            .any(|magic| bytes.starts_with(magic))
}

#[cfg(test)]
mod tests {
    use super::{CLASSIC_PCAP_MAGICS, PCAPNG_MAGIC, is_classic_pcap};

    #[test]
    fn detects_pcapng_magic() {
        assert!([0x0a, 0x0d, 0x0d, 0x0a, 0, 0].starts_with(&PCAPNG_MAGIC));
        assert!(![0xd4, 0xc3, 0xb2, 0xa1, 0, 0].starts_with(&PCAPNG_MAGIC));
    }

    #[test]
    fn accepts_only_complete_classic_pcap_headers() {
        for magic in CLASSIC_PCAP_MAGICS {
            let mut bytes = vec![0; 24];
            bytes[..4].copy_from_slice(&magic);
            assert!(is_classic_pcap(&bytes));
        }
        assert!(!is_classic_pcap(&[0xd4, 0xc3, 0xb2, 0xa1]));
        assert!(!is_classic_pcap(&[0; 24]));
    }
}
