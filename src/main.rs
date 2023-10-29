use once_cell::sync::Lazy;
use std::collections::HashSet;

use std::io::{self};

mod file_utils;

const PATTERNS: Lazy<HashSet<String>> = Lazy::new(|| {
    include_str!("../patterns.txt")
        .lines()
        .map(|s| s.to_string())
        .collect()
});

fn main() -> io::Result<()> {
    let dir = "./examples";
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
    file_utils::move_files::create_category_directories(dir)?;
    file_utils::traverse_directory(dir, patterns_ref, &prefixes, true)?;
    Ok(())
}
