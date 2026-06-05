#!/usr/bin/env bash
set -euo pipefail

manifest="examples/benchmarks/Cargo.toml"
results_dir="${BENCH_RESULTS_DIR:-target/bench-results}"
host="${BENCH_HOST:-http://127.0.0.1:8080}"

mkdir -p "$results_dir"

run_server() {
  local bin="$1"
  cargo run --manifest-path "$manifest" --release --bin "$bin" >"$results_dir/$bin.server.log" 2>&1 &
  echo "$!"
}

wait_for_server() {
  local url="$1"
  for _ in {1..80}; do
    if curl -fsS "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  return 1
}

wait_for_h2c() {
  local url="$1"
  if ! command -v h2load >/dev/null 2>&1; then
    echo "h2load is not installed; skipping h2c readiness probe" | tee "$results_dir/h2c.skipped.txt"
    return 2
  fi

  for _ in {1..80}; do
    if h2load -n 1 -c 1 -m 1 "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  return 1
}

run_target() {
  local bin="$1"
  local path="$2"
  local pid
  pid="$(run_server "$bin")"
  trap 'kill "$pid" >/dev/null 2>&1 || true' RETURN

  if ! wait_for_server "$host$path"; then
    echo "server $bin did not become ready" | tee "$results_dir/$bin.error.txt"
    return 1
  fi

  bash scripts/bench/wrk.sh "$host$path" "$results_dir/$bin.wrk.txt"
  bash scripts/bench/hey.sh "$host$path" "$results_dir/$bin.hey.txt"

  kill "$pid" >/dev/null 2>&1 || true
  wait "$pid" 2>/dev/null || true
  trap - RETURN
}

run_h2c_target() {
  local bin="h2c"
  local path="/h2c"
  local pid
  pid="$(run_server "$bin")"
  trap 'kill "$pid" >/dev/null 2>&1 || true' RETURN

  set +e
  wait_for_h2c "$host$path"
  local ready=$?
  set -e

  if [ "$ready" -eq 2 ]; then
    kill "$pid" >/dev/null 2>&1 || true
    wait "$pid" 2>/dev/null || true
    trap - RETURN
    return 0
  fi

  if [ "$ready" -ne 0 ]; then
    echo "server $bin did not become ready for HTTP/2 prior knowledge" | tee "$results_dir/$bin.error.txt"
    return 1
  fi

  bash scripts/bench/h2load.sh "$host$path" "$results_dir/$bin.h2load.txt"

  kill "$pid" >/dev/null 2>&1 || true
  wait "$pid" 2>/dev/null || true
  trap - RETURN
}

cargo build --manifest-path "$manifest" --release

run_target plaintext /plaintext
run_target json /json
run_target path_params /users/42
run_target middleware /middleware
run_target static_files /static/
run_h2c_target

echo "benchmark results written to $results_dir"
