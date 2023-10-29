use regex::Regex;
use std::fs;
use std::io::{self};
use std::path::Path;

pub fn delete_files_matching_patterns<P: AsRef<Path>>(
    file_path: P,
    patterns: &[String],
) -> io::Result<()> {
    let path = file_path.as_ref();
    if path.is_file() {
        let file_name = match path.file_name() {
            Some(name) => name.to_string_lossy(),
            None => return Ok(()),
        };
        for pattern in patterns {
            let regex_pattern = format!("^{}.*$", pattern.replace("*", ".*"));
            let re = Regex::new(&regex_pattern).unwrap();
            if re.is_match(&file_name) {
                println!("will delete file: {:?}", path);
                // 取消下面这行注释以启用删除功能
                fs::remove_file(path)?;
                break;
            }
        }
    }
    Ok(())
}
