#!/usr/bin/env python3
import importlib.util
import datetime
import pathlib
import sys
import unittest


PATH = pathlib.Path(__file__).with_name("official-5188-open-readiness.py")
SPEC = importlib.util.spec_from_file_location("readiness", PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def status(
    frames: int = 10,
    *,
    missing: int = 0,
    rejected: int = 0,
    errors=None,
    failed: int = 0,
):
    return {
        "authenticated": True,
        "selected_endpoint": "192.0.2.1:5188",
        "initialized": True,
        "connection_count": 10,
        "code_table_count": 4,
        "receive_list_codes": 7177,
        "shadow": {
            "running": True,
            "frames_received": frames,
            "delta_2704_frames": frames,
            "receive_terminations": 0,
            "decoder_attempted_frames": frames,
            "decoder_decoded_frames": frames - failed,
            "decoder_partial_frames": 0,
            "decoder_failed_frames": failed,
            "decoder_error_kinds": errors or {},
            "decoder_decoded_records": 100,
            "decoder_oem_state_updates": 100 - missing - rejected,
            "decoder_missing_previous_close_seeds": missing,
            "decoder_rejected_public_quotes": rejected,
        },
    }


TRADE_DATE = datetime.date(2026, 9, 7)


def quotes(**overrides):
    quote = {
        "timestamp": 1788746400,
        "price": 10.0,
        "last_close": 9.9,
        "open": 9.8,
        "high": 10.1,
        "low": 9.7,
        "volume": 100.0,
        "amount": 1000.0,
        "ask_prices": [0.0] * 10,
        "ask_volumes": [0.0] * 10,
        "bid_prices": [0.0] * 10,
        "bid_volumes": [0.0] * 10,
    }
    quote.update(overrides)
    return {
        "source": "netzipRustOfficial5188Shadow",
        "publication": "disabled",
        "quote_count": 1,
        "quotes": [quote],
    }


class ReadinessTests(unittest.TestCase):
    def test_trade_quotes_ready(self):
        first = status(10)
        first["shadow"]["decoder_decoded_records"] = 90
        first["shadow"]["decoder_oem_state_updates"] = 90
        self.assertTrue(MODULE.evaluate(first, status(11), True, quotes(), TRADE_DATE).ready)

    def test_transport_does_not_require_quote_projection(self):
        self.assertTrue(MODULE.evaluate(status(10), status(11, missing=5), False).ready)

    def test_missing_metadata_blocks_trade_quotes(self):
        first = status(10)
        first["shadow"]["decoder_decoded_records"] = 90
        first["shadow"]["decoder_oem_state_updates"] = 90
        verdict = MODULE.evaluate(first, status(11, missing=5), True, quotes(), TRADE_DATE)
        self.assertFalse(verdict.ready)
        self.assertIn("metadata_ready", verdict.reasons)

    def test_stalled_stream_blocks_readiness(self):
        self.assertIn("stream_advancing", MODULE.evaluate(status(10), status(10), False).reasons)

    def test_semantically_rejected_projection_blocks_trade_quotes(self):
        first = status(10)
        first["shadow"]["decoder_decoded_records"] = 90
        first["shadow"]["decoder_oem_state_updates"] = 90
        verdict = MODULE.evaluate(
            first, status(11, rejected=1), True, quotes(), TRADE_DATE
        )
        self.assertIn("metadata_ready", verdict.reasons)
        self.assertTrue(verdict.checks["record_accounting"])
        self.assertEqual(verdict.metrics["rejected_public_quotes"], 1)

    def test_ladder_failure_still_blocks_strict_trade_gate(self):
        first = status(10)
        first["shadow"]["decoder_decoded_records"] = 90
        first["shadow"]["decoder_oem_state_updates"] = 90
        verdict = MODULE.evaluate(
            first,
            status(11, errors={"ladder_volumes:bitstream_exhausted": 1}, failed=1),
            True,
            quotes(),
            TRADE_DATE,
        )
        self.assertIn("trade_fields_clean", verdict.reasons)

    def test_partial_frame_blocks_trade_quotes(self):
        first = status(10)
        first["shadow"]["decoder_decoded_records"] = 90
        first["shadow"]["decoder_oem_state_updates"] = 90
        second = status(11)
        second["shadow"]["decoder_partial_frames"] = 1
        self.assertIn(
            "trade_fields_clean",
            MODULE.evaluate(first, second, True, quotes(), TRADE_DATE).reasons,
        )

    def test_historical_failure_does_not_poison_clean_interval(self):
        first = status(10, errors={"ohlc:bitstream_exhausted": 4}, failed=4)
        first["shadow"]["decoder_decoded_records"] = 90
        first["shadow"]["decoder_oem_state_updates"] = 90
        second = status(11, errors={"ohlc:bitstream_exhausted": 4}, failed=4)
        self.assertTrue(MODULE.evaluate(first, second, True, quotes(), TRADE_DATE).ready)

    def test_missing_quote_snapshot_blocks_trade_quotes(self):
        first = status(10)
        first["shadow"]["decoder_decoded_records"] = 90
        first["shadow"]["decoder_oem_state_updates"] = 90
        verdict = MODULE.evaluate(first, status(11), True, None, TRADE_DATE)
        self.assertIn("public_quotes_semantically_valid", verdict.reasons)

    def test_wrong_day_timestamp_blocks_trade_quotes(self):
        first = status(10)
        first["shadow"]["decoder_decoded_records"] = 90
        first["shadow"]["decoder_oem_state_updates"] = 90
        verdict = MODULE.evaluate(
            first, status(11), True, quotes(timestamp=1133), TRADE_DATE
        )
        self.assertIn("public_quotes_semantically_valid", verdict.reasons)
        self.assertEqual(verdict.metrics["invalid_public_timestamp_count"], 1)

    def test_negative_or_nonfinite_numeric_value_blocks_trade_quotes(self):
        first = status(10)
        first["shadow"]["decoder_decoded_records"] = 90
        first["shadow"]["decoder_oem_state_updates"] = 90
        verdict = MODULE.evaluate(
            first, status(11), True, quotes(volume=-1.0), TRADE_DATE
        )
        self.assertIn("public_quotes_semantically_valid", verdict.reasons)
        self.assertEqual(verdict.metrics["invalid_public_numeric_count"], 1)
        self.assertEqual(verdict.metrics["negative_public_scalar_count"], 1)

    def test_incoherent_ohlc_blocks_trade_quotes(self):
        first = status(10)
        first["shadow"]["decoder_decoded_records"] = 90
        first["shadow"]["decoder_oem_state_updates"] = 90
        verdict = MODULE.evaluate(
            first, status(11), True, quotes(high=9.0, low=10.0), TRADE_DATE
        )
        self.assertIn("public_quotes_semantically_valid", verdict.reasons)
        self.assertEqual(verdict.metrics["incoherent_public_ohlc_count"], 1)

    def test_broad_price_and_quantity_bounds_block_corrupt_positive_values(self):
        first = status(10)
        first["shadow"]["decoder_decoded_records"] = 90
        first["shadow"]["decoder_oem_state_updates"] = 90
        bad_price = quotes()
        bad_price["quotes"][0]["ask_prices"][0] = 21_000_000.0
        verdict = MODULE.evaluate(first, status(11), True, bad_price, TRADE_DATE)
        self.assertEqual(verdict.metrics["out_of_range_public_price_count"], 1)
        bad_quantity = quotes(volume=1_000_000_000_000.0)
        verdict = MODULE.evaluate(first, status(11), True, bad_quantity, TRADE_DATE)
        self.assertEqual(verdict.metrics["out_of_range_public_quantity_count"], 1)


if __name__ == "__main__":
    unittest.main()
