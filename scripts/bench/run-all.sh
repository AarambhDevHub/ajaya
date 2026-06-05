#!/usr/bin/env bash
set -euo pipefail

manifest="examples/benchmarks/Cargo.toml"
results_dir="${BENCH_RESULTS_DIR:-target/bench-results}"
host="${BENCH_HOST:-http://127.0.0.1:8080}"
port="${BENCH_PORT:-8080}"
h2c_enabled="${BENCH_H2C:-1}"
summary_tsv="$results_dir/summary.tsv"

current_pid=""
current_log=""
current_name=""

mkdir -p "$results_dir"
: >"$summary_tsv"

cleanup_current_server() {
  if [ -n "$current_pid" ]; then
    if kill -0 "$current_pid" >/dev/null 2>&1; then
      kill "$current_pid" >/dev/null 2>&1 || true
      wait "$current_pid" 2>/dev/null || true
    fi
    current_pid=""
    current_log=""
    current_name=""
  fi
}

cleanup() {
  cleanup_current_server
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

json_number_or_null() {
  if [ -n "$1" ]; then
    printf '%s' "$1"
  else
    printf 'null'
  fi
}

record_result() {
  local mode="$1"
  local scenario="$2"
  local tool="$3"
  local status="$4"
  local rps="${5:-}"
  local started="${6:-}"
  local succeeded="${7:-}"
  local failed="${8:-}"
  local errored="${9:-}"
  local raw="${10:-}"
  local note="${11:-}"

  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$mode" "$scenario" "$tool" "$status" "$rps" "$started" "$succeeded" "$failed" "$errored" "$raw" "$note" \
    >>"$summary_tsv"
}

write_summary_files() {
  {
    echo "# Arvik Benchmark Summary"
    echo
    echo "Raw benchmark output is saved next to this summary. Treat 5s smoke runs as validation only; publish numbers from intentional manual benchmark artifacts."
    echo
    echo "| Mode | Scenario | Tool | Status | Requests/sec | Started | Succeeded | Failed | Errored | Raw output | Note |"
    echo "| --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | --- | --- |"
    while IFS=$'\t' read -r mode scenario tool status rps started succeeded failed errored raw note; do
      [ -n "$mode" ] || continue
      echo "| $mode | $scenario | $tool | $status | ${rps:-} | ${started:-} | ${succeeded:-} | ${failed:-} | ${errored:-} | $raw | $note |"
    done <"$summary_tsv"
  } >"$results_dir/summary.md"

  {
    echo "{"
    printf '  "generated_at": "%s",\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    printf '  "wrk_duration": "%s",\n' "$(json_escape "${WRK_DURATION:-10s}")"
    printf '  "hey_duration": "%s",\n' "$(json_escape "${HEY_DURATION:-10s}")"
    printf '  "h2load_duration": "%s",\n' "$(json_escape "${H2LOAD_DURATION:-}")"
    printf '  "h2load_requests": %s,\n' "$(json_number_or_null "${H2LOAD_REQUESTS:-10000}")"
    echo '  "results": ['

    local first=1
    while IFS=$'\t' read -r mode scenario tool status rps started succeeded failed errored raw note; do
      [ -n "$mode" ] || continue
      if [ "$first" -eq 0 ]; then
        echo ","
      fi
      first=0

      printf '    {"mode":"%s","scenario":"%s","tool":"%s","status":"%s","requests_per_second":%s,"started":%s,"succeeded":%s,"failed":%s,"errored":%s,"raw_output":"%s","note":"%s"}' \
        "$(json_escape "$mode")" \
        "$(json_escape "$scenario")" \
        "$(json_escape "$tool")" \
        "$(json_escape "$status")" \
        "$(json_number_or_null "$rps")" \
        "$(json_number_or_null "$started")" \
        "$(json_number_or_null "$succeeded")" \
        "$(json_number_or_null "$failed")" \
        "$(json_number_or_null "$errored")" \
        "$(json_escape "$raw")" \
        "$(json_escape "$note")"
    done <"$summary_tsv"

    echo
    echo '  ]'
    echo "}"
  } >"$results_dir/summary.json"
}

print_server_log() {
  if [ -n "$current_log" ] && [ -f "$current_log" ]; then
    echo
    echo "---- server log: $current_log ----"
    tail -n 200 "$current_log" || true
    echo "---- end server log ----"
  fi
}

fail_with_logs() {
  local message="$1"
  echo "$message" >&2
  print_server_log >&2
  write_summary_files
  exit 1
}

is_positive() {
  awk -v value="${1:-0}" 'BEGIN { exit !(value + 0 > 0) }'
}

is_zero() {
  awk -v value="${1:-0}" 'BEGIN { exit !(value + 0 == 0) }'
}

assert_port_free() {
  if timeout 1 bash -c ":</dev/tcp/127.0.0.1/$port" >/dev/null 2>&1; then
    fail_with_logs "port $port is already in use before starting the benchmark server; stop the stale process and rerun"
  fi
}

start_server() {
  local bin="$1"
  local label="$2"

  cleanup_current_server
  assert_port_free

  current_name="$label"
  current_log="$results_dir/${label}.server.log"
  cargo run --manifest-path "$manifest" --release --bin "$bin" >"$current_log" 2>&1 &
  current_pid="$!"
}

stop_server() {
  cleanup_current_server
  sleep 0.2
}

http_status() {
  local url="$1"
  curl -fsS -o /dev/null -w '%{http_code}' "$url" 2>/dev/null || true
}

wait_for_http1_200() {
  local url="$1"
  local code

  for _ in {1..80}; do
    code="$(http_status "$url")"
    if [ "$code" = "200" ]; then
      return 0
    fi
    sleep 0.25
  done

  return 1
}

parse_wrk_rps() {
  awk '/Requests\/sec:/ { print $2; exit }' "$1"
}

parse_wrk_requests() {
  awk '/ requests in / { print $1; exit }' "$1"
}

validate_wrk_output() {
  local output="$1"
  local rps
  local requests

  rps="$(parse_wrk_rps "$output")"
  requests="$(parse_wrk_requests "$output")"

  if ! is_positive "$rps" || ! is_positive "$requests"; then
    return 1
  fi

  if grep -Eq 'Non-2xx or 3xx responses:[[:space:]]*[1-9][0-9]*' "$output"; then
    return 1
  fi

  printf '%s' "$rps"
}

parse_hey_rps() {
  awk '/Requests\/sec:/ { print $2; exit }' "$1"
}

parse_hey_status_count() {
  local output="$1"
  local status="$2"
  awk -v status="[$status]" '$1 == status { print $2; found = 1 } END { if (!found) print 0 }' "$output"
}

validate_hey_output() {
  local output="$1"
  local rps
  local ok_count
  local bad_count

  rps="$(parse_hey_rps "$output")"
  ok_count="$(parse_hey_status_count "$output" 200)"
  bad_count="$(awk '/^[[:space:]]*\[[0-9][0-9][0-9]\]/ && $1 != "[200]" { total += $2 } END { print total + 0 }' "$output")"

  if ! is_positive "$rps" || ! is_positive "$ok_count" || ! is_zero "$bad_count"; then
    return 1
  fi

  printf '%s' "$rps"
}

parse_h2load_rps() {
  sed -nE 's/^finished in .*, ([0-9.]+) req\/s,.*/\1/p' "$1" | head -n 1
}

parse_h2load_started() {
  sed -nE 's/^requests: [0-9]+ total, ([0-9]+) started,.*/\1/p' "$1" | head -n 1
}

parse_h2load_succeeded() {
  sed -nE 's/^requests: [0-9]+ total, [0-9]+ started, [0-9]+ done, ([0-9]+) succeeded,.*/\1/p' "$1" | head -n 1
}

parse_h2load_failed() {
  sed -nE 's/^requests: [0-9]+ total, [0-9]+ started, [0-9]+ done, [0-9]+ succeeded, ([0-9]+) failed,.*/\1/p' "$1" | head -n 1
}

parse_h2load_errored() {
  sed -nE 's/^requests: [0-9]+ total, [0-9]+ started, [0-9]+ done, [0-9]+ succeeded, [0-9]+ failed, ([0-9]+) errored,.*/\1/p' "$1" | head -n 1
}

parse_h2load_2xx() {
  sed -nE 's/^status codes: ([0-9]+) 2xx,.*/\1/p' "$1" | head -n 1
}

wait_for_h2c() {
  local url="$1"
  local probe="$results_dir/h2load_h2c_readiness.txt"
  local started
  local succeeded
  local two_xx

  for _ in {1..80}; do
    h2load -n 1 -c 1 -m 1 "$url" >"$probe" 2>&1 || true

    started="$(parse_h2load_started "$probe")"
    succeeded="$(parse_h2load_succeeded "$probe")"
    two_xx="$(parse_h2load_2xx "$probe")"

    if is_positive "$started" && is_positive "$succeeded" && is_positive "$two_xx"; then
      return 0
    fi

    sleep 0.25
  done

  return 1
}

validate_h2load_output() {
  local output="$1"
  local rps
  local started
  local succeeded
  local failed
  local errored
  local two_xx

  rps="$(parse_h2load_rps "$output")"
  started="$(parse_h2load_started "$output")"
  succeeded="$(parse_h2load_succeeded "$output")"
  failed="$(parse_h2load_failed "$output")"
  errored="$(parse_h2load_errored "$output")"
  two_xx="$(parse_h2load_2xx "$output")"

  if ! is_positive "$started"; then
    return 2
  fi

  if ! is_positive "$rps" || ! is_positive "$succeeded" || ! is_positive "$two_xx"; then
    return 1
  fi

  if ! is_zero "${failed:-0}" || ! is_zero "${errored:-0}"; then
    return 1
  fi

  printf '%s\t%s\t%s\t%s\t%s' "$rps" "$started" "$succeeded" "${failed:-0}" "${errored:-0}"
}

run_http1_tool() {
  local scenario="$1"
  local tool="$2"
  local url="$3"
  local output="$4"
  local rps

  if [ "$(http_status "$url")" != "200" ]; then
    record_result "http1" "$scenario" "$tool" "failed" "" "" "" "" "" "$(basename "$output")" "pre-benchmark status was not 200"
    fail_with_logs "$tool $scenario pre-check did not return HTTP 200"
  fi

  if ! bash "scripts/bench/$tool.sh" "$url" "$output"; then
    record_result "http1" "$scenario" "$tool" "failed" "" "" "" "" "" "$(basename "$output")" "$tool returned non-zero"
    fail_with_logs "$tool benchmark failed for $scenario"
  fi

  case "$tool" in
    wrk)
      rps="$(validate_wrk_output "$output" || true)"
      ;;
    hey)
      rps="$(validate_hey_output "$output" || true)"
      ;;
    *)
      fail_with_logs "unknown HTTP/1 benchmark tool: $tool"
      ;;
  esac

  if [ -z "$rps" ]; then
    record_result "http1" "$scenario" "$tool" "failed" "" "" "" "" "" "$(basename "$output")" "no successful HTTP 200 responses were parsed"
    fail_with_logs "$tool output validation failed for $scenario"
  fi

  if [ "$(http_status "$url")" != "200" ]; then
    record_result "http1" "$scenario" "$tool" "failed" "$rps" "" "" "" "" "$(basename "$output")" "post-benchmark status was not 200"
    fail_with_logs "$tool $scenario post-check did not return HTTP 200"
  fi

  record_result "http1" "$scenario" "$tool" "ok" "$rps" "" "" "" "" "$(basename "$output")" ""
}

run_http1_target() {
  local bin="$1"
  local scenario="$2"
  local path="$3"
  local url="$host$path"

  start_server "$bin" "http1_$scenario"

  if ! wait_for_http1_200 "$url"; then
    record_result "http1" "$scenario" "server" "failed" "" "" "" "" "" "$(basename "$current_log")" "server did not become ready with HTTP 200"
    fail_with_logs "HTTP/1 benchmark server $bin did not become ready at $url"
  fi

  run_http1_tool "$scenario" "wrk" "$url" "$results_dir/wrk_${scenario}.txt"
  run_http1_tool "$scenario" "hey" "$url" "$results_dir/hey_${scenario}.txt"

  stop_server
}

h2c_is_enabled() {
  case "$h2c_enabled" in
    0|false|FALSE|no|NO|off|OFF)
      return 1
      ;;
    *)
      return 0
      ;;
  esac
}

run_h2c_plaintext() {
  local scenario="h2c_plaintext"
  local output="$results_dir/h2load_h2c_plaintext.txt"
  local url="$host/plaintext"
  local parsed
  local rps
  local started
  local succeeded
  local failed
  local errored

  if ! h2c_is_enabled; then
    echo "h2c benchmark server is disabled with BENCH_H2C=$h2c_enabled; skipping h2load" | tee "$output"
    record_result "h2c" "plaintext" "h2load" "skipped" "" "" "" "" "" "$(basename "$output")" "h2c server disabled"
    return 0
  fi

  if ! command -v h2load >/dev/null 2>&1; then
    echo "h2load is not installed; skipping h2c benchmark" | tee "$output"
    record_result "h2c" "plaintext" "h2load" "skipped" "" "" "" "" "" "$(basename "$output")" "h2load not installed"
    return 0
  fi

  start_server "h2c" "$scenario"

  if ! wait_for_h2c "$url"; then
    record_result "h2c" "plaintext" "h2load" "failed" "" "0" "0" "" "" "$(basename "$output")" "h2c server did not become ready"
    fail_with_logs "h2c benchmark server did not become ready at $url"
  fi

  if ! bash scripts/bench/h2load.sh "$url" "$output"; then
    record_result "h2c" "plaintext" "h2load" "failed" "" "" "" "" "" "$(basename "$output")" "h2load returned non-zero"
    fail_with_logs "h2load benchmark failed for h2c plaintext"
  fi

  parsed="$(validate_h2load_output "$output" || true)"
  if [ -z "$parsed" ]; then
    started="$(parse_h2load_started "$output")"
    if ! is_positive "${started:-0}"; then
      record_result "h2c" "plaintext" "h2load" "failed" "" "${started:-0}" "0" "" "" "$(basename "$output")" "h2load started zero requests"
      fail_with_logs "h2load started zero requests against the enabled h2c server"
    fi

    record_result "h2c" "plaintext" "h2load" "failed" "" "${started:-}" "$(parse_h2load_succeeded "$output")" "$(parse_h2load_failed "$output")" "$(parse_h2load_errored "$output")" "$(basename "$output")" "h2load output validation failed"
    fail_with_logs "h2load output validation failed for h2c plaintext"
  fi

  IFS=$'\t' read -r rps started succeeded failed errored <<<"$parsed"
  record_result "h2c" "plaintext" "h2load" "ok" "$rps" "$started" "$succeeded" "$failed" "$errored" "$(basename "$output")" ""

  stop_server
}

require_http1_tools() {
  if ! command -v wrk >/dev/null 2>&1; then
    fail_with_logs "wrk is required for HTTP/1 benchmarks"
  fi

  if ! command -v hey >/dev/null 2>&1; then
    fail_with_logs "hey is required for HTTP/1 benchmarks"
  fi
}

build_benchmark_bins() {
  local http1_bins=(plaintext json path_params middleware static_files)
  local bin

  for bin in "${http1_bins[@]}"; do
    cargo build --manifest-path "$manifest" --release --bin "$bin"
  done

  if h2c_is_enabled; then
    cargo build --manifest-path "$manifest" --release --bin h2c
  fi
}

require_http1_tools
build_benchmark_bins

run_http1_target "plaintext" "plaintext" "/plaintext"
run_http1_target "json" "json" "/json"
run_http1_target "path_params" "path_params" "/users/42"
run_http1_target "middleware" "middleware" "/middleware"
run_http1_target "static_files" "static_files" "/static/"
run_h2c_plaintext

write_summary_files

echo "benchmark results written to $results_dir"
