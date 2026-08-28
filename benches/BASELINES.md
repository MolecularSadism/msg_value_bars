# Benchmark baselines

Criterion baseline `base`, captured 2026-08-26.

| | |
|---|---|
| Commit | `546ba2fecaa7` |
| Branch | `bench-baselines` |
| Toolchain | rustc 1.98.0 (88d9e12ae 2026-08-18) |
| Host | Linux x86_64 container (shared/virtualised) |

## Results

Mean with 95% confidence interval.

| Benchmark | Mean | 95% CI |
|---|---:|---|
| `display_index_both_directions_8_slots` | 6.23 ns | 6.166 ns – 6.304 ns |
| `slot_split_8_slots_1001_values` | 7.347 µs | 7.328 µs – 7.367 µs |

## Reproducing

The bench target must be named. A bare `cargo bench` also runs the lib test
harness, which rejects criterion's flags with
`error: Unrecognized option: 'baseline'`.

```sh
# capture
cargo bench --bench segmented -- --save-baseline base

# compare against it
cargo bench --bench segmented -- --baseline base
```

## How much to trust these

Taken in a shared virtualised container. Treat them as an order-of-magnitude
record, not a regression gate.

Re-running `take_batch/3x1000_pending` on byte-identical code about an hour
later on the *same* host reported `+31%` with `p = 0.00`. Criterion's
significance test measures sampling noise within a run; it cannot see the
host's load drifting between runs. So a reported change of this size here is
not evidence of a real regression.

To draw a conclusion from a comparison, capture the baseline and the
comparison back to back in one sitting, and treat anything under roughly
1.5x as inconclusive. Comparing these absolute numbers against a different
machine is meaningless.
