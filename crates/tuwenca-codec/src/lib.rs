use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::mem::size_of;
use std::slice;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct InstrumentId(String);

impl InstrumentId {
    pub fn parse(value: &str) -> Result<Self, CodecError> {
        let bytes = value.as_bytes();
        let valid_market = matches!(bytes.get(..2), Some(b"SH" | b"SZ" | b"BJ"));
        let valid_code = bytes.len() == 8 && bytes[2..].iter().all(u8::is_ascii_digit);
        if !valid_market || !valid_code {
            return Err(CodecError::InvalidInstrument(value.to_owned()));
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct QuoteSnapshot {
    pub instrument: InstrumentId,
    pub name: String,
    pub timestamp: u32,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub last_close: f32,
    pub volume: f32,
    pub amount: f32,
    pub ask_prices: [f32; 10],
    pub ask_volumes: [f32; 10],
    pub bid_prices: [f32; 10],
    pub bid_volumes: [f32; 10],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodecError {
    EmptyBatch,
    InvalidInstrument(String),
    FieldTooLong {
        field: &'static str,
        capacity: usize,
    },
    BatchTooLarge(usize),
}

impl Display for CodecError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBatch => f.write_str("realtime batch is empty"),
            Self::InvalidInstrument(value) => {
                write!(f, "invalid market-qualified instrument: {value}")
            }
            Self::FieldTooLong { field, capacity } => {
                write!(f, "{field} exceeds UTF-16 capacity {capacity}")
            }
            Self::BatchTooLarge(count) => write!(f, "batch contains too many reports: {count}"),
        }
    }
}

impl Error for CodecError {}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct OemDataHead {
    pub packet_type: [u16; 10],
    pub len: i32,
    pub count: i32,
    pub label: [u16; 12],
    pub name: [u16; 32],
    pub reserved: [u8; 62],
    pub value: [u32; 3],
    pub flag: i8,
    pub ask_id: u32,
    pub power: i8,
    pub oem_ver: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct OemReport {
    pub label: [u16; 12],
    pub name: [u16; 32],
    pub time: u32,
    pub foot: i32,
    pub open_date: u32,
    pub open_time: u32,
    pub close_date: u32,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
    pub amount: f32,
    pub in_vol: f32,
    pub price_sell: [f32; 10],
    pub vol_sell: [f32; 10],
    pub v_sell_cha: [f32; 10],
    pub price_buy: [f32; 10],
    pub vol_buy: [f32; 10],
    pub v_buy_cha: [f32; 10],
    pub jing_jia: u8,
    pub av_price: f32,
    pub is_buy: i8,
    pub now_v: f32,
    pub now_a: f32,
    pub change: f32,
    pub wei_bi: f32,
    pub liang_bi: f32,
    pub position: f32,
    pub last: f32,
    pub limit_up: f32,
    pub limit_down: f32,
    pub is_index: u8,
    pub is_da_pan: u8,
    pub is_stock: u8,
    pub bs_num: i8,
    pub tick_num: i32,
    pub temp: [u8; 74],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct OemKline {
    pub time: u32,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
    pub amount: f32,
    pub temp: f32,
}

pub fn encode_realtime_packet(
    quotes: &[QuoteSnapshot],
    ask_id: u32,
) -> Result<Vec<u8>, CodecError> {
    if quotes.is_empty() {
        return Err(CodecError::EmptyBatch);
    }
    let count = i32::try_from(quotes.len()).map_err(|_| CodecError::BatchTooLarge(quotes.len()))?;
    let payload_len = size_of::<OemReport>()
        .checked_mul(quotes.len())
        .and_then(|len| i32::try_from(len).ok())
        .ok_or(CodecError::BatchTooLarge(quotes.len()))?;

    let head = OemDataHead {
        packet_type: utf16_field("实时数据", "packet_type")?,
        len: payload_len,
        count,
        label: [0; 12],
        name: [0; 32],
        reserved: [0; 62],
        value: [0; 3],
        flag: 0,
        ask_id,
        power: 0,
        oem_ver: 0,
    };

    let mut packet = Vec::with_capacity(size_of::<OemDataHead>() + payload_len as usize);
    append_packed(&mut packet, &head);
    for quote in quotes {
        let report = OemReport {
            label: utf16_field(quote.instrument.as_str(), "instrument")?,
            name: utf16_field(&quote.name, "name")?,
            time: quote.timestamp,
            foot: 0,
            open_date: 0,
            open_time: 0,
            close_date: 0,
            open: quote.open,
            high: quote.high,
            low: quote.low,
            close: quote.close,
            volume: quote.volume,
            amount: quote.amount,
            in_vol: 0.0,
            price_sell: quote.ask_prices,
            vol_sell: quote.ask_volumes,
            v_sell_cha: [0.0; 10],
            price_buy: quote.bid_prices,
            vol_buy: quote.bid_volumes,
            v_buy_cha: [0.0; 10],
            jing_jia: 0,
            av_price: 0.0,
            is_buy: 0,
            now_v: 0.0,
            now_a: 0.0,
            change: 0.0,
            wei_bi: 0.0,
            liang_bi: 0.0,
            position: 0.0,
            last: quote.last_close,
            limit_up: 0.0,
            limit_down: 0.0,
            is_index: 0,
            is_da_pan: 0,
            is_stock: 0,
            bs_num: 0,
            tick_num: 0,
            temp: [0; 74],
        };
        append_packed(&mut packet, &report);
    }
    Ok(packet)
}

fn utf16_field<const N: usize>(value: &str, field: &'static str) -> Result<[u16; N], CodecError> {
    let encoded: Vec<u16> = value.encode_utf16().collect();
    if encoded.len() > N {
        return Err(CodecError::FieldTooLong { field, capacity: N });
    }
    let mut output = [0; N];
    output[..encoded.len()].copy_from_slice(&encoded);
    Ok(output)
}

fn append_packed<T>(output: &mut Vec<u8>, value: &T) {
    // Packed OEM structures contain only integer/float fields and fixed arrays.
    let bytes = unsafe { slice::from_raw_parts((value as *const T).cast::<u8>(), size_of::<T>()) };
    output.extend_from_slice(bytes);
}

const _: () = assert!(size_of::<OemDataHead>() == 200);
const _: () = assert!(size_of::<OemReport>() == 500);
const _: () = assert!(size_of::<OemKline>() == 32);
