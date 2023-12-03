use log::info;
use log::trace;
use log::error;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;


pub fn move_and_delete_directory<P: AsRef<Path>>(path: P, new_path: P) -> io::Result<()> {
    let path = path.as_ref();
    let new_path = new_path.as_ref();

    // 直接重命名整个目录
    match fs::rename(path, new_path) {
        Ok(_) => info!("Directory moved from {:?} to {:?}", path, new_path),
        Err(e) => {
            error!("Error moving directory {:?} to {:?}: {}", path, new_path, e);
            return Err(e);
        }
    }

    Ok(())
}

pub fn remove_nfo_files<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();
    if path.is_dir() {
        match fs::read_dir(path) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => {
                            let path = entry.path();
                            if path.is_dir() {
                                if let Err(e) = remove_nfo_files(&path) {
                                    error!("Error removing nfo files in directory {:?}: {}", path, e);
                                }
                            } else if path.is_file() {
                                if let Some(extension) = path.extension() {
                                    if extension == "nfo" {
                                        info!("Deleting file {:?}", path);
                                        if let Err(e) = fs::remove_file(&path) {
                                            error!("Error deleting file {:?}: {}", path, e);
                                        }
                                    }
                                }
                            }
                        },
                        Err(e) => error!("Error reading entry in directory {:?}: {}", path, e),
                    }
                }
            },
            Err(e) => error!("Error reading directory {:?}: {}", path, e),
        }
    }
    Ok(())
}

pub fn rename_file(path: &Path, new_path: &PathBuf) -> io::Result<()> {
    if new_path.exists() {
        trace!("File with the name {:?} already exists", new_path);
        if new_path.is_dir() {
            // 遍历目录中的所有文件和子目录
            move_and_delete_directory(path, new_path)?;

            // 删除new_path文件夹下的nfo文件
            remove_nfo_files(&new_path)?;

        } else {
            info!(
                "File with the name {:?} already exists, cannot rename {:?}",
                new_path, path
            );
        }
    } else {
        info!("Renaming {:?} to {:?}", path, new_path);
        fs::rename(&path, new_path)?;
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
