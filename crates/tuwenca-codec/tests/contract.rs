use std::mem::size_of;

use tuwenca_codec::{
    InstrumentId, OemDataHead, OemKline, OemReport, QuoteSnapshot, encode_realtime_packet,
};

#[test]
fn market_qualified_instruments_do_not_collapse_stock_and_index() {
    let sh_index = InstrumentId::parse("SH000001").expect("Shanghai index");
    let sz_stock = InstrumentId::parse("SZ000001").expect("Shenzhen stock");

    assert_ne!(sh_index, sz_stock);
    assert_eq!(sh_index.as_str(), "SH000001");
    assert_eq!(sz_stock.as_str(), "SZ000001");
    assert!(InstrumentId::parse("000001").is_err());
}

#[test]
fn oem_wire_structures_keep_the_vendor_packed_sizes() {
    assert_eq!(size_of::<OemDataHead>(), 200);
    assert_eq!(size_of::<OemReport>(), 500);
    assert_eq!(size_of::<OemKline>(), 32);
}

#[test]
fn realtime_packet_contains_one_market_qualified_report() {
    let quote = QuoteSnapshot {
        instrument: InstrumentId::parse("SH600000").expect("qualified code"),
        name: "浦发银行".to_string(),
        timestamp: 1_785_024_000,
        open: 9.08,
        high: 9.12,
        low: 9.02,
        close: 9.04,
        last_close: 9.05,
        volume: 506_751.0,
        amount: 459_285_312.0,
        ask_prices: [9.04, 9.05, 9.06, 9.07, 9.08, 0.0, 0.0, 0.0, 0.0, 0.0],
        ask_volumes: [
            206.0, 888.0, 918.0, 6_288.0, 5_408.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ],
        bid_prices: [9.03, 9.02, 9.01, 9.00, 8.99, 0.0, 0.0, 0.0, 0.0, 0.0],
        bid_volumes: [
            1_202.0, 11_583.0, 1_081.0, 2_390.0, 1_314.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ],
    };

    let packet = encode_realtime_packet(&[quote], 123).expect("encoded packet");

    assert_eq!(packet.len(), 200 + 500);
    assert_eq!(
        &packet[0..8],
        &[0x9e, 0x5b, 0xf6, 0x65, 0x70, 0x65, 0x6e, 0x63]
    );
    assert_eq!(i32::from_le_bytes(packet[20..24].try_into().unwrap()), 500);
    assert_eq!(i32::from_le_bytes(packet[24..28].try_into().unwrap()), 1);
    assert_eq!(
        u32::from_le_bytes(packet[191..195].try_into().unwrap()),
        123
    );

    let report = &packet[200..];
    assert_eq!(
        &report[0..16],
        &[
            0x53, 0x00, 0x48, 0x00, 0x36, 0x00, 0x30, 0x00, 0x30, 0x00, 0x30, 0x00, 0x30, 0x00,
            0x30, 0x00
        ]
    );
    assert_eq!(
        f32::from_le_bytes(report[120..124].try_into().unwrap()),
        9.04
    );
    assert_eq!(
        f32::from_le_bytes(report[128..132].try_into().unwrap()),
        459_285_312.0
    );
}

#[test]
fn realtime_encoder_rejects_an_empty_batch() {
    let error = encode_realtime_packet(&[], 1).expect_err("empty packet must be rejected");
    assert!(error.to_string().contains("empty"));
}
