use std::path::PathBuf;
use std::process::{Command, Stdio};

use renert::Parser;

#[path = "rules.rs"]
mod rules;

fn benches_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches")
}

fn load_control_lines() -> Vec<String> {
    let text = std::fs::read_to_string(benches_root().join("data/address_small.txt"))
        .expect("Failed to read control dataset");
    text.lines().take(20).map(str::to_owned).collect()
}

fn rust_counts(lines: &[String]) -> Vec<usize> {
    renert::load("data/dict").expect("Dictionary must be loaded for parity test");
    let (registry, root_id) = rules::build_address_rules();
    let registry = Box::leak(Box::new(registry));
    let parser = Parser::new(registry, root_id);

    lines
        .iter()
        .map(|line| parser.findall(line.as_str()).len())
        .collect()
}

fn python_counts(lines: &[String]) -> Vec<usize> {
    let script = r#"
import json
import sys
from parser import build_address_parser

lines = json.load(sys.stdin)
parser = build_address_parser()
result = [len(list(parser.findall(line))) for line in lines]
json.dump(result, sys.stdout)
"#;

    let py_path = benches_root().join("python");
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(script)
        .env("PYTHONPATH", py_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start python3 for parity test");

    {
        let stdin = child.stdin.as_mut().expect("Failed to open python stdin");
        serde_json::to_writer(stdin, lines).expect("Failed to serialize lines for python");
    }

    let output = child
        .wait_with_output()
        .expect("Failed to wait for python process");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("Python parity process failed: {stderr}");
    }

    serde_json::from_slice::<Vec<usize>>(&output.stdout)
        .expect("Python parity process must return JSON list")
}

#[test]
#[ignore = "requires python3 + yargy dependencies installed"]
fn rust_and_python_rules_have_same_match_count() {
    let lines = load_control_lines();
    let rust = rust_counts(&lines);
    let python = python_counts(&lines);
    assert_eq!(rust, python, "Rust and Python parser match counts differ");
}
