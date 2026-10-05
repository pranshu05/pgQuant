use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_var_es(c: &mut Criterion) {
    let n = 100_000;
    let mut data = vec![0.0; n];
    // Generate pseudo-random returns
    let mut seed = 123456789u64;
    for i in 0..n {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let u = (seed >> 11) as f64 / (1u64 << 53) as f64;
        data[i] = (u - 0.5) * 0.1;
    }

    let confidence = 0.95;
    let alpha = 1.0 - confidence;
    let mut index = (alpha * data.len() as f64).round() as usize;
    if index == 0 {
        index = 1;
    }
    let index = index.min(data.len()) - 1;

    let mut group = c.benchmark_group("VaR_ES_Historical");
    group.sample_size(20);

    group.bench_function("sort_by (O(N log N))", |b| {
        b.iter(|| {
            let mut returns = data.clone();
            returns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let var_threshold = returns[index];
            let tail_returns: Vec<f64> = returns
                .into_iter()
                .filter(|&r| r <= var_threshold)
                .collect();

            let sum: f64 = tail_returns.iter().sum();
            let avg = sum / tail_returns.len() as f64;
            black_box((-var_threshold, -avg));
        })
    });

    group.bench_function("select_nth_unstable_by (O(N))", |b| {
        b.iter(|| {
            let mut returns = data.clone();

            let (left, &mut var_threshold, _) = returns.select_nth_unstable_by(index, |a, b| {
                a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
            });

            let tail_len = index + 1;
            let left_sum: f64 = left.iter().sum();
            let total_sum = left_sum + var_threshold;
            let avg = total_sum / (tail_len as f64);

            black_box((-var_threshold, -avg));
        })
    });

    group.finish();
}

criterion_group!(benches, bench_var_es);
criterion_main!(benches);
