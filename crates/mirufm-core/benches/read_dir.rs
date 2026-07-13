use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};
use mirufm_core::fs::read_dir;
use mirufm_core::sort::{sort, SortKey};

fn bench_large_dir(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..100_000 {
        std::fs::write(dir.path().join(format!("file_{i:06}")), b"").unwrap();
    }
    let cancel = Arc::new(AtomicBool::new(false));

    c.bench_function("read_dir_100k", |b| {
        b.iter(|| {
            let entries = read_dir(dir.path(), &cancel).unwrap();
            assert_eq!(entries.len(), 100_000);
            std::hint::black_box(entries);
        });
    });

    c.bench_function("read_and_sort_100k", |b| {
        b.iter(|| {
            let mut entries = read_dir(dir.path(), &cancel).unwrap();
            sort(&mut entries, SortKey::Name, true);
            std::hint::black_box(entries);
        });
    });
}

criterion_group!(benches, bench_large_dir);
criterion_main!(benches);
