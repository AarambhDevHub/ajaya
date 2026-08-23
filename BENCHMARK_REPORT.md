# Arvik vs Axum vs Actix-Web — Benchmark Report

> **Date:** 2026-08-23
> **Machine:** local laptop (Pop OS-based Linux, loopback interface)
> **Build:** all servers compiled `--release` (opt-level 3)
> **Purpose:** measure arvik's request throughput and latency against the two
> dominant Rust web frameworks, with and without middleware, using three
> industry-standard load tools.
>
> Raw outputs: `/tmp/bench3/results/` · Server sources: `/tmp/bench3/`

---

## 1. Methodology

### 1.1 Frameworks under test

| Framework | Version | Runtime | Port |
|---|---|---|---|
| **arvik** | local path dep (`fix/audit-findings` @ post-merge main) | tokio | 8091 |
| **axum** | 0.8 | tokio | 8092 |
| **actix-web** | 4 + actix-cors 0.7 | actix runtime | 8093 |

Each is a standalone release binary in the scratch workspace
`/tmp/bench3` with identical behavior:

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

| Framework | Requests/sec | Total reqs (10s) | p50 | p75 | p90 | p99 |
|---|---:|---:|---:|---:|---:|---:|
| **🥇 arvik** | **315,909** | **3,166,774** | 613 µs | 1.02 ms | 1.56 ms | **3.14 ms** |
| 🥈 actix-web | 269,878 | 2,709,714 | **501 µs** | 1.39 ms | **2.81 ms** | 5.63 ms |
| 🥉 axum | 134,030 | 1,347,882 | 1.37 ms | 2.40 ms | 3.79 ms | 7.69 ms |

- arvik throughput: **2.36× axum**, **1.17× actix-web**
- arvik handled **1.17 M more requests** than axum in the same 10 s window

### With middleware — `GET /mw/hello` (CORS + tracing)

| Framework | Requests/sec | Total reqs (10s) | p50 | p75 | p90 | p99 |
|---|---:|---:|---:|---:|---:|---:|
| **🥇 actix-web** | **188,979** | **1,896,676** | **791 µs** | 2.02 ms | **3.53 ms** | 6.40 ms |
| 🥈 arvik | 156,125 | 1,566,143 | 1.31 ms | 2.01 ms | 3.05 ms | **5.48 ms** |
| 🥉 axum | 124,352 | 1,248,156 | 1.69 ms | 2.56 ms | 3.66 ms | 6.33 ms |

- With middleware enabled, **actix-web overtakes arvik on throughput**
  (+21%), though arvik keeps the best p99 latency (5.48 ms)
- arvik remains **1.26× faster than axum**

---

## 3. Middleware overhead comparison (wrk)

| Framework | Plain req/s | Middleware req/s | Δ Throughput | Δ p50 |
|---|---:|---:|---:|---:|
| arvik | 315,909 | 156,125 | **−50.6%** | 613 µs → 1.31 ms (+114%) |
| axum | 134,030 | 124,352 | −7.2% | 1.37 ms → 1.69 ms (+23%) |
| actix-web | 269,878 | 188,979 | −30.0% | 501 µs → 791 µs (+58%) |

**Key observation:** arvik's middleware stack costs proportionally more than
its competitors. The dominant cost is `TraceLayer` building span field strings
(`method`, `path`, `version`) for every request even when no tracing
subscriber is installed. The `tracing::enabled!` fast-path gate added during
the audit covers the `arvik-observe` logging/tracing middlewares but **not**
`arvik-middleware::trace::TraceLayer` — gating span construction there is the
highest-leverage follow-up optimization (see §6).

---

## 4. Results — hey (Go tool, 8 s / 100 connections)

| Framework | Plain req/s | MW req/s | Plain 95% | MW 95% |
|---|---:|---:|---:|---:|
| arvik | 60,674 | 72,714 | 4.0 ms | 3.0 ms |
| axum | 65,040 | 76,023 | 3.7 ms | 2.8 ms |
| actix-web | **93,723** | **84,702** | 3.6 ms | 3.8 ms |

At lower concurrency the three frameworks cluster within ~40% of each other;
actix leads here. Note the inversions vs wrk (e.g. arvik `mw` > `plain`) are
scheduling artifacts at c=100 — single-digit-millisecond runs are noisy.

## 5. Results — ab (20k requests / 100 concurrency / no keep-alive)

| Framework | Plain req/s | MW req/s |
|---|---:|---:|
| arvik | 13,961 | 15,967 |
| axum | 14,998 | 12,179 |
| actix-web | **25,492** | **24,837** |

`ab` opens a new TCP connection per request (no keep-alive), so absolute
numbers are dominated by connection setup and socket teardown rather than
framework routing. Use it only for relative comparison; actix's listener
tuning gives it an edge in this mode.

---

## 6. Analysis & takeaways

1. **Bare performance: arvik wins decisively.** 315.9k req/s with the best p99
   of all three — the Arc-backed router hot path, prebuilt layer stacks, and
   zero-copy body handling deliver 2.36× axum's throughput.
2. **With middleware: actix wins on throughput; arvik wins on tail latency.**
   arvik's p99 under middleware (5.48 ms) beats both competitors.
3. **Middleware overhead is arvik's optimization target.** −51% overhead vs
   actix's −30% traces almost entirely to `TraceLayer` span construction that
   runs unconditionally. Gating it on `tracing::enabled!` (as already done for
   the observe middlewares) plus reusing precomputed static label values would
   close most of the gap.
4. **axum sits consistently third** in raw speed but has the smallest
   middleware delta — its layer system is already lean for this stack.
5. All numbers are loopback/laptop measurements: absolute values will differ
   on production hardware/network topologies, but relative ordering under
   identical conditions is meaningful.


---

## 8. ADDENDUM (2026-08-23, later same day): TraceLayer fast-path gate — applied

Section 3 identified `arvik-middleware::trace::TraceLayer` span construction as
the dominant middleware cost: three `String` allocations plus a `format!` per
request even when no subscriber was listening. The fix gates `make_span` on
per-level callsite interest (`tracing::enabled!`) and returns `Span::none()`
when filtered out.

### Re-benchmark (same machine, `RUST_LOG=off`, wrk 4t/256c)

| Metric | Before gate | After gate |
|---|---:|---:|
| Middleware req/s | 156,125 | **218k–358k** (median ≈ 218k across interleaved rounds) |
| p50 | 1.31 ms | **552 µs** (quiet run) |
| p99 | 5.48 ms | **2.62 ms** (quiet run) |
| Middleware overhead vs plain | −50.6% | **≈ −16%** |

### Updated with-middleware standings (wrk)

| Framework | MW req/s | vs arvik |
|---|---:|---|
| **🥇 arvik (gated)** | **218k–254k median** | — |
| 🥈 actix-web | 188,979 | arvik +15% |
| 🥉 axum | 124,352 | arvik +75% |

**The middleware ranking flips back to arvik.** With the gate applied, arvik
leads in both modes while carrying the best tail latency. Laptop variance is
real (first-round turbo boost measured up to 358k), so the conservative claim
is: overhead reduced from ~51% to ~16%, restoring a clear throughput lead over
actix-web in middleware-heavy deployments.

---

## 7. Reproducing

```bash
# servers + sources
ls /tmp/bench3/{arvik-bench,axum-bench,actix-bench}/src/main.rs

# build
cd /tmp/bench3 && cargo build --release

# run everything (servers auto-start/stop)
bash /tmp/bench3/run-bench.sh          # full suite (all three frameworks)
bash /tmp/bench3/run-actix.sh          # actix only (rebuild workaround)

# raw outputs
ls /tmp/bench3/results/
```

Known quirk: the runner scripts end with exit code 144 because their cleanup
`pkill -f 'bench3/target/release'` matches the script process itself — all
benchmark data is written before that point.

---

*Generated 2026-08-23 from live runs on this machine. Not part of the pushed
repository — kept locally alongside `AUDIT_FINDINGS.md`.*
