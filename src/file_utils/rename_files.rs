use log::info;
use log::trace;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

fn rename_file(path: &Path, new_path: &PathBuf) -> io::Result<()> {
    if !new_path.exists() {
        info!("Renaming {:?} to {:?}", path, new_path);
        fs::rename(&path, new_path)?;
    } else {
        info!(
            "Directory with the name {:?} already exists, cannot rename {:?}",
            new_path, path
        );
    }
    Ok(())
}

pub fn rename_files_removing_prefixes<P: AsRef<Path>>(
    file_path: P,
    prefixes: &[String],
) -> io::Result<()> {
    let path = file_path.as_ref();
    if path.is_file() {
        if let Some(file_name) = path.file_name() {
            let file_name_str = file_name.to_string_lossy();
            for prefix in prefixes {
                if file_name_str.starts_with(prefix) {
                    let new_file_name = file_name_str.replacen(prefix, "", 1);
                    let new_path = path.with_file_name(new_file_name);

                    rename_file(path, &new_path)?;

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
                rename_file(path, &new_path)?;
            }
        }
    }
    Ok(())
}

pub fn rename_files_removing_uncensored<P: AsRef<Path>>(file_path: P) -> io::Result<()> {
    let path = file_path.as_ref();
    if path.is_file() {
        if let Some(file_stem) = path.file_stem() {
            let file_stem_str = file_stem.to_string_lossy();
            trace!("rename_files_removing_uncensored: {:?}", file_stem_str);
            let new_file_stem = if file_stem_str.ends_with("-U") {
                file_stem_str.replacen("-U", "", 1)
            } else if file_stem_str.ends_with("-UC") {
                file_stem_str.replacen("-UC", "-C", 1)
            } else {
                file_stem_str.into_owned()
            };

            if let Some(extension) = path.extension() {
                let new_file_name = format!("{}.{}", new_file_stem, extension.to_string_lossy());
                let new_path = path.with_file_name(new_file_name);
                rename_file(path, &new_path)?;
            }
        }
    }
    Ok(())
}
