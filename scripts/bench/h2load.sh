#!/usr/bin/env bash
set -euo pipefail

url="${1:-http://127.0.0.1:8080/plaintext}"
output="${2:-target/bench-results/h2load_h2c_plaintext.txt}"
requests="${H2LOAD_REQUESTS:-10000}"
clients="${H2LOAD_CLIENTS:-100}"
streams="${H2LOAD_STREAMS:-100}"
threads="${H2LOAD_THREADS:-4}"
duration="${H2LOAD_DURATION:-}"

mkdir -p "$(dirname "$output")"

if ! command -v h2load >/dev/null 2>&1; then
  echo "h2load is not installed; skipping $url" | tee "$output"
  exit 0
fi

args=(-c "$clients" -m "$streams" -t "$threads")
if [ -n "$duration" ]; then
  args+=(-D "${duration%s}")
else
  args+=(-n "$requests")
fi

h2load "${args[@]}" "$url" | tee "$output"
