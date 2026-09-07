from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("analyze-official-5188-ladder-volume-trace-diff.py")
SPEC = importlib.util.spec_from_file_location("ladder_volume_trace_diff", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


SUCCESS_ERROR = (
    "5188 value record 6 mask=0xe8 mask_class=0x28 header=0x03 "
    "clear_ladder=true raw_level_count=0 level_count=0 baseline_mode=absolute "
    "stored_present=true baseline_present=false mask_has_bit0=false "
    "stored_source=resolver_record committed_state_present=true "
    "current_last_before=0 stored_last_before=1552 committed_last_before=1552 "
    "baseline_last_before=none record_start=296 stage=ladder_volumes bit=446: "
    "market=[83, 72] symbol_index=24548 move_code=0 move_bits=374..374 "
    "ladder_bits=374..392 layout=0x2d flags=0x00 price_mask=0x0e4 "
    "anchor=E:0 volume_slot=5 volume_mask=0x3ff baseline_ladder=none: "
    "5188 bitstream exhausted at bit 446: need 8, have 2"
)


class LadderVolumeTraceDiffTests(unittest.TestCase):
    def test_parse_error_extracts_lifecycle_and_need_have(self) -> None:
        parsed = MODULE.parse_error(SUCCESS_ERROR)
        assert parsed is not None
        self.assertEqual(parsed["mask"], 0xE8)
        self.assertEqual(parsed["header"], 0x03)
        self.assertEqual(parsed["baseline_mode"], "absolute")
        self.assertEqual(parsed["ladder_layout"], 0x2D)
        self.assertEqual(parsed["ladder_volume_mask"], 0x3FF)
        self.assertEqual(parsed["need_bits"], 8)
        self.assertEqual(parsed["have_bits"], 2)
        self.assertEqual(parsed["current_last_before"], 0)
        self.assertEqual(parsed["stored_last_before"], 1552)
        self.assertFalse(parsed["mask_has_bit0"])
        self.assertTrue(parsed["committed_state_present"])

    def test_same_key_success_and_failure_are_paired(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "ok.value-trace.json").write_text(
                json.dumps(
                    [
                        {
                            "record_index": 0,
                            "market": [83, 72],
                            "symbol_index": 1,
                            "mask": 0xE8,
                            "header": 3,
                            "baseline_mode": "absolute",
                            "stored_present": True,
                            "baseline_present": False,
                            "mask_class": 0x28,
                            "clear_ladder": True,
                            "raw_level_count": 0,
                            "level_count": 0,
                            "record_start": 0,
                            "header_end": 13,
                            "timestamp_end": 13,
                            "prefix_values_end": 80,
                            "ladder_move_bits": [80, 80],
                            "ladder_values_bits": [80, 84],
                            "ladder_layout": 45,
                            "ladder_flags": 0,
                            "ladder_price_mask": 0,
                            "ladder_anchor_value": 999999,
                            "ladder_anchor_operation": 69,
                            "ladder_volume_mask": 1023,
                            "ladder_volumes_end": 220,
                            "trailing_da_end": 221,
                            "aligned_end": 224,
                        }
                    ]
                )
            )
            (root / "bad.value-trace.json").write_text(
                json.dumps(
                    [
                        {
                            "record_index": 0,
                            "market": [83, 72],
                            "symbol_index": 2,
                            "mask": 128,
                            "header": 3,
                            "baseline_mode": "relative",
                            "stored_present": True,
                            "baseline_present": True,
                            "mask_class": 0,
                            "clear_ladder": True,
                            "raw_level_count": 0,
                            "level_count": 0,
                            "record_start": 0,
                            "header_end": 13,
                            "timestamp_end": 13,
                            "prefix_values_end": 32,
                            "ladder_move_bits": [32, 33],
                            "ladder_values_bits": [33, 40],
                            "ladder_layout": 15,
                            "ladder_volume_mask": 0,
                            "ladder_volumes_end": 40,
                            "trailing_da_end": 41,
                            "aligned_end": 48,
                        }
                    ]
                )
            )
            (root / "manifest.json").write_text(
                json.dumps(
                    [
                        {
                            "wire_kind": "2704",
                            "frame_index": 1,
                            "src": "a:5188",
                            "dst": "b:1",
                            "payload_file": "ok.payload.bin",
                            "decoded_value_trace_file": "ok.value-trace.json",
                            "decoded_value_error": None,
                            "omitted_tail_records": 0,
                            "decoded_value_stream_bits": 224,
                            "decoded_value_remaining_bits": 0,
                            "decoded_value_residue_bit_offset": 0,
                            "delta_index_count": 1,
                        },
                        {
                            "wire_kind": "2704",
                            "frame_index": 2,
                            "src": "a:5188",
                            "dst": "b:1",
                            "payload_file": "bad.payload.bin",
                            "decoded_value_trace_file": "bad.value-trace.json",
                            "decoded_value_error": SUCCESS_ERROR,
                            "omitted_tail_records": 0,
                            "decoded_value_stream_bits": 448,
                            "decoded_value_remaining_bits": 0,
                            "decoded_value_residue_bit_offset": 0,
                            "delta_index_count": 7,
                        },
                    ]
                )
            )
            report = MODULE.analyze(root)
        bucket = report["priority_bucket"]
        self.assertEqual(bucket["success_count"], 1)
        self.assertEqual(bucket["failure_count"], 1)
        self.assertEqual(report["frame_level"]["clean"], 1)
        self.assertEqual(report["frame_level"]["partial"], 1)
        self.assertEqual(report["short_tail"]["omitted_records"], 0)
        failure = bucket["failure_samples"][0]
        self.assertEqual(failure["need_bits"], 8)
        self.assertEqual(failure["have_bits"], 2)
        self.assertEqual(failure["ladder_layout"], 0x2D)
        self.assertIn("cause", failure["classification"])
        self.assertEqual(report["publication"], "disabled")

    def test_absolute_baseline_present_false_is_not_treated_as_lifecycle_cause(self) -> None:
        fail = {
            "value_stream_bits": 448,
            "ladder_volumes_end": 446,
            "have_bits": 2,
            "need_bits": 8,
            "record_start": 296,
            "previous_record_aligned_end": 296,
            "record_index": 6,
            "delta_index_count": 7,
            "omitted_tail_records": 0,
            "token_prefix_mismatch": False,
            "bitstream_exhausted": True,
            "baseline_mode": "absolute",
            "baseline_present": False,
            "mask_has_bit0": False,
            "committed_state_present": True,
            "stored_present": True,
            "level_count": 0,
            "current_last_before": 0,
            "ladder_price_mask": 0x0E4,
            "prefix_bits": 78,
        }
        result = MODULE.classify_failure(
            fail,
            {"aligned_width": [224], "prefix_bits": [80, 70, 90]},
            __import__("collections").Counter({0x0E4: 3}),
        )
        self.assertNotEqual(result["cause"], "previous_record_state_lifecycle")
        self.assertTrue(result["counter"]["absolute_baseline_present_is_tautological"])

    def test_prefix_overread_vs_true_short_payload(self) -> None:
        overread = MODULE.classify_prefix_consumption(400, 200.0, 160, 80, 168)
        self.assertEqual(overread["cause"], "prefix_overread")
        short = MODULE.classify_prefix_consumption(150, 160.0, 148, 80, 168)
        self.assertEqual(short["cause"], "true_short_payload")
        ambiguous = MODULE.classify_prefix_consumption(200, 200.0, 160, 200, 168)
        self.assertEqual(ambiguous["cause"], "ambiguous")


if __name__ == "__main__":
    unittest.main()
