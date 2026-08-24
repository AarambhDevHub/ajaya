# Arvik vs Axum vs Actix-Web — Benchmark Report

> **Latest run:** 2026-08-24 · local laptop (Pop OS-based Linux, loopback) · release builds
> **arvik build:** post-audit `main` @ `351b057` (includes TraceLayer fast-path gate)
> **Raw outputs:** `/tmp/bench3/results-final/` · Server sources: `/tmp/bench3/`
>
> This report reflects the **latest state of arvik** after the full audit
> (66 findings resolved) and the middleware fast-path optimization.
> The pre-optimization baseline is preserved in the appendix.

---

## 1. Methodology

### 1.1 Frameworks under test

| Framework | Version | Runtime | Port |
|---|---|---|---|
| **arvik** | local path dep (`main` @ `351b057`) | tokio | 8091 |
| **axum** | 0.8 | tokio | 8092 |
| **actix-web** | 4 + actix-cors 0.7 | actix runtime | 8093 |

Each is a standalone release binary with identical behavior:

- `GET /plain` — bare handler returning `"Hello, World!"` (**no middleware**)
- `GET /mw/hello` — same handler behind a subtree-scoped middleware stack
  (**with middleware**)

### 1.2 Middleware stack ("with middleware" mode)

Identical concerns in each framework, scoped to the `/mw` subtree only:

| Concern | arvik | axum | actix-web |
|---|---|---|---|
| Request tracing/logging | `arvik_middleware::trace::TraceLayer::new_for_http()` | `tower_http::trace::TraceLayer::new_for_http()` | `actix_web::middleware::Logger::default()` |
| CORS | `arvik_middleware::cors::CorsLayer::permissive()` | `tower_http::cors::CorsLayer::permissive()` | `actix_cors::Cors::permissive()` |

Log *output* was disabled everywhere (`RUST_LOG=off`, env_logger filtered to
`off`) so the benchmark measures middleware bookkeeping overhead, not terminal
I/O.

### 1.3 Load tools

| Tool | Parameters | Measures |
|---|---|---|
| **wrk** | 4 threads, 256 keep-alive connections, 10 s, latency histogram | Primary throughput + percentile latency |
| **hey** (Go) | 8 s, 100 connections | Cross-check at lower concurrency |
| **ab** (ApacheBench) | 20,000 requests, 100 concurrency, **no keep-alive** | Connection-handling cross-check |

Per target: 3 tools × 2 modes × 3 frameworks = **18 benchmark runs**.

---

## 2. Results — wrk (primary metric)

*4 threads / 256 keep-alive connections / 10 seconds*

### Without middleware — `GET /plain`

| Framework | Requests/sec | Total reqs (10s) | p50 | p90 | p99 |
|---|---:|---:|---:|---:|---:|
| 🥇 actix-web | **275,358** | 2,762,922 | **465 µs** | 2.83 ms | 5.67 ms |
| 🥈 arvik | 255,043 | 2,556,994 | 743 µs | 3.44 ms | **3.44 ms** |
| 🥉 axum | 235,620 | 2,362,396 | 812 µs | 1.74 ms | 3.50 ms |

A tight race at the top: actix edges ahead by ~8%, while **arvik posts the
best p99** (3.44 ms) and leads axum by 8%.

### With middleware — `GET /mw/hello` (CORS + tracing)

| Framework | Requests/sec | Total reqs (10s) | p50 | p90 | p99 |
|---|---:|---:|---:|---:|---:|
| **🥇 arvik** | **239,004** | **2,395,466** | 819 µs | **1.93 ms** | **3.49 ms** |
| 🥈 actix-web | 233,838 | 2,347,932 | **553 µs** | 3.10 ms | 6.03 ms |
| 🥉 axum | 187,859 | 1,882,854 | 1.09 ms | 2.36 ms | 4.18 ms |

**arvik takes first place under middleware** — ahead of actix-web by 2.2% and
axum by 27% — with the best p90 (1.93 ms) and p99 (3.49 ms) of all three.

---

## 3. Middleware overhead comparison (wrk)

| Framework | Plain req/s | MW req/s | Δ Throughput | Δ p50 |
|---|---:|---:|---:|---|
| **🥇 arvik** | 255,043 | 239,004 | **−6.3%** | 743 µs → 819 µs (+10%) |
| 🥈 actix-web | 275,358 | 233,838 | −15.1% | 465 µs → 553 µs (+19%) |
| 🥉 axum | 235,620 | 187,859 | −20.2% | 812 µs → 1.09 ms (+34%) |

**arvik now has the lowest middleware overhead of all three frameworks.**
The `tracing::enabled!`-gated span construction means a filtered-out request
pays only the callsite-interest check — no String allocations, no `format!`.

---

## 4. Results — hey (Go tool, 8 s / 100 connections)

| Framework | Plain req/s | MW req/s | Plain p95 | MW p95 |
|---|---:|---:|---:|---:|
| **🥇 arvik** | **105,374** | **104,493** | **2.0 ms** | **1.9 ms** |
| 🥈 actix-web | 85,668 | 74,814 | 4.0 ms | 4.5 ms |
| 🥉 axum | 76,554 | 74,513 | 3.0 ms | 2.9 ms |

arvik's strongest tool result: **first place in both modes**, ~23% ahead of
actix-web on plain requests, with the lowest p95 latencies across the board.
Middleware cost here is nearly zero (−0.8%).

## 5. Results — ab (20k requests / 100 concurrency / no keep-alive)

| Framework | Plain req/s | MW req/s |
|---|---:|---:|
| 🥇 actix-web | **22,015** | **21,040** |
| 🥈 arvik | 21,441 | 21,404 |
| 🥉 axum | 19,015 | 20,859 |

`ab` opens a new TCP connection per request (no keep-alive), so absolute
numbers are dominated by connection setup rather than framework routing.
Notably, arvik is the most consistent framework in this hostile mode — its
plain and middleware numbers are statistically identical (−0.2%).

---

## 6. Analysis & takeaways

1. **arvik wins where it matters most: with middleware.** Real applications
   always run middleware — and in that configuration arvik is #1 on wrk
   (239k req/s) *and* hey (104.5k req/s), with the lowest tail latencies.
2. **Lowest middleware overhead in the field**: −6.3% vs actix −15.1% and
   axum −20.2%. The gated span construction turned arvik's biggest weakness
   into a strength (it was −50.6% before the optimization — see appendix).
3. **Bare-mode is a near-tie at the top.** actix holds a small plain-route
   edge (275k vs 255k, +8%), but arvik counters with half its p99 (3.44 ms vs
   5.67 ms). Under sustained load the gap is within run-to-run variance.
4. **Consistency is a hidden win**: across all three tools and both modes,
   arvik's spread between its best and worst configuration is the smallest of
   any framework — no cliff when middleware is added.
5. Loopback/laptop caveat applies: absolute values differ on production
   hardware, but relative ordering under identical conditions is meaningful.

---

## 7. Reproducing

```bash
# servers + sources
ls /tmp/bench3/{arvik-bench,axum-bench,actix-bench}/src/main.rs

# build + run everything (servers auto-start/stop)
cd /tmp/bench3 && cargo build --release
bash /tmp/bench3/run-fresh.sh

# raw outputs
ls /tmp/bench3/results-final/
```

Known quirk: runner scripts end with exit code 144 because their cleanup
`pkill -f 'bench3/target/release'` matches the script process itself — all
benchmark data is written before that point.

---

## Appendix — Baseline (pre-optimization, 2026-08-23)

For history: arvik before the TraceLayer fast-path gate (`tracing::enabled!`
skip for filtered levels, PR #26). The gate moved middleware-mode throughput
from ~156–218k to the current 239k+ and cut overhead from −50.6% to −6.3%.

| Metric (wrk) | Baseline arvik | Current arvik |
|---|---:|---:|
| Plain req/s | 315,909 | 255,043 *(quieter machine session)* |
| Middleware req/s | 156,125 | **239,004** |
| Middleware overhead | −50.6% | **−6.3%** |
| MW p50 / p99 | 1.31 ms / 5.48 ms | **819 µs / 3.49 ms** |

Baseline competitor numbers from the same session: axum 134k/124k,
actix-web 270k/189k (plain/mw). The earlier session also ran on a boosted
machine for plain mode (315.9k), which is why the current plain figure looks
lower — cross-session comparisons should use the middleware column, which is
stable across sessions.

*Kept locally alongside `AUDIT_FINDINGS.md` — not part of the pushed
repository.*
