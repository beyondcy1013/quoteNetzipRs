#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use std::mem::size_of;

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct RawOemDataHead {
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
#[derive(Clone, Copy, Debug)]
pub struct RawOemStkInfo {
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

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct RawOemMarketHeader {
    pub mk_id: u16,
    pub name: [u16; 32],
    pub tm_count: i16,
    pub open_time: [i16; 8],
    pub close_time: [i16; 8],
    pub date: u32,
    pub num: u16,
    pub temp: [u8; 94],
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct RawOemReport {
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
#[derive(Clone, Copy, Debug)]
pub struct RawOemTick {
    pub time: u32,
    pub close: f32,
    pub volume: f32,
    pub amount: f32,
    pub trace_num: i32,
    pub price_buy: [f32; 5],
    pub vol_buy: [f32; 5],
    pub price_sell: [f32; 5],
    pub vol_sell: [f32; 5],
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct RawOemKline {
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
#[derive(Clone, Copy, Debug)]
pub struct RawOemSplitHead {
    pub label: [u16; 12],
    pub name: [u16; 32],
    pub num: u16,
    pub volume: u32,
    pub temp: [u8; 106],
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct RawOemSplit {
    pub time: u32,
    pub give: f32,
    pub allocate: f32,
    pub price: f32,
    pub earnings: f32,
    pub explain: [u16; 90],
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct RawOemFinance {
    pub label: [u16; 12],
    pub name: [u16; 32],
    pub time: i32,
    pub bao_gao: i32,
    pub shang_shi: i32,
    pub metrics: [f32; 48],
    pub temp: [u8; 58],
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct RawOemF10Info {
    pub title: [u16; 12],
    pub from: i32,
    pub len: i32,
    pub temp: [u8; 18],
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct RawOemBuySell610 {
    pub av_buy: f32,
    pub av_sell: f32,
    pub buy_vol: f32,
    pub sell_vol: f32,
    pub bs_price_6: [f32; 10],
    pub bs_volume_6: [f32; 10],
}

#[derive(Clone, Debug)]
pub struct DecodedHead {
    pub packet_type: String,
    pub len: i32,
    pub count: i32,
    pub label: String,
    pub name: String,
    pub value: [u32; 3],
    pub flag: i8,
    pub ask_id: u32,
    pub power: i8,
    pub oem_ver: u32,
}

#[derive(Clone, Debug)]
pub struct DecodedStkInfo {
    pub label: String,
    pub name: String,
    pub pinyin: String,
    pub code: u16,
    pub market: u8,
    pub last: f32,
    pub limit_up: f32,
    pub limit_down: f32,
}

#[derive(Clone, Debug)]
pub struct DecodedMarket {
    pub mk_id: u16,
    pub name: String,
    pub date: u32,
    pub num: u16,
    pub stocks: Vec<DecodedStkInfo>,
}

#[derive(Clone, Debug)]
pub struct DecodedReport {
    pub label: String,
    pub name: String,
    pub time: u32,
    pub close: f32,
    pub now_v: f32,
    pub amount: f32,
}

#[derive(Clone, Debug)]
pub struct DecodedTick {
    pub time: u32,
    pub close: f32,
    pub volume: f32,
    pub amount: f32,
}

#[derive(Clone, Debug)]
pub struct DecodedKline {
    pub time: u32,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
    pub amount: f32,
}

#[derive(Clone, Debug)]
pub struct DecodedSplit {
    pub time: u32,
    pub give: f32,
    pub allocate: f32,
    pub price: f32,
    pub earnings: f32,
    pub explain: String,
}

#[derive(Clone, Debug)]
pub struct DecodedSplitGroup {
    pub label: String,
    pub name: String,
    pub items: Vec<DecodedSplit>,
}

#[derive(Clone, Debug)]
pub struct DecodedFinance {
    pub label: String,
    pub name: String,
    pub time: i32,
    pub bao_gao: i32,
    pub shang_shi: i32,
    pub metrics: [f32; 48],
}

#[derive(Clone, Debug)]
pub struct DecodedF10Section {
    pub title: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct DecodedBuySell610 {
    pub av_buy: f32,
    pub av_sell: f32,
    pub buy_vol: f32,
    pub sell_vol: f32,
    pub bs_price_6: [f32; 10],
    pub bs_volume_6: [f32; 10],
}

#[derive(Clone, Debug)]
pub enum Packet {
    InvalidRequest {
        reason: String,
        detail: String,
    },
    CodeTable {
        markets: Vec<DecodedMarket>,
    },
    Realtime {
        items: Vec<DecodedReport>,
    },
    Tick {
        label: String,
        items: Vec<DecodedTick>,
    },
    Trend {
        label: String,
        items: Vec<DecodedKline>,
    },
    Kline {
        kind: String,
        label: String,
        items: Vec<DecodedKline>,
    },
    Split {
        groups: Vec<DecodedSplitGroup>,
    },
    Finance {
        items: Vec<DecodedFinance>,
    },
    F10 {
        label: String,
        sections: Vec<DecodedF10Section>,
    },
    BuySell610 {
        label: String,
        name: String,
        data: DecodedBuySell610,
    },
    Unknown {
        head: DecodedHead,
    },
}

impl Packet {
    pub fn summary(&self) -> String {
        match self {
            Packet::InvalidRequest { reason, detail } => {
                format!("无效请求: 原因={reason}, 详情={detail}")
            }
            Packet::CodeTable { markets } => {
                if let Some(first_market) = markets.first() {
                    let first_stock = first_market
                        .stocks
                        .first()
                        .map(|stock| format!("，首只证券={}({})", stock.label, stock.name))
                        .unwrap_or_default();
                    format!(
                        "代码表: 市场数={} 首市场={} 证券数={}{}",
                        markets.len(),
                        first_market.name,
                        first_market.stocks.len(),
                        first_stock
                    )
                } else {
                    "代码表: 空包".to_string()
                }
            }
            Packet::Realtime { items } => {
                if let Some(first) = items.first() {
                    format!(
                        "实时数据: count={} 首条={}({}) close={:.2} nowv={:.0} amount={:.0} time={}",
                        items.len(),
                        first.label,
                        first.name,
                        first.close,
                        first.now_v,
                        first.amount,
                        first.time
                    )
                } else {
                    "实时数据: 空包".to_string()
                }
            }
            Packet::Tick { label, items } => {
                if let Some(first) = items.first() {
                    format!(
                        "分笔: {} count={} 首条 time={} close={:.2} vol={:.0}",
                        label,
                        items.len(),
                        first.time,
                        first.close,
                        first.volume
                    )
                } else {
                    format!("分笔: {label} count=0")
                }
            }
            Packet::Trend { label, items } => {
                if let Some(last) = items.last() {
                    format!(
                        "分时: {} count={} last_time={} close={:.2} vol={:.0}",
                        label,
                        items.len(),
                        last.time,
                        last.close,
                        last.volume
                    )
                } else {
                    format!("分时: {label} count=0")
                }
            }
            Packet::Kline { kind, label, items } => {
                if let Some(last) = items.last() {
                    format!(
                        "{}: {} count={} last_time={} open={:.2} high={:.2} low={:.2} close={:.2}",
                        kind,
                        label,
                        items.len(),
                        last.time,
                        last.open,
                        last.high,
                        last.low,
                        last.close
                    )
                } else {
                    format!("{kind}: {label} count=0")
                }
            }
            Packet::Split { groups } => {
                if let Some(first) = groups.first() {
                    format!(
                        "除权: {}({}) num={}",
                        first.label,
                        first.name,
                        first.items.len()
                    )
                } else {
                    "除权: 空包".to_string()
                }
            }
            Packet::Finance { items } => {
                if let Some(first) = items.first() {
                    format!(
                        "财务: count={} 首条 {}({}) time={} baogao={}",
                        items.len(),
                        first.label,
                        first.name,
                        first.time,
                        first.bao_gao
                    )
                } else {
                    "财务: 空包".to_string()
                }
            }
            Packet::F10 { label, sections } => {
                let first_title = sections
                    .first()
                    .map(|section| section.title.as_str())
                    .unwrap_or("");
                format!(
                    "F10资料: {} sections={} {}",
                    label,
                    sections.len(),
                    if first_title.is_empty() {
                        String::new()
                    } else {
                        format!("首节={first_title}")
                    }
                )
            }
            Packet::BuySell610 { label, name, data } => format!(
                "6到10档挂单: {}({}) av_buy={:.2} av_sell={:.2}",
                label, name, data.av_buy, data.av_sell
            ),
            Packet::Unknown { head } => format!(
                "包类型={} label={} name={} count={} len={} ask_id={} flag={} power={} oem_ver={}",
                head.packet_type,
                head.label,
                head.name,
                head.count.max(0),
                head.len,
                head.ask_id,
                head.flag,
                head.power,
                head.oem_ver
            ),
        }
    }
}

impl RawOemDataHead {
    pub fn decode(self) -> DecodedHead {
        let RawOemDataHead {
            packet_type,
            len,
            count,
            label,
            name,
            reserved: _,
            value,
            flag,
            ask_id,
            power,
            oem_ver,
        } = self;

        DecodedHead {
            packet_type: utf16_array_to_string(&packet_type),
            len,
            count,
            label: utf16_array_to_string(&label),
            name: utf16_array_to_string(&name),
            value,
            flag,
            ask_id,
            power,
            oem_ver,
        }
    }
}

impl RawOemStkInfo {
    pub fn decode(self) -> DecodedStkInfo {
        let RawOemStkInfo {
            label,
            name,
            pinyin,
            code,
            market,
            block: _,
            point_num: _,
            hand: _,
            last,
            limit_up,
            limit_down,
            is_index: _,
            is_da_pan: _,
            is_stock: _,
            bs_num: _,
            tm_count: _,
            open_time: _,
            close_time: _,
            temp: _,
            temp_end: _,
        } = self;

        DecodedStkInfo {
            label: utf16_array_to_string(&label),
            name: utf16_array_to_string(&name),
            pinyin: utf16_array_to_string(&pinyin),
            code,
            market,
            last,
            limit_up,
            limit_down,
        }
    }
}

impl RawOemReport {
    pub fn decode(self) -> DecodedReport {
        let RawOemReport {
            label,
            name,
            time,
            foot: _,
            open_date: _,
            open_time: _,
            close_date: _,
            open: _,
            high: _,
            low: _,
            close,
            volume: _,
            amount,
            in_vol: _,
            price_sell: _,
            vol_sell: _,
            v_sell_cha: _,
            price_buy: _,
            vol_buy: _,
            v_buy_cha: _,
            jing_jia: _,
            av_price: _,
            is_buy: _,
            now_v,
            now_a: _,
            change: _,
            wei_bi: _,
            liang_bi: _,
            position: _,
            last: _,
            limit_up: _,
            limit_down: _,
            is_index: _,
            is_da_pan: _,
            is_stock: _,
            bs_num: _,
            tick_num: _,
            temp: _,
        } = self;

        DecodedReport {
            label: utf16_array_to_string(&label),
            name: utf16_array_to_string(&name),
            time,
            close,
            now_v,
            amount,
        }
    }
}

impl RawOemTick {
    pub fn decode(self) -> DecodedTick {
        let RawOemTick {
            time,
            close,
            volume,
            amount,
            trace_num: _,
            price_buy: _,
            vol_buy: _,
            price_sell: _,
            vol_sell: _,
        } = self;

        DecodedTick {
            time,
            close,
            volume,
            amount,
        }
    }
}

impl RawOemKline {
    pub fn decode(self) -> DecodedKline {
        let RawOemKline {
            time,
            open,
            high,
            low,
            close,
            volume,
            amount,
            temp: _,
        } = self;

        DecodedKline {
            time,
            open,
            high,
            low,
            close,
            volume,
            amount,
        }
    }
}

impl RawOemSplit {
    pub fn decode(self) -> DecodedSplit {
        let RawOemSplit {
            time,
            give,
            allocate,
            price,
            earnings,
            explain,
        } = self;

        DecodedSplit {
            time,
            give,
            allocate,
            price,
            earnings,
            explain: utf16_array_to_string(&explain),
        }
    }
}

impl RawOemFinance {
    pub fn decode(self) -> DecodedFinance {
        let RawOemFinance {
            label,
            name,
            time,
            bao_gao,
            shang_shi,
            metrics,
            temp: _,
        } = self;

        DecodedFinance {
            label: utf16_array_to_string(&label),
            name: utf16_array_to_string(&name),
            time,
            bao_gao,
            shang_shi,
            metrics,
        }
    }
}

impl RawOemF10Info {
    pub fn decode(self) -> (String, usize, usize) {
        let RawOemF10Info {
            title,
            from,
            len,
            temp: _,
        } = self;

        (
            utf16_array_to_string(&title),
            from.max(0) as usize,
            len.max(0) as usize,
        )
    }
}

impl RawOemBuySell610 {
    pub fn decode(self) -> DecodedBuySell610 {
        let RawOemBuySell610 {
            av_buy,
            av_sell,
            buy_vol,
            sell_vol,
            bs_price_6,
            bs_volume_6,
        } = self;

        DecodedBuySell610 {
            av_buy,
            av_sell,
            buy_vol,
            sell_vol,
            bs_price_6,
            bs_volume_6,
        }
    }
}

pub fn utf16_array_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&v| v == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

pub unsafe fn utf16_ptr_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }

    let mut len = 0usize;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf16_lossy(slice)
}

pub fn to_wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn is_kline_type(value: &str) -> bool {
    matches!(
        value,
        "1分钟线"
            | "5分钟线"
            | "15分钟线"
            | "30分钟线"
            | "60分钟线"
            | "日线"
            | "周线"
            | "月线"
            | "季线"
            | "年线"
            | "多日线"
            | "分时"
    )
}

pub unsafe fn read_unaligned<T: Copy>(ptr: *const u8) -> T {
    unsafe { ptr.cast::<T>().read_unaligned() }
}

pub unsafe fn parse_answer_buffer(answer: &[u8]) -> Option<Packet> {
    if answer.len() < size_of::<RawOemDataHead>() {
        return None;
    }

    let head = unsafe { read_unaligned::<RawOemDataHead>(answer.as_ptr()) }.decode();
    Some(unsafe { parse_from_parts(answer.as_ptr(), &head) })
}

pub unsafe fn summarize_from_answer_buffer(answer: &[u8]) -> Option<String> {
    unsafe { parse_answer_buffer(answer) }.map(|packet| packet.summary())
}

pub unsafe fn summarize_callback(form: &str, ptr: *const u8) -> String {
    match form {
        "股票数据" => {
            let head = unsafe { read_unaligned::<RawOemDataHead>(ptr) }.decode();
            unsafe { parse_from_parts(ptr, &head) }.summary()
        }
        _ => unsafe { utf16_ptr_to_string(ptr.cast::<u16>()) },
    }
}

unsafe fn parse_from_parts(base: *const u8, head: &DecodedHead) -> Packet {
    let data = unsafe { base.add(size_of::<RawOemDataHead>()) };
    let data_len = head.len.max(0) as usize;
    let count = head.count.max(0) as usize;

    match head.packet_type.as_str() {
        "无效请求" => Packet::InvalidRequest {
            reason: head.label.clone(),
            detail: if data_len > 0 {
                unsafe { utf16_ptr_to_string(data.cast::<u16>()) }
            } else {
                String::new()
            },
        },
        "代码表" => Packet::CodeTable {
            markets: unsafe { parse_code_table(data, data_len) },
        },
        "实时数据" => Packet::Realtime {
            items: unsafe {
                parse_vec::<RawOemReport, DecodedReport>(data, count, |item| item.decode())
            },
        },
        "分笔" => Packet::Tick {
            label: head.label.clone(),
            items: unsafe {
                parse_vec::<RawOemTick, DecodedTick>(data, count, |item| item.decode())
            },
        },
        "分时" => Packet::Trend {
            label: head.label.clone(),
            items: unsafe {
                parse_vec::<RawOemKline, DecodedKline>(data, count, |item| item.decode())
            },
        },
        packet_type if is_kline_type(packet_type) => Packet::Kline {
            kind: packet_type.to_string(),
            label: head.label.clone(),
            items: unsafe {
                parse_vec::<RawOemKline, DecodedKline>(data, count, |item| item.decode())
            },
        },
        "除权" => Packet::Split {
            groups: unsafe { parse_split_groups(data, data_len) },
        },
        "财务" => Packet::Finance {
            items: unsafe {
                parse_vec::<RawOemFinance, DecodedFinance>(data, count, |item| item.decode())
            },
        },
        "F10资料" => Packet::F10 {
            label: head.label.clone(),
            sections: unsafe { parse_f10_sections(data, data_len, count) },
        },
        "6到10档挂单" => Packet::BuySell610 {
            label: head.label.clone(),
            name: head.name.clone(),
            data: unsafe { read_unaligned::<RawOemBuySell610>(data) }.decode(),
        },
        _ => Packet::Unknown { head: head.clone() },
    }
}

unsafe fn parse_vec<T: Copy, U>(
    data: *const u8,
    count: usize,
    mut decode: impl FnMut(T) -> U,
) -> Vec<U> {
    let mut items = Vec::with_capacity(count);
    let node_size = size_of::<T>();
    for index in 0..count {
        let ptr = unsafe { data.add(index * node_size) };
        let raw = unsafe { read_unaligned::<T>(ptr) };
        items.push(decode(raw));
    }
    items
}

unsafe fn parse_code_table(data: *const u8, data_len: usize) -> Vec<DecodedMarket> {
    let mut markets = Vec::new();
    let mut cursor = 0usize;
    let header_size = size_of::<RawOemMarketHeader>();
    let stock_size = size_of::<RawOemStkInfo>();

    while cursor + header_size <= data_len {
        let header = unsafe { read_unaligned::<RawOemMarketHeader>(data.add(cursor)) };
        cursor += header_size;

        let RawOemMarketHeader {
            mk_id,
            name,
            tm_count: _,
            open_time: _,
            close_time: _,
            date,
            num,
            temp: _,
        } = header;

        let stock_count = num as usize;
        let stocks_bytes = stock_count.saturating_mul(stock_size);
        if cursor + stocks_bytes > data_len {
            break;
        }

        let mut stocks = Vec::with_capacity(stock_count);
        for index in 0..stock_count {
            let ptr = unsafe { data.add(cursor + index * stock_size) };
            let stock = unsafe { read_unaligned::<RawOemStkInfo>(ptr) }.decode();
            stocks.push(stock);
        }
        cursor += stocks_bytes;

        markets.push(DecodedMarket {
            mk_id,
            name: utf16_array_to_string(&name),
            date,
            num,
            stocks,
        });
    }

    markets
}

unsafe fn parse_split_groups(data: *const u8, data_len: usize) -> Vec<DecodedSplitGroup> {
    let mut groups = Vec::new();
    let mut cursor = 0usize;
    let head_size = size_of::<RawOemSplitHead>();
    let item_size = size_of::<RawOemSplit>();

    while cursor + head_size <= data_len {
        let raw_head = unsafe { read_unaligned::<RawOemSplitHead>(data.add(cursor)) };
        cursor += head_size;

        let RawOemSplitHead {
            label,
            name,
            num,
            volume: _,
            temp: _,
        } = raw_head;

        let item_count = num as usize;
        let items_bytes = item_count.saturating_mul(item_size);
        if cursor + items_bytes > data_len {
            break;
        }

        let mut items = Vec::with_capacity(item_count);
        for index in 0..item_count {
            let ptr = unsafe { data.add(cursor + index * item_size) };
            let item = unsafe { read_unaligned::<RawOemSplit>(ptr) }.decode();
            items.push(item);
        }
        cursor += items_bytes;

        groups.push(DecodedSplitGroup {
            label: utf16_array_to_string(&label),
            name: utf16_array_to_string(&name),
            items,
        });
    }

    groups
}

unsafe fn parse_f10_sections(
    data: *const u8,
    data_len: usize,
    count: usize,
) -> Vec<DecodedF10Section> {
    let item_size = size_of::<RawOemF10Info>();
    let index_bytes = count.saturating_mul(item_size);
    if index_bytes > data_len {
        return Vec::new();
    }

    let text_ptr = unsafe { data.add(index_bytes) };
    let text_u16_len = (data_len - index_bytes) / 2;
    let text = unsafe { std::slice::from_raw_parts(text_ptr.cast::<u16>(), text_u16_len) };

    let mut sections = Vec::with_capacity(count);
    for index in 0..count {
        let ptr = unsafe { data.add(index * item_size) };
        let (title, from, len) = unsafe { read_unaligned::<RawOemF10Info>(ptr) }.decode();
        if from + len > text.len() {
            continue;
        }
        sections.push(DecodedF10Section {
            title,
            text: String::from_utf16_lossy(&text[from..from + len]),
        });
    }

    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_struct_sizes_match_vendor_header() {
        assert_eq!(size_of::<RawOemDataHead>(), 200);
        assert_eq!(size_of::<RawOemStkInfo>(), 250);
        assert_eq!(size_of::<RawOemMarketHeader>(), 200);
        assert_eq!(size_of::<RawOemReport>(), 500);
        assert_eq!(size_of::<RawOemTick>(), 100);
        assert_eq!(size_of::<RawOemKline>(), 32);
        assert_eq!(size_of::<RawOemSplitHead>(), 200);
        assert_eq!(size_of::<RawOemSplit>(), 200);
        assert_eq!(size_of::<RawOemFinance>(), 350);
        assert_eq!(size_of::<RawOemF10Info>(), 50);
        assert_eq!(size_of::<RawOemBuySell610>(), 96);
    }

    #[test]
    fn utf16_fixed_buffer_decode_stops_at_nul() {
        let buf = [
            '实' as u16,
            '时' as u16,
            '数' as u16,
            '据' as u16,
            0,
            'X' as u16,
        ];
        assert_eq!(utf16_array_to_string(&buf), "实时数据");
    }
}
