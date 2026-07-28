#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use std::ffi::c_void;
use std::mem::size_of;

use serde::Serialize;
use serde_json::Value;

use crate::packet::{Packet, RawOemDataHead, read_unaligned};
use crate::{parse_answer_buffer, summarize_callback, utf16_ptr_to_string};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub enum StockMessageChannel {
    SyncReturn,
    Callback,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub enum StockMessageKind {
    Empty,
    Text,
    Packet,
    UnknownBinary,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub enum StockTextFormat {
    Json,
    Plain,
}

#[derive(Clone, Debug, Serialize)]
pub struct StockMessage {
    pub channel: StockMessageChannel,
    pub kind: StockMessageKind,
    pub accepted_async: bool,
    pub form: Option<String>,
    pub ask_id: Option<i32>,
    pub return_code: Option<i32>,
    pub raw_len: Option<usize>,
    pub text: Option<String>,
    pub text_format: Option<StockTextFormat>,
    pub text_request: Option<String>,
    pub text_data: Option<String>,
    pub packet_type: Option<String>,
    pub packet_label: Option<String>,
    pub packet_name: Option<String>,
    pub packet_count: Option<i32>,
    pub packet_len: Option<i32>,
    pub packet_flag: Option<i8>,
    pub packet_ask_id: Option<u32>,
    pub packet_power: Option<i8>,
    pub packet_oem_ver: Option<u32>,
    pub summary: String,
    pub head_hex: Option<String>,
}

impl StockMessage {
    pub fn is_text(&self) -> bool {
        self.kind == StockMessageKind::Text
    }

    pub fn is_packet(&self) -> bool {
        self.kind == StockMessageKind::Packet
    }
}

pub fn interpret_sync_answer(return_code: i32, answer: &[u8]) -> StockMessage {
    if return_code == 0 {
        return StockMessage {
            channel: StockMessageChannel::SyncReturn,
            kind: StockMessageKind::Empty,
            accepted_async: true,
            form: None,
            ask_id: None,
            return_code: Some(return_code),
            raw_len: Some(0),
            text: None,
            text_format: None,
            text_request: None,
            text_data: None,
            packet_type: None,
            packet_label: None,
            packet_name: None,
            packet_count: None,
            packet_len: None,
            packet_flag: None,
            packet_ask_id: None,
            packet_power: None,
            packet_oem_ver: None,
            summary: "同步返回为空(ret=0)，请求已受理，等待 callback 或异步推送".to_string(),
            head_hex: None,
        };
    }
    if return_code < 0 {
        return StockMessage {
            channel: StockMessageChannel::SyncReturn,
            kind: StockMessageKind::Empty,
            accepted_async: false,
            form: None,
            ask_id: None,
            return_code: Some(return_code),
            raw_len: Some(0),
            text: None,
            text_format: None,
            text_request: None,
            text_data: None,
            packet_type: None,
            packet_label: None,
            packet_name: None,
            packet_count: None,
            packet_len: None,
            packet_flag: None,
            packet_ask_id: None,
            packet_power: None,
            packet_oem_ver: None,
            summary: format!("同步返回非正值(ret={return_code})，当前未识别为有效载荷"),
            head_hex: None,
        };
    }

    let raw_len = (return_code as usize).min(answer.len());
    let bytes = &answer[..raw_len];

    if let Some(packet) = unsafe { parse_answer_buffer(bytes) } {
        let packet_type = packet_kind_name(&packet);
        let head = unsafe { read_unaligned::<RawOemDataHead>(bytes.as_ptr()) }.decode();
        return StockMessage {
            channel: StockMessageChannel::SyncReturn,
            kind: StockMessageKind::Packet,
            accepted_async: false,
            form: Some("股票数据".to_string()),
            ask_id: None,
            return_code: Some(return_code),
            raw_len: Some(raw_len),
            text: None,
            text_format: None,
            text_request: None,
            text_data: None,
            packet_type: Some(packet_type),
            packet_label: normalize_empty_string(&head.label),
            packet_name: normalize_empty_string(&head.name),
            packet_count: Some(head.count),
            packet_len: Some(head.len),
            packet_flag: Some(head.flag),
            packet_ask_id: Some(head.ask_id),
            packet_power: Some(head.power),
            packet_oem_ver: Some(head.oem_ver),
            summary: packet.summary(),
            head_hex: Some(preview_hex(bytes, 32)),
        };
    }

    if let Some((text, text_format)) = interpret_text_bytes(bytes) {
        let (text_request, text_data) = extract_text_meta(&text);
        return StockMessage {
            channel: StockMessageChannel::SyncReturn,
            kind: StockMessageKind::Text,
            accepted_async: false,
            form: None,
            ask_id: None,
            return_code: Some(return_code),
            raw_len: Some(raw_len),
            text: Some(text.clone()),
            text_format: Some(text_format),
            text_request,
            text_data,
            packet_type: None,
            packet_label: None,
            packet_name: None,
            packet_count: None,
            packet_len: None,
            packet_flag: None,
            packet_ask_id: None,
            packet_power: None,
            packet_oem_ver: None,
            summary: text,
            head_hex: Some(preview_hex(bytes, 32)),
        };
    }

    StockMessage {
        channel: StockMessageChannel::SyncReturn,
        kind: StockMessageKind::UnknownBinary,
        accepted_async: false,
        form: None,
        ask_id: None,
        return_code: Some(return_code),
        raw_len: Some(raw_len),
        text: None,
        text_format: None,
        text_request: None,
        text_data: None,
        packet_type: None,
        packet_label: None,
        packet_name: None,
        packet_count: None,
        packet_len: None,
        packet_flag: None,
        packet_ask_id: None,
        packet_power: None,
        packet_oem_ver: None,
        summary: format!("同步返回 {raw_len} 字节，但未识别为文本或 OEM 包"),
        head_hex: Some(preview_hex(bytes, 32)),
    }
}

pub unsafe fn interpret_callback_ptr(
    form_ptr: *const u16,
    data_ptr: *const c_void,
    ask_id: i32,
) -> StockMessage {
    let form = unsafe { utf16_ptr_to_string(form_ptr) };
    unsafe { interpret_callback_form_data(&form, data_ptr, ask_id) }
}

pub unsafe fn interpret_callback_form_data(
    form: &str,
    data_ptr: *const c_void,
    ask_id: i32,
) -> StockMessage {
    if form == "股票数据" {
        let data_ptr = data_ptr.cast::<u8>();
        let head = unsafe { read_unaligned::<RawOemDataHead>(data_ptr) }.decode();
        let raw_len = size_of::<RawOemDataHead>() + head.len.max(0) as usize;
        return StockMessage {
            channel: StockMessageChannel::Callback,
            kind: StockMessageKind::Packet,
            accepted_async: false,
            form: Some(form.to_string()),
            ask_id: Some(ask_id),
            return_code: None,
            raw_len: Some(raw_len),
            text: None,
            text_format: None,
            text_request: None,
            text_data: None,
            packet_type: Some(head.packet_type.clone()),
            packet_label: normalize_empty_string(&head.label),
            packet_name: normalize_empty_string(&head.name),
            packet_count: Some(head.count),
            packet_len: Some(head.len),
            packet_flag: Some(head.flag),
            packet_ask_id: Some(head.ask_id),
            packet_power: Some(head.power),
            packet_oem_ver: Some(head.oem_ver),
            summary: unsafe { summarize_callback(form, data_ptr) },
            head_hex: Some(preview_ptr_hex(data_ptr, 32)),
        };
    }

    let text = unsafe { utf16_ptr_to_string(data_ptr.cast::<u16>()) };
    let text_format = classify_text_format(&text);
    let (text_request, text_data) = extract_text_meta(&text);
    StockMessage {
        channel: StockMessageChannel::Callback,
        kind: StockMessageKind::Text,
        accepted_async: false,
        form: Some(form.to_string()),
        ask_id: Some(ask_id),
        return_code: None,
        raw_len: None,
        text: Some(text.clone()),
        text_format,
        text_request,
        text_data,
        packet_type: None,
        packet_label: None,
        packet_name: None,
        packet_count: None,
        packet_len: None,
        packet_flag: None,
        packet_ask_id: None,
        packet_power: None,
        packet_oem_ver: None,
        summary: text,
        head_hex: None,
    }
}

fn packet_kind_name(packet: &Packet) -> String {
    match packet {
        Packet::InvalidRequest { .. } => "无效请求".to_string(),
        Packet::CodeTable { .. } => "代码表".to_string(),
        Packet::Realtime { .. } => "实时数据".to_string(),
        Packet::Tick { .. } => "分笔".to_string(),
        Packet::Trend { .. } => "分时".to_string(),
        Packet::Kline { kind, .. } => kind.clone(),
        Packet::Split { .. } => "除权".to_string(),
        Packet::Finance { .. } => "财务".to_string(),
        Packet::F10 { .. } => "F10资料".to_string(),
        Packet::BuySell610 { .. } => "6到10档挂单".to_string(),
        Packet::Unknown { .. } => "未知类型".to_string(),
    }
}

fn interpret_text_bytes(bytes: &[u8]) -> Option<(String, StockTextFormat)> {
    preview_utf16le_text(bytes)
        .or_else(|| preview_utf8_text(bytes))
        .map(|text| {
            let text_format = classify_text_format(&text).unwrap_or(StockTextFormat::Plain);
            (text, text_format)
        })
}

fn extract_text_meta(text: &str) -> (Option<String>, Option<String>) {
    let trimmed = text.trim();
    let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
        return (None, None);
    };
    let Some(object) = value.as_object() else {
        return (None, None);
    };

    let request = object
        .get("请求")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let data = object
        .get("数据")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    if request.is_some() || data.is_some() {
        return (request, data);
    }

    if let Some(text) = object
        .get("提示信息")
        .and_then(Value::as_str)
        .map(ToString::to_string)
    {
        return (Some("提示信息".to_string()), Some(text));
    }

    if object.len() == 1 {
        if let Some((key, value)) = object.iter().next() {
            if let Some(text) = value.as_str() {
                return (Some(key.clone()), Some(text.to_string()));
            }
        }
    }

    (None, None)
}

fn classify_text_format(text: &str) -> Option<StockTextFormat> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else if trimmed.starts_with('{') || trimmed.starts_with('[') {
        Some(StockTextFormat::Json)
    } else {
        Some(StockTextFormat::Plain)
    }
}

fn normalize_empty_string(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn preview_utf16le_text(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 2 || bytes.len() % 2 != 0 {
        return None;
    }

    let mut words = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        let word = u16::from_le_bytes([chunk[0], chunk[1]]);
        if word == 0 {
            break;
        }
        words.push(word);
    }

    if words.is_empty() {
        return None;
    }

    let text = String::from_utf16_lossy(&words);
    let text = text.trim_matches(char::from(0)).trim();
    looks_like_text(text).then(|| text.to_string())
}

fn preview_utf8_text(bytes: &[u8]) -> Option<String> {
    let nul = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());
    let text = std::str::from_utf8(&bytes[..nul]).ok()?.trim();
    looks_like_text(text).then(|| text.to_string())
}

fn looks_like_text(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    text.starts_with('{')
        || text.starts_with('[')
        || text.contains("提示")
        || text.contains("登录")
        || text.contains("初始化")
        || text
            .chars()
            .all(|ch| !ch.is_control() || ch.is_whitespace())
}

fn preview_hex(bytes: &[u8], limit: usize) -> String {
    bytes
        .iter()
        .take(limit)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn preview_ptr_hex(ptr: *const u8, limit: usize) -> String {
    if ptr.is_null() {
        return String::new();
    }

    let bytes = unsafe { std::slice::from_raw_parts(ptr, limit) };
    preview_hex(bytes, limit)
}

#[cfg(test)]
mod tests {
    use std::ffi::c_void;
    use std::ptr;

    use super::{
        StockMessageKind, StockTextFormat, interpret_callback_form_data, interpret_callback_ptr,
        interpret_sync_answer,
    };
    use crate::packet::RawOemDataHead;
    use crate::to_wide_null;

    fn encode_utf16_fixed<const N: usize>(text: &str) -> [u16; N] {
        let mut out = [0u16; N];
        for (index, word) in text.encode_utf16().take(N).enumerate() {
            out[index] = word;
        }
        out
    }

    fn build_empty_packet(packet_type: &str, name: &str) -> Vec<u8> {
        let raw = RawOemDataHead {
            packet_type: encode_utf16_fixed(packet_type),
            len: 0,
            count: 0,
            label: encode_utf16_fixed(""),
            name: encode_utf16_fixed(name),
            reserved: [0u8; 62],
            value: [0u32; 3],
            flag: 0,
            ask_id: 5,
            power: 0,
            oem_ver: 0,
        };

        let mut bytes = vec![0u8; std::mem::size_of::<RawOemDataHead>()];
        unsafe {
            ptr::copy_nonoverlapping(
                (&raw as *const RawOemDataHead).cast::<u8>(),
                bytes.as_mut_ptr(),
                bytes.len(),
            );
        }
        bytes
    }

    #[test]
    fn interpret_sync_answer_zero_is_empty() {
        let message = interpret_sync_answer(0, &[]);
        assert_eq!(message.kind, StockMessageKind::Empty);
        assert!(message.accepted_async);
        assert!(message.summary.contains("等待 callback"));
    }

    #[test]
    fn interpret_sync_answer_utf16_json_is_text() {
        let utf16: Vec<u16> = "{\"请求\":\"提示信息\",\"数据\":\"初始化完成\"}"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut bytes = Vec::with_capacity(utf16.len() * 2);
        for word in utf16 {
            bytes.extend_from_slice(&word.to_le_bytes());
        }

        let message = interpret_sync_answer(bytes.len() as i32, &bytes);
        assert_eq!(message.kind, StockMessageKind::Text);
        assert_eq!(message.text_format, Some(StockTextFormat::Json));
        assert_eq!(message.text_request.as_deref(), Some("提示信息"));
        assert_eq!(message.text_data.as_deref(), Some("初始化完成"));
        assert!(message.summary.contains("初始化完成"));
    }

    #[test]
    fn interpret_sync_answer_oem_packet_is_packet() {
        let bytes = build_empty_packet("实时数据", "实时数据");
        let message = interpret_sync_answer(bytes.len() as i32, &bytes);
        assert_eq!(message.kind, StockMessageKind::Packet);
        assert_eq!(message.packet_type.as_deref(), Some("实时数据"));
        assert_eq!(message.packet_name.as_deref(), Some("实时数据"));
        assert_eq!(message.packet_count, Some(0));
        assert_eq!(message.packet_len, Some(0));
        assert_eq!(message.packet_ask_id, Some(5));
        assert!(message.summary.contains("实时数据"));
    }

    #[test]
    fn interpret_callback_text_is_text() {
        let form = to_wide_null("消息");
        let payload = to_wide_null("{\"请求\":\"提示信息\",\"数据\":\"登录成功\"}");
        let message =
            unsafe { interpret_callback_ptr(form.as_ptr(), payload.as_ptr().cast::<c_void>(), 0) };
        assert_eq!(message.kind, StockMessageKind::Text);
        assert_eq!(message.text_format, Some(StockTextFormat::Json));
        assert_eq!(message.text_request.as_deref(), Some("提示信息"));
        assert_eq!(message.text_data.as_deref(), Some("登录成功"));
        assert!(message.summary.contains("登录成功"));
    }

    #[test]
    fn interpret_callback_stock_packet_is_packet() {
        let bytes = build_empty_packet("代码表", "代码表");
        let message = unsafe {
            interpret_callback_form_data("股票数据", bytes.as_ptr().cast::<c_void>(), 7)
        };
        assert_eq!(message.kind, StockMessageKind::Packet);
        assert_eq!(message.packet_type.as_deref(), Some("代码表"));
        assert_eq!(message.ask_id, Some(7));
        assert_eq!(message.packet_name.as_deref(), Some("代码表"));
        assert_eq!(message.packet_ask_id, Some(5));
    }

    #[test]
    fn interpret_callback_tip_json_extracts_single_key_payload() {
        let payload = to_wide_null("{\"提示信息\":\"连接 网际风.exe 成功\"}");
        let message = unsafe {
            interpret_callback_form_data("提示信息", payload.as_ptr().cast::<c_void>(), 0)
        };
        assert_eq!(message.kind, StockMessageKind::Text);
        assert_eq!(message.text_request.as_deref(), Some("提示信息"));
        assert_eq!(message.text_data.as_deref(), Some("连接 网际风.exe 成功"));
    }
}
