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

#[derive(Clone, Debug)]
pub struct RealtimeSnapshot {
    pub instrument: InstrumentId,
    pub name: String,
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

#[derive(Clone, Copy, Debug)]
pub struct KlineSnapshot {
    pub timestamp: u32,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
    pub amount: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SplitSnapshot {
    pub timestamp: u32,
    pub give: f32,
    pub allocate: f32,
    pub price: f32,
    pub earnings: f32,
}

#[derive(Clone, Debug)]
pub struct SplitGroupSnapshot {
    pub instrument: InstrumentId,
    pub name: String,
    pub splits: Vec<SplitSnapshot>,
}

#[derive(Clone, Debug)]
pub struct FinanceSnapshot {
    pub instrument: InstrumentId,
    pub name: String,
    pub time: i32,
    pub bao_gao: i32,
    pub shang_shi: i32,
    pub metrics: [f32; 48],
}

#[derive(Clone, Debug)]
pub struct StockInfoSnapshot {
    pub instrument: InstrumentId,
    pub name: String,
    pub pinyin: String,
    pub code_index: u16,
    pub market: u8,
    pub block: u8,
    pub point_num: i8,
    pub hand: u16,
    pub last: f32,
    pub limit_up: f32,
    pub limit_down: f32,
    pub is_index: u8,
    pub is_da_pan: u8,
    pub is_stock: u8,
    pub bs_num: i8,
    pub tm_count: i16,
    pub open_time: [i16; 8],
    pub close_time: [i16; 8],
}

#[derive(Clone, Debug)]
pub struct MarketInfoSnapshot {
    pub market_id: u16,
    pub name: String,
    pub tm_count: i16,
    pub open_time: [i16; 8],
    pub close_time: [i16; 8],
    pub date: u32,
    pub stocks: Vec<StockInfoSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlMessageKind {
    Notice,
    StatusUpdate,
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
    InvalidMarketDate(u32),
    ControlTextContainsNul {
        field: &'static str,
    },
    ControlPacketTooLarge,
}

impl Display for CodecError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBatch => f.write_str("packet batch is empty"),
            Self::InvalidInstrument(value) => {
                write!(f, "invalid market-qualified instrument: {value}")
            }
            Self::FieldTooLong { field, capacity } => {
                write!(f, "{field} exceeds UTF-16 capacity {capacity}")
            }
            Self::BatchTooLarge(count) => write!(f, "batch contains too many reports: {count}"),
            Self::InvalidMarketDate(date) => write!(f, "invalid OEM market date: {date}"),
            Self::ControlTextContainsNul { field } => {
                write!(f, "control packet {field} contains a NUL character")
            }
            Self::ControlPacketTooLarge => f.write_str("control packet exceeds u32 length fields"),
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

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct OemSplitHead {
    pub label: [u16; 12],
    pub name: [u16; 32],
    pub num: u16,
    pub volume: u32,
    pub temp: [u8; 106],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct OemSplit {
    pub time: u32,
    pub give: f32,
    pub allocate: f32,
    pub price: f32,
    pub earnings: f32,
    pub explain: [u16; 90],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct OemFinance {
    pub label: [u16; 12],
    pub name: [u16; 32],
    pub time: i32,
    pub bao_gao: i32,
    pub shang_shi: i32,
    pub metrics: [f32; 48],
    pub temp: [u8; 58],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct OemMarketInfo {
    pub market_id: u16,
    pub name: [u16; 32],
    pub tm_count: i16,
    pub open_time: [i16; 8],
    pub close_time: [i16; 8],
    pub date: u32,
    pub num: u16,
    pub temp: [u8; 94],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct OemStockInfo {
    pub label: [u16; 12],
    pub name: [u16; 32],
    pub pinyin: [u16; 32],
    pub code: u16,
    pub market: u8,
    pub block: u8,
    pub point_num: i8,
    pub hand: u16,
    pub last: f32,
    pub limit_up: f32,
    pub limit_down: f32,
    pub is_index: u8,
    pub is_da_pan: u8,
    pub is_stock: u8,
    pub bs_num: i8,
    pub tm_count: i16,
    pub open_time: [i16; 8],
    pub close_time: [i16; 8],
    pub temp: [u8; 37],
    pub temp_end: i32,
}

pub fn encode_code_table_packet(
    table_name: &str,
    markets: &[MarketInfoSnapshot],
    ask_id: u32,
) -> Result<Vec<u8>, CodecError> {
    let stock_count = markets.iter().try_fold(0usize, |count, market| {
        count.checked_add(market.stocks.len())
    });
    let Some(stock_count) = stock_count else {
        return Err(CodecError::BatchTooLarge(usize::MAX));
    };
    if markets.is_empty() || stock_count == 0 {
        return Err(CodecError::EmptyBatch);
    }
    let payload_len = markets.iter().try_fold(0usize, |length, market| {
        if market.stocks.len() > usize::from(u16::MAX) {
            return None;
        }
        size_of::<OemStockInfo>()
            .checked_mul(market.stocks.len())
            .and_then(|stocks_len| size_of::<OemMarketInfo>().checked_add(stocks_len))
            .and_then(|market_len| length.checked_add(market_len))
    });
    let payload_len = payload_len
        .and_then(|length| i32::try_from(length).ok())
        .ok_or(CodecError::BatchTooLarge(stock_count))?;
    let count = i32::try_from(stock_count).map_err(|_| CodecError::BatchTooLarge(stock_count))?;
    let market_timestamp = china_midnight_timestamp(markets[0].date)?;
    let head = OemDataHead {
        packet_type: utf16_field("代码表", "packet_type")?,
        len: payload_len,
        count,
        label: [0; 12],
        name: utf16_field(table_name, "name")?,
        reserved: [0; 62],
        value: [market_timestamp, 0, 0],
        flag: 0,
        ask_id,
        power: 0,
        oem_ver: 0,
    };

    let mut packet = Vec::with_capacity(size_of::<OemDataHead>() + payload_len as usize);
    append_packed(&mut packet, &head);
    for market in markets {
        append_packed(
            &mut packet,
            &OemMarketInfo {
                market_id: market.market_id,
                name: utf16_field(&market.name, "market_name")?,
                tm_count: market.tm_count,
                open_time: market.open_time,
                close_time: market.close_time,
                date: market.date,
                num: market.stocks.len() as u16,
                temp: [0; 94],
            },
        );
        for stock in &market.stocks {
            append_packed(
                &mut packet,
                &OemStockInfo {
                    label: utf16_field(stock.instrument.as_str(), "instrument")?,
                    name: utf16_field(&stock.name, "stock_name")?,
                    pinyin: utf16_field(&stock.pinyin, "pinyin")?,
                    code: stock.code_index,
                    market: stock.market,
                    block: stock.block,
                    point_num: stock.point_num,
                    hand: stock.hand,
                    last: stock.last,
                    limit_up: stock.limit_up,
                    limit_down: stock.limit_down,
                    is_index: stock.is_index,
                    is_da_pan: stock.is_da_pan,
                    is_stock: stock.is_stock,
                    bs_num: stock.bs_num,
                    tm_count: stock.tm_count,
                    open_time: stock.open_time,
                    close_time: stock.close_time,
                    temp: [0; 37],
                    temp_end: 0,
                },
            );
        }
    }
    Ok(packet)
}

fn china_midnight_timestamp(date: u32) -> Result<u32, CodecError> {
    let year = i64::from(date / 10_000);
    let month = i64::from(date / 100 % 100);
    let day = i64::from(date % 100);
    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => 0,
    };
    if year < 1970 || days_in_month == 0 || !(1..=days_in_month).contains(&day) {
        return Err(CodecError::InvalidMarketDate(date));
    }
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let unix_days = era * 146_097 + day_of_era - 719_468;
    let unix_seconds = unix_days * 86_400 - 8 * 3_600;
    u32::try_from(unix_seconds).map_err(|_| CodecError::InvalidMarketDate(date))
}

pub fn encode_realtime_packet(
    quotes: &[QuoteSnapshot],
    ask_id: u32,
) -> Result<Vec<u8>, CodecError> {
    if quotes.is_empty() {
        return Err(CodecError::EmptyBatch);
    }
    let snapshots = quotes
        .iter()
        .map(|quote| RealtimeSnapshot {
            instrument: quote.instrument.clone(),
            name: quote.name.clone(),
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
        })
        .collect::<Vec<_>>();
    encode_realtime_snapshots(&snapshots, ask_id)
}

pub fn encode_realtime_snapshots(
    snapshots: &[RealtimeSnapshot],
    ask_id: u32,
) -> Result<Vec<u8>, CodecError> {
    let count =
        i32::try_from(snapshots.len()).map_err(|_| CodecError::BatchTooLarge(snapshots.len()))?;
    let payload_len = size_of::<OemReport>()
        .checked_mul(snapshots.len())
        .and_then(|len| i32::try_from(len).ok())
        .ok_or(CodecError::BatchTooLarge(snapshots.len()))?;

    let head = OemDataHead {
        packet_type: utf16_field("实时数据", "packet_type")?,
        len: payload_len,
        count,
        label: [0; 12],
        name: utf16_field("实时数据", "name")?,
        reserved: [0; 62],
        value: [0; 3],
        flag: 0,
        ask_id,
        power: 0,
        oem_ver: 0,
    };

    let mut packet = Vec::with_capacity(size_of::<OemDataHead>() + payload_len as usize);
    append_packed(&mut packet, &head);
    for snapshot in snapshots {
        let report = OemReport {
            label: utf16_field(snapshot.instrument.as_str(), "instrument")?,
            name: utf16_field(&snapshot.name, "name")?,
            time: snapshot.time,
            foot: snapshot.foot,
            open_date: snapshot.open_date,
            open_time: snapshot.open_time,
            close_date: snapshot.close_date,
            open: snapshot.open,
            high: snapshot.high,
            low: snapshot.low,
            close: snapshot.close,
            volume: snapshot.volume,
            amount: snapshot.amount,
            in_vol: snapshot.in_vol,
            price_sell: snapshot.price_sell,
            vol_sell: snapshot.vol_sell,
            v_sell_cha: snapshot.v_sell_cha,
            price_buy: snapshot.price_buy,
            vol_buy: snapshot.vol_buy,
            v_buy_cha: snapshot.v_buy_cha,
            jing_jia: snapshot.jing_jia,
            av_price: snapshot.av_price,
            is_buy: snapshot.is_buy,
            now_v: snapshot.now_v,
            now_a: snapshot.now_a,
            change: snapshot.change,
            wei_bi: snapshot.wei_bi,
            liang_bi: snapshot.liang_bi,
            position: snapshot.position,
            last: snapshot.last,
            limit_up: snapshot.limit_up,
            limit_down: snapshot.limit_down,
            is_index: snapshot.is_index,
            is_da_pan: snapshot.is_da_pan,
            is_stock: snapshot.is_stock,
            bs_num: snapshot.bs_num,
            tick_num: snapshot.tick_num,
            temp: snapshot.temp,
        };
        append_packed(&mut packet, &report);
    }
    Ok(packet)
}

pub fn encode_kline_packet(
    packet_type: &str,
    instrument: &InstrumentId,
    name: &str,
    bars: &[KlineSnapshot],
    ask_id: u32,
    power: i8,
) -> Result<Vec<u8>, CodecError> {
    if bars.is_empty() {
        return Err(CodecError::EmptyBatch);
    }
    let count = i32::try_from(bars.len()).map_err(|_| CodecError::BatchTooLarge(bars.len()))?;
    let payload_len = size_of::<OemKline>()
        .checked_mul(bars.len())
        .and_then(|len| i32::try_from(len).ok())
        .ok_or(CodecError::BatchTooLarge(bars.len()))?;
    let head = OemDataHead {
        packet_type: utf16_field(packet_type, "packet_type")?,
        len: payload_len,
        count,
        label: utf16_field(instrument.as_str(), "instrument")?,
        name: utf16_field(name, "name")?,
        reserved: [0; 62],
        value: [2, 0, 0],
        flag: 0,
        ask_id,
        power,
        oem_ver: 0,
    };

    let mut packet = Vec::with_capacity(size_of::<OemDataHead>() + payload_len as usize);
    append_packed(&mut packet, &head);
    for bar in bars {
        append_packed(
            &mut packet,
            &OemKline {
                time: bar.timestamp,
                open: bar.open,
                high: bar.high,
                low: bar.low,
                close: bar.close,
                volume: bar.volume,
                amount: bar.amount,
                temp: 0.0,
            },
        );
    }
    Ok(packet)
}

pub fn encode_split_packet(
    groups: &[SplitGroupSnapshot],
    ask_id: u32,
) -> Result<Vec<u8>, CodecError> {
    if groups.is_empty() {
        return Err(CodecError::EmptyBatch);
    }
    let unit_count = groups.iter().try_fold(0usize, |count, group| {
        if group.splits.len() > usize::from(u16::MAX) {
            return None;
        }
        count
            .checked_add(1)
            .and_then(|count| count.checked_add(group.splits.len()))
    });
    let unit_count = unit_count.ok_or(CodecError::BatchTooLarge(usize::MAX))?;
    let count = i32::try_from(unit_count).map_err(|_| CodecError::BatchTooLarge(unit_count))?;
    let payload_len = size_of::<OemSplitHead>()
        .checked_mul(unit_count)
        .and_then(|length| i32::try_from(length).ok())
        .ok_or(CodecError::BatchTooLarge(unit_count))?;
    let head = OemDataHead {
        packet_type: utf16_field("除权", "packet_type")?,
        len: payload_len,
        count,
        label: [0; 12],
        name: utf16_field("除权数据", "name")?,
        reserved: [0; 62],
        value: [0; 3],
        flag: 0,
        ask_id,
        power: 0,
        oem_ver: 0,
    };

    let mut packet = Vec::with_capacity(size_of::<OemDataHead>() + payload_len as usize);
    append_packed(&mut packet, &head);
    for group in groups {
        append_packed(
            &mut packet,
            &OemSplitHead {
                label: utf16_field(group.instrument.as_str(), "instrument")?,
                name: utf16_field(&group.name, "name")?,
                num: group.splits.len() as u16,
                volume: 0,
                temp: [0; 106],
            },
        );
        for split in &group.splits {
            append_packed(
                &mut packet,
                &OemSplit {
                    time: split.timestamp,
                    give: split.give,
                    allocate: split.allocate,
                    price: split.price,
                    earnings: split.earnings,
                    explain: [0; 90],
                },
            );
        }
    }
    Ok(packet)
}

pub fn encode_finance_packet(
    records: &[FinanceSnapshot],
    ask_id: u32,
) -> Result<Vec<u8>, CodecError> {
    if records.is_empty() {
        return Err(CodecError::EmptyBatch);
    }
    let count =
        i32::try_from(records.len()).map_err(|_| CodecError::BatchTooLarge(records.len()))?;
    let payload_len = size_of::<OemFinance>()
        .checked_mul(records.len())
        .and_then(|length| i32::try_from(length).ok())
        .ok_or(CodecError::BatchTooLarge(records.len()))?;
    let head = OemDataHead {
        packet_type: utf16_field("财务", "packet_type")?,
        len: payload_len,
        count,
        label: [0; 12],
        name: utf16_field("财务数据", "name")?,
        reserved: [0; 62],
        value: [0; 3],
        flag: 0,
        ask_id,
        power: 0,
        oem_ver: 0,
    };

    let mut packet = Vec::with_capacity(size_of::<OemDataHead>() + payload_len as usize);
    append_packed(&mut packet, &head);
    for record in records {
        append_packed(
            &mut packet,
            &OemFinance {
                label: utf16_field(record.instrument.as_str(), "instrument")?,
                name: utf16_field(&record.name, "name")?,
                time: record.time,
                bao_gao: record.bao_gao,
                shang_shi: record.shang_shi,
                metrics: record.metrics,
                temp: [0; 58],
            },
        );
    }
    Ok(packet)
}

pub fn encode_file_packet(label: &str, payload: &[u8], ask_id: u32) -> Result<Vec<u8>, CodecError> {
    if payload.is_empty() {
        return Err(CodecError::EmptyBatch);
    }
    let payload_len =
        i32::try_from(payload.len()).map_err(|_| CodecError::BatchTooLarge(payload.len()))?;
    let head = OemDataHead {
        packet_type: utf16_field("文件", "packet_type")?,
        len: payload_len,
        count: 1,
        label: utf16_field(label, "label")?,
        name: [0; 32],
        reserved: [0; 62],
        value: [0; 3],
        flag: 0,
        ask_id,
        power: 0,
        oem_ver: 0,
    };

    let mut packet = Vec::with_capacity(size_of::<OemDataHead>() + payload.len());
    append_packed(&mut packet, &head);
    packet.extend_from_slice(payload);
    Ok(packet)
}

pub fn encode_main_data_outer_object(inner: &[u8]) -> Result<Vec<u8>, CodecError> {
    if inner.is_empty() {
        return Err(CodecError::EmptyBatch);
    }
    let inner_len = u32::try_from(inner.len()).map_err(|_| CodecError::ControlPacketTooLarge)?;
    let packet_len = inner_len
        .checked_add(268)
        .ok_or(CodecError::ControlPacketTooLarge)?;
    let payload_len = inner_len
        .checked_add(200)
        .ok_or(CodecError::ControlPacketTooLarge)?;
    let mut prefix = MAIN_DATA_OUTER_PREFIX;
    for offset in [32, 36] {
        write_u32_at(&mut prefix, offset, packet_len);
    }
    write_u32_at(&mut prefix, 44, payload_len);
    write_u32_at(&mut prefix, 56, payload_len + 28);
    write_u32_at(&mut prefix, 98, payload_len);
    write_u32_at(&mut prefix, 102, payload_len);
    write_u32_at(&mut prefix, 242, inner_len);
    write_u32_at(&mut prefix, 254, inner_len + 28);

    let mut packet = Vec::with_capacity(packet_len as usize);
    packet.extend_from_slice(&prefix);
    packet.extend_from_slice(inner);
    packet.extend_from_slice(&OUTER_OBJECT_SUFFIX);
    debug_assert_eq!(packet.len(), packet_len as usize);
    Ok(packet)
}

pub fn encode_file_outer_object(inner: &[u8]) -> Result<Vec<u8>, CodecError> {
    if inner.is_empty() {
        return Err(CodecError::EmptyBatch);
    }
    let inner_len = u32::try_from(inner.len()).map_err(|_| CodecError::ControlPacketTooLarge)?;
    let packet_len = inner_len
        .checked_add(136)
        .ok_or(CodecError::ControlPacketTooLarge)?;
    let payload_len = inner_len
        .checked_add(68)
        .ok_or(CodecError::ControlPacketTooLarge)?;
    let mut prefix = FILE_OUTER_PREFIX;
    for offset in [32, 36] {
        write_u32_at(&mut prefix, offset, packet_len);
    }
    write_u32_at(&mut prefix, 44, payload_len);
    write_u32_at(&mut prefix, 56, payload_len + 28);
    write_u32_at(&mut prefix, 98, payload_len);
    write_u32_at(&mut prefix, 102, payload_len);
    write_u32_at(&mut prefix, 110, inner_len);
    write_u32_at(&mut prefix, 122, inner_len + 28);

    let mut packet = Vec::with_capacity(packet_len as usize);
    packet.extend_from_slice(&prefix);
    packet.extend_from_slice(inner);
    packet.extend_from_slice(&OUTER_OBJECT_SUFFIX);
    debug_assert_eq!(packet.len(), packet_len as usize);
    Ok(packet)
}

pub fn encode_control_message(
    kind: ControlMessageKind,
    title: &str,
    body: &str,
) -> Result<Vec<u8>, CodecError> {
    let title = nul_terminated_utf16(title, "title")?;
    let body = nul_terminated_utf16(body, "body")?;
    let title_bytes = title.len() - 2;
    let body_bytes = body.len() - 2;
    let title_units = title_bytes / 2;
    let packet_len = 256usize
        .checked_add(
            title_bytes
                .checked_mul(2)
                .ok_or(CodecError::ControlPacketTooLarge)?,
        )
        .and_then(|length| length.checked_add(body_bytes.checked_mul(2)?))
        .ok_or(CodecError::ControlPacketTooLarge)?;
    let packet_len = u32::try_from(packet_len).map_err(|_| CodecError::ControlPacketTooLarge)?;
    let payload_len = packet_len - 68;
    let title_units = u32::try_from(title_units).map_err(|_| CodecError::ControlPacketTooLarge)?;
    let title_bytes = u32::try_from(title_bytes).map_err(|_| CodecError::ControlPacketTooLarge)?;
    let body_bytes = u32::try_from(body_bytes).map_err(|_| CodecError::ControlPacketTooLarge)?;

    let mut prefix = SIMPLE_CONTROL_PREFIX;
    for offset in [32, 36] {
        write_u32_at(&mut prefix, offset, packet_len);
    }
    write_u32_at(&mut prefix, 44, payload_len);
    write_u32_at(&mut prefix, 56, payload_len + 28);
    let kind_marker = match kind {
        ControlMessageKind::Notice => 0x606f_u16,
        ControlMessageKind::StatusUpdate => 0x65b0_u16,
    };
    prefix[72..74].copy_from_slice(&kind_marker.to_le_bytes());
    write_u32_at(&mut prefix, 98, payload_len);
    write_u32_at(&mut prefix, 102, payload_len);
    write_u32_at(&mut prefix, 106, title_units);
    write_u32_at(&mut prefix, 110, body_bytes);
    write_u32_at(
        &mut prefix,
        122,
        24_u32
            .checked_add(title_bytes)
            .and_then(|length| length.checked_add(body_bytes))
            .ok_or(CodecError::ControlPacketTooLarge)?,
    );

    let span = 96_u32
        .checked_add(title_bytes)
        .and_then(|length| length.checked_add(body_bytes))
        .ok_or(CodecError::ControlPacketTooLarge)?;
    let mut middle = SIMPLE_CONTROL_MIDDLE;
    write_u32_at(&mut middle, 4, span);
    write_u32_at(&mut middle, 16, span + 28);
    write_u32_at(&mut middle, 58, span);

    let mut inner = SIMPLE_CONTROL_INNER;
    write_u32_at(&mut inner, 4, body_bytes);
    write_u32_at(&mut inner, 16, body_bytes + 28);

    let mut packet = Vec::with_capacity(packet_len as usize);
    packet.extend_from_slice(&prefix);
    packet.extend_from_slice(&title);
    packet.extend_from_slice(&body);
    packet.extend_from_slice(&middle);
    packet.extend_from_slice(&title);
    packet.extend_from_slice(&inner);
    packet.extend_from_slice(&body);
    packet.extend_from_slice(&SIMPLE_CONTROL_SUFFIX);
    debug_assert_eq!(packet.len(), packet_len as usize);
    Ok(packet)
}

pub fn encode_initialization_control_packet() -> Vec<u8> {
    INITIALIZATION_CONTROL_PACKET.to_vec()
}

fn nul_terminated_utf16(value: &str, field: &'static str) -> Result<Vec<u8>, CodecError> {
    if value.contains('\0') {
        return Err(CodecError::ControlTextContainsNul { field });
    }
    let units = value.encode_utf16().collect::<Vec<_>>();
    let capacity = units
        .len()
        .checked_add(1)
        .and_then(|length| length.checked_mul(2))
        .ok_or(CodecError::ControlPacketTooLarge)?;
    let mut bytes = Vec::with_capacity(capacity);
    for unit in units {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes.extend_from_slice(&[0, 0]);
    Ok(bytes)
}

fn write_u32_at<const N: usize>(bytes: &mut [u8; N], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
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

// Objects 01/02/03/04/09/10 share this exact skeleton. Only the eight
// u32 length fields patched by encode_main_data_outer_object vary.
const MAIN_DATA_OUTER_PREFIX: [u8; 264] = [
    0x51, 0x7f, 0xdc, 0x7e, 0x05, 0x53, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xd4, 0x01, 0x00, 0x00, 0xd4, 0x01, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x90, 0x01, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0xac, 0x01, 0x00, 0x00, 0x70, 0x65, 0x6e, 0x63,
    0x00, 0x00, 0xa1, 0x80, 0x68, 0x79, 0x70, 0x65, 0x6e, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x90, 0x01, 0x00, 0x00, 0x90, 0x01, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x06, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x22, 0x00, 0x00, 0x00, 0xf7, 0x8b,
    0x42, 0x6c, 0x00, 0x00, 0x1d, 0x52, 0xcb, 0x59, 0x16, 0x53, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
    0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x22, 0x00, 0x00, 0x00,
    0x94, 0x5e, 0x54, 0x7b, 0x00, 0x00, 0x1d, 0x52, 0xcb, 0x59, 0x16, 0x53, 0x00, 0x00, 0x02, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1c, 0x00,
    0x00, 0x00, 0x21, 0x6a, 0x57, 0x57, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x06, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, 0x00, 0x00, 0x05, 0x53,
    0x0d, 0x54, 0xf0, 0x79, 0x00, 0x00, 0x94, 0x5e, 0x54, 0x7b, 0x05, 0x53, 0x00, 0x00, 0x02, 0x00,
    0x00, 0x00, 0xc8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0xe4, 0x00,
    0x00, 0x00, 0x70, 0x65, 0x6e, 0x63, 0x00, 0x00,
];

// Object 05 is the only captured oem132 file wrapper. Its inner object is
// passed through unchanged while the observed length fields are recalculated.
const FILE_OUTER_PREFIX: [u8; 132] = [
    0x51, 0x7f, 0xdc, 0x7e, 0x05, 0x53, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x6e, 0x2f, 0x0f, 0x00, 0x6e, 0x2f, 0x0f, 0x00, 0x02, 0x00, 0x00, 0x00, 0x2a, 0x2f, 0x0f, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x46, 0x2f, 0x0f, 0x00, 0x70, 0x65, 0x6e, 0x63,
    0x00, 0x00, 0xa1, 0x80, 0x68, 0x79, 0x70, 0x65, 0x6e, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x2a, 0x2f, 0x0f, 0x00, 0x2a, 0x2f, 0x0f, 0x00, 0x02, 0x00, 0x00, 0x00, 0xe6, 0x2e,
    0x0f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x02, 0x2f, 0x0f, 0x00, 0x70, 0x65,
    0x6e, 0x63, 0x00, 0x00,
];

const OUTER_OBJECT_SUFFIX: [u8; 4] = [0, 0, 0, 0];

// Stable serialized skeleton from Wine objects 06/07/08/11. Length and text
// fields are patched by encode_control_message; all remaining bytes are equal.
const SIMPLE_CONTROL_PREFIX: [u8; 126] = [
    0x51, 0x7f, 0xdc, 0x7e, 0x05, 0x53, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x24, 0x01, 0x00, 0x00, 0x24, 0x01, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0xe0, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0xfc, 0x00, 0x00, 0x00, 0x70, 0x65, 0x6e, 0x63,
    0x00, 0x00, 0x88, 0x6d, 0x6f, 0x60, 0x00, 0x00, 0x6f, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0xe0, 0x00, 0x00, 0x00, 0xe0, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x0a, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x2a, 0x00, 0x00, 0x00,
];

const SIMPLE_CONTROL_MIDDLE: [u8; 92] = [
    0x02, 0x00, 0x00, 0x00, 0x72, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00,
    0x8e, 0x00, 0x00, 0x00, 0x70, 0x65, 0x6e, 0x63, 0x00, 0x00, 0x88, 0x6d, 0x6f, 0x60, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x72, 0x00, 0x00, 0x00, 0x3c, 0x08,
    0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x24, 0x00, 0x00, 0x00, 0xf7, 0x8b, 0x42, 0x6c, 0x00, 0x00,
];

const SIMPLE_CONTROL_INNER: [u8; 26] = [
    0x02, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x26, 0x00, 0x00, 0x00, 0x70, 0x65, 0x6e, 0x63, 0x00, 0x00,
];

const SIMPLE_CONTROL_SUFFIX: [u8; 4] = [0, 0, 0, 0];

// Object 12 is a compound request/response/module/status record. Only one
// authoritative capture exists, so preserve it as a fixed semantic template.
const INITIALIZATION_CONTROL_PACKET: [u8; 460] = [
    0x51, 0x7f, 0xdc, 0x7e, 0x05, 0x53, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xcc, 0x01, 0x00, 0x00, 0xcc, 0x01, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x88, 0x01, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0xa4, 0x01, 0x00, 0x00, 0x70, 0x65, 0x6e, 0x63,
    0x00, 0x00, 0x88, 0x6d, 0x6f, 0x60, 0x00, 0x00, 0x6e, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x88, 0x01, 0x00, 0x00, 0x88, 0x01, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x06, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x22, 0x00, 0x00, 0x00, 0xf7, 0x8b,
    0x42, 0x6c, 0x00, 0x00, 0x1d, 0x52, 0xcb, 0x59, 0x16, 0x53, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
    0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x22, 0x00, 0x00, 0x00,
    0x94, 0x5e, 0x54, 0x7b, 0x00, 0x00, 0x1d, 0x52, 0xcb, 0x59, 0x16, 0x53, 0x00, 0x00, 0x02, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1c, 0x00,
    0x00, 0x00, 0x21, 0x6a, 0x57, 0x57, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x06, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, 0x00, 0x00, 0x05, 0x53,
    0x0d, 0x54, 0xf0, 0x79, 0x00, 0x00, 0x94, 0x5e, 0x54, 0x7b, 0x05, 0x53, 0x00, 0x00, 0x04, 0x00,
    0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x24, 0x00,
    0x00, 0x00, 0x94, 0x5e, 0x54, 0x7b, 0x16, 0x7f, 0xf7, 0x53, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x2a, 0x00, 0x00, 0x00, 0xd0, 0x63, 0x3a, 0x79, 0xe1, 0x4f, 0x6f, 0x60, 0x00, 0x00,
    0x1d, 0x52, 0xcb, 0x59, 0x16, 0x53, 0x8c, 0x5b, 0x10, 0x62, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
    0x72, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x8e, 0x00, 0x00, 0x00,
    0x70, 0x65, 0x6e, 0x63, 0x00, 0x00, 0x88, 0x6d, 0x6f, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x72, 0x00, 0x00, 0x00, 0x3c, 0x08, 0x00, 0x00, 0x02, 0x00,
    0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00,
    0x00, 0x00, 0xf7, 0x8b, 0x42, 0x6c, 0x00, 0x00, 0xd0, 0x63, 0x3a, 0x79, 0xe1, 0x4f, 0x6f, 0x60,
    0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x26, 0x00, 0x00, 0x00, 0x70, 0x65, 0x6e, 0x63, 0x00, 0x00, 0x1d, 0x52, 0xcb, 0x59,
    0x16, 0x53, 0x8c, 0x5b, 0x10, 0x62, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const _: () = assert!(size_of::<OemDataHead>() == 200);
const _: () = assert!(size_of::<OemReport>() == 500);
const _: () = assert!(size_of::<OemKline>() == 32);
const _: () = assert!(size_of::<OemSplitHead>() == 200);
const _: () = assert!(size_of::<OemSplit>() == 200);
const _: () = assert!(size_of::<OemFinance>() == 350);
const _: () = assert!(size_of::<OemMarketInfo>() == 200);
const _: () = assert!(size_of::<OemStockInfo>() == 250);
