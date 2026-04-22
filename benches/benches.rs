use criterion::{criterion_group, criterion_main, Criterion};

fn bench_compress(c: &mut Criterion) {
    // Generate synthetic log
    let log: String = (0..5000)
        .map(|i| {
            format!(
                "2026-04-21T14:32:{:02}.123456789Z INFO processing request for user_id=1234 session=abcdef path=/api/v1/users request_id={i}\n",
                i % 60
            )
        })
        .collect();

    c.bench_function("compress_5k_lines", |b| {
        b.iter(|| {
            logzip::compress_text(&log, 5, 3844, true, None, true);
        })
    });
}

criterion_group!(benches, bench_compress);
criterion_main!(benches);
