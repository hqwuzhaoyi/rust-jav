use clap::Parser;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::path::PathBuf;

mod file_utils;

const PATTERNS: Lazy<HashSet<String>> = Lazy::new(|| {
    include_str!("../patterns.txt")
        .lines()
        .map(|s| s.to_string())
        .collect()
});

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Name of the person to greet
    #[arg(short, long)]
    dir: String,

    /// Whether to use the --upper-case flag
    #[arg(short, long)]
    upper_case: bool,

    /// Organized target folder
    #[arg(short, long)]
    output_dir: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let dir = &cli.dir; // 获取 dir 参数的值
    let output_dir = &cli.output_dir; // 获取 target 参数的值
    let output_dir_path = PathBuf::from(output_dir);
    let pattern_slice: Vec<String> = PATTERNS.iter().cloned().collect();
    let patterns_ref = &pattern_slice;
    println!("all pattern {:?}", pattern_slice);

    let prefixes = [
        "hhd800.com@",
        "zzpp01.com@",
        "第一會所新片@SIS001@",
        "zzpp05.com@",
        "RH2048.COM@",
        "[7sht.me]@",
        "[98t.tv]@",
        "[ThZu.Cc]@",
        "[99u.me]@",
        "[22sht.me]@",
    ];

    file_utils::move_files::create_category_directories(output_dir_path.clone())?;
    file_utils::traverse_directory(dir, output_dir_path.clone(), patterns_ref, &prefixes, true)?;
    Ok(())
}
