use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::collections::VecDeque;

fn bench_rolling(c: &mut Criterion) {
    let n = 1_000_000;
    let mut data = vec![0.0; n];
    let mut seed = 123456789u64;
    for i in 0..n {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let u = (seed >> 11) as f64 / (1u64 << 53) as f64;
        data[i] = (u - 0.5) * 0.1;
    }

    let window = 50;

    let mut group = c.benchmark_group("Rolling_Volatility");
    group.sample_size(10);

    group.bench_function("naive_sum_sq", |b| {
        b.iter(|| {
            let mut window_vals: VecDeque<f64> = VecDeque::new();
            let mut sum = 0.0;
            let mut sum_sq = 0.0;
            let mut results = Vec::with_capacity(data.len());

            for &val in &data {
                window_vals.push_back(val);
                sum += val;
                sum_sq += val * val;

                if window_vals.len() > window {
                    let removed = window_vals.pop_front().unwrap();
                    sum -= removed;
                    sum_sq -= removed * removed;
                }

                if window_vals.len() == window {
                    let w = window as f64;
                    let mut variance = (sum_sq - (sum * sum) / w) / (w - 1.0);
                    if variance < 0.0 {
                        variance = 0.0;
                    }
                    results.push(variance.sqrt());
                }
            }
            black_box(results);
        })
    });

    group.bench_function("welford_sliding_window", |b| {
        b.iter(|| {
            let mut window_vals: VecDeque<f64> = VecDeque::new();
            let mut mean = 0.0;
            let mut m2 = 0.0;
            let mut results = Vec::with_capacity(data.len());

            for &val in &data {
                if window_vals.len() < window {
                    window_vals.push_back(val);
                    let old_mean = mean;
                    mean += (val - old_mean) / window_vals.len() as f64;
                    m2 += (val - old_mean) * (val - mean);

                    if window_vals.len() == window {
                        let w = window as f64;
                        let mut variance = m2 / (w - 1.0);
                        if variance < 0.0 {
                            variance = 0.0;
                        }
                        results.push(variance.sqrt());
                    }
                } else {
                    let x_old = window_vals.pop_front().unwrap();
                    let x_new = val;
                    window_vals.push_back(val);

                    let old_mean = mean;
                    mean += (x_new - x_old) / window as f64;
                    m2 += (x_new - old_mean) * (x_new - mean) - (x_old - old_mean) * (x_old - mean);

                    let w = window as f64;
                    let mut variance = m2 / (w - 1.0);
                    if variance < 0.0 {
                        variance = 0.0;
                    }
                    results.push(variance.sqrt());
                }
            }
            black_box(results);
        })
    });

    group.finish();
}

criterion_group!(benches, bench_rolling);
criterion_main!(benches);
