#![allow(dead_code)]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use renert::error::Result;
use renert::{Parser, RuleId, RuleRegistry};

pub const DATASETS: [(&str, &str); 3] = [
    ("address_small", "benches/data/address_small.txt"),
    ("address_medium", "benches/data/address_medium.txt"),
    ("address", "benches/data/address.txt"),
];

static INIT_STATUS: OnceLock<std::result::Result<(), String>> = OnceLock::new();
static PARSER_REGISTRY: OnceLock<(&'static RuleRegistry<'static>, RuleId)> = OnceLock::new();

pub fn dict_dir() -> PathBuf {
    env::var("YARGY_DICT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data/dict"))
}

pub fn init_dict() -> Result<()> {
    let status =
        INIT_STATUS.get_or_init(|| renert::load(dict_dir()).map_err(|err| err.to_string()));
    match status {
        Ok(()) => Ok(()),
        Err(message) if message.contains("already been called") => Ok(()),
        Err(message) => panic!("Failed to load dictionary: {message}"),
    }
}

pub fn load_dataset(path: &str) -> (Vec<String>, u64) {
    let text = fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("Failed to read dataset at {path}: {err}");
    });
    let bytes = text.len() as u64;
    let lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    (lines, bytes)
}

pub fn build_parser() -> Parser<'static> {
    init_dict().expect("Dictionary must be loaded before benchmark");
    let (registry, root_id) = PARSER_REGISTRY.get_or_init(|| {
        let (registry, root_id) = crate::rules::build_address_rules();
        (Box::leak(Box::new(registry)), root_id)
    });
    Parser::new(*registry, *root_id)
}
