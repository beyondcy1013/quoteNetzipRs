#!/usr/bin/env python3

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("audit-official-5188-partition-alignment.py")
SPEC = importlib.util.spec_from_file_location("partition_alignment", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class CurrentExtractorFixtureTests(unittest.TestCase):
    def test_decodes_code_table_json(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "0001-0104.code-table.json"
            path.write_text(
                json.dumps(
                    {
                        "market": [83, 72],
                        "records": [
                            {"symbol_index": 7, "code": "600000"},
                            {"symbol_index": 8, "code": ""},
                        ],
                    }
                )
            )
            self.assertEqual(
                MODULE.decode_0104_table(str(path)),
                {"market": "SH", "codes": [(7, "600000"), (8, "")]},
            )

    def test_rejects_misaligned_current_payload(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "0001-2a10.payload.bin"
            path.write_bytes(b"\0" * 11)
            with self.assertRaisesRegex(ValueError, "invalid 2a10 payload length"):
                MODULE.load_2a10_frames(directory)


if __name__ == "__main__":
    unittest.main()
