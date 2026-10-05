use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nalgebra::{DMatrix, DVector};

fn bench_covariance(c: &mut Criterion) {
    let num_dates = 5000;
    let num_symbols = 500;
    let mut data = vec![0.0; num_dates * num_symbols];
    for i in 0..data.len() {
        data[i] = (i % 100) as f64 * 0.01;
    }

    let mut group = c.benchmark_group("Covariance");
    group.sample_size(10); // Since it takes ~80ms per run, 10 samples is reasonable

    group.bench_function("manual_centering", |b| {
        b.iter(|| {
            let mut returns_matrix = DMatrix::from_row_slice(num_dates, num_symbols, &data);
            for j in 0..num_symbols {
                let mut sum = 0.0;
                for i in 0..num_dates {
                    sum += returns_matrix[(i, j)];
                }
                let mean = sum / (num_dates as f64);
                for i in 0..num_dates {
                    returns_matrix[(i, j)] -= mean;
                }
            }
            let cov = (returns_matrix.transpose() * returns_matrix) / ((num_dates - 1) as f64);
            black_box(cov);
        })
    });

    group.bench_function("matrix_formula", |b| {
        b.iter(|| {
            let returns_matrix = DMatrix::from_row_slice(num_dates, num_symbols, &data);

            let mut sums = DVector::zeros(num_symbols);
            for j in 0..num_symbols {
                let mut sum = 0.0;
                for i in 0..num_dates {
                    sum += returns_matrix[(i, j)];
                }
                sums[j] = sum;
            }

            let xt_x = returns_matrix.transpose() * &returns_matrix;
            let s_st = &sums * sums.transpose();

            let cov = (xt_x - s_st / (num_dates as f64)) / ((num_dates - 1) as f64);
            black_box(cov);
        })
    });

    group.finish();
}

criterion_group!(benches, bench_covariance);
criterion_main!(benches);
