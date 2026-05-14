use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use renert::token::{MorphTokenizer, Tokenizer};

#[path = "common.rs"]
mod common;
#[allow(dead_code)]
#[path = "rules.rs"]
mod rules;

fn bench_tokenize(c: &mut Criterion) {
    let plain_tokenizer = Tokenizer::new();

    common::init_dict().expect("Dictionary should be loaded for morph tokenizer");
    let morph_tokenizer =
        MorphTokenizer::open_at(common::dict_dir()).expect("Failed to create MorphTokenizer");

    let mut group = c.benchmark_group("renert/tokenize");

    for (dataset_name, dataset_path) in common::DATASETS {
        let (lines, bytes) = common::load_dataset(dataset_path);
        group.throughput(Throughput::Bytes(bytes));

        group.bench_function(BenchmarkId::new("plain", dataset_name), |b| {
            b.iter(|| {
                let mut total_tokens = 0usize;
                for line in &lines {
                    total_tokens += plain_tokenizer.tokenize(black_box(line.as_str())).len();
                }
                black_box(total_tokens);
            });
        });

        group.bench_function(BenchmarkId::new("morph", dataset_name), |b| {
            b.iter(|| {
                let mut total_tokens = 0usize;
                for line in &lines {
                    total_tokens += morph_tokenizer.tokenize(black_box(line.as_str())).len();
                }
                black_box(total_tokens);
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_tokenize);
criterion_main!(benches);
