use std::env;
use std::error::Error;
use std::path::PathBuf;

fn default_out_dir() -> PathBuf {
    PathBuf::from("data/dict")
}

fn print_usage() {
    eprintln!(
        "Usage: cargo run --bin add_dictionary -- <path/to/dict.opcorpora.xml> [--out <dir>]"
    );
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<String> = env::args().skip(1).collect();

    let out_dir = match args.iter().position(|a| a == "--out") {
        Some(i) => {
            if i + 1 >= args.len() {
                print_usage();
                return Err("--out requires a directory path".into());
            }
            let dir = PathBuf::from(args.remove(i + 1));
            args.remove(i);
            dir
        }
        None => default_out_dir(),
    };

    let xml = match args.first() {
        Some(p) => PathBuf::from(p),
        None => {
            print_usage();
            return Err("missing path to dict.opcorpora.xml".into());
        }
    };

    renert::init(&xml, &out_dir)?;
    println!(
        "Dictionary is ready. XML: {}, cache: {}",
        xml.display(),
        out_dir.display()
    );
    Ok(())
}
