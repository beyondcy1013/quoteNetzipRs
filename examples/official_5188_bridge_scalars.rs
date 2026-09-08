//! Isolated 0x49ad20 scalar bridge comparison, not a public quote producer.
//! Explicit metadata is conditional; scalar amount modes do not implement the full state bridge.
use serde_json::json;

fn bridge_scalars(timestamp: u32, volume: i64) -> (u32, i32) {
    let day = timestamp / 86_400;
    let seconds = timestamp % 86_400;
    let timestamp = if seconds >= 25_200 {
        day * 86_400 + 25_200
    } else {
        timestamp
    };
    // 0x49af61/0x49af67 copy one dword, not the full internal accumulator.
    (timestamp, volume as i32)
}

fn oem_volume(volume_word: i32) -> f32 {
    // 0x4a79a0..0x4a79c1 adds 2^32 for the sign bit before float32 rounding.
    (volume_word as u32) as f32
}

fn direct_amount(amount: i64) -> (i32, f32, bool) {
    let rounded = amount as f32;
    // The native helper preserves a negative integer's two's-complement bits.
    // Positive i64::MAX rounds to 2^63, which remains valid in the unsigned path.
    let integer = if rounded < 0.0 {
        (rounded as i64) as u64
    } else {
        rounded as u64
    };
    let word = if integer <= 0x7fff_ffff {
        integer as u32
    } else {
        (integer >> 16) as u32 | 0x8000_0000
    };
    let value = if word & 0x8000_0000 != 0 {
        u64::from(word & 0x7fff_ffff) << 16
    } else {
        u64::from(word)
    };
    (word as i32, value as f32, false)
}

fn mode1_amount(amount: i64, volume: u32) -> (i32, f32, bool) {
    let mut invalid_conversion = false;
    let packed = if volume == 0 {
        0
    } else {
        let value = ((amount as f32 / volume as f32) - 1000.0) * 100.0 + 0.5;
        // Preserve x86 CVTTSS2SI's indefinite result, not Rust cast saturation.
        if !value.is_finite() || !(-2_147_483_648.0..2_147_483_648.0).contains(&value) {
            invalid_conversion = true;
            i32::MIN
        } else {
            value as i32
        }
    };
    // Same mode1 readback formula as netzip-supplement::decode_wine_realtime_amount.
    (
        packed,
        (packed as f32 / 100.0 + 1000.0) * volume as f32,
        invalid_conversion,
    )
}

fn mode2_amount(amount: i64, volume: u32, close: i32, bits: u8) -> (i32, f32, bool) {
    let low = 10_u32.pow(u32::from(bits & 3)) as f32;
    let high_exp = (bits >> 5) & 7;
    let high = if high_exp <= 6 {
        10_u32.pow(u32::from(high_exp)) as f32
    } else {
        1.0
    };
    let value = (((amount as f32 / volume as f32) / low * high) - close as f32) * 30.0 + 0.5;
    let invalid = volume != 0
        && (!value.is_finite() || !(-2_147_483_648.0..2_147_483_648.0).contains(&value));
    let packed = if volume == 0 {
        0
    } else if invalid {
        i32::MIN
    } else {
        value as i32
    };
    let readback = ((packed as f32 / 30.0 + close as f32) * volume as f32) * low / high;
    (packed, readback, invalid)
}

fn category9_quantities(volume: i64, delta: i64, book: [i32; 10]) -> (i64, i64, [i32; 10]) {
    let delta = delta.wrapping_add(if delta > 0 { 50 } else { -50 }) / 100;
    (
        volume.wrapping_add(50) / 100,
        delta,
        book.map(|value| value.wrapping_add(50) / 100),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if !matches!(args.len(), 3 | 6 | 7 | 9) {
        return Err("usage: official_5188_bridge_scalars TIMESTAMP VOLUME_I64 [AMOUNT_I64 DELTA_I64 CATEGORY [MODE [CLOSE_I32 SCALE_BITS]]]".into());
    }
    let timestamp: u32 = args[1].parse()?;
    let volume: i64 = args[2].parse()?;
    let amount = args.get(3).map(|v| v.parse::<i64>()).transpose()?;
    let delta = args.get(4).map(|v| v.parse::<i64>()).transpose()?;
    let category = args.get(5).map(|v| v.parse::<u32>()).transpose()?;
    let (adjusted_volume, adjusted_delta) = if category == Some(9) {
        let (volume, delta, _) = category9_quantities(volume, delta.unwrap_or(0), [0; 10]);
        (volume, Some(delta))
    } else {
        (volume, delta)
    };
    let (bridge_timestamp, bridge_volume) = bridge_scalars(timestamp, adjusted_volume);
    let mode = args.get(6).map(|v| v.parse::<u8>()).transpose()?;
    if mode.is_some_and(|value| !matches!(value, 0..=2)) {
        return Err("only explicit conditional amount modes 0, 1 and 2 are implemented".into());
    }
    let converted_amount = if mode == Some(2) {
        Some(mode2_amount(
            amount.ok_or("amount required")?,
            bridge_volume as u32,
            args.get(7).ok_or("mode2 requires close")?.parse()?,
            args.get(8).ok_or("mode2 requires scale bits")?.parse()?,
        ))
    } else if mode == Some(0) {
        if args.len() == 9 {
            return Err("close/scale bits require mode2".into());
        }
        Some(direct_amount(amount.ok_or("amount required")?))
    } else if mode == Some(1) {
        if args.len() == 9 {
            return Err("close/scale bits require mode2".into());
        }
        Some(mode1_amount(
            amount.ok_or("amount required")?,
            bridge_volume as u32,
        ))
    } else {
        None
    };
    println!(
        "{}",
        json!({
            "experimental": true, "data_class": "noncanonical",
            "native_session_parity": false, "category9_preprocessing": category == Some(9),
            "category": category, "category_provenance": "caller_supplied_not_verified",
            "source_timestamp": timestamp, "source_volume_i64": volume,
            "bridge_timestamp": bridge_timestamp, "bridge_volume_i32": bridge_volume,
            "oem_volume_f32": oem_volume(bridge_volume),
            "bridge_amount_f32": amount.map(|value| value as f32),
            "adjusted_delta_i64": adjusted_delta,
            "amount_mode": mode,
            "packed_amount_i32": converted_amount.map(|value| value.0),
            "oem_amount": converted_amount.map(|value| value.1),
            "invalid_conversion": converted_amount.map(|value| value.2),
            "scope": "scalar slices only; not the full 311B-to-256B or OEM conversion"
        })
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_direct_amount_edges() {
        assert_eq!(super::direct_amount(-1), (-1, 140737488355328.0, false));
        assert_eq!(super::direct_amount(-12345), (-1, 140737488355328.0, false));
        assert_eq!(super::direct_amount(i64::MAX), (i32::MIN, 0.0, false));
        assert_eq!(super::direct_amount(12345), (12345, 12345.0, false));
    }
    use super::{bridge_scalars, category9_quantities, mode1_amount, mode2_amount, oem_volume};

    #[test]
    fn mode2_zero_volume_and_overflow() {
        assert_eq!(mode2_amount(12345, 0, 873, 0), (0, 0.0, false));
        let (packed, _, invalid) = mode2_amount(i64::MAX, 1, 873, 0);
        assert_eq!(packed, i32::MIN);
        assert!(invalid);
    }

    #[test]
    fn native_mode1_amount_vectors() {
        assert_eq!(mode1_amount(12345, 0), (0, 0.0, false));
        assert_eq!(mode1_amount(164124, 188), (-12699, 164125.875, false));
        assert_eq!(mode1_amount(100000001, 12345), (710045, 100000056.0, false));
        assert_eq!(mode1_amount(i64::MAX, 1), (i32::MIN, -21473836.0, true));
        assert_eq!(mode1_amount(i64::MIN, 1), (i32::MIN, -21473836.0, true));
    }

    #[test]
    fn native_category9_quantities() {
        let book = [-151, -150, -100, -51, -50, -1, 0, 1, 49, 50];
        for (volume, delta, expected_volume, expected_delta) in [
            (1, 1, 0, 0),
            (49, 49, 0, 0),
            (50, 50, 1, 1),
            (-150, -150, -1, -2),
            (0, 0, 0, 0),
        ] {
            assert_eq!(
                category9_quantities(volume, delta, book),
                (
                    expected_volume,
                    expected_delta,
                    [-1, -1, 0, 0, 0, 0, 0, 0, 0, 1]
                )
            );
        }
    }

    #[test]
    fn native_unsigned_oem_volume() {
        for (word, expected) in [
            (0, 0.0),
            (17, 17.0),
            (16_777_217, 16_777_216.0),
            (i32::MIN, 2_147_483_648.0),
            (-1, 4_294_967_296.0),
        ] {
            assert_eq!(oem_volume(word), expected);
        }
    }

    #[test]
    fn native_volume_vectors() {
        for (input, expected) in [
            (0, 0),
            (4_294_967_313, 17),
            (2_147_483_648, i32::MIN),
            (-520_882_088, -520_882_088),
        ] {
            assert_eq!(bridge_scalars(3600, input), (3600, expected));
        }
    }

    #[test]
    fn native_timestamp_vectors() {
        for (input, expected) in [
            (25199, 25199),
            (25200, 25200),
            (57600, 25200),
            (86399, 25200),
            (86400, 86400),
            (u32::MAX, u32::MAX),
        ] {
            assert_eq!(bridge_scalars(input, 17), (expected, 17));
        }
    }
}
