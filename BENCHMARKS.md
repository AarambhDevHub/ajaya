# Benchmark Validation

## Local Runs

Build and run the benchmark suite from the workspace root:

```bash
BENCH_PROFILE=smoke scripts/bench/run-all.sh
```

Smoke mode uses one 10-second measured run per duration-based tool. Use it to
validate benchmark wiring after code changes.

For manual artifacts:

```bash
BENCH_PROFILE=manual BENCH_RUNS=3 scripts/bench/run-all.sh
```

Manual mode defaults to 30-second `wrk`, `hey`, and `h2load` runs, and three
measured runs per scenario. Raw output and summaries are written to
`target/bench-results`.

## Tools And Parameters

- `wrk`: `-t4 -c100 -d$WRK_DURATION`
- `hey`: `-z $HEY_DURATION -c 100`
- `ab`: `-k -n 100000 -c 100`
- `h2load`: `-c100 -m100 -t4 -D $H2LOAD_DURATION` when duration is set

HTTP/1 and h2c results are reported separately. Do not compare HTTP/1 and h2c
numbers as the same protocol category.

## GitHub Workflow

Run the manual workflow from Actions:

1. Open **Benchmarks**.
2. Choose `smoke` for validation or `manual` for artifact runs.
3. Keep the default `30s` duration for manual runs unless a longer intentional
   run is needed.
4. Download the `arvik-benchmark-results` artifact.

CI hosts are shared virtual machines, so results can be noisy. Use workflow
artifacts for regression signals and local dedicated hardware for careful
performance analysis.

## Output Files

Each run writes raw tool output, server logs, and summaries:

- `summary.md`
- `summary.json`
- `summary.tsv`
- `environment.txt`
- `wrk_<scenario>_run<N>.txt`
- `hey_<scenario>_run<N>.txt`
- `ab_<scenario>_run<N>.txt`
- `h2load_h2c_plaintext_run<N>.txt`

The environment file includes rustc version, OS/kernel, CPU details when
available, commit SHA, build profile, and tool versions.

## Median Calculation

For each scenario/tool pair:

- Sort successful measured-run RPS values numerically.
- For odd run counts, use the middle value.
- For even run counts, average the two middle values.
- Report min and max from the same successful measured-run set.

Failed or skipped runs are kept in raw artifacts and excluded from median
calculation. Any failed measured run should be investigated before publishing
the benchmark artifact.
