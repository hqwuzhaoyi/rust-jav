use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::fs;
use std::io::{self};
use std::path::Path;
use regex::Regex;

const PATTERNS: Lazy<HashSet<String>> = Lazy::new(|| {
    include_str!("../patterns.txt")
        .lines()
        .map(|s| s.to_string())
        .collect()
});

fn delete_files_matching_patterns<P: AsRef<Path>>(path: P, patterns: &[String]) -> io::Result<()> {
    for entry in fs::read_dir(&path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let file_name = match path.file_name() {
                Some(name) => name.to_string_lossy(),
                None => continue,
            };
            for pattern in patterns {
                let regex_pattern = format!("^{}.*$", pattern.replace("*", ".*"));
                let re = Regex::new(&regex_pattern).unwrap();

                if re.is_match(&file_name) {
                    println!("will delete file: {:?}", path);
                    // 取消下面这行注释以启用删除功能
                    fs::remove_file(&path)?;
                    break;
                }
            }
        } else if path.is_dir() {
            // 如果是目录，则递归调用
            delete_files_matching_patterns(&path, patterns)?;
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let dir = "./examples";
    let pattern_slice: Vec<String> = PATTERNS.iter().cloned().collect();
    let patterns_ref = &pattern_slice;
    println!("all pattern {:?}", pattern_slice);
    delete_files_matching_patterns(dir, patterns_ref)?;
    Ok(())
}
