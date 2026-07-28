use std::env;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const PCAPNG_MAGIC: [u8; 4] = [0x0a, 0x0d, 0x0d, 0x0a];

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
    if !output.status.success() {
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

    let converted = fs::read(&temp_path)?;
    let _ = fs::remove_file(&temp_path);
    Ok(converted)
}

#[cfg(test)]
mod tests {
    use super::PCAPNG_MAGIC;

    #[test]
    fn detects_pcapng_magic() {
        assert!([0x0a, 0x0d, 0x0d, 0x0a, 0, 0].starts_with(&PCAPNG_MAGIC));
        assert!(![0xd4, 0xc3, 0xb2, 0xa1, 0, 0].starts_with(&PCAPNG_MAGIC));
    }
}
