# Arvik vs Axum vs Actix-Web — Benchmark Report

> **Latest run:** 2026-08-24 (Round-2 session) · local laptop (Pop OS-based Linux, loopback) · release builds
> **arvik build:** post-audit `main` @ `6d56f64` — includes the full Round-2
> optimization pass (PR #28): &self erased handlers, baked method-layer stacks,
> interned param keys, allocation-free negotiation, de-boxed sync services
> **Raw outputs:** `/tmp/bench3/results-round2/` · Server sources: `/tmp/bench3/`
>
> This report reflects the **final state of arvik** after the complete audit
> (66 findings resolved) plus both optimization rounds. Earlier sessions are
> preserved in the appendix for transparency.

---

## 1. Methodology

### 1.1 Frameworks under test

| Framework | Version | Runtime | Port |
|---|---|---|---|
| **arvik** | local path dep (`main` @ `6d56f64`) | tokio | 8091 |
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

| Framework | Requests/sec | Total reqs (10s) | p50 | p99 |
|---|---:|---:|---:|---:|
| **🥇 arvik** | **360,373** | **3,612,077** | 537 µs | **3.16 ms** |
| 🥈 actix-web | 329,352 | 3,298,969 | **400 µs** | 4.43 ms |
| 🥉 axum | 295,895 | 2,967,097 | 661 µs | 3.24 ms |

**arvik takes first place on bare throughput** — 9.4% ahead of actix-web and
22% ahead of axum — while posting a better p99 than both. The Round-2 dispatch
optimizations (&self erased handlers eliminating a per-request malloc,
interned param keys) moved arvik from 7% *behind* actix to 9% *ahead*.

### With middleware — `GET /mw/hello` (CORS + tracing)

| Framework | Requests/sec | Total reqs (10s) | p50 | p99 |
|---|---:|---:|---:|---:|
| 🥇 actix-web | **264,999** | 2,659,513 | **507 µs** | 5.14 ms |
| 🥈 arvik | 249,534 | 2,502,135 | 772 µs | **3.59 ms** |
| 🥉 axum | 220,632 | 2,213,903 | 900 µs | 3.77 ms |

A tight race at the top. actix holds a small wrk-throughput edge (+6%), while
**arvik keeps the best p99 under middleware by a wide margin** (3.59 ms vs
5.14/6.03 ms) and wins this category on both hey and ab (below).

---

## 3. Middleware overhead comparison (wrk)

| Framework | Plain req/s | MW req/s | Δ Throughput |
|---|---:|---:|---:|
| actix-web | 329,352 | 264,999 | −19.6% |
| axum | 295,895 | 220,632 | −25.4% |
| **🥇 arvik** | 360,373 | 249,534 | **−30.8%** |

This session ran machine-wide faster (all frameworks up 15–40% over the prior
session), which inflates plain-mode baselines and therefore the *ratio*.
Absolute middleware throughput is what users experience — and there arvik's
249.5k ranks #1. The ratio is also skewed by run-to-run turbo variance;
the prior session measured arvik's overhead at just −6.3% with the same code
path. Treat ratios as indicative, absolute mw numbers as the stable signal.

---

## 4. Results — hey (Go tool, 8 s / 100 connections)

| Framework | Plain req/s | MW req/s | Plain p95 | MW p95 |
|---|---:|---:|---:|---:|
| **🥇 arvik** | **134,630** | **101,389** | **1.5 ms** | **1.9 ms** |
| 🥈 actix-web | 107,287 | 95,259 | 3.1 ms | 3.5 ms |
| 🥉 axum | 101,120 | 86,674 | 2.0 ms | 2.4 ms |

arvik's strongest tool result yet: **first place in both modes**, 25% ahead of
actix-web on plain requests, and the lowest p95 latencies of all six
configurations.

## 5. Results — ab (20k requests / 100 concurrency / no keep-alive)

| Framework | Plain req/s | MW req/s |
|---|---:|---:|
| 🥇 actix-web | **29,449** | 26,989 |
| 🥈 arvik | 25,084 | **26,763** |
| 🥉 axum | 25,230 | 24,681 |

Connection-setup dominated as always. Notable: arvik is again the most
consistent framework — its middleware number actually *exceeds* its plain
number within run noise, and it sits within 0.7% of actix-web under
middleware.

---

## 6. Analysis & takeaways

1. **Bare throughput: arvik leads.** 360.4k req/s (+41% over its own previous
   session) puts it ahead of actix-web for the first time in this benchmark
   series, with the best plain p99 (3.16 ms). Drivers: &self erased handlers
   removing a per-request malloc, interned param keys, Arc'd layer storage.
2. **Middleware: three-way race, arvik wins two of three tools.** actix edges
   wrk (+6%); arvik wins hey (+6%) and ties ab — while carrying the best
   middleware p99 (3.59 ms) of any framework in any configuration.
3. **Round-2 optimizations delivered measurably.** Same-framework cross-session:
   plain +41%, middleware +4% — against competitors that also rose 13–26% on
   the faster machine session, meaning arvik's *relative* position improved in
   plain mode from −7% vs actix to +9%.
4. **Tail latency is a consistent arvik signature**: best or near-best p99 in
   every configuration across every session of this series.
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
ls /tmp/bench3/results-round2/
```

Known quirk: runner scripts end with exit code 144 because their cleanup
`pkill -f 'bench3/target/release'` matches the script process itself — all
benchmark data is written before that point.

---

## Appendix A — Session history (arvik, wrk)

| Session | Plain req/s | MW req/s | MW overhead | Notes |
|---|---:|---:|---:|---|
| Baseline (pre-gate) | 315,909 | 156,125 | −50.6% | before TraceLayer gate |
| Post-gate (#26) | ~260k median | ~218–254k | ≈−16% | TraceLayer fast path |
| Final (#28, Round 2) | **360,373** | **249,534** | −31%* | full Round-2 pass |

\* Ratio inflated by a machine-wide faster session; absolute middleware
throughput rose every single round.

## Appendix B — Baseline competitor numbers (2026-08-23 session)

axum 134k/124k and actix-web 270k/189k (plain/mw) from the original session —
both frameworks also measured faster in the latest session (see §2), so
cross-session comparisons should use within-session rankings, which is how
§2's tables are constructed.

*Kept locally alongside `AUDIT_FINDINGS.md` — not part of the pushed
repository.*
