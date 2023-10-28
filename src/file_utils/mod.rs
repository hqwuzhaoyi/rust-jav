use std::fs;
use std::io;
use std::path::Path;

pub mod delete_files;
pub mod rename;

pub fn traverse_directory<P: AsRef<Path>>(
    path: P,
    patterns: &[String],
    prefixes: &[&str],
) -> io::Result<()> {
    for entry in fs::read_dir(&path)? {
        let entry = entry?;
        let path = entry.path();

        // 对每个文件执行删除操作
        delete_files::delete_files_matching_patterns(&path, patterns)?;

        // 对每个文件执行重命名操作
        rename::rename_files_removing_prefixes(&path, prefixes)?;

        // 如果是目录，则递归调用
        if path.is_dir() {
            traverse_directory(&path, patterns, prefixes)?;
        }
    }
    Ok(())
}
