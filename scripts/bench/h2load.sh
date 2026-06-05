#!/usr/bin/env bash
set -euo pipefail

url="${1:-http://127.0.0.1:8080/h2c}"
output="${2:-target/bench-results/h2load.txt}"
requests="${H2LOAD_REQUESTS:-10000}"
clients="${H2LOAD_CLIENTS:-100}"
streams="${H2LOAD_STREAMS:-100}"
threads="${H2LOAD_THREADS:-4}"

mkdir -p "$(dirname "$output")"

if ! command -v h2load >/dev/null 2>&1; then
  echo "h2load is not installed; skipping $url" | tee "$output"
  exit 0
fi

h2load -n "$requests" -c "$clients" -m "$streams" -t "$threads" "$url" | tee "$output"
