#!/usr/bin/env python3
"""Same-key success/failure trace diff for 2704 ladder_volumes errors.

Read-only: consumes an official_5188_extract directory. It does not change
decoder semantics, token tables, or mask interpretation.
"""

from __future__ import annotations

import argparse
import collections
import json
import re
import statistics
from pathlib import Path
from typing import Any


PRIORITY_KEY = (0xE8, 0x03, "absolute", 0x2D, 0x3FF)
POLLUTION_TARGETS = (
    {
        "label": "SZ301136",
        "ordinal": 43,
        "symbol_index": 3882,
        "market": [83, 90],
        "expected_mask": 0xF8,
        "expected_bits": (776, 821),
    },
    {
        "label": "SH603696",
        "ordinal": 836,
        "symbol_index": 25095,
        "market": [83, 72],
        "expected_mask": 0x20,
        "expected_bits": (584, 744),
    },
    {
        "label": "SZ301068",
        "ordinal": 7644,
        "symbol_index": 3821,
        "market": [83, 90],
        "expected_mask": 0x36,
        "expected_bits": (1984, 2124),
    },
)

ERROR_RE = re.compile(
    r"record (?P<record_index>\d+) "
    r"mask=(?P<mask>0x[0-9a-f]+) "
    r"(?:mask_class=(?P<mask_class>0x[0-9a-f]+) )?"
    r"header=(?P<header>0x[0-9a-f]+) "
    r"(?:clear_ladder=(?P<clear_ladder>true|false) )?"
    r"(?:raw_level_count=(?P<raw_level_count>\d+) )?"
    r"(?:level_count=(?P<level_count>\d+) )?"
    r"baseline_mode=(?P<baseline_mode>\w+) "
    r"(?:stored_present=(?P<stored_present>true|false) )?"
    r"(?:baseline_present=(?P<baseline_present>true|false) )?"
    r"(?:mask_has_bit0=(?P<mask_has_bit0>true|false) )?"
    r"(?:stored_source=(?P<stored_source>\w+) )?"
    r"(?:committed_state_present=(?P<committed_state_present>true|false) )?"
    r"(?:current_last_before=(?P<current_last_before>-?\d+) )?"
    r"(?:stored_last_before=(?P<stored_last_before>none|-?\d+) )?"
    r"(?:committed_last_before=(?P<committed_last_before>none|-?\d+) )?"
    r"(?:baseline_last_before=(?P<baseline_last_before>none|-?\d+) )?"
    r"(?:record_start=(?P<record_start>\d+) )?"
    r"stage=(?P<stage>\w+) "
    r"bit=(?P<bit>\d+)"
)
LADDER_RE = re.compile(
    r"market=\[(?P<m0>\d+), (?P<m1>\d+)\] "
    r"symbol_index=(?P<symbol_index>\d+) "
    r"move_code=(?P<move_code>-?\d+) "
    r"move_bits=(?P<move_start>\d+)\.\.(?P<move_end>\d+) "
    r"ladder_bits=(?P<ladder_start>\d+)\.\.(?P<ladder_end>\d+) "
    r"layout=(?P<layout>0x[0-9a-f]+) "
    r"flags=(?P<flags>0x[0-9a-f]+) "
    r"price_mask=(?P<price_mask>0x[0-9a-f]+) "
    r"anchor=(?P<anchor_op>.)[:=](?P<anchor_value>-?\d+) "
    r"volume_slot=(?P<volume_slot>\d+) "
    r"volume_mask=(?P<volume_mask>0x[0-9a-f]+)"
)
NEED_HAVE_RE = re.compile(r"need (?P<need>\d+), have (?P<have>\d+)")


def parse_optional_int(value: str | None) -> int | None:
    if value in (None, "none"):
        return None
    return int(value)


def parse_optional_bool(value: str | None) -> bool | None:
    if value is None:
        return None
    return value == "true"


def parse_error(message: str) -> dict[str, Any] | None:
    match = ERROR_RE.search(message)
    if match is None:
        return None
    row: dict[str, Any] = match.groupdict()
    row["record_index"] = int(row["record_index"])
    row["mask"] = int(row["mask"], 16)
    row["header"] = int(row["header"], 16)
    row["bit"] = int(row["bit"])
    if row.get("mask_class"):
        row["mask_class"] = int(row["mask_class"], 16)
    else:
        row["mask_class"] = row["mask"] & 0x38
    for name in ("raw_level_count", "level_count", "record_start"):
        row[name] = int(row[name]) if row.get(name) is not None else None
    for name in (
        "clear_ladder",
        "stored_present",
        "baseline_present",
        "mask_has_bit0",
        "committed_state_present",
    ):
        row[name] = parse_optional_bool(row.get(name))
    if row["mask_has_bit0"] is None:
        row["mask_has_bit0"] = row["mask"] & 1 != 0
    for name in (
        "current_last_before",
        "stored_last_before",
        "committed_last_before",
        "baseline_last_before",
    ):
        row[name] = parse_optional_int(row.get(name))
    ladder = LADDER_RE.search(message)
    if ladder is not None:
        payload = ladder.groupdict()
        row["market"] = [int(payload["m0"]), int(payload["m1"])]
        row["symbol_index"] = int(payload["symbol_index"])
        row["move_code"] = int(payload["move_code"])
        row["ladder_move_bits"] = [int(payload["move_start"]), int(payload["move_end"])]
        row["ladder_values_bits"] = [int(payload["ladder_start"]), int(payload["ladder_end"])]
        row["ladder_layout"] = int(payload["layout"], 16)
        row["ladder_flags"] = int(payload["flags"], 16)
        row["ladder_price_mask"] = int(payload["price_mask"], 16)
        row["ladder_anchor_operation"] = ord(payload["anchor_op"])
        row["ladder_anchor_value"] = int(payload["anchor_value"])
        row["volume_slot"] = int(payload["volume_slot"])
        row["ladder_volume_mask"] = int(payload["volume_mask"], 16)
    need = NEED_HAVE_RE.search(message)
    if need is not None:
        row["need_bits"] = int(need.group("need"))
        row["have_bits"] = int(need.group("have"))
    row["token_prefix_mismatch"] = "token prefix did not match" in message
    row["bitstream_exhausted"] = "bitstream exhausted" in message
    row["rejected_stage"] = row["stage"]
    return row


def key_tuple(
    mask: int,
    header: int,
    baseline_mode: str,
    layout: int | None,
    volume_mask: int | None,
) -> tuple[int, int, str, int | None, int | None]:
    return (mask, header, baseline_mode, layout, volume_mask)


def key_text(key: tuple[int, int, str, int | None, int | None]) -> str:
    layout = "none" if key[3] is None else f"{key[3]:#04x}"
    volume = "none" if key[4] is None else f"{key[4]:#05x}"
    return (
        f"mask={key[0]:#04x} header={key[1]:#04x} baseline_mode={key[2]} "
        f"layout={layout} volume_mask={volume}"
    )


def load_json(path: Path) -> Any:
    return json.loads(path.read_text())


def i32_at(record: list[int], offset: int) -> int:
    return int.from_bytes(bytes(record[offset : offset + 4]), "little", signed=True)


def last_from_decoded(record: list[int]) -> int:
    return i32_at(record, 0x10)


def span_end(span: list[int] | tuple[int, int] | None) -> int | None:
    if not span:
        return None
    return int(span[1])


def snapshot_from_trace(
    trace: dict[str, Any],
    frame: dict[str, Any],
    previous: dict[str, Any] | None,
    previous_same_symbol: dict[str, Any] | None,
) -> dict[str, Any]:
    values = trace.get("ladder_values_bits")
    volumes_end = trace.get("ladder_volumes_end")
    header_end = int(trace["header_end"])
    timestamp_end = int(trace["timestamp_end"])
    prefix_end = int(trace["prefix_values_end"])
    record_start = int(trace["record_start"])
    aligned_end = int(trace["aligned_end"])
    mask = int(trace["mask"])
    layout = trace.get("ladder_layout")
    volume_mask = trace.get("ladder_volume_mask")
    return {
        "outcome": "success",
        "frame_index": frame.get("frame_index"),
        "src": frame.get("src"),
        "dst": frame.get("dst"),
        "payload_file": frame.get("payload_file"),
        "trace_file": frame.get("decoded_value_trace_file"),
        "record_index": trace.get("record_index"),
        "slot": None,
        "market": trace.get("market"),
        "symbol_index": trace.get("symbol_index"),
        "mask": mask,
        "header": trace.get("header"),
        "baseline_mode": trace.get("baseline_mode"),
        "mask_class": trace.get("mask_class", mask & 0x38),
        "mask_has_bit0": trace.get("mask_has_bit0", mask & 1 != 0),
        "stored_present": trace.get("stored_present"),
        "baseline_present": trace.get("baseline_present"),
        "stored_source": trace.get("stored_source"),
        "committed_state_present": trace.get("committed_state_present"),
        "clear_ladder": trace.get("clear_ladder"),
        "raw_level_count": trace.get("raw_level_count"),
        "level_count": trace.get("level_count"),
        "current_last_before": trace.get("current_last_before"),
        "stored_last_before": trace.get("stored_last_before"),
        "committed_last_before": trace.get("committed_last_before"),
        "baseline_last_before": trace.get("baseline_last_before"),
        "anchor_source": trace.get("anchor_source"),
        "record_start": record_start,
        "header_end": header_end,
        "timestamp_start": header_end,
        "timestamp_end": timestamp_end,
        "prefix_values_start": timestamp_end,
        "prefix_values_end": prefix_end,
        "ladder_move_start": None if not trace.get("ladder_move_bits") else trace["ladder_move_bits"][0],
        "ladder_move_end": None if not trace.get("ladder_move_bits") else trace["ladder_move_bits"][1],
        "ladder_values_start": None if not values else values[0],
        "ladder_values_end": None if not values else values[1],
        "volume_mask_start": None if not values else values[1],
        "volume_mask_end": None if not values else values[1],
        "ladder_volumes_start": None if not values else values[1],
        "ladder_volumes_end": volumes_end,
        "trailing_da_start": volumes_end,
        "trailing_da_end": trace.get("trailing_da_end"),
        "aligned_end": aligned_end,
        "value_stream_bits": frame.get("decoded_value_stream_bits"),
        "need_bits": None,
        "have_bits": None,
        "previous_record_aligned_end": None if previous is None else previous.get("aligned_end"),
        "previous_record_success": None if previous is None else True,
        "previous_same_symbol_last": None
        if previous_same_symbol is None
        else previous_same_symbol.get("last"),
        "prefix_bits": prefix_end - record_start,
        "ladder_values_bits_width": None if not values else values[1] - values[0],
        "ladder_volumes_bits_width": None
        if not values or volumes_end is None
        else volumes_end - values[1],
        "aligned_width": aligned_end - record_start,
        "ladder_layout": layout,
        "ladder_flags": trace.get("ladder_flags"),
        "ladder_price_mask": trace.get("ladder_price_mask"),
        "ladder_anchor_value": trace.get("ladder_anchor_value"),
        "ladder_anchor_operation": trace.get("ladder_anchor_operation"),
        "ladder_volume_mask": volume_mask,
        "ladder_move_code": trace.get("ladder_move_code"),
        "omitted_tail_records": frame.get("omitted_tail_records") or 0,
        "residue_bit_offset": frame.get("decoded_value_residue_bit_offset"),
        "remaining_bits": frame.get("decoded_value_remaining_bits"),
        "delta_index_count": frame.get("delta_index_count"),
        "error": None,
    }


def snapshot_from_error(
    parsed: dict[str, Any],
    frame: dict[str, Any],
    previous: dict[str, Any] | None,
    previous_same_symbol: dict[str, Any] | None,
) -> dict[str, Any]:
    values = parsed.get("ladder_values_bits")
    record_start = parsed.get("record_start")
    header_end = None if record_start is None else record_start + 13
    move = parsed.get("ladder_move_bits")
    return {
        "outcome": "failure",
        "frame_index": frame.get("frame_index"),
        "src": frame.get("src"),
        "dst": frame.get("dst"),
        "payload_file": frame.get("payload_file"),
        "trace_file": frame.get("decoded_value_trace_file"),
        "record_index": parsed.get("record_index"),
        "slot": parsed.get("volume_slot"),
        "market": parsed.get("market"),
        "symbol_index": parsed.get("symbol_index"),
        "mask": parsed.get("mask"),
        "header": parsed.get("header"),
        "baseline_mode": parsed.get("baseline_mode"),
        "mask_class": parsed.get("mask_class"),
        "mask_has_bit0": parsed.get("mask_has_bit0"),
        "stored_present": parsed.get("stored_present"),
        "baseline_present": parsed.get("baseline_present"),
        "stored_source": parsed.get("stored_source"),
        "committed_state_present": parsed.get("committed_state_present"),
        "clear_ladder": parsed.get("clear_ladder"),
        "raw_level_count": parsed.get("raw_level_count"),
        "level_count": parsed.get("level_count"),
        "current_last_before": parsed.get("current_last_before"),
        "stored_last_before": parsed.get("stored_last_before"),
        "committed_last_before": parsed.get("committed_last_before"),
        "baseline_last_before": parsed.get("baseline_last_before"),
        "anchor_source": parsed.get("anchor_source"),
        "record_start": record_start,
        "header_end": header_end,
        "timestamp_start": header_end,
        "timestamp_end": None if not move else move[0],
        "prefix_values_start": header_end,
        "prefix_values_end": None if not move else move[0],
        "ladder_move_start": None if not move else move[0],
        "ladder_move_end": None if not move else move[1],
        "ladder_values_start": None if not values else values[0],
        "ladder_values_end": None if not values else values[1],
        "volume_mask_start": None if not values else values[1],
        "volume_mask_end": None if not values else values[1],
        "ladder_volumes_start": None if not values else values[1],
        "ladder_volumes_end": parsed.get("bit"),
        "trailing_da_start": None,
        "trailing_da_end": None,
        "aligned_end": None,
        "value_stream_bits": frame.get("decoded_value_stream_bits"),
        "need_bits": parsed.get("need_bits"),
        "have_bits": parsed.get("have_bits"),
        "previous_record_aligned_end": None if previous is None else previous.get("aligned_end"),
        "previous_record_success": None if previous is None else True,
        "previous_same_symbol_last": None
        if previous_same_symbol is None
        else previous_same_symbol.get("last"),
        "prefix_bits": None
        if record_start is None or not move
        else move[0] - record_start,
        "ladder_values_bits_width": None if not values else values[1] - values[0],
        "ladder_volumes_bits_width": None
        if not values
        else parsed.get("bit", 0) - values[1],
        "aligned_width": None,
        "ladder_layout": parsed.get("ladder_layout"),
        "ladder_flags": parsed.get("ladder_flags"),
        "ladder_price_mask": parsed.get("ladder_price_mask"),
        "ladder_anchor_value": parsed.get("ladder_anchor_value"),
        "ladder_anchor_operation": parsed.get("ladder_anchor_operation"),
        "ladder_volume_mask": parsed.get("ladder_volume_mask"),
        "ladder_move_code": parsed.get("move_code"),
        "omitted_tail_records": frame.get("omitted_tail_records") or 0,
        "residue_bit_offset": frame.get("decoded_value_residue_bit_offset"),
        "remaining_bits": frame.get("decoded_value_remaining_bits"),
        "delta_index_count": frame.get("delta_index_count"),
        "rejected_stage": parsed.get("rejected_stage"),
        "token_prefix_mismatch": parsed.get("token_prefix_mismatch"),
        "bitstream_exhausted": parsed.get("bitstream_exhausted"),
        "error": frame.get("decoded_value_error"),
    }


def median(values: list[int]) -> float | None:
    if not values:
        return None
    return float(statistics.median(values))


def classify_failure(
    fail: dict[str, Any],
    success_widths: dict[str, list[int]],
    success_price_masks: collections.Counter[int],
) -> dict[str, Any]:
    stream_bits = fail.get("value_stream_bits")
    fail_bit = fail.get("ladder_volumes_end")
    have = fail.get("have_bits")
    need = fail.get("need_bits")
    record_start = fail.get("record_start")
    previous_end = fail.get("previous_record_aligned_end")
    support: dict[str, Any] = {}
    counter: dict[str, Any] = {}

    end_of_stream = (
        stream_bits is not None
        and have is not None
        and fail_bit is not None
        and fail_bit + have == stream_bits
    )
    remaining_at_start = (
        None
        if stream_bits is None or record_start is None
        else stream_bits - record_start
    )
    success_aligned = success_widths.get("aligned_width") or []
    success_median = median(success_aligned)
    offset_contiguous = previous_end is None or previous_end == record_start
    last_attempted = fail.get("record_index") is not None and fail.get(
        "delta_index_count"
    ) is not None
    indexes = fail.get("delta_index_count") or 0
    completed_before = fail.get("record_index") or 0
    is_last_index = last_attempted and completed_before + 1 + (
        fail.get("omitted_tail_records") or 0
    ) == indexes
    token_mismatch = bool(fail.get("token_prefix_mismatch"))
    exhausted = bool(fail.get("bitstream_exhausted"))
    absolute = fail.get("baseline_mode") == "absolute"
    baseline_present = fail.get("baseline_present")
    price_mask = fail.get("ladder_price_mask")
    success_has_same_price_mask = (
        price_mask is not None and success_price_masks[price_mask] > 0
    )
    prefix_width = fail.get("prefix_bits")
    success_prefix = success_widths.get("prefix_bits") or []
    prefix_within_success = (
        prefix_width is not None
        and success_prefix
        and min(success_prefix) <= prefix_width <= max(success_prefix)
    )

    truncation = (
        exhausted
        and end_of_stream
        and offset_contiguous
        and not token_mismatch
        and (
            success_median is None
            or remaining_at_start is None
            or remaining_at_start < success_median
        )
    )
    pollution = (not offset_contiguous) or (
        prefix_width is not None
        and success_prefix
        and prefix_width > max(success_prefix)
        and not end_of_stream
    )
    lifecycle = False
    if fail.get("mask_has_bit0") and baseline_present is False:
        lifecycle = True
    if (
        fail.get("committed_state_present") is False
        and fail.get("stored_present") is True
        and fail.get("level_count") == 0
        and fail.get("current_last_before") == 0
    ):
        lifecycle = True
    # Absolute records never select a relative baseline. That flag cannot
    # by itself explain a same-key success/failure split.
    if absolute and baseline_present is False:
        counter["absolute_baseline_present_is_tautological"] = True
        if fail.get("mask_has_bit0") is False:
            lifecycle = False

    semantics = (
        exhausted
        and success_has_same_price_mask
        and offset_contiguous
        and prefix_within_success
        and (have or 0) > 0
        and not token_mismatch
        and remaining_at_start is not None
        and success_median is not None
        and remaining_at_start >= success_median
    )

    if truncation:
        cause = "payload_physical_truncation"
        strength = "strong" if end_of_stream and is_last_index else "moderate"
    elif pollution:
        cause = "prior_token_or_offset_pollution"
        strength = "strong" if not offset_contiguous else "moderate"
    elif lifecycle:
        cause = "previous_record_state_lifecycle"
        strength = "moderate"
    elif semantics or token_mismatch:
        cause = "missing_layout_or_volume_mask_semantics"
        strength = "moderate" if success_has_same_price_mask else "weak"
    else:
        cause = "unresolved_same_key_bitstream_gap"
        strength = "weak"

    support.update(
        {
            "end_of_stream": end_of_stream,
            "offset_contiguous": offset_contiguous,
            "is_last_index": is_last_index,
            "remaining_bits_at_record_start": remaining_at_start,
            "success_median_aligned_width": success_median,
            "token_prefix_mismatch": token_mismatch,
            "bitstream_exhausted": exhausted,
            "need_bits": need,
            "have_bits": have,
            "success_same_price_mask": success_has_same_price_mask,
            "prefix_within_success_range": prefix_within_success,
            "absolute_mode": absolute,
            "baseline_present": baseline_present,
        }
    )
    if absolute:
        counter["success_and_failure_share_absolute_baseline_present_false"] = True
    return {
        "cause": cause,
        "strength": strength,
        "support": support,
        "counter": counter,
    }


def summarize_widths(rows: list[dict[str, Any]]) -> dict[str, list[int]]:
    widths: dict[str, list[int]] = collections.defaultdict(list)
    for row in rows:
        for name in (
            "prefix_bits",
            "ladder_values_bits_width",
            "ladder_volumes_bits_width",
            "aligned_width",
        ):
            value = row.get(name)
            if isinstance(value, int):
                widths[name].append(value)
    return widths


def pollution_row(extract_dir: Path, spec: dict[str, Any]) -> dict[str, Any]:
    manifest = load_json(extract_dir / "manifest.json")
    ordinal = spec["ordinal"]
    frame = manifest[ordinal]
    traces = []
    if frame.get("decoded_value_trace_file"):
        traces = load_json(extract_dir / frame["decoded_value_trace_file"])
    decoded_rows = []
    if frame.get("decoded_values_file"):
        decoded_rows = load_json(extract_dir / frame["decoded_values_file"])
    trace = next(
        (
            row
            for row in traces
            if row.get("symbol_index") == spec["symbol_index"]
            and row.get("market") == spec["market"]
        ),
        None,
    )
    decoded = next(
        (
            row
            for row in decoded_rows
            if row.get("index", {}).get("symbol_index") == spec["symbol_index"]
            and row.get("index", {}).get("market") == spec["market"]
        ),
        None,
    )
    previous = None
    target_key = (tuple(spec["market"]), spec["symbol_index"])
    for earlier in reversed(manifest[:ordinal]):
        if earlier.get("dst") != frame.get("dst"):
            continue
        trace_name = earlier.get("decoded_value_trace_file")
        if not trace_name:
            continue
        earlier_traces = load_json(extract_dir / trace_name)
        if not any(
            row.get("symbol_index") == spec["symbol_index"]
            and row.get("market") == spec["market"]
            for row in earlier_traces
        ):
            continue
        decoded_name = earlier.get("decoded_values_file")
        if not decoded_name:
            continue
        for row in load_json(extract_dir / decoded_name):
            index = row["index"]
            if (tuple(index["market"]), index["symbol_index"]) != target_key:
                continue
            previous = {
                "last": last_from_decoded(row["record"]),
                "timestamp": i32_at(row["record"], 0),
                "frame_file": decoded_name,
            }
            break
        if previous is not None:
            break
    current_last = None if decoded is None else last_from_decoded(decoded["record"])
    observed_bits = None
    if trace is not None:
        observed_bits = (trace.get("record_start"), trace.get("trailing_da_end") or trace.get("aligned_end"))
    return {
        "label": spec["label"],
        "ordinal": ordinal,
        "dst": frame.get("dst"),
        "frame_error": frame.get("decoded_value_error"),
        "trace": trace,
        "current_last": current_last,
        "previous_same_symbol": previous,
        "observed_bits": observed_bits,
        "expected_mask": spec["expected_mask"],
        "expected_bits": list(spec["expected_bits"]),
        "mask_match": None if trace is None else int(trace.get("mask", -1)) == spec["expected_mask"],
        "stored_present": None if trace is None else trace.get("stored_present"),
        "baseline_present": None if trace is None else trace.get("baseline_present"),
        "mask_has_bit0": None
        if trace is None
        else trace.get("mask_has_bit0", int(trace.get("mask", 0)) & 1 != 0),
        "stored_source": None if trace is None else trace.get("stored_source"),
        "committed_state_present": None
        if trace is None
        else trace.get("committed_state_present"),
        "current_last_before": None if trace is None else trace.get("current_last_before"),
        "stored_last_before": None if trace is None else trace.get("stored_last_before"),
        "committed_last_before": None if trace is None else trace.get("committed_last_before"),
        "baseline_last_before": None if trace is None else trace.get("baseline_last_before"),
        "anchor_source": None if trace is None else trace.get("anchor_source"),
        "clear_ladder": None if trace is None else trace.get("clear_ladder"),
        "raw_level_count": None if trace is None else trace.get("raw_level_count"),
        "level_count": None if trace is None else trace.get("level_count"),
        "record_index": None if trace is None else trace.get("record_index"),
        "interpretation": {
            "absolute_baseline_present_false_is_tautological": None
            if trace is None
            else (int(trace.get("mask", 1)) & 1 == 0 and not trace.get("baseline_present", True)),
            "stored_present_can_be_0104_seed": previous is None
            and (trace or {}).get("stored_present") is True,
            "committed_previous_last": None if previous is None else previous.get("last"),
            "did_not_use_stored_last_as_anchor": current_last == 0
            and previous is not None
            and previous.get("last") not in (None, 0),
            "no_zero_fill_or_guessed_anchor": True,
        },
    }


def classify_prefix_consumption(
    actual_prefix_bits: int,
    expected_median_sum: float | None,
    expected_min_sum: int | None,
    remaining_bits: int | None,
    success_min_aligned: int | None,
) -> dict[str, Any]:
    overread = (
        expected_median_sum is not None and actual_prefix_bits > expected_median_sum + 16
    )
    short_payload = (
        expected_min_sum is not None
        and actual_prefix_bits <= expected_min_sum + 8
        and remaining_bits is not None
        and success_min_aligned is not None
        and remaining_bits < success_min_aligned
    )
    if overread and not short_payload:
        cause = "prefix_overread"
    elif short_payload and not overread:
        cause = "true_short_payload"
    elif overread and short_payload:
        cause = "mixed"
    else:
        cause = "ambiguous"
    return {
        "cause": cause,
        "actual_prefix_bits": actual_prefix_bits,
        "expected_median_sum": expected_median_sum,
        "expected_min_sum": expected_min_sum,
        "remaining_bits_at_record_start": remaining_bits,
        "success_min_aligned_width": success_min_aligned,
        "overread": overread,
        "short_payload": short_payload,
    }


def analyze(
    extract_dir: Path,
    full_init_dir: Path | None = None,
    max_samples: int = 3,
) -> dict[str, Any]:
    manifest = load_json(extract_dir / "manifest.json")
    frame_counts: collections.Counter[str] = collections.Counter()
    error_stages: collections.Counter[str] = collections.Counter()
    omitted_frames = 0
    omitted_records = 0
    residue_offset_5 = 0
    missing_fresh = 0
    completed_records = 0
    failed_records = 0
    success_by_key: dict[tuple[Any, ...], list[dict[str, Any]]] = collections.defaultdict(list)
    fail_by_key: dict[tuple[Any, ...], list[dict[str, Any]]] = collections.defaultdict(list)
    success_counts: collections.Counter[tuple[Any, ...]] = collections.Counter()
    fail_counts: collections.Counter[tuple[Any, ...]] = collections.Counter()
    cause_counts_by_key: dict[tuple[Any, ...], collections.Counter[str]] = collections.defaultdict(
        collections.Counter
    )
    success_width_acc: dict[tuple[Any, ...], dict[str, list[int]]] = collections.defaultdict(
        lambda: collections.defaultdict(list)
    )
    success_price_masks: dict[tuple[Any, ...], collections.Counter[int]] = collections.defaultdict(
        collections.Counter
    )
    latest_last: dict[tuple[str, tuple[int, ...], int], dict[str, Any]] = {}
    clean_aligned_widths: dict[tuple[Any, ...], list[int]] = collections.defaultdict(list)
    priority_prefix_cases: list[dict[str, Any]] = []

    for frame in manifest:
        if frame.get("wire_kind") != "2704":
            continue
        error = frame.get("decoded_value_error")
        traces = []
        if frame.get("decoded_value_trace_file"):
            traces = load_json(extract_dir / str(frame["decoded_value_trace_file"]))
        omitted = int(frame.get("omitted_tail_records") or 0)
        if omitted:
            omitted_frames += 1
            omitted_records += omitted
        if frame.get("decoded_value_residue_bit_offset") == 5 and error:
            residue_offset_5 += 1
        if error is None:
            frame_counts["clean"] += 1
        elif traces:
            frame_counts["partial"] += 1
        else:
            frame_counts["failed"] += 1
        previous_trace = None
        for trace in traces:
            symbol_key = (
                str(frame.get("dst")),
                tuple(trace.get("market") or []),
                int(trace.get("symbol_index") or 0),
            )
            previous_same = latest_last.get(symbol_key)
            snap = snapshot_from_trace(trace, frame, previous_trace, previous_same)
            completed_records += 1
            key = key_tuple(
                int(trace["mask"]),
                int(trace["header"]),
                str(trace["baseline_mode"]),
                trace.get("ladder_layout"),
                trace.get("ladder_volume_mask"),
            )
            success_counts[key] += 1
            if len(success_by_key[key]) < max_samples:
                success_by_key[key].append(snap)
            widths = success_width_acc[key]
            for name in (
                "prefix_bits",
                "ladder_values_bits_width",
                "ladder_volumes_bits_width",
                "aligned_width",
            ):
                value = snap.get(name)
                if isinstance(value, int):
                    widths[name].append(value)
            price_mask = snap.get("ladder_price_mask")
            if isinstance(price_mask, int):
                success_price_masks[key][price_mask] += 1
            if error is None:
                aligned = snap.get("aligned_width")
                if isinstance(aligned, int):
                    clean_aligned_widths[key].append(aligned)
            previous_trace = trace
            latest_last[symbol_key] = {"last": trace.get("current_last_before")}
        if error:
            failed_records += 1
            parsed = parse_error(str(error))
            if parsed and parsed.get("baseline_mode") == "missing_fresh":
                missing_fresh += 1
            if parsed and parsed.get("stage"):
                error_stages[str(parsed["stage"])] += 1
            else:
                error_stages["unparsed"] += 1
            if parsed and parsed.get("stage") == "ladder_volumes":
                key = key_tuple(
                    int(parsed["mask"]),
                    int(parsed["header"]),
                    str(parsed["baseline_mode"]),
                    parsed.get("ladder_layout"),
                    parsed.get("ladder_volume_mask"),
                )
                symbol_key = (
                    str(frame.get("dst")),
                    tuple(parsed.get("market") or []),
                    int(parsed.get("symbol_index") or 0),
                )
                snap = snapshot_from_error(
                    parsed, frame, previous_trace, latest_last.get(symbol_key)
                )
                snap["classification"] = classify_failure(
                    snap,
                    success_width_acc.get(key, {}),
                    success_price_masks.get(key, collections.Counter()),
                )
                fail_counts[key] += 1
                cause_counts_by_key[key][str(snap["classification"]["cause"])] += 1
                if len(fail_by_key[key]) < max_samples:
                    fail_by_key[key].append(snap)
                if key == PRIORITY_KEY:
                    stream_bits = frame.get("decoded_value_stream_bits")
                    record_start = parsed.get("record_start")
                    remaining = (
                        None
                        if stream_bits is None or record_start is None
                        else stream_bits - record_start
                    )
                    priority_prefix_cases.append(
                        {
                            "payload_file": frame.get("payload_file"),
                            "frame_index": frame.get("frame_index"),
                            "dst": frame.get("dst"),
                            "record_index": parsed.get("record_index"),
                            "record_start": record_start,
                            "stream_bits": stream_bits,
                            "remaining_bits_at_record_start": remaining,
                            "need_bits": parsed.get("need_bits"),
                            "have_bits": parsed.get("have_bits"),
                            "price_mask": parsed.get("ladder_price_mask"),
                            "volume_slot": parsed.get("volume_slot"),
                            "completed": [
                                {
                                    "key": key_tuple(
                                        int(row["mask"]),
                                        int(row["header"]),
                                        str(row["baseline_mode"]),
                                        row.get("ladder_layout"),
                                        row.get("ladder_volume_mask"),
                                    ),
                                    "aligned_width": int(row["aligned_end"])
                                    - int(row["record_start"]),
                                    "record_index": row.get("record_index"),
                                }
                                for row in traces
                            ],
                        }
                    )

    pairs = []
    for key, failures in fail_by_key.items():
        successes = success_by_key.get(key, [])
        if not successes:
            continue
        pair = {
            "key": key_text(key),
            "success_count": success_counts[key],
            "failure_count": fail_counts[key],
            "success_count_sampled": len(successes),
            "failure_count_sampled": len(failures),
            "failure_causes": dict(cause_counts_by_key[key]),
            "success_width_summary": {
                name: {
                    "n": len(values),
                    "min": min(values),
                    "max": max(values),
                    "median": median(values),
                }
                for name, values in success_width_acc[key].items()
            },
            "success_price_masks": dict(success_price_masks[key]),
            "success_sample": successes[0],
            "failure_sample": failures[0],
            "bit_diff": {
                name: {
                    "success": successes[0].get(name),
                    "failure": failures[0].get(name),
                }
                for name in (
                    "record_start",
                    "header_end",
                    "timestamp_start",
                    "timestamp_end",
                    "prefix_values_start",
                    "prefix_values_end",
                    "ladder_move_start",
                    "ladder_move_end",
                    "ladder_values_start",
                    "ladder_values_end",
                    "volume_mask_start",
                    "volume_mask_end",
                    "ladder_volumes_start",
                    "ladder_volumes_end",
                    "trailing_da_start",
                    "trailing_da_end",
                    "aligned_end",
                    "value_stream_bits",
                    "need_bits",
                    "have_bits",
                    "prefix_bits",
                    "ladder_values_bits_width",
                    "ladder_volumes_bits_width",
                    "previous_record_aligned_end",
                    "previous_record_success",
                    "stored_present",
                    "baseline_present",
                    "mask_has_bit0",
                    "stored_source",
                    "committed_state_present",
                    "current_last_before",
                    "stored_last_before",
                    "committed_last_before",
                    "baseline_last_before",
                    "anchor_source",
                    "clear_ladder",
                    "raw_level_count",
                    "level_count",
                    "ladder_price_mask",
                    "ladder_anchor_value",
                    "ladder_anchor_operation",
                    "slot",
                )
            },
            "failure_classification": failures[0].get("classification"),
        }
        pairs.append(pair)
    pairs.sort(key=lambda row: (0 if "mask=0xe8 header=0x03" in row["key"] else 1, row["key"]))

    priority_key = PRIORITY_KEY
    priority_failures = fail_by_key.get(priority_key, [])
    priority_successes = success_by_key.get(priority_key, [])

    pollution = []
    if full_init_dir is not None:
        for spec in POLLUTION_TARGETS:
            pollution.append(pollution_row(full_init_dir, spec))

    success_min_aligned = None
    aligned_clean = clean_aligned_widths.get(priority_key) or success_width_acc.get(
        priority_key, {}
    ).get("aligned_width")
    if aligned_clean:
        success_min_aligned = min(aligned_clean)
    prefix_audits = []
    prefix_causes: collections.Counter[str] = collections.Counter()
    remaining_lt_min: collections.Counter[str] = collections.Counter()
    remaining_ge_min: collections.Counter[str] = collections.Counter()
    for case in priority_prefix_cases:
        expected_median = 0.0
        expected_min = 0
        unknown_keys = 0
        serial_completed = []
        for row in case["completed"]:
            key = row["key"]
            widths = clean_aligned_widths.get(key) or []
            if not widths:
                unknown_keys += 1
                expected_median += row["aligned_width"]
                expected_min += row["aligned_width"]
            else:
                expected_median += float(statistics.median(widths))
                expected_min += min(widths)
            serial_completed.append(
                {
                    "key": key_text(key),
                    "aligned_width": row["aligned_width"],
                    "record_index": row["record_index"],
                    "clean_median_aligned": None
                    if not widths
                    else float(statistics.median(widths)),
                    "clean_min_aligned": None if not widths else min(widths),
                }
            )
        remaining = case["remaining_bits_at_record_start"]
        record_start = case["record_start"] or 0
        audit = classify_prefix_consumption(
            record_start,
            expected_median,
            expected_min,
            remaining,
            success_min_aligned,
        )
        audit.update(
            {
                "payload_file": case["payload_file"],
                "frame_index": case["frame_index"],
                "dst": case["dst"],
                "record_index": case["record_index"],
                "need_bits": case["need_bits"],
                "have_bits": case["have_bits"],
                "price_mask": case["price_mask"],
                "volume_slot": case["volume_slot"],
                "unknown_prefix_keys": unknown_keys,
                "completed_count": len(serial_completed),
            }
        )
        prefix_causes[str(audit["cause"])] += 1
        if (
            remaining is not None
            and success_min_aligned is not None
            and remaining < success_min_aligned
        ):
            remaining_lt_min[str(audit["cause"])] += 1
        else:
            remaining_ge_min[str(audit["cause"])] += 1
        if len(prefix_audits) < 8:
            audit["completed"] = serial_completed
            prefix_audits.append(audit)

    return {
        "schema": "netzip.2704-ladder-volume-trace-diff.v1",
        "source": str(extract_dir),
        "priority_key": key_text(priority_key),
        "frame_level": {
            "clean": frame_counts["clean"],
            "partial": frame_counts["partial"],
            "failed": frame_counts["failed"],
            "error": frame_counts["partial"] + frame_counts["failed"],
            "omitted_frames": omitted_frames,
            "residue_offset_5_failures": residue_offset_5,
        },
        "record_level": {
            "completed": completed_records,
            "failed": failed_records,
            "omitted_records": omitted_records,
            "missing_fresh": missing_fresh,
            "rejected_public_quotes": None,
            "missing_seed_projection": None,
            "note": "rejected/missing_seed are runtime public-quote projection counters, not bitstream decoder outcomes",
        },
        "error_stages": dict(error_stages),
        "short_tail": {
            "omitted_frames": omitted_frames,
            "omitted_records": omitted_records,
            "separated_from_ladder_volume_errors": True,
            "contract": "omit only when remaining value bits < 13 and every remaining index uses_baseline=true; do not synthesize quotes or commit state",
        },
        "priority_bucket": {
            "key": key_text(priority_key),
            "success_count": success_counts[priority_key],
            "failure_count": fail_counts[priority_key],
            "success_samples": priority_successes,
            "failure_samples": priority_failures,
            "failure_causes": dict(cause_counts_by_key[priority_key]),
            "success_width_summary": {
                name: {
                    "n": len(values),
                    "min": min(values),
                    "max": max(values),
                    "median": median(values),
                }
                for name, values in success_width_acc.get(priority_key, {}).items()
            },
            "lifecycle_note": (
                "absolute records never select a relative baseline, so "
                "baseline_present=false is tautological for this key and cannot "
                "alone explain the success/failure split"
            ),
        },
        "same_key_pairs": pairs[:20],
        "pollution_ordinals": pollution,
        "priority_prefix_audit": {
            "method": (
                "sum clean-frame median/min aligned_width of each completed prefix "
                "record's 5-tuple; compare to fail.record_start and remaining bits"
            ),
            "counts": dict(prefix_causes),
            "remaining_lt_success_min": dict(remaining_lt_min),
            "remaining_ge_success_min": dict(remaining_ge_min),
            "success_min_aligned_width": success_min_aligned,
            "samples": prefix_audits,
            "note": (
                "prefix_overread is vs clean-frame 5-tuple median+16, not proof of "
                "token over-consumption; 5-tuple aligned widths have wide ranges. "
                "remaining < success_min with prefix near median still looks like "
                "this-record short payload, not prefix overread"
            ),
        },
        "publication": "disabled",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("extract_dir", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--full-init-extract", type=Path)
    parser.add_argument("--max-samples", type=int, default=3)
    args = parser.parse_args()
    report = analyze(args.extract_dir, args.full_init_extract, args.max_samples)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(
        json.dumps(
            {
                "output": str(args.output),
                "frame_level": report["frame_level"],
                "record_level": report["record_level"],
                "error_stages": report["error_stages"],
                "priority_failure_causes": report["priority_bucket"]["failure_causes"],
                "priority_prefix_audit": report["priority_prefix_audit"]["counts"],
                "remaining_lt_success_min": report["priority_prefix_audit"][
                    "remaining_lt_success_min"
                ],
                "same_key_pairs": len(report["same_key_pairs"]),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
