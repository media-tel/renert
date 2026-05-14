use std::path::Path;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;

use criterion::{BenchmarkId, Criterion};

#[path = "common.rs"]
mod common;
#[path = "rules.rs"]
mod rules;

const CHILD_ENV: &str = "YARGY_INIT_BENCH_CHILD";

fn run_child_once(dict_dir: &Path) -> Duration {
    let mut command =
        Command::new(std::env::current_exe().expect("Failed to resolve bench binary"));
    command.env(CHILD_ENV, "1");
    command.env("YARGY_DICT_DIR", dict_dir);
    let output = command
        .output()
        .expect("Failed to spawn child process for init benchmark");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("Init benchmark child failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let nanos = stdout
        .lines()
        .last()
        .unwrap_or("0")
        .trim()
        .parse::<u64>()
        .expect("Child output must contain elapsed nanoseconds");
    Duration::from_nanos(nanos)
}

fn run_child_workload() -> Result<(), String> {
    let start = Instant::now();
    renert::load(common::dict_dir()).map_err(|err| err.to_string())?;
    let _ = rules::build_address_rules();
    println!("{}", start.elapsed().as_nanos());
    Ok(())
}

fn bench_init(c: &mut Criterion) {
    let mut group = c.benchmark_group("renert/init");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));

    let dict_dir = common::dict_dir();
    group.bench_function(
        BenchmarkId::new("load_and_build_rules", "fresh_process"),
        |b| {
            b.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    total += run_child_once(&dict_dir);
                }
                total
            });
        },
    );

    group.finish();
}

fn main() {
    if std::env::var_os(CHILD_ENV).is_some() {
        if let Err(err) = run_child_workload() {
            eprintln!("{err}");
            std::process::exit(1);
        }
        return;
    }

    let mut criterion = Criterion::default().configure_from_args();
    bench_init(&mut criterion);
    criterion.final_summary();
}
