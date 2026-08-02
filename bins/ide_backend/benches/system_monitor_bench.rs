use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nexus_ide_backend::system_monitor::SystemMonitor;

fn bench_system_monitor(c: &mut Criterion) {
    c.bench_function("system_monitor_get_vitals", |b| {
        let mut monitor = SystemMonitor::new();
        b.iter(|| {
            black_box(monitor.get_vitals());
        });
    });
}

criterion_group!(benches, bench_system_monitor);
criterion_main!(benches);
