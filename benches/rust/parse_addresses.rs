use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

#[path = "common.rs"]
mod common;
#[path = "rules.rs"]
mod rules;

fn bench_parse(c: &mut Criterion) {
    let parser = common::build_parser();

    let mut group = c.benchmark_group("renert/parse");

    for (dataset_name, dataset_path) in common::DATASETS {
        let (lines, bytes) = common::load_dataset(dataset_path);
        group.throughput(Throughput::Bytes(bytes));

        group.bench_function(BenchmarkId::new("findall", dataset_name), |b| {
            b.iter(|| {
                let mut total_matches = 0usize;
                for line in &lines {
                    let matches = parser.findall(black_box(line.as_str()));
                    total_matches += matches.len();
                }
                black_box(total_matches);
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
