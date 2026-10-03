# SCC-local extraction measurements

These are small synthetic comparisons against pristine egg
`73975c9819efb4af2f3c6c1b44fa53f8c5742574`, not application-scale guarantees.
The final run used Linux x86_64 (AMD EPYC 9V74), Rust 1.94.1, good_lp 1.15.3,
and CBC 2.10.12. Each variant had its own checkout and Cargo target directory
and the same Cargo.lock. Runs were sequential, alternating variant order
between cases. CPU affinity and complete machine idleness were not controlled.

Each optimal case has one warmup followed by five measured repetitions.
Median total times include extractor construction, solving, and decoding:

| Case | Baseline ms | SCC ms | Baseline cost | SCC cost | Exact optimum |
| --- | ---: | ---: | ---: | ---: | ---: |
| chain500 | 11.697 | 12.811 | 501 | 501 | 501 |
| duplicate500 | 20.571 | 18.561 | 501 | 501 | 501 |
| shared32 | 445.456 | 410.607 | 20.2 | 20.2 | 20.2 |
| ring8 | 1.678 | 1.547 | 1000 | 5 | 5 |
| ring16 | 1.535 | 2.543 | 1000 | 9 | 9 |
| ring32 | 2.069 | 3.616 | 17 | 17 | 17 |
| ring128 | 5.726 | 11.615 | 1000 | 65 | 65 |

Every SCC result reported `Optimal` on these cases and matched its independent
cost oracle. Baseline `Optimal` describes the pruned model and does not imply
the true extraction optimum. Acyclic cases have identical row counts in both
models. Small speedups are not claimed as statistically reliable; ring128
roughly doubled in time while recovering the much cheaper valid selection.

For the larger acyclic shared-choice cases, both variants hit the five-second
limit. All three process-independent attempts returned cost 63.4 for
shared100 (optimum 61), and 361.8 for shared500 (optimum 301). These are
suboptimal incumbents, not time-to-optimum measurements. Timeout handling does
not guarantee incumbent quality on arbitrary cyclic inputs.

`results.csv` preserves all 132 samples, including warmups and timeout
attempts. `verify_oracles.py` checks the expected costs independently: exact
integer-fifths dynamic programming for shared choices, enumeration of every
ring exit, and unit-node counting for chains. It never calls CBC or reads the
SCC implementation. Run it with Python 3; its output is
`oracle-verification.json`.

## Reproduce

Create separate checkouts of the baseline commit above and this change. Copy
`lp_perf_probe.rs` into `tests/lp_perf_probe.rs` in both checkouts and use the
included `Cargo.lock.tested` as `Cargo.lock` in each checkout. This preserves
the exact dependency resolution used for these measurements; it is an
optional benchmark artifact, not a new lockfile policy for the egg crate.
Install CBC as described in LpExtractor's
documentation. Give each checkout a distinct `CARGO_TARGET_DIR` to prevent
cross-checkout artifact reuse. From each checkout, run:

```sh
# Set VARIANT to baseline or scc; set TARGET to a variant-specific directory.
CARGO_TARGET_DIR="$TARGET" cargo +1.94.1 test --locked --release \
  --features lp --test lp_perf_probe --no-run

PERF_VARIANT="$VARIANT" PERF_CASE=ring16 PERF_REPEATS=5 \
  CARGO_TARGET_DIR="$TARGET" cargo +1.94.1 test --locked --release \
  --features lp --test lp_perf_probe -- --nocapture --test-threads=1
```

The cases are chain100, chain500, duplicate100, duplicate500, shared16,
shared32, shared100, shared500, ring8, ring16, ring32, and ring128. Discard
iteration 0 as warmup only when the solver reports `Optimal`. For cases that
report `TimeLimit`, use three independent invocations with `PERF_REPEATS=0`
and report every incumbent and status. A backend time limit may be slightly
overshot. This benchmark fixture is deliberately outside the default test
suite and is not a timing-sensitive CI assertion.
