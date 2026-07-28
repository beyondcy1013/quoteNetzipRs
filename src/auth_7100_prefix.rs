use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Auth7100PrefixHints {
    pub object_type_id: u32,
    pub packet_len_declared: u32,
    pub packet_len_duplicate: u32,
    pub packet_len_matches: bool,
    pub field40_constant: u32,
    pub payload_len_hint: u32,
    pub attr_flags_hint: u32,
    pub object_span_len_hint: u32,
    pub fixed_overhead_len_hint: Option<u32>,
    pub layout_hint: String,
}

pub fn summarize_auth_7100_prefix_hints(
    object_name: &str,
    tail: &str,
    field_20: u32,
    field_32: u32,
    field_36: u32,
    field_40: u32,
    field_44: u32,
    field_52: u32,
    field_56: u32,
) -> Auth7100PrefixHints {
    let layout_hint = match (object_name, tail, field_44, field_52, field_56) {
        ("网络包", tail, 8, 0, 36) if tail.contains("ZSTD") => "compressed_zstd".to_string(),
        ("网络包", tail, 12, 0, 40) if tail.contains("ZSTD") => {
            "compressed_zstd_dict".to_string()
        }
        (_, tail, _, _, _) if tail.contains("下载文件") => "download_file_record".to_string(),
        ("认证", _, 4, 0, 32) if field_20 == 8 => "auth_reply_record".to_string(),
        ("认证", tail, 4, 0, 32) if tail.contains("请求") => "auth_request_record".to_string(),
        ("认证", tail, 4, 0, 32) if tail.contains("应答") => "auth_reply_record".to_string(),
        _ => "unknown".to_string(),
    };

    Auth7100PrefixHints {
        object_type_id: field_20,
        packet_len_declared: field_32,
        packet_len_duplicate: field_36,
        packet_len_matches: field_32 == field_36,
        field40_constant: field_40,
        payload_len_hint: field_44,
        attr_flags_hint: field_52,
        object_span_len_hint: field_56,
        fixed_overhead_len_hint: field_56.checked_sub(field_44),
        layout_hint,
    }
}

#[cfg(test)]
mod tests {
    use super::{Auth7100PrefixHints, summarize_auth_7100_prefix_hints};

    #[test]
    fn summarizes_zstd_network_packet_prefix() {
        let hints =
            summarize_auth_7100_prefix_hints("网络包", "压缩ZSTD", 4, 611, 611, 2, 8, 0, 36);
        assert_eq!(
            hints,
            Auth7100PrefixHints {
                object_type_id: 4,
                packet_len_declared: 611,
                packet_len_duplicate: 611,
                packet_len_matches: true,
                field40_constant: 2,
                payload_len_hint: 8,
                attr_flags_hint: 0,
                object_span_len_hint: 36,
                fixed_overhead_len_hint: Some(28),
                layout_hint: "compressed_zstd".to_string(),
            }
        );
    }

    #[test]
    fn summarizes_download_file_prefix() {
        let hints = summarize_auth_7100_prefix_hints(
            "网络包",
            "数据下载文件",
            1,
            1540,
            1540,
            2,
            1472,
            9,
            1500,
        );
        assert_eq!(hints.object_type_id, 1);
        assert_eq!(hints.payload_len_hint, 1472);
        assert_eq!(hints.attr_flags_hint, 9);
        assert_eq!(hints.object_span_len_hint, 1500);
        assert_eq!(hints.fixed_overhead_len_hint, Some(28));
        assert_eq!(hints.layout_hint, "download_file_record");
    }

    #[test]
    fn summarizes_auth_reply_record_even_when_tail_reuses_request_text() {
        let hints = summarize_auth_7100_prefix_hints("认证", "请求测速", 8, 328, 328, 2, 4, 0, 32);
        assert_eq!(hints.object_type_id, 8);
        assert_eq!(hints.layout_hint, "auth_reply_record");
    }
}
