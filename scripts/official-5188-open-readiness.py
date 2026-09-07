#!/usr/bin/env python3
"""Fail-closed opening readiness check for the official 5188 shadow lane.

This checker never logs in, starts a session, or changes publication routing. It
only reads the resident service status twice and distinguishes transport
readiness from trade-quote readiness.
"""

from __future__ import annotations

import argparse
import datetime
import json
import math
import sys
import time
import urllib.request
from dataclasses import dataclass, asdict
from typing import Any
from zoneinfo import ZoneInfo


@dataclass
class Verdict:
    ready: bool
    level: str
    checks: dict[str, bool]
    reasons: list[str]
    metrics: dict[str, Any]


def _number(value: Any) -> int:
    return value if isinstance(value, int) and value >= 0 else 0


def _delta(current: dict[str, Any], previous: dict[str, Any], key: str) -> int:
    return max(0, _number(current.get(key)) - _number(previous.get(key)))


def _quote_semantics(
    snapshot: dict[str, Any] | None, trade_date: datetime.date
) -> tuple[bool, dict[str, int]]:
    metrics = {
        "public_quote_count": 0,
        "invalid_public_quote_count": 0,
        "invalid_public_timestamp_count": 0,
        "invalid_public_numeric_count": 0,
        "negative_public_scalar_count": 0,
        "negative_public_book_price_count": 0,
        "negative_public_book_volume_count": 0,
        "incoherent_public_ohlc_count": 0,
        "out_of_range_public_price_count": 0,
        "out_of_range_public_quantity_count": 0,
    }
    if not isinstance(snapshot, dict):
        return False, metrics
    quotes = snapshot.get("quotes")
    if (
        snapshot.get("source") != "netzipRustOfficial5188Shadow"
        or snapshot.get("publication") != "disabled"
        or not isinstance(quotes, list)
        or snapshot.get("quote_count") != len(quotes)
    ):
        return False, metrics

    china = ZoneInfo("Asia/Shanghai")
    day_start = int(datetime.datetime.combine(trade_date, datetime.time(), china).timestamp())
    day_end = int(
        datetime.datetime.combine(
            trade_date + datetime.timedelta(days=1), datetime.time(), china
        ).timestamp()
    )
    scalar_fields = ("price", "last_close", "open", "high", "low", "volume", "amount")
    array_fields = ("ask_prices", "ask_volumes", "bid_prices", "bid_volumes")
    for quote in quotes:
        metrics["public_quote_count"] += 1
        timestamp = quote.get("timestamp") if isinstance(quote, dict) else None
        timestamp_ok = isinstance(timestamp, int) and day_start <= timestamp < day_end
        numeric_ok = isinstance(quote, dict)
        if numeric_ok:
            scalar_values: list[Any] = [quote.get(field) for field in scalar_fields]
            values = list(scalar_values)
            book_prices: list[Any] = []
            book_volumes: list[Any] = []
            for field in array_fields:
                array = quote.get(field)
                if not isinstance(array, list) or len(array) != 10:
                    numeric_ok = False
                    break
                values.extend(array)
                if field.endswith("prices"):
                    book_prices.extend(array)
                else:
                    book_volumes.extend(array)
            numeric_ok = numeric_ok and all(
                isinstance(value, (int, float))
                and not isinstance(value, bool)
                and math.isfinite(value)
                and value >= 0
                for value in values
            )
            if any(isinstance(value, (int, float)) and value < 0 for value in scalar_values):
                metrics["negative_public_scalar_count"] += 1
            if any(isinstance(value, (int, float)) and value < 0 for value in book_prices):
                metrics["negative_public_book_price_count"] += 1
            if any(isinstance(value, (int, float)) and value < 0 for value in book_volumes):
                metrics["negative_public_book_volume_count"] += 1
            high = quote.get("high")
            low = quote.get("low")
            ohlc_bad = (
                isinstance(high, (int, float))
                and isinstance(low, (int, float))
                and high != 0
                and low != 0
                and high < low
            )
            price = quote.get("price")
            open_price = quote.get("open")
            last_close = quote.get("last_close")
            if all(isinstance(value, (int, float)) for value in (price, open_price, high, low)):
                ohlc_bad = ohlc_bad or (
                    high != 0
                    and any(value != 0 and value > high for value in (price, open_price))
                ) or (
                    low != 0
                    and any(value != 0 and value < low for value in (price, open_price))
                )
            if ohlc_bad:
                metrics["incoherent_public_ohlc_count"] += 1
                numeric_ok = False
            if isinstance(last_close, (int, float)):
                max_price = last_close * 10 if last_close > 0 else 1_000_000
                if any(
                    isinstance(value, (int, float)) and value > max_price
                    for value in [price, open_price, high, low, *book_prices]
                ):
                    metrics["out_of_range_public_price_count"] += 1
                    numeric_ok = False
            if (
                isinstance(quote.get("volume"), (int, float))
                and quote["volume"] > 100_000_000_000
            ) or (
                isinstance(quote.get("amount"), (int, float))
                and quote["amount"] > 1_000_000_000_000_000
            ) or any(
                isinstance(value, (int, float)) and value > 1_000_000_000
                for value in book_volumes
            ):
                metrics["out_of_range_public_quantity_count"] += 1
                numeric_ok = False
        if not timestamp_ok:
            metrics["invalid_public_timestamp_count"] += 1
        if not numeric_ok:
            metrics["invalid_public_numeric_count"] += 1
        if not timestamp_ok or not numeric_ok:
            metrics["invalid_public_quote_count"] += 1
    return metrics["public_quote_count"] > 0 and metrics["invalid_public_quote_count"] == 0, metrics


def evaluate(
    first: dict[str, Any],
    second: dict[str, Any],
    require_quotes: bool,
    public_quotes: dict[str, Any] | None = None,
    trade_date: datetime.date | None = None,
) -> Verdict:
    shadow = second.get("shadow") or {}
    previous_shadow = first.get("shadow") or {}
    attempted = _delta(shadow, previous_shadow, "decoder_attempted_frames")
    decoded = _delta(shadow, previous_shadow, "decoder_decoded_frames")
    partial = _delta(shadow, previous_shadow, "decoder_partial_frames")
    failed = _delta(shadow, previous_shadow, "decoder_failed_frames")
    records = _delta(shadow, previous_shadow, "decoder_decoded_records")
    updates = _delta(shadow, previous_shadow, "decoder_oem_state_updates")
    missing = _delta(shadow, previous_shadow, "decoder_missing_previous_close_seeds")
    rejected = _delta(shadow, previous_shadow, "decoder_rejected_public_quotes")
    errors = shadow.get("decoder_error_kinds") or {}
    previous_errors = previous_shadow.get("decoder_error_kinds") or {}
    non_ladder_errors = sum(
        max(0, _number(count) - _number(previous_errors.get(kind)))
        for kind, count in errors.items()
        if not kind.startswith("ladder_volumes:")
    )
    ladder_errors = sum(
        max(0, _number(count) - _number(previous_errors.get(kind)))
        for kind, count in errors.items()
        if kind.startswith("ladder_volumes:")
    )
    semantic_ready, semantic_metrics = _quote_semantics(
        public_quotes, trade_date or datetime.datetime.now(ZoneInfo("Asia/Shanghai")).date()
    )

    checks = {
        "authenticated": second.get("authenticated") is True,
        "endpoint_5188": str(second.get("selected_endpoint") or "").endswith(":5188"),
        "initialized": second.get("initialized") is True,
        "ten_connections": _number(second.get("connection_count")) == 10,
        "code_tables_present": _number(second.get("code_table_count")) > 0,
        "receive_list_present": _number(second.get("receive_list_codes")) > 0,
        "shadow_running": shadow.get("running") is True,
        "business_frames_seen": _number(shadow.get("delta_2704_frames")) > 0,
        "stream_advancing": _number(shadow.get("frames_received"))
        > _number(previous_shadow.get("frames_received")),
        "no_receive_termination": _number(shadow.get("receive_terminations")) == 0,
        "record_accounting": records == updates + missing + rejected,
        "metadata_ready": records > 0 and updates > 0 and missing == 0 and rejected == 0,
        # A ladder failure can truncate later records in the same frame. It is
        # therefore non-publishable until the decoder can skip it losslessly.
        "trade_fields_clean": failed == 0 and partial == 0 and non_ladder_errors == 0,
        "public_quotes_semantically_valid": semantic_ready,
    }
    connection_keys = (
        "authenticated", "endpoint_5188", "initialized", "ten_connections",
        "code_tables_present", "receive_list_present", "shadow_running",
        "business_frames_seen", "stream_advancing", "no_receive_termination",
    )
    quote_keys = connection_keys + (
        "record_accounting", "metadata_ready", "trade_fields_clean",
        "public_quotes_semantically_valid",
    )
    required = quote_keys if require_quotes else connection_keys
    reasons = [name for name in required if not checks[name]]
    metrics = {
        "attempted_frames": attempted,
        "decoded_frames": decoded,
        "partial_frames": partial,
        "failed_frames": failed,
        "decoded_records": records,
        "oem_updates": updates,
        "missing_metadata": missing,
        "rejected_public_quotes": rejected,
        "non_ladder_errors": non_ladder_errors,
        "ladder_volume_errors": ladder_errors,
        **semantic_metrics,
    }
    return Verdict(not reasons, "trade-quotes" if require_quotes else "transport", checks, reasons, metrics)


def fetch(url: str, timeout: float) -> dict[str, Any]:
    with urllib.request.urlopen(url, timeout=timeout) as response:
        value = json.load(response)
    if not isinstance(value, dict):
        raise ValueError("status response is not a JSON object")
    return value


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", default="http://127.0.0.1:16893/api/fullpull/official-5188/status")
    parser.add_argument("--interval", type=float, default=5.0)
    parser.add_argument("--timeout", type=float, default=3.0)
    parser.add_argument("--level", choices=("transport", "trade-quotes"), default="trade-quotes")
    parser.add_argument(
        "--quotes-url",
        default="http://127.0.0.1:16893/api/fullpull/official-5188/shadow/quotes",
    )
    parser.add_argument("--trade-date", type=datetime.date.fromisoformat)
    parser.add_argument("--first-json", help="offline first status fixture")
    parser.add_argument("--second-json", help="offline second status fixture")
    parser.add_argument("--quotes-json", help="offline shadow quote fixture")
    args = parser.parse_args()
    try:
        if bool(args.first_json) != bool(args.second_json):
            raise ValueError("--first-json and --second-json must be supplied together")
        public_quotes = None
        if args.first_json:
            with open(args.first_json, encoding="utf-8") as handle:
                first = json.load(handle)
            with open(args.second_json, encoding="utf-8") as handle:
                second = json.load(handle)
            if args.quotes_json:
                with open(args.quotes_json, encoding="utf-8") as handle:
                    public_quotes = json.load(handle)
        else:
            first = fetch(args.url, args.timeout)
            time.sleep(max(args.interval, 0.0))
            second = fetch(args.url, args.timeout)
            if args.level == "trade-quotes":
                public_quotes = fetch(args.quotes_url, args.timeout)
        verdict = evaluate(
            first,
            second,
            args.level == "trade-quotes",
            public_quotes,
            args.trade_date,
        )
        print(json.dumps(asdict(verdict), ensure_ascii=False, indent=2))
        return 0 if verdict.ready else 2
    except Exception as error:
        print(json.dumps({"ready": False, "level": args.level, "error": str(error)}, ensure_ascii=False))
        return 3


if __name__ == "__main__":
    sys.exit(main())
