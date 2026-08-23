//! Micro-benchmarks for the pure `SegmentedBar` helpers.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use msg_value_bars::prelude::*;

/// Fixed, deterministic set of normalized values covering boundaries,
/// near-boundary offsets, and mid-slot partials.
fn sample_values() -> Vec<f32> {
    (0..=1000).map(|i| i as f32 / 1000.0).collect()
}

fn bench_slot_split(c: &mut Criterion) {
    let bar = SegmentedBar::new(8);
    let values = sample_values();
    c.bench_function("slot_split_8_slots_1001_values", |b| {
        b.iter(|| {
            for &value in &values {
                black_box(bar.slot_split(black_box(value)));
            }
        });
    });
}

fn bench_display_index(c: &mut Criterion) {
    let normal = SegmentedBar::new(8);
    let inverse = SegmentedBar::new(8).with_fill_direction(SegmentFillDirection::Inverse);
    c.bench_function("display_index_both_directions_8_slots", |b| {
        b.iter(|| {
            for index in 0..8 {
                black_box(normal.display_index(black_box(index)));
                black_box(inverse.display_index(black_box(index)));
            }
        });
    });
}

criterion_group!(benches, bench_slot_split, bench_display_index);
criterion_main!(benches);
