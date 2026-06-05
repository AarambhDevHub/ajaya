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

cargo build --manifest-path "$manifest" --release

run_target plaintext /plaintext
run_target json /json
run_target path_params /users/42
run_target middleware /middleware
run_target static_files /static/
run_target h2c /h2c

echo "benchmark results written to $results_dir"
