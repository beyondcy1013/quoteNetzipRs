from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("analyze-official-5188-residue-controls.py")
SPEC = importlib.util.spec_from_file_location("residue_controls", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ResidueControlsTests(unittest.TestCase):
    def test_reports_offset_five_and_clean_bit_five_controls(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "bad.payload.bin").write_bytes(b"bad")
            (root / "clean.trace.json").write_text(
                json.dumps(
                    [
                        {
                            "mask": 0xC0,
                            "header": 7,
                            "baseline_mode": "absolute",
                            "record_start": 5,
                        }
                    ]
                )
            )
            manifest = root / "manifest.json"
            manifest.write_text(
                json.dumps(
                    [
                        {
                            "payload_file": "bad.payload.bin",
                            "decoded_value_error": "record 2 mask=0xc0 header=0x07 baseline_mode=absolute stage=ladder_volumes bit=10",
                            "decoded_value_residue_bit_offset": 5,
                            "decoded_value_completed_prefix_bits": 8,
                            "payload_len": 3,
                            "delta_index_count": 2,
                            "decoded_value_trace_file": None,
                        },
                        {
                            "payload_file": "clean.payload.bin",
                            "decoded_value_error": None,
                            "decoded_value_trace_file": "clean.trace.json",
                        },
                    ]
                )
            )
            failures = root / "failures.json"
            failures.write_text(
                json.dumps(
                    [
                        {
                            "file": "bad.payload.bin",
                            "error": "record 2 mask=0xc0 header=0x07 baseline_mode=absolute stage=ladder_volumes bit=10",
                        }
                    ]
                )
            )
            result = MODULE.analyze(manifest, failures)
        self.assertEqual(len(result["residue_offset_5_failures"]), 1)
        row = result["residue_offset_5_failures"][0]
        self.assertEqual(row["clean_same_shape_bit5"], 1)


if __name__ == "__main__":
    unittest.main()
