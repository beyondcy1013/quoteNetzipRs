use std::error::Error;
use std::fs;
use std::path::PathBuf;

use netzip_fullpull::auth_7100::decode_stock_dictionary_netpacket;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let input = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: official_6100_decode <packet-or-reassembled-stream.bin>")?;
    let bytes = fs::read(&input)?;
    let packets = split_netpackets(&bytes)?;

    println!("input: {}", input.display());
    println!("network-packets: {}", packets.len());
    for (index, packet) in packets.iter().enumerate() {
        let decoded = decode_stock_dictionary_netpacket(packet)?;
        println!(
            "packet[{index}]: wire={} decoded={}",
            packet.len(),
            decoded.len()
        );
        print_object(&decoded)?;
    }
    Ok(())
}

fn split_netpackets(bytes: &[u8]) -> Result<Vec<&[u8]>, Box<dyn Error>> {
    let mut packets = Vec::new();
    let mut offset = 0usize;
    while offset < bytes.len() {
        let header = bytes
            .get(offset..offset + 40)
            .ok_or("truncated network packet header")?;
        let length = u32::from_le_bytes(header[32..36].try_into()?) as usize;
        if length < 74 {
            return Err(
                format!("invalid network packet length {length} at offset {offset}").into(),
            );
        }
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or("network packet length exceeds input")?;
        packets.push(&bytes[offset..end]);
        offset = end;
    }
    Ok(packets)
}

fn print_object(bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    if bytes.len() < 40 {
        return Err("decoded object is shorter than its header".into());
    }
    let field_count = read_u32(bytes, 20)? as usize;
    let declared = read_u32(bytes, 32)? as usize;
    let repeated = read_u32(bytes, 36)? as usize;
    if declared != bytes.len() || repeated != bytes.len() {
        return Err(format!(
            "decoded object length mismatch: declared={declared}/{repeated} actual={}",
            bytes.len()
        )
        .into());
    }

    let root = decode_utf16(&bytes[..20]);
    if is_readable(&root) {
        println!("  root={root:?} fields={field_count}");
    } else {
        println!(
            "  root-magic={} fields={field_count}",
            hex_prefix(&bytes[..20], 20)
        );
    }

    let mut offset = 40usize;
    for field_index in 0..field_count {
        let header = bytes
            .get(offset..offset + 20)
            .ok_or("truncated decoded field header")?;
        let type_id = read_u32(header, 0)?;
        let value_len = read_u32(header, 4)? as usize;
        let span_len = read_u32(header, 16)? as usize;
        let end = offset
            .checked_add(span_len)
            .filter(|end| *end <= bytes.len())
            .ok_or("invalid decoded field span")?;
        let tail = bytes
            .get(offset + 20..end)
            .ok_or("truncated decoded field body")?;
        let label_len = utf16_terminator(tail).ok_or("unterminated decoded field label")?;
        let label = decode_utf16(&tail[..label_len]);
        let value_start = offset + 20 + label_len + 2;
        let value_end = value_start
            .checked_add(value_len)
            .filter(|value_end| value_end.checked_add(2) == Some(end))
            .ok_or("decoded field value does not close its span")?;
        let value = &bytes[value_start..value_end];
        let formatted_value = if is_sensitive_label(&label) {
            "<redacted>".to_string()
        } else {
            format_value(value)
        };
        println!(
            "  field[{field_index}] label={label:?} type={type_id} len={value_len} value={}",
            formatted_value
        );
        offset = end;
    }
    if offset != bytes.len() {
        return Err("decoded object has trailing bytes".into());
    }
    Ok(())
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, Box<dyn Error>> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or("missing u32")?
            .try_into()?,
    ))
}

fn utf16_terminator(bytes: &[u8]) -> Option<usize> {
    bytes
        .chunks_exact(2)
        .position(|word| word == [0, 0])
        .map(|words| words * 2)
}

fn decode_utf16(bytes: &[u8]) -> String {
    String::from_utf16_lossy(
        &bytes
            .chunks_exact(2)
            .map(|word| u16::from_le_bytes([word[0], word[1]]))
            .take_while(|word| *word != 0)
            .collect::<Vec<_>>(),
    )
}

fn is_readable(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| !ch.is_control() && ch != '\u{fffd}')
}

fn is_sensitive_label(label: &str) -> bool {
    let exact_identity = matches!(
        label,
        "\u{8d26}\u{53f7}"
            | "\u{5e10}\u{53f7}"
            | "\u{8d26}\u{6237}"
            | "\u{7528}\u{6237}\u{540d}"
            | "\u{7528}\u{6237}\u{8d26}\u{53f7}"
    );
    exact_identity
        || ["\u{5bc6}\u{7801}", "\u{53e3}\u{4ee4}", "token"]
            .iter()
            .any(|secret| label.to_lowercase().contains(secret))
}

fn format_value(value: &[u8]) -> String {
    if value.len() == 4 {
        let number = u32::from_le_bytes(value.try_into().expect("four bytes"));
        return format!("u32:{number} hex:{}", hex_prefix(value, 4));
    }
    if value.len() >= 2 && value.len().is_multiple_of(2) {
        let text = decode_utf16(value);
        if text.encode_utf16().count() * 2 == value.len() && is_readable(&text) {
            return format!("utf16:{text:?}");
        }
    }
    format!("hex:{}", hex_prefix(value, 32))
}

fn hex_prefix(bytes: &[u8], limit: usize) -> String {
    let mut text = bytes
        .iter()
        .take(limit)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if bytes.len() > limit {
        text.push_str("...");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{is_sensitive_label, split_netpackets};

    #[test]
    fn splits_concatenated_complete_network_packets() {
        let mut first = vec![0u8; 74];
        first[32..36].copy_from_slice(&74u32.to_le_bytes());
        let mut second = vec![0u8; 80];
        second[32..36].copy_from_slice(&80u32.to_le_bytes());
        first.extend_from_slice(&second);

        let packets = split_netpackets(&first).expect("split packets");
        assert_eq!(
            packets
                .iter()
                .map(|packet| packet.len())
                .collect::<Vec<_>>(),
            [74, 80]
        );
    }

    #[test]
    fn redacts_credential_bearing_fields() {
        assert!(is_sensitive_label("\u{8d26}\u{53f7}"));
        assert!(is_sensitive_label("access_token"));
        assert!(!is_sensitive_label("\u{8d26}\u{53f7}\u{5230}\u{671f}"));
    }
}
