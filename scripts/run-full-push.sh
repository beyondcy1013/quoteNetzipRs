#!/usr/bin/env bash
set -uo pipefail

gateway_addr="${NETZIP_QUOTE_GATEWAY_ADDR:-127.0.0.1:16886}"
service_addr="${NETZIP_SERVICE_ADDR:-127.0.0.1:16893}"
interval_secs="${NETZIP_FULL_PUSH_INTERVAL_SECS:-5}"
idle_interval_secs="${NETZIP_FULL_PUSH_IDLE_INTERVAL_SECS:-5}"
batch_size="${NETZIP_FULL_PUSH_BATCH_SIZE:-100}"
limit="${NETZIP_FULL_PUSH_LIMIT:-6000}"
worker_count="${NETZIP_FULL_PUSH_WORKERS:-64}"
# Native push is the primary resident path; poll remains an explicit fallback.
push_mode="${NETZIP_FULL_PUSH_MODE:-push}"
native_session_secs="${NETZIP_NATIVE_PUSH_SESSION_SECS:-240}"
native_audit_interval_secs="${NETZIP_NATIVE_PUSH_AUDIT_INTERVAL_SECS:-30}"
native_bj_poll_interval_secs="${NETZIP_NATIVE_BJ_POLL_INTERVAL_SECS:-3}"
official_5188_retry_secs="${NETZIP_OFFICIAL_5188_RETRY_SECS:-60}"
last_official_5188_attempt=0

if [[ "$push_mode" != "poll" && "$push_mode" != "push" ]]; then
    echo "NETZIP_FULL_PUSH_MODE must be poll or push" >&2
    exit 2
fi

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

clock_to_seconds() {
    local clock="$1"
    local hour=$((10#${clock:0:2}))
    local minute=$((10#${clock:2:2}))
    local second=$((10#${clock:4:2}))
    printf '%s\n' $((hour * 3600 + minute * 60 + second))
}

official_5188_ready() {
    jq -e '
        .authenticated == true
        and .control_session_retained == true
        and .initialized == true
        and .connection_count == 10
        and (.slots | length) == 10
        and (.shadows | length) == 10
        and all(.shadows[]; .running == true and .last_error == null)
    ' >/dev/null 2>&1
}

ensure_official_5188() {
    local now status
    now="$(date +%s)"
    if (( now - last_official_5188_attempt < official_5188_retry_secs )); then
        return 0
    fi
    last_official_5188_attempt="$now"

    status="$(curl --noproxy '*' -fsS --max-time 5 \
        "http://$service_addr/api/fullpull/official-5188/status" 2>/dev/null || true)"
    if official_5188_ready <<<"$status"; then
        return 0
    fi

    echo "official 5188 requires ten live slots; repairing session"
    if ! jq -e '.authenticated == true and .control_session_retained == true' \
        >/dev/null 2>&1 <<<"$status"; then
        if ! curl --noproxy '*' -fsS --max-time 180 -X POST \
            "http://$service_addr/api/auth/login" 2>/dev/null \
            | jq -e '.accepted == true and .status.login_success_confirmed == true' \
                >/dev/null 2>&1; then
            echo "official 5188 authentication failed; retrying later" >&2
            return 1
        fi
    else
        curl --noproxy '*' -fsS --max-time 60 -X POST \
            "http://$service_addr/api/fullpull/official-5188/disconnect" \
            >/dev/null 2>&1 || true
    fi

    if curl --noproxy '*' -fsS --max-time 600 -X POST \
        "http://$service_addr/api/fullpull/official-5188/connect" 2>/dev/null \
        | official_5188_ready; then
        echo "official 5188 ready: ten live slots"
        return 0
    fi
    echo "official 5188 ten-slot initialization failed; retrying later" >&2
    return 1
}

worklist_url="http://$gateway_addr/api/codes/worklist?include_unknown=true&include_halted=false&limit=1"
outside_window_logged=0
last_worklist_wait=""

while :; do
    should_backoff=0
    ensure_official_5188 || true
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
        if [[ "$push_mode" == "push" ]]; then
            remaining_secs=$(( $(clock_to_seconds "$session_end") - $(clock_to_seconds "$clock") ))
            if (( remaining_secs < 5 )); then
                should_backoff=1
                if [[ "${NETZIP_FULL_PUSH_ONCE:-0}" == "1" ]]; then
                    break
                fi
                sleep "$interval_secs"
                continue
            fi
            run_secs="$native_session_secs"
            if (( run_secs > remaining_secs )); then
                run_secs="$remaining_secs"
            fi
            request_timeout=$((run_secs + native_audit_interval_secs + 60))
            publish_stage="push-worklist"
            publish_url="http://$service_addr/api/hqw/push-worklist"
            publish_body="{\"duration_secs\":$run_secs,\"audit_interval_secs\":$native_audit_interval_secs,\"bj_poll_interval_secs\":$native_bj_poll_interval_secs,\"worker_count\":$worker_count,\"publish\":true}"
        else
            request_timeout=300
            publish_stage="publish-worklist"
            publish_url="http://$service_addr/api/hqw/publish-worklist"
            publish_body="{\"batch_size\":$batch_size,\"limit\":$limit,\"worker_count\":$worker_count}"
        fi
        publish_result="$(curl --noproxy '*' -sS --max-time "$request_timeout" --write-out $'\n%{http_code}' -X POST "$publish_url" \
            -H 'Content-Type: application/json' -d "$publish_body")"
        curl_status=$?
        if [[ "$publish_result" == *$'\n'* ]]; then
            http_status="${publish_result##*$'\n'}"
            publish_response="${publish_result%$'\n'*}"
        else
            http_status="unknown"
            publish_response="$publish_result"
        fi
        if (( curl_status != 0 )) || [[ ! "$http_status" =~ ^2[0-9][0-9]$ ]]; then
            echo "netzip full push failed: stage=$publish_stage http_status=$http_status curl_status=$curl_status; retrying after ${interval_secs}s" >&2
            if [[ -n "$publish_response" ]]; then
                printf 'netzip full push response_body=%s\n' "$publish_response" >&2
            fi
            should_backoff=1
        else
            printf '%s\n' "$publish_response"
            published_count="$(jq -r '.published_count // .published_records // 0' <<<"$publish_response" 2>/dev/null || printf '0')"
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
