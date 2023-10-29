use std::fs;
use std::io;
use std::path::Path;

pub fn rename_files_removing_prefixes<P: AsRef<Path>>(
    file_path: P,
    prefixes: &[&str],
) -> io::Result<()> {
    let path = file_path.as_ref();
    if path.is_file() {
        if let Some(file_name) = path.file_name() {
            let file_name_str = file_name.to_string_lossy();
            for prefix in prefixes {
                if file_name_str.starts_with(prefix) {
                    let new_file_name = file_name_str.replacen(prefix, "", 1);
                    let new_path = path.with_file_name(new_file_name);
                    println!("将 {:?} 更改为 {:?}", path, new_path);
                    fs::rename(path, new_path)?;
                    break;
                }
            }
        }
    }
    Ok(())
}
pub fn rename_directories_to_uppercase<P: AsRef<Path>>(file_path: P) -> io::Result<()> {
    let path = file_path.as_ref();
    if path.is_dir() {
        if let Some(dir_name) = path.file_name() {
            let dir_name_str = dir_name.to_string_lossy();

            // 检查是否包含小写字母
            if dir_name_str.chars().any(|c| c.is_ascii_lowercase()) {
                let uppercased_name = dir_name_str.to_ascii_uppercase();
                let new_path = path.with_file_name(uppercased_name);
                println!("Renaming {:?} to {:?}", path, new_path);

                fs::rename(&path, new_path)?;
            }
        }
    }
    Ok(())
}
