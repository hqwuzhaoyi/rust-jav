use async_recursion::async_recursion;
use log::error;
use log::info;
use log::trace;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::Result as IoResult;

pub async fn move_and_delete_directory<P: AsRef<Path>>(path: P, new_path: P) -> IoResult<()> {
    let path = path.as_ref();
    let new_path = new_path.as_ref();

    // 使用异步方法重命名整个目录
    match fs::rename(path, new_path).await {
        Ok(_) => info!("Directory moved from {:?} to {:?}", path, new_path),
        Err(e) => {
            error!("Error moving directory {:?} to {:?}: {}", path, new_path, e);
            return Err(e);
        }
    }

    Ok(())
}

#[async_recursion]
pub async fn remove_nfo_files<P: AsRef<Path> + Send + Sync + 'static>(path: P) -> IoResult<()> {
    let path = path.as_ref();
    let metadata = fs::metadata(path).await?;
    if metadata.is_dir() {
        let mut entries = fs::read_dir(path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let is_dir: bool = fs::metadata(&path)
                .await
                .map(|m| m.is_dir())
                .unwrap_or(false);
            let is_file: bool = fs::metadata(&path)
                .await
                .map(|m| m.is_file())
                .unwrap_or(false);
            if is_dir {
                if let Err(e) = remove_nfo_files(path.clone()).await {
                    error!("Error removing nfo files in directory {:?}: {}", path, e);
                }
            } else if is_file {
                if let Some(extension) = path.extension() {
                    if extension == "nfo" {
                        info!("Deleting file {:?}", path);
                        if let Err(e) = fs::remove_file(&path).await {
                            error!("Error deleting file {:?}: {}", path, e);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub async fn rename_file(path: &Path, new_path: &PathBuf) -> IoResult<()> {
    if fs::metadata(new_path).await.is_ok() {
        trace!("File with the name {:?} already exists", new_path);
        if fs::metadata(new_path).await?.is_dir() {
            // 异步遍历目录中的所有文件和子目录
            move_and_delete_directory(path, new_path).await?;

            // 异步删除 new_path 文件夹下的 nfo 文件
            remove_nfo_files(new_path.clone()).await?;
        } else {
            info!(
                "File with the name {:?} already exists, cannot rename {:?}",
                new_path, path
            );
        }
    } else if fs::metadata(path).await.is_ok() {
        info!("Renaming {:?} to {:?}", path, new_path);
        fs::rename(path, new_path).await?;
    } else {
        trace!("File {:?} does not exist", path)
    }
    Ok(())
}

pub async fn rename_files_removing_prefixes<P: AsRef<Path>>(
    file_path: P,
    prefixes: &[String],
) -> IoResult<()> {
    let path = file_path.as_ref();
    trace!("rename_files_removing_prefixes: {:?}", path);
    // 异步检查文件是否存在
    match fs::metadata(path).await {
        Ok(metadata) => {
            if metadata.is_file() {
                if let Some(file_name) = path.file_name() {
                    let file_name_str = file_name.to_string_lossy();
                    for prefix in prefixes {
                        if file_name_str.starts_with(prefix) {
                            let new_file_name = file_name_str.replacen(prefix, "", 1);
                            let new_path = path.with_file_name(new_file_name);

                            rename_file(path, &new_path).await?;

                            break;
                        }
                    }
                }
            }
        }
        Err(e) => error!("Error accessing file {:?}: {}", path, e),
    }
    Ok(())
}

pub async fn rename_directories_to_uppercase<P: AsRef<Path>>(file_path: P) -> IoResult<()> {
    let path = file_path.as_ref();
    let metadata = fs::metadata(&path).await?;
    if metadata.is_dir() {
        if let Some(dir_name) = path.file_name() {
            let dir_name_str = dir_name.to_string_lossy();

            if dir_name_str.chars().any(|c| c.is_ascii_lowercase()) {
                let uppercased_name = dir_name_str.to_ascii_uppercase();
                let new_path = path.with_file_name(uppercased_name);

                rename_file(path, &new_path).await?;
            }
        }
    }
    Ok(())
}

pub async fn rename_files_removing_uncensored<P: AsRef<Path>>(file_path: P) -> IoResult<()> {
    let path = file_path.as_ref();
    let metadata = fs::metadata(&path).await?;
    if metadata.is_file() {
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

                rename_file(path, &new_path).await?;
            }
        }
    }
    Ok(())
}
