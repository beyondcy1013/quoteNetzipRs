#!/usr/bin/env bash
set -uo pipefail

gateway_addr="${NETZIP_QUOTE_GATEWAY_ADDR:-127.0.0.1:16886}"
service_addr="${NETZIP_SERVICE_ADDR:-127.0.0.1:16893}"
interval_secs="${NETZIP_FULL_PUSH_INTERVAL_SECS:-5}"
idle_interval_secs="${NETZIP_FULL_PUSH_IDLE_INTERVAL_SECS:-5}"
batch_size="${NETZIP_FULL_PUSH_BATCH_SIZE:-100}"
limit="${NETZIP_FULL_PUSH_LIMIT:-6000}"

current_weekday_clock() {
    if [[ -n "${NETZIP_FULL_PUSH_NOW:-}" ]]; then
        printf '%s\n' "$NETZIP_FULL_PUSH_NOW"
    else
        date '+%u:%H%M%S'
    fi
}

session_end_for() {
    local weekday_clock="$1"
    local weekday="${weekday_clock%%:*}"
    local clock="${weekday_clock#*:}"
    local clock_number=$((10#$clock))
    if (( weekday < 1 || weekday > 5 )); then
        return 1
    fi
    if (( clock_number >= 91400 && clock_number <= 113100 )); then
        printf '%s\n' 113100
        return 0
    fi
    if (( clock_number >= 125900 && clock_number <= 150100 )); then
        printf '%s\n' 150100
        return 0
    fi
    return 1
}

worklist_url="http://$gateway_addr/api/codes/worklist?include_unknown=true&include_halted=false&limit=1"
publish_url="http://$service_addr/api/hqw/publish-worklist"
outside_window_logged=0
last_worklist_wait=""

while :; do
    should_backoff=0
    weekday_clock="$(current_weekday_clock)"
    if ! session_end="$(session_end_for "$weekday_clock")"; then
        if (( outside_window_logged == 0 )); then
            echo "netzip full push online and idle: outside trading window ($weekday_clock)"
            outside_window_logged=1
        fi
        if [[ "${NETZIP_FULL_PUSH_DRY_RUN:-0}" == "1" || "${NETZIP_FULL_PUSH_ONCE:-0}" == "1" ]]; then
            break
        fi
        sleep "$idle_interval_secs"
        continue
    fi
    outside_window_logged=0
    if [[ "${NETZIP_FULL_PUSH_DRY_RUN:-0}" == "1" ]]; then
        echo "netzip full push eligible: window_end=$session_end"
        break
    fi

    clock="${weekday_clock#*:}"

    worklist="$(curl --noproxy '*' -fsS --max-time 10 "$worklist_url" 2>/dev/null || true)"
    as_of_date="$(jq -r '.payload.as_of_date // ""' <<<"$worklist" 2>/dev/null || true)"
    trade_date="$(jq -r '.payload.freshness.required_quote_trade_date // ""' <<<"$worklist" 2>/dev/null || true)"
    if [[ "${NETZIP_FULL_PUSH_FORCE:-0}" == "1" || ( -n "$as_of_date" && "$as_of_date" == "$trade_date" ) ]]; then
        if ! publish_response="$(curl --noproxy '*' -fsS --max-time 300 -X POST "$publish_url" \
            -H 'Content-Type: application/json' \
            -d "{\"batch_size\":$batch_size,\"limit\":$limit}")"; then
            echo "netzip full push failed; retrying after ${interval_secs}s" >&2
            should_backoff=1
        else
            printf '%s\n' "$publish_response"
            published_count="$(jq -r '.published_count // 0' <<<"$publish_response" 2>/dev/null || printf '0')"
            if [[ "$published_count" == "0" ]]; then
                should_backoff=1
            fi
            last_worklist_wait=""
        fi
    else
        worklist_wait="as_of=$as_of_date required=$trade_date"
        if [[ "$worklist_wait" != "$last_worklist_wait" ]]; then
            echo "netzip full push waiting for current trading date: $worklist_wait"
            last_worklist_wait="$worklist_wait"
        fi
        should_backoff=1
    fi

    if [[ "${NETZIP_FULL_PUSH_ONCE:-0}" == "1" ]]; then
        break
    fi
    if (( should_backoff )); then
        sleep "$interval_secs"
    fi
done
