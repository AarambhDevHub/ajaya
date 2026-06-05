#!/usr/bin/env bash
set -euo pipefail

url="${1:-http://127.0.0.1:8080/plaintext}"
output="${2:-target/bench-results/wrk.txt}"
duration="${WRK_DURATION:-10s}"
threads="${WRK_THREADS:-4}"
connections="${WRK_CONNECTIONS:-100}"

mkdir -p "$(dirname "$output")"

if ! command -v wrk >/dev/null 2>&1; then
  echo "wrk is not installed; skipping $url" | tee "$output"
  exit 0
fi

wrk -t"$threads" -c"$connections" -d"$duration" "$url" | tee "$output"
