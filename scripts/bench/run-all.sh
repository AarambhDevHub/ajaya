#!/usr/bin/env bash
set -euo pipefail

manifest="examples/benchmarks/Cargo.toml"
results_dir="${BENCH_RESULTS_DIR:-target/bench-results}"
host="${BENCH_HOST:-http://127.0.0.1:8080}"
port="${BENCH_PORT:-8080}"
profile="${BENCH_PROFILE:-manual}"
h2c_enabled="${BENCH_H2C:-1}"
summary_tsv="$results_dir/summary.tsv"
metadata_file="$results_dir/environment.txt"

if [ "$profile" = "smoke" ]; then
  runs="${BENCH_RUNS:-1}"
  export WRK_DURATION="${WRK_DURATION:-10s}"
  export HEY_DURATION="${HEY_DURATION:-10s}"
  export H2LOAD_DURATION="${H2LOAD_DURATION:-10s}"
else
  runs="${BENCH_RUNS:-3}"
  export WRK_DURATION="${WRK_DURATION:-30s}"
  export HEY_DURATION="${HEY_DURATION:-30s}"
  export H2LOAD_DURATION="${H2LOAD_DURATION:-30s}"
fi

export AB_REQUESTS="${AB_REQUESTS:-100000}"
export AB_CONNECTIONS="${AB_CONNECTIONS:-100}"
export AB_KEEPALIVE="${AB_KEEPALIVE:-1}"
export WRK_THREADS="${WRK_THREADS:-4}"
export WRK_CONNECTIONS="${WRK_CONNECTIONS:-100}"
export HEY_CONNECTIONS="${HEY_CONNECTIONS:-100}"
export H2LOAD_REQUESTS="${H2LOAD_REQUESTS:-10000}"
export H2LOAD_CLIENTS="${H2LOAD_CLIENTS:-100}"
export H2LOAD_STREAMS="${H2LOAD_STREAMS:-100}"
export H2LOAD_THREADS="${H2LOAD_THREADS:-4}"
bench_rustflags="${BENCH_RUSTFLAGS:-}"

warmup_duration="${BENCH_WARMUP_DURATION:-5s}"
current_pid=""
current_log=""
had_failure=0

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
  if [ -n "${1:-}" ] && [ "$1" != "-" ]; then
    printf '%s' "$1"
  else
    printf 'null'
  fi
}

tool_version() {
  local tool="$1"
  shift

  if command -v "$tool" >/dev/null 2>&1; then
    "$tool" "$@" 2>&1 | head -n 1
  else
    printf '%s not installed\n' "$tool"
  fi
}

record_result() {
  local mode="$1"
  local scenario="$2"
  local tool="$3"
  local status="$4"
  local run_count="${5:-}"
  local median_rps="${6:-}"
  local min_rps="${7:-}"
  local max_rps="${8:-}"
  local total_requests="${9:-}"
  local failed_requests="${10:-}"
  local avg_latency="${11:-}"
  local p50="${12:-}"
  local p90="${13:-}"
  local p95="${14:-}"
  local p99="${15:-}"
  local raw="${16:-}"
  local note="${17:-}"
  local fields=(
    "$mode" "$scenario" "$tool" "$status" "$run_count" "$median_rps" "$min_rps" "$max_rps"
    "$total_requests" "$failed_requests" "$avg_latency" "$p50" "$p90" "$p95" "$p99" "$raw" "$note"
  )

  for index in "${!fields[@]}"; do
    if [ -z "${fields[$index]}" ]; then
      fields[$index]="-"
    fi
  done

  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "${fields[@]}" \
    >>"$summary_tsv"
}

write_metadata() {
  {
    echo "profile=$profile"
    echo "runs=$runs"
    echo "warmup_duration=$warmup_duration"
    echo "wrk_duration=$WRK_DURATION"
    echo "hey_duration=$HEY_DURATION"
    echo "ab_requests=$AB_REQUESTS"
    echo "ab_connections=$AB_CONNECTIONS"
    echo "ab_keepalive=$AB_KEEPALIVE"
    echo "h2load_duration=$H2LOAD_DURATION"
    echo "h2load_requests=$H2LOAD_REQUESTS"
    echo "commit=$(git rev-parse HEAD 2>/dev/null || true)"
    echo "commit_short=$(git rev-parse --short HEAD 2>/dev/null || true)"
    echo "branch=$(git branch --show-current 2>/dev/null || true)"
    echo "rustc=$(rustc -V 2>/dev/null || true)"
    echo "kernel=$(uname -a 2>/dev/null || true)"
    if [ -r /etc/os-release ]; then
      . /etc/os-release
      echo "os=${PRETTY_NAME:-}"
    fi
    echo "build_profile=release"
    echo "bench_rustflags=$bench_rustflags"
    echo "wrk=$(tool_version wrk --version)"
    echo "hey=$(tool_version hey -version)"
    echo "ab=$(tool_version ab -V)"
    echo "h2load=$(tool_version h2load -v)"
    if command -v lscpu >/dev/null 2>&1; then
      lscpu | sed -n 's/^\(Model name\|CPU(s)\|Thread(s) per core\|Core(s) per socket\|Socket(s)\): */cpu_\1=/p'
    fi
  } >"$metadata_file"
}

write_summary_files() {
  {
    echo "# Arvik Benchmark Summary"
    echo
    echo "Raw benchmark output is saved next to this summary. Smoke runs validate the benchmark path; manual runs are intended for artifact review."
    echo
    echo "| Mode | Scenario | Tool | Status | Runs | Median RPS | Min RPS | Max RPS | Total requests | Failed requests | Avg latency | p50 | p90 | p95 | p99 | Raw output | Note |"
    echo "| --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- | --- | --- | --- | --- | --- |"
    while IFS=$'\t' read -r mode scenario tool status run_count median_rps min_rps max_rps total_requests failed_requests avg_latency p50 p90 p95 p99 raw note; do
      [ -n "$mode" ] || continue
      echo "| $mode | $scenario | $tool | $status | ${run_count:-} | ${median_rps:-} | ${min_rps:-} | ${max_rps:-} | ${total_requests:-} | ${failed_requests:-} | ${avg_latency:-} | ${p50:-} | ${p90:-} | ${p95:-} | ${p99:-} | $raw | $note |"
    done <"$summary_tsv"
  } >"$results_dir/summary.md"

  {
    echo "{"
    printf '  "generated_at": "%s",\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    printf '  "profile": "%s",\n' "$(json_escape "$profile")"
    printf '  "runs": %s,\n' "$(json_number_or_null "$runs")"
    printf '  "environment_file": "%s",\n' "$(json_escape "$(basename "$metadata_file")")"
    echo '  "results": ['

    local first=1
    while IFS=$'\t' read -r mode scenario tool status run_count median_rps min_rps max_rps total_requests failed_requests avg_latency p50 p90 p95 p99 raw note; do
      [ -n "$mode" ] || continue
      if [ "$first" -eq 0 ]; then
        echo ","
      fi
      first=0

      printf '    {"mode":"%s","scenario":"%s","tool":"%s","status":"%s","runs":%s,"median_rps":%s,"min_rps":%s,"max_rps":%s,"total_requests":%s,"failed_requests":%s,"avg_latency":"%s","p50":"%s","p90":"%s","p95":"%s","p99":"%s","raw_output":"%s","note":"%s"}' \
        "$(json_escape "$mode")" \
        "$(json_escape "$scenario")" \
        "$(json_escape "$tool")" \
        "$(json_escape "$status")" \
        "$(json_number_or_null "$run_count")" \
        "$(json_number_or_null "$median_rps")" \
        "$(json_number_or_null "$min_rps")" \
        "$(json_number_or_null "$max_rps")" \
        "$(json_number_or_null "$total_requests")" \
        "$(json_number_or_null "$failed_requests")" \
        "$(json_escape "$avg_latency")" \
        "$(json_escape "$p50")" \
        "$(json_escape "$p90")" \
        "$(json_escape "$p95")" \
        "$(json_escape "$p99")" \
        "$(json_escape "$raw")" \
        "$(json_escape "$note")"
    done <"$summary_tsv"

    echo
    echo '  ]'
    echo "}"
  } >"$results_dir/summary.json"
}

write_comparison_artifacts() {
  {
    echo "# Arvik Before/After"
    echo
    echo "This artifact is generated for local before/after analysis. It records the current Arvik run from \`summary.tsv\`; attach a baseline run when publishing an engineering comparison."
    echo
    echo "| Scenario | Tool | Current median RPS | Current min RPS | Current max RPS | Runs |"
    echo "| --- | --- | ---: | ---: | ---: | ---: |"
    while IFS=$'\t' read -r mode scenario tool status run_count median_rps min_rps max_rps _total_requests _failed_requests _avg_latency _p50 _p90 _p95 _p99 _raw _note; do
      [ "$status" = "ok" ] || continue
      echo "| $scenario | $tool | $median_rps | $min_rps | $max_rps | $run_count |"
    done <"$summary_tsv"
  } >"$results_dir/before_after_arvik.md"

  {
    echo "# Cross-Framework Common Endpoints"
    echo
    echo "The in-repo runner executes Arvik scenarios. Axum and Actix baseline servers live under \`examples/benchmarks/baselines/\` and should be run separately with the same tool versions, duration, concurrency, and machine state."
    echo
    echo "Use these numbers only as engineering baselines. Do not turn cross-framework benchmark output into marketing claims."
    echo
    echo "| Endpoint | Framework | wrk RPS | wrk avg latency | hey RPS | hey avg latency | ab RPS | ab avg latency |"
    echo "| --- | --- | ---: | --- | ---: | --- | ---: | --- |"
    echo "| /plaintext | Arvik current run | see summary.md | see raw output | see summary.md | see raw output | see summary.md | see raw output |"
    echo "| /json | Arvik current run | see summary.md | see raw output | see summary.md | see raw output | see summary.md | see raw output |"
  } >"$results_dir/cross_framework_common_endpoints.md"
}

is_positive() {
  awk -v value="${1:-0}" 'BEGIN { exit !(value + 0 > 0) }'
}

is_zero() {
  awk -v value="${1:-0}" 'BEGIN { exit !(value + 0 == 0) }'
}

assert_port_free() {
  if timeout 1 bash -c ":</dev/tcp/127.0.0.1/$port" >/dev/null 2>&1; then
    echo "port $port is already in use before starting the benchmark server" >&2
    exit 1
  fi
}

start_server() {
  local bin="$1"
  local label="$2"
  shift 2
  local cargo_env=()

  if [ -n "$bench_rustflags" ]; then
    cargo_env=(RUSTFLAGS="$bench_rustflags")
  fi

  cleanup_current_server
  assert_port_free

  current_log="$results_dir/${label}.server.log"
  env "${cargo_env[@]}" "$@" cargo run --manifest-path "$manifest" --release --bin "$bin" >"$current_log" 2>&1 &
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

parse_wrk_latency() {
  awk '/^[[:space:]]*Latency/ { print $2; exit }' "$1"
}

parse_wrk_failed() {
  awk '/Non-2xx or 3xx responses:/ { print $5; found = 1 } END { if (!found) print 0 }' "$1"
}

parse_hey_rps() {
  awk '/Requests\/sec:/ { print $2; exit }' "$1"
}

parse_hey_latency() {
  awk '/Average:/ { print $2; exit }' "$1"
}

parse_hey_percentile() {
  local percentile="$2"
  awk -v percentile="$percentile" '$1 == percentile && $2 == "in" { print $3; exit }' "$1"
}

parse_hey_total() {
  awk '/^[[:space:]]*\[[0-9][0-9][0-9]\]/ { total += $2 } END { print total + 0 }' "$1"
}

parse_hey_failed() {
  awk '/^[[:space:]]*\[[0-9][0-9][0-9]\]/ && $1 != "[200]" { total += $2 } END { print total + 0 }' "$1"
}

parse_ab_rps() {
  sed -nE 's/^Requests per second:[[:space:]]*([0-9.]+).*/\1/p' "$1" | head -n 1
}

parse_ab_requests() {
  awk '/Complete requests:/ { print $3; exit }' "$1"
}

parse_ab_failed() {
  awk '/Failed requests:/ { print $3; exit }' "$1"
}

parse_ab_latency() {
  awk '/Time per request:/ && /\(mean\)$/ { print $4 "ms"; exit }' "$1"
}

parse_ab_percentile() {
  local percentile="$2"
  awk -v percentile="$percentile%" '$1 == percentile { print $2 "ms"; exit }' "$1"
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

median_from_file() {
  sort -n "$1" | awk '{ values[NR] = $1 } END { if (NR == 0) exit 1; if (NR % 2 == 1) print values[(NR + 1) / 2]; else printf "%.2f", (values[NR / 2] + values[NR / 2 + 1]) / 2 }'
}

min_from_file() {
  sort -n "$1" | head -n 1
}

max_from_file() {
  sort -n "$1" | tail -n 1
}

sum_column() {
  local file="$1"
  local column="$2"
  awk -v column="$column" '{ total += $column } END { print total + 0 }' "$file"
}

last_column() {
  local file="$1"
  local column="$2"
  awk -v column="$column" 'NF >= column && $column != "" { value = $column } END { print value }' "$file"
}

warmup_http1() {
  local scenario="$1"
  local url="$2"
  local output="$results_dir/warmup_${scenario}.txt"

  if command -v wrk >/dev/null 2>&1; then
    WRK_DURATION="$warmup_duration" bash scripts/bench/wrk.sh "$url" "$output" >/dev/null 2>&1 || true
  else
    : >"$output"
    for _ in {1..20}; do
      curl -fsS -o /dev/null "$url" >>"$output" 2>&1 || true
    done
  fi
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

    if is_positive "${started:-0}" && is_positive "${succeeded:-0}" && is_positive "${two_xx:-0}"; then
      return 0
    fi

    sleep 0.25
  done

  return 1
}

run_http1_tool() {
  local scenario="$1"
  local tool="$2"
  local url="$3"
  local script="scripts/bench/$tool.sh"
  local metrics="$results_dir/${tool}_${scenario}.metrics"
  local rps_file="$results_dir/${tool}_${scenario}.rps"
  local successful_runs=0
  local output
  local rps
  local total
  local failed
  local avg
  local p50
  local p90
  local p95
  local p99

  : >"$metrics"
  : >"$rps_file"

  if ! command -v "$tool" >/dev/null 2>&1; then
    output="$results_dir/${tool}_${scenario}_skipped.txt"
    echo "$tool is not installed; skipping $url" >"$output"
    record_result "http1" "$scenario" "$tool" "skipped" "" "" "" "" "" "" "" "" "" "" "" "$(basename "$output")" "$tool not installed"
    return 0
  fi

  for run in $(seq 1 "$runs"); do
    output="$results_dir/${tool}_${scenario}_run${run}.txt"
    if ! bash "$script" "$url" "$output"; then
      had_failure=1
      record_result "http1" "$scenario" "$tool" "failed" "$run" "" "" "" "" "" "" "" "" "" "" "$(basename "$output")" "$tool returned non-zero"
      continue
    fi

    case "$tool" in
      wrk)
        rps="$(parse_wrk_rps "$output")"
        total="$(parse_wrk_requests "$output")"
        failed="$(parse_wrk_failed "$output")"
        avg="$(parse_wrk_latency "$output")"
        p50=""
        p90=""
        p95=""
        p99=""
        ;;
      hey)
        rps="$(parse_hey_rps "$output")"
        total="$(parse_hey_total "$output")"
        failed="$(parse_hey_failed "$output")"
        avg="$(parse_hey_latency "$output")"
        p50="$(parse_hey_percentile "$output" "50%")"
        p90="$(parse_hey_percentile "$output" "90%")"
        p95="$(parse_hey_percentile "$output" "95%")"
        p99="$(parse_hey_percentile "$output" "99%")"
        ;;
      ab)
        rps="$(parse_ab_rps "$output")"
        total="$(parse_ab_requests "$output")"
        failed="$(parse_ab_failed "$output")"
        avg="$(parse_ab_latency "$output")"
        p50="$(parse_ab_percentile "$output" "50")"
        p90="$(parse_ab_percentile "$output" "90")"
        p95="$(parse_ab_percentile "$output" "95")"
        p99="$(parse_ab_percentile "$output" "99")"
        ;;
      *)
        echo "unknown HTTP/1 benchmark tool: $tool" >&2
        exit 1
        ;;
    esac

    if ! is_positive "${rps:-0}"; then
      had_failure=1
      record_result "http1" "$scenario" "$tool" "failed" "$run" "" "" "" "${total:-}" "${failed:-}" "$avg" "$p50" "$p90" "$p95" "$p99" "$(basename "$output")" "no positive RPS parsed"
      continue
    fi

    if ! is_zero "${failed:-0}"; then
      had_failure=1
    fi

    successful_runs=$((successful_runs + 1))
    printf '%s\n' "$rps" >>"$rps_file"
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$rps" "${total:-0}" "${failed:-0}" "$avg" "$p50" "$p90" "$p95" "$p99" >>"$metrics"
  done

  if [ "$successful_runs" -eq 0 ]; then
    record_result "http1" "$scenario" "$tool" "failed" "0" "" "" "" "" "" "" "" "" "" "" "${tool}_${scenario}_run*.txt" "no successful measured runs"
    had_failure=1
    return 0
  fi

  record_result \
    "http1" \
    "$scenario" \
    "$tool" \
    "ok" \
    "$successful_runs" \
    "$(median_from_file "$rps_file")" \
    "$(min_from_file "$rps_file")" \
    "$(max_from_file "$rps_file")" \
    "$(sum_column "$metrics" 2)" \
    "$(sum_column "$metrics" 3)" \
    "$(last_column "$metrics" 4)" \
    "$(last_column "$metrics" 5)" \
    "$(last_column "$metrics" 6)" \
    "$(last_column "$metrics" 7)" \
    "$(last_column "$metrics" 8)" \
    "${tool}_${scenario}_run*.txt" \
    ""
}

run_http1_target() {
  local bin="$1"
  local scenario="$2"
  local path="$3"
  shift 3
  local url="$host$path"

  start_server "$bin" "http1_$scenario" "$@"

  if ! wait_for_http1_200 "$url"; then
    record_result "http1" "$scenario" "server" "failed" "" "" "" "" "" "" "" "" "" "" "" "$(basename "$current_log")" "server did not become ready with HTTP 200"
    had_failure=1
    stop_server
    return 0
  fi

  warmup_http1 "$scenario" "$url"
  run_http1_tool "$scenario" "wrk" "$url"
  run_http1_tool "$scenario" "hey" "$url"
  run_http1_tool "$scenario" "ab" "$url"

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
  local output
  local metrics="$results_dir/h2load_${scenario}.metrics"
  local rps_file="$results_dir/h2load_${scenario}.rps"
  local url="$host/plaintext"
  local successful_runs=0
  local rps
  local started
  local succeeded
  local failed
  local errored
  local two_xx

  : >"$metrics"
  : >"$rps_file"

  if ! h2c_is_enabled; then
    output="$results_dir/h2load_${scenario}_skipped.txt"
    echo "h2c benchmark server is disabled with BENCH_H2C=$h2c_enabled" >"$output"
    record_result "h2c" "plaintext" "h2load" "skipped" "" "" "" "" "" "" "" "" "" "" "" "$(basename "$output")" "h2c disabled"
    return 0
  fi

  if ! command -v h2load >/dev/null 2>&1; then
    output="$results_dir/h2load_${scenario}_skipped.txt"
    echo "h2load is not installed; skipping $url" >"$output"
    record_result "h2c" "plaintext" "h2load" "skipped" "" "" "" "" "" "" "" "" "" "" "" "$(basename "$output")" "h2load not installed"
    return 0
  fi

  start_server "h2c" "$scenario"

  if ! wait_for_h2c "$url"; then
    record_result "h2c" "plaintext" "h2load" "failed" "" "" "" "" "" "" "" "" "" "" "" "$(basename "$current_log")" "h2c server did not become ready"
    had_failure=1
    stop_server
    return 0
  fi

  for run in $(seq 1 "$runs"); do
    output="$results_dir/h2load_${scenario}_run${run}.txt"
    if ! bash scripts/bench/h2load.sh "$url" "$output"; then
      had_failure=1
      record_result "h2c" "plaintext" "h2load" "failed" "$run" "" "" "" "" "" "" "" "" "" "" "$(basename "$output")" "h2load returned non-zero"
      continue
    fi

    rps="$(parse_h2load_rps "$output")"
    started="$(parse_h2load_started "$output")"
    succeeded="$(parse_h2load_succeeded "$output")"
    failed="$(parse_h2load_failed "$output")"
    errored="$(parse_h2load_errored "$output")"
    two_xx="$(parse_h2load_2xx "$output")"

    if ! is_positive "${rps:-0}" || ! is_positive "${succeeded:-0}" || ! is_positive "${two_xx:-0}"; then
      had_failure=1
      record_result "h2c" "plaintext" "h2load" "failed" "$run" "" "" "" "${started:-}" "${failed:-}" "" "" "" "" "" "$(basename "$output")" "h2load output validation failed"
      continue
    fi

    if ! is_zero "${failed:-0}" || ! is_zero "${errored:-0}"; then
      had_failure=1
    fi

    successful_runs=$((successful_runs + 1))
    printf '%s\n' "$rps" >>"$rps_file"
    printf '%s\t%s\t%s\n' "$rps" "${started:-0}" "$((${failed:-0} + ${errored:-0}))" >>"$metrics"
  done

  stop_server

  if [ "$successful_runs" -eq 0 ]; then
    record_result "h2c" "plaintext" "h2load" "failed" "0" "" "" "" "" "" "" "" "" "" "" "h2load_${scenario}_run*.txt" "no successful measured runs"
    had_failure=1
    return 0
  fi

  record_result \
    "h2c" \
    "plaintext" \
    "h2load" \
    "ok" \
    "$successful_runs" \
    "$(median_from_file "$rps_file")" \
    "$(min_from_file "$rps_file")" \
    "$(max_from_file "$rps_file")" \
    "$(sum_column "$metrics" 2)" \
    "$(sum_column "$metrics" 3)" \
    "" \
    "" \
    "" \
    "" \
    "" \
    "h2load_${scenario}_run*.txt" \
    ""
}

build_benchmark_bins() {
  local bins=(plaintext json path_params middleware static_files)
  local bin
  local cargo_env=()

  if [ -n "$bench_rustflags" ]; then
    cargo_env=(RUSTFLAGS="$bench_rustflags")
  fi

  for bin in "${bins[@]}"; do
    env "${cargo_env[@]}" cargo build --manifest-path "$manifest" --release --bin "$bin"
  done

  if h2c_is_enabled; then
    env "${cargo_env[@]}" cargo build --manifest-path "$manifest" --release --bin h2c
  fi
}

write_metadata
build_benchmark_bins

run_http1_target "plaintext" "plaintext" "/plaintext"
run_http1_target "json" "json" "/json"
run_http1_target "path_params" "path_params" "/users/42"

for middleware_variant in none request_id headers tracing cors rate_limit compression full; do
  run_http1_target "middleware" "middleware_${middleware_variant}" "/middleware" "BENCH_MIDDLEWARE_SCENARIO=$middleware_variant"
done

run_http1_target "static_files" "static_files" "/static/"
run_h2c_plaintext

write_summary_files
write_comparison_artifacts

echo "benchmark results written to $results_dir"

if [ "$had_failure" -ne 0 ]; then
  exit 1
fi
