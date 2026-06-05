#!/usr/bin/env bash
set -euo pipefail

url="${1:-http://127.0.0.1:8080/plaintext}"
output="${2:-target/bench-results/hey.txt}"
duration="${HEY_DURATION:-10s}"
connections="${HEY_CONNECTIONS:-100}"

mkdir -p "$(dirname "$output")"

if ! command -v hey >/dev/null 2>&1; then
  echo "hey is not installed; skipping $url" | tee "$output"
  exit 0
fi

hey -z "$duration" -c "$connections" "$url" | tee "$output"
