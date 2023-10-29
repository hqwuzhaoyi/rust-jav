use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

pub mod delete_files;
pub mod move_files;
pub mod rename_files;

pub fn traverse_directory<P: AsRef<Path>>(
    path: P,
    output_dir_path: PathBuf,
    patterns: &[String],
    prefixes: &[&str],
    is_root: bool,
) -> io::Result<()> {
    for entry in fs::read_dir(&path)? {
        let entry = entry?;
        let path = entry.path();

        // 对每个文件执行删除操作
        delete_files::delete_files_matching_patterns(&path, patterns)?;

        // 对每个文件执行重命名操作
        rename_files::rename_files_removing_prefixes(&path, prefixes)?;

        if is_root {
            rename_files::rename_directories_to_uppercase(&path)?;
            move_files::move_directories(&path)?;
        }

        // 如果是目录，则递归调用
        if path.is_dir() {
            traverse_directory(&path, output_dir_path.clone(), patterns, prefixes, false)?;
        }
    }
    Ok(())
}
