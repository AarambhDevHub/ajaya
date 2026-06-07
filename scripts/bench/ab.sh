#!/usr/bin/env bash
set -euo pipefail

url="${1:-http://127.0.0.1:8080/plaintext}"
output="${2:-target/bench-results/ab_plaintext.txt}"
requests="${AB_REQUESTS:-100000}"
connections="${AB_CONNECTIONS:-100}"
keepalive="${AB_KEEPALIVE:-1}"

mkdir -p "$(dirname "$output")"

if ! command -v ab >/dev/null 2>&1; then
  echo "ab is not installed; cannot benchmark $url" | tee "$output"
  exit 127
fi

args=(-n "$requests" -c "$connections")
case "$keepalive" in
  0|false|FALSE|no|NO|off|OFF)
    ;;
  *)
    args=(-k "${args[@]}")
    ;;
esac

ab "${args[@]}" "$url" | tee "$output"
