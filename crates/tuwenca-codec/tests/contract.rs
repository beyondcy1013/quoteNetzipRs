use std::mem::size_of;

use tuwenca_codec::{
    ControlMessageKind, FinanceSnapshot, InstrumentId, KlineSnapshot, MarketInfoSnapshot,
    OemDataHead, OemFinance, OemKline, OemMarketInfo, OemReport, OemSplit, OemSplitHead,
    OemStockInfo, QuoteSnapshot, RealtimeSnapshot, SplitGroupSnapshot, SplitSnapshot,
    StockInfoSnapshot, encode_code_table_packet, encode_control_message, encode_file_outer_object,
    encode_file_packet, encode_finance_packet, encode_initialization_control_packet,
    encode_kline_packet, encode_main_data_outer_object, encode_realtime_packet,
    encode_realtime_snapshots, encode_split_packet,
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
    assert_eq!(size_of::<OemMarketInfo>(), 200);
    assert_eq!(size_of::<OemStockInfo>(), 250);
    assert_eq!(size_of::<OemSplitHead>(), 200);
    assert_eq!(size_of::<OemSplit>(), 200);
    assert_eq!(size_of::<OemFinance>(), 350);
}

#[test]
fn captured_control_messages_match_wine_objects_byte_for_byte() {
    let captured = [
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../windows_debug/tmp_netzip_probe_20260329/",
            "frida_ws2_trace_20260330_v16_init_full_2000_chunks/",
            "outer_object_catalog/object_bins/object_06.bin"
        ))
        .as_slice(),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../windows_debug/tmp_netzip_probe_20260329/",
            "frida_ws2_trace_20260330_v16_init_full_2000_chunks/",
            "outer_object_catalog/object_bins/object_07.bin"
        ))
        .as_slice(),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../windows_debug/tmp_netzip_probe_20260329/",
            "frida_ws2_trace_20260330_v16_init_full_2000_chunks/",
            "outer_object_catalog/object_bins/object_08.bin"
        ))
        .as_slice(),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../windows_debug/tmp_netzip_probe_20260329/",
            "frida_ws2_trace_20260330_v16_init_full_2000_chunks/",
            "outer_object_catalog/object_bins/object_11.bin"
        ))
        .as_slice(),
    ];
    let encoded = [
        encode_control_message(ControlMessageKind::Notice, "提示信息", "欢迎使用。").unwrap(),
        encode_control_message(
            ControlMessageKind::Notice,
            "提示信息",
            "从认证服务器获取股票登录信息出错。",
        )
        .unwrap(),
        encode_control_message(
            ControlMessageKind::StatusUpdate,
            "状态更新",
            "股票备用服务器",
        )
        .unwrap(),
        encode_control_message(ControlMessageKind::Notice, "提示信息", "初始化完成").unwrap(),
    ];

    for (encoded, captured) in encoded.iter().zip(captured) {
        assert_eq!(encoded, captured);
    }

    for (duplicate, original) in [(13, 6), (14, 7), (15, 8)] {
        let duplicate = std::fs::read(captured_object_path(duplicate)).unwrap();
        let original = std::fs::read(captured_object_path(original)).unwrap();
        assert_eq!(duplicate, original);
    }
}

#[test]
fn initialization_control_packet_matches_wine_object_12_byte_for_byte() {
    let captured = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../windows_debug/tmp_netzip_probe_20260329/",
        "frida_ws2_trace_20260330_v16_init_full_2000_chunks/",
        "outer_object_catalog/object_bins/object_12.bin"
    ));
    assert_eq!(encode_initialization_control_packet(), captured);
}

#[test]
fn captured_main_and_file_outer_objects_round_trip_byte_for_byte() {
    for index in [1, 2, 3, 4, 9, 10] {
        let captured = std::fs::read(captured_object_path(index)).unwrap();
        let inner = &captured[264..captured.len() - 4];
        assert_eq!(encode_main_data_outer_object(inner).unwrap(), captured);
    }

    let captured = std::fs::read(captured_object_path(5)).unwrap();
    let inner = &captured[132..captured.len() - 4];
    assert_eq!(encode_file_outer_object(inner).unwrap(), captured);
}

fn captured_object_path(index: u8) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../windows_debug/tmp_netzip_probe_20260329/\
         frida_ws2_trace_20260330_v16_init_full_2000_chunks/\
        outer_object_catalog/object_bins/object_{index:02}.bin"
    ))
}

#[test]
fn complete_realtime_packet_matches_normalized_wine_object() {
    let object = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../windows_debug/tmp_netzip_probe_20260329/",
        "frida_ws2_trace_20260330_v16_init_full_2000_chunks/",
        "outer_object_catalog/object_bins/object_09.bin"
    ));

    // One zero was inserted after record 4919 jing_jia, shifting all later records.
    // The displaced final zero precedes the remaining three-byte outer footer.
    let mut captured = object[264..object.len() - 3].to_vec();
    let inserted_zero = 200 + 4_918 * 500 + 377;
    assert_eq!(captured.remove(inserted_zero), 0);
    assert_eq!(captured.len(), 200 + 6_298 * 500);

    let records = (0..6_298)
        .map(|index| {
            let offset = 200 + index * 500;
            let record = &captured[offset..offset + 500];
            let f32_array = |start: usize| {
                std::array::from_fn(|item| {
                    let offset = start + item * 4;
                    f32::from_le_bytes(record[offset..offset + 4].try_into().unwrap())
                })
            };
            RealtimeSnapshot {
                instrument: InstrumentId::parse(&decode_utf16_field(&record[0..24])).unwrap(),
                name: decode_utf16_field(&record[24..88]),
                time: u32::from_le_bytes(record[88..92].try_into().unwrap()),
                foot: i32::from_le_bytes(record[92..96].try_into().unwrap()),
                open_date: u32::from_le_bytes(record[96..100].try_into().unwrap()),
                open_time: u32::from_le_bytes(record[100..104].try_into().unwrap()),
                close_date: u32::from_le_bytes(record[104..108].try_into().unwrap()),
                open: f32::from_le_bytes(record[108..112].try_into().unwrap()),
                high: f32::from_le_bytes(record[112..116].try_into().unwrap()),
                low: f32::from_le_bytes(record[116..120].try_into().unwrap()),
                close: f32::from_le_bytes(record[120..124].try_into().unwrap()),
                volume: f32::from_le_bytes(record[124..128].try_into().unwrap()),
                amount: f32::from_le_bytes(record[128..132].try_into().unwrap()),
                in_vol: f32::from_le_bytes(record[132..136].try_into().unwrap()),
                price_sell: f32_array(136),
                vol_sell: f32_array(176),
                v_sell_cha: f32_array(216),
                price_buy: f32_array(256),
                vol_buy: f32_array(296),
                v_buy_cha: f32_array(336),
                jing_jia: record[376],
                av_price: f32::from_le_bytes(record[377..381].try_into().unwrap()),
                is_buy: record[381] as i8,
                now_v: f32::from_le_bytes(record[382..386].try_into().unwrap()),
                now_a: f32::from_le_bytes(record[386..390].try_into().unwrap()),
                change: f32::from_le_bytes(record[390..394].try_into().unwrap()),
                wei_bi: f32::from_le_bytes(record[394..398].try_into().unwrap()),
                liang_bi: f32::from_le_bytes(record[398..402].try_into().unwrap()),
                position: f32::from_le_bytes(record[402..406].try_into().unwrap()),
                last: f32::from_le_bytes(record[406..410].try_into().unwrap()),
                limit_up: f32::from_le_bytes(record[410..414].try_into().unwrap()),
                limit_down: f32::from_le_bytes(record[414..418].try_into().unwrap()),
                is_index: record[418],
                is_da_pan: record[419],
                is_stock: record[420],
                bs_num: record[421] as i8,
                tick_num: i32::from_le_bytes(record[422..426].try_into().unwrap()),
                temp: record[426..500].try_into().unwrap(),
            }
        })
        .collect::<Vec<_>>();

    let encoded = encode_realtime_snapshots(&records, 0).unwrap();
    let mut normalized = captured;
    for range in [0..20, 28..52, 52..116] {
        zero_utf16_tail(&mut normalized[range]);
    }
    for index in 0..records.len() {
        let offset = 200 + index * 500;
        for range in [offset..offset + 24, offset + 24..offset + 88] {
            zero_utf16_tail(&mut normalized[range]);
        }
    }
    assert_eq!(encoded, normalized);
}

#[test]
fn empty_realtime_snapshot_packet_matches_wine_object_10() {
    let object = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../windows_debug/tmp_netzip_probe_20260329/",
        "frida_ws2_trace_20260330_v16_init_full_2000_chunks/",
        "outer_object_catalog/object_bins/object_10.bin"
    ));
    let captured = &object[264..object.len() - 4];
    assert_eq!(captured.len(), 200);
    assert_eq!(i32::from_le_bytes(captured[20..24].try_into().unwrap()), 0);
    assert_eq!(i32::from_le_bytes(captured[24..28].try_into().unwrap()), 0);

    let encoded = encode_realtime_snapshots(&[], 0).unwrap();
    let mut normalized = captured.to_vec();
    for range in [0..20, 28..52, 52..116] {
        zero_utf16_tail(&mut normalized[range]);
    }
    assert_eq!(encoded, normalized);
}

#[test]
fn complete_finance_packet_matches_normalized_wine_object() {
    let object = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../windows_debug/tmp_netzip_probe_20260329/",
        "frida_ws2_trace_20260330_v16_init_full_2000_chunks/",
        "outer_object_catalog/object_bins/object_04.bin"
    ));
    let captured = &object[264..object.len() - 4];
    let count = i32::from_le_bytes(captured[24..28].try_into().unwrap()) as usize;
    assert_eq!(count, 5_993);
    let records = (0..count)
        .map(|index| {
            let offset = 200 + index * 350;
            let record = &captured[offset..offset + 350];
            FinanceSnapshot {
                instrument: InstrumentId::parse(&decode_utf16_field(&record[0..24])).unwrap(),
                name: decode_utf16_field(&record[24..88]),
                time: i32::from_le_bytes(record[88..92].try_into().unwrap()),
                bao_gao: i32::from_le_bytes(record[92..96].try_into().unwrap()),
                shang_shi: i32::from_le_bytes(record[96..100].try_into().unwrap()),
                metrics: std::array::from_fn(|metric| {
                    let start = 100 + metric * 4;
                    f32::from_le_bytes(record[start..start + 4].try_into().unwrap())
                }),
            }
        })
        .collect::<Vec<_>>();

    let encoded = encode_finance_packet(&records, 0).unwrap();
    assert_eq!(encoded.len(), 200 + 5_993 * 350);
    assert_eq!(
        i32::from_le_bytes(encoded[20..24].try_into().unwrap()),
        2_097_550
    );
    assert_eq!(
        i32::from_le_bytes(encoded[24..28].try_into().unwrap()),
        5_993
    );

    let mut normalized = captured.to_vec();
    for range in [0..20, 28..52, 52..116] {
        zero_utf16_tail(&mut normalized[range]);
    }
    for index in 0..count {
        let offset = 200 + index * 350;
        for range in [offset..offset + 24, offset + 24..offset + 88] {
            zero_utf16_tail(&mut normalized[range]);
        }
        normalized[offset + 292..offset + 350].fill(0);
    }
    if encoded != normalized {
        let offset = encoded
            .iter()
            .zip(&normalized)
            .position(|(left, right)| left != right)
            .unwrap_or(encoded.len().min(normalized.len()));
        panic!(
            "finance packet differs at offset {offset}: encoded_len={} captured_len={} encoded={:02x?} captured={:02x?}",
            encoded.len(),
            normalized.len(),
            &encoded[offset.saturating_sub(8)..(offset + 9).min(encoded.len())],
            &normalized[offset.saturating_sub(8)..(offset + 9).min(normalized.len())]
        );
    }
}

#[test]
fn complete_file_packet_matches_normalized_wine_object() {
    let object = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../windows_debug/tmp_netzip_probe_20260329/",
        "frida_ws2_trace_20260330_v16_init_full_2000_chunks/",
        "outer_object_catalog/object_bins/object_05.bin"
    ));
    let captured = &object[132..object.len() - 4];
    let payload = &captured[200..];
    assert_eq!(payload.len(), 994_846);

    let encoded = encode_file_packet("数据\\财务V6.fin", payload, 0).unwrap();
    assert_eq!(encoded.len(), 200 + 994_846);
    assert_eq!(
        i32::from_le_bytes(encoded[20..24].try_into().unwrap()),
        994_846
    );
    assert_eq!(i32::from_le_bytes(encoded[24..28].try_into().unwrap()), 1);
    assert_eq!(&encoded[200..], payload);

    let mut normalized = captured.to_vec();
    for range in [0..20, 28..52, 52..116] {
        zero_utf16_tail(&mut normalized[range]);
    }
    assert_eq!(encoded, normalized);
}

#[allow(clippy::chunks_exact_to_as_chunks)]
fn decode_utf16_field(field: &[u8]) -> String {
    let units = field
        .chunks_exact(2)
        .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
        .take_while(|unit| *unit != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&units).unwrap()
}

#[test]
fn complete_split_packet_matches_normalized_wine_object() {
    let groups_csv = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../windows_debug/tmp_netzip_probe_20260329/",
        "frida_ws2_trace_20260330_v15_object3_split_full/parsed_split_full/groups.csv"
    ));
    let splits_csv = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../windows_debug/tmp_netzip_probe_20260329/",
        "frida_ws2_trace_20260330_v15_object3_split_full/parsed_split_full/splits.csv"
    ));
    let object = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../windows_debug/tmp_netzip_probe_20260329/",
        "frida_ws2_trace_20260330_v16_init_full_2000_chunks/",
        "outer_object_catalog/object_bins/object_03.bin"
    ));

    let mut groups = groups_csv
        .lines()
        .skip(1)
        .map(|line| {
            let fields = line.split(',').collect::<Vec<_>>();
            assert_eq!(
                fields[4].parse::<usize>().unwrap(),
                fields[5].parse::<usize>().unwrap()
            );
            SplitGroupSnapshot {
                instrument: InstrumentId::parse(fields[2]).unwrap(),
                name: fields[3].to_string(),
                splits: Vec::with_capacity(fields[4].parse().unwrap()),
            }
        })
        .collect::<Vec<_>>();
    for line in splits_csv.lines().skip(1) {
        let fields = line.split(',').collect::<Vec<_>>();
        let group_index = fields[0].parse::<usize>().unwrap() - 1;
        let record_offset = fields[2].parse::<usize>().unwrap();
        let record = &object[record_offset..record_offset + 200];
        groups[group_index].splits.push(SplitSnapshot {
            timestamp: u32::from_le_bytes(record[0..4].try_into().unwrap()),
            give: f32::from_le_bytes(record[4..8].try_into().unwrap()),
            allocate: f32::from_le_bytes(record[8..12].try_into().unwrap()),
            price: f32::from_le_bytes(record[12..16].try_into().unwrap()),
            earnings: f32::from_le_bytes(record[16..20].try_into().unwrap()),
        });
    }
    assert_eq!(groups.len(), 5_224);
    assert_eq!(
        groups.iter().map(|group| group.splits.len()).sum::<usize>(),
        54_376
    );

    let encoded = encode_split_packet(&groups, 0).unwrap();
    assert_eq!(encoded.len(), 200 + 59_600 * 200);
    assert_eq!(
        i32::from_le_bytes(encoded[20..24].try_into().unwrap()),
        11_920_000
    );
    assert_eq!(
        i32::from_le_bytes(encoded[24..28].try_into().unwrap()),
        59_600
    );

    let mut captured = object[264..object.len() - 4].to_vec();
    normalize_split_utf16_tails(&mut captured);
    if encoded != captured {
        let offset = encoded
            .iter()
            .zip(&captured)
            .position(|(left, right)| left != right)
            .unwrap_or(encoded.len().min(captured.len()));
        panic!(
            "split packet differs at offset {offset}: encoded_len={} captured_len={} encoded={:02x?} captured={:02x?}",
            encoded.len(),
            captured.len(),
            &encoded[offset.saturating_sub(8)..(offset + 9).min(encoded.len())],
            &captured[offset.saturating_sub(8)..(offset + 9).min(captured.len())]
        );
    }
}

fn normalize_split_utf16_tails(packet: &mut [u8]) {
    for range in [0..20, 28..52, 52..116] {
        zero_utf16_tail(&mut packet[range]);
    }
    let mut offset = 200usize;
    while offset < packet.len() {
        for range in [offset..offset + 24, offset + 24..offset + 88] {
            zero_utf16_tail(&mut packet[range]);
        }
        let count = u16::from_le_bytes(packet[offset + 88..offset + 90].try_into().unwrap());
        offset += 200;
        for _ in 0..count {
            zero_utf16_tail(&mut packet[offset + 20..offset + 200]);
            offset += 200;
        }
    }
    assert_eq!(offset, packet.len());
}

#[test]
fn code_table_stock_record_matches_normalized_wine_capture() {
    let packet = encode_code_table_packet(
        "深圳证券代码表",
        &[MarketInfoSnapshot {
            market_id: u16::from_le_bytes(*b"SZ"),
            name: "深圳证券交易所".to_string(),
            tm_count: 2,
            open_time: [570, 780, 0, 0, 0, 0, 0, 0],
            close_time: [690, 900, 0, 0, 0, 0, 0, 0],
            date: 20_260_327,
            stocks: vec![StockInfoSnapshot {
                instrument: InstrumentId::parse("SZ000001").unwrap(),
                name: "平安银行".to_string(),
                pinyin: "PAYH".to_string(),
                code_index: 0,
                market: 1,
                block: 16,
                point_num: 2,
                hand: 100,
                last: 10.94,
                limit_up: 12.03,
                limit_down: 9.85,
                is_index: 0,
                is_da_pan: 0,
                is_stock: 1,
                bs_num: 5,
                tm_count: 2,
                open_time: [570, 780, 0, 0, 0, 0, 0, 0],
                close_time: [690, 900, 0, 0, 0, 0, 0, 0],
            }],
        }],
        0,
    )
    .unwrap();

    assert_eq!(packet.len(), 200 + 200 + 250);
    assert_eq!(i32::from_le_bytes(packet[20..24].try_into().unwrap()), 450);
    assert_eq!(i32::from_le_bytes(packet[24..28].try_into().unwrap()), 1);

    let mut captured = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../windows_debug/tmp_netzip_probe_20260329/",
        "frida_ws2_trace_20260330_v14_object2_sz_code_table/record_array_split/",
        "records/record_0001_SZ000001.bin"
    ))
    .to_vec();
    for range in [0..24, 24..88, 88..152] {
        zero_utf16_tail(&mut captured[range]);
    }
    assert_eq!(&packet[400..650], captured.as_slice());
}

#[test]
fn complete_sh_and_sz_code_table_packets_match_normalized_wine_objects() {
    let fixtures = [
        (
            "SH",
            "上海证券代码表",
            "上海证券交易所",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../windows_debug/tmp_netzip_probe_20260329/",
                "frida_ws2_trace_20260330_v13_code_table_full_chunks/",
                "record_array_split/records.csv"
            )),
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../windows_debug/tmp_netzip_probe_20260329/",
                "frida_ws2_trace_20260330_v16_init_full_2000_chunks/",
                "outer_object_catalog/object_bins/object_01.bin"
            ))
            .as_slice(),
        ),
        (
            "SZ",
            "深圳证券代码表",
            "深圳证券交易所",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../windows_debug/tmp_netzip_probe_20260329/",
                "frida_ws2_trace_20260330_v14_object2_sz_code_table/",
                "record_array_split/records.csv"
            )),
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../windows_debug/tmp_netzip_probe_20260329/",
                "frida_ws2_trace_20260330_v16_init_full_2000_chunks/",
                "outer_object_catalog/object_bins/object_02.bin"
            ))
            .as_slice(),
        ),
    ];

    for (market, table_name, market_name, csv, object) in fixtures {
        let stocks = csv
            .lines()
            .skip(1)
            .map(|line| {
                let fields = line.split(',').collect::<Vec<_>>();
                StockInfoSnapshot {
                    instrument: InstrumentId::parse(fields[4]).unwrap(),
                    name: fields[5].to_string(),
                    pinyin: fields[6].to_string(),
                    code_index: fields[7].parse().unwrap(),
                    market: fields[8].parse().unwrap(),
                    block: fields[9].parse().unwrap(),
                    point_num: fields[10].parse().unwrap(),
                    hand: fields[11].parse().unwrap(),
                    last: fields[12].parse().unwrap(),
                    limit_up: fields[13].parse().unwrap(),
                    limit_down: fields[14].parse().unwrap(),
                    is_index: fields[15].parse().unwrap(),
                    is_da_pan: fields[16].parse().unwrap(),
                    is_stock: fields[17].parse().unwrap(),
                    bs_num: fields[18].parse().unwrap(),
                    tm_count: fields[19].parse().unwrap(),
                    open_time: [570, 780, 0, 0, 0, 0, 0, 0],
                    close_time: [690, 900, 0, 0, 0, 0, 0, 0],
                }
            })
            .collect::<Vec<_>>();
        let encoded = encode_code_table_packet(
            table_name,
            &[MarketInfoSnapshot {
                market_id: u16::from_le_bytes(market.as_bytes().try_into().unwrap()),
                name: market_name.to_string(),
                tm_count: 2,
                open_time: [570, 780, 0, 0, 0, 0, 0, 0],
                close_time: [690, 900, 0, 0, 0, 0, 0, 0],
                date: 20_260_327,
                stocks,
            }],
            0,
        )
        .unwrap();
        let mut captured = object[264..object.len() - 4].to_vec();
        normalize_code_table_utf16_tails(&mut captured);
        if encoded != captured {
            let offset = encoded
                .iter()
                .zip(&captured)
                .position(|(left, right)| left != right)
                .unwrap_or(encoded.len().min(captured.len()));
            panic!(
                "{market} packet differs at offset {offset}: encoded_len={} captured_len={} encoded={:02x?} captured={:02x?}",
                encoded.len(),
                captured.len(),
                &encoded[offset.saturating_sub(8)..(offset + 9).min(encoded.len())],
                &captured[offset.saturating_sub(8)..(offset + 9).min(captured.len())]
            );
        }
    }
}

fn normalize_code_table_utf16_tails(packet: &mut [u8]) {
    for range in [0..20, 28..52, 52..116, 202..266] {
        zero_utf16_tail(&mut packet[range]);
    }
    let count = i32::from_le_bytes(packet[24..28].try_into().unwrap()) as usize;
    for index in 0..count {
        let offset = 400 + index * 250;
        for range in [
            offset..offset + 24,
            offset + 24..offset + 88,
            offset + 88..offset + 152,
        ] {
            zero_utf16_tail(&mut packet[range]);
        }
    }
}

#[allow(clippy::chunks_exact_to_as_chunks)]
fn zero_utf16_tail(field: &mut [u8]) {
    if let Some(offset) = field
        .chunks_exact(2)
        .position(|unit| unit == [0, 0])
        .map(|index| index * 2)
    {
        field[offset..].fill(0);
    }
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

#[test]
fn kline_packet_matches_the_captured_wine_header_contract() {
    let bars = [
        KlineSnapshot {
            timestamp: 1_774_368_000,
            open: 10.10,
            high: 10.10,
            low: 10.10,
            close: 10.10,
            volume: 1.0,
            amount: 10.10,
        },
        KlineSnapshot {
            timestamp: 1_774_540_800,
            open: 10.02,
            high: 10.02,
            low: 10.02,
            close: 10.02,
            volume: 2.0,
            amount: 20.04,
        },
    ];
    let packet = encode_kline_packet(
        "日线",
        &InstrumentId::parse("SH600000").unwrap(),
        "浦发银行",
        &bars,
        6,
        0,
    )
    .unwrap();

    assert_eq!(packet.len(), 200 + 2 * 32);
    assert_eq!(&packet[0..4], &[0xe5, 0x65, 0xbf, 0x7e]);
    assert_eq!(i32::from_le_bytes(packet[20..24].try_into().unwrap()), 64);
    assert_eq!(i32::from_le_bytes(packet[24..28].try_into().unwrap()), 2);
    assert_eq!(
        &packet[28..44],
        &[
            0x53, 0, 0x48, 0, 0x36, 0, 0x30, 0, 0x30, 0, 0x30, 0, 0x30, 0, 0x30, 0
        ]
    );
    assert!(packet[116..178].iter().all(|byte| *byte == 0));
    assert_eq!(u32::from_le_bytes(packet[178..182].try_into().unwrap()), 2);
    assert!(packet[182..191].iter().all(|byte| *byte == 0));
    assert_eq!(u32::from_le_bytes(packet[191..195].try_into().unwrap()), 6);
    assert_eq!(packet[195], 0);
    assert_eq!(
        u32::from_le_bytes(packet[200..204].try_into().unwrap()),
        1_774_368_000
    );
    assert_eq!(
        f32::from_le_bytes(packet[216..220].try_into().unwrap()),
        10.10
    );
    assert_eq!(
        f32::from_le_bytes(packet[228..232].try_into().unwrap()),
        0.0
    );
}

#[test]
#[allow(clippy::chunks_exact_to_as_chunks)]
fn complete_kline_packets_match_captured_wine_answers() {
    let fixtures = [
        (
            "日线",
            4,
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../windows_debug/kline_full_probe_20260901/fixtures/",
                "wine_daily_SH600000_3.bin"
            ))
            .as_slice(),
        ),
        (
            "5分钟线",
            5,
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../windows_debug/kline_full_probe_20260901/fixtures/",
                "wine_five_minute_SH600000_3.bin"
            ))
            .as_slice(),
        ),
        (
            "1分钟线",
            6,
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../windows_debug/kline_full_probe_20260901/fixtures/",
                "wine_one_minute_SH600000_3.bin"
            ))
            .as_slice(),
        ),
    ];
    let instrument = InstrumentId::parse("SH600000").unwrap();

    for (packet_type, ask_id, captured) in fixtures {
        assert_eq!(captured.len(), 200 + 3 * 32);
        let bars = captured[200..]
            .chunks_exact(32)
            .map(|record| KlineSnapshot {
                timestamp: u32::from_le_bytes(record[0..4].try_into().unwrap()),
                open: f32::from_le_bytes(record[4..8].try_into().unwrap()),
                high: f32::from_le_bytes(record[8..12].try_into().unwrap()),
                low: f32::from_le_bytes(record[12..16].try_into().unwrap()),
                close: f32::from_le_bytes(record[16..20].try_into().unwrap()),
                volume: f32::from_le_bytes(record[20..24].try_into().unwrap()),
                amount: f32::from_le_bytes(record[24..28].try_into().unwrap()),
            })
            .collect::<Vec<_>>();

        assert_eq!(
            encode_kline_packet(packet_type, &instrument, "浦发银行", &bars, ask_id, 0,).unwrap(),
            captured,
            "{packet_type} differs from the complete Wine answer"
        );
    }
}

#[test]
fn kline_encoder_rejects_an_empty_batch() {
    let instrument = InstrumentId::parse("SH600000").unwrap();
    assert!(encode_kline_packet("日线", &instrument, "浦发银行", &[], 1, 0).is_err());
}
