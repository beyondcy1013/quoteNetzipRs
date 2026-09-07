from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("verify-official-5188-partition-summary.py")
SPEC = importlib.util.spec_from_file_location("partition_verifier", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
VERIFIER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFIER)


class PartitionSummaryVerifierTests(unittest.TestCase):
    def write_summary(self, document: object) -> Path:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        path = Path(directory.name) / "summary.json"
        path.write_text(json.dumps(document), encoding="utf-8")
        return path

    def test_accepts_unique_flagged_entries_and_reports_continuity(self) -> None:
        path = self.write_summary(
            [
                {"entries": 2, "ranges": [["SH", 65536, 2]]},
                {"entries": 1, "ranges": [["SH", 65538, 1]]},
            ]
        )
        result = VERIFIER.validate(path, [2, 1])
        self.assertEqual(result["total_entries"], 3)
        self.assertEqual(result["unique_entries"], 3)
        self.assertEqual(result["boundary_continuity"], [True])

    def test_rejects_declared_count_mismatch(self) -> None:
        path = self.write_summary(
            [{"entries": 2, "ranges": [["SH", 65536, 1]]}]
        )
        with self.assertRaisesRegex(ValueError, "declared 2, expanded 1"):
            VERIFIER.validate(path, None)

    def test_rejects_wire_value_without_flag(self) -> None:
        path = self.write_summary(
            [{"entries": 1, "ranges": [["SZ", 42, 1]]}]
        )
        with self.assertRaisesRegex(ValueError, "lacks 0x10000 flag"):
            VERIFIER.validate(path, None)

    def test_rejects_duplicate_entries_across_partitions(self) -> None:
        path = self.write_summary(
            [
                {"entries": 1, "ranges": [["SZ", 65536, 1]]},
                {"entries": 1, "ranges": [["SZ", 65536, 1]]},
            ]
        )
        with self.assertRaisesRegex(ValueError, "duplicate"):
            VERIFIER.validate(path, None)

    def test_rejects_unexpected_partition_sizes(self) -> None:
        path = self.write_summary(
            [{"entries": 1, "ranges": [["SH", 65536, 1]]}]
        )
        with self.assertRaisesRegex(ValueError, "do not match expected"):
            VERIFIER.validate(path, [1024])


if __name__ == "__main__":
    unittest.main()
