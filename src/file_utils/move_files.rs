use log::info;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io;

use crate::file_utils::rename_files::rename_files_removing_uncensored;

pub async fn move_directories<P: AsRef<Path>>(file_path: P, output_dir_path: P) -> io::Result<()> {
    let path = file_path.as_ref();
    let output_path = output_dir_path.as_ref();
    let is_dir: bool = fs::metadata(&path)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false);
    println!("is_file: {:?}", is_dir);
    if is_dir {
        if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
            let new_dir_name = if dir_name.ends_with("-UC")
                || dir_name.contains("UNCENSORED")
                || dir_name.contains("uncensored")
            {
                dir_name.replace("-UC", "")
            } else {
                dir_name.to_string()
            };

            println!("dir_name: {:?}", dir_name);

            let new_path = if dir_name.ends_with("-UC")
                || dir_name.contains("UNCENSORED")
                || dir_name.contains("uncensored")
            {
                output_path.join("UNCENSORED").join(&new_dir_name)
            } else if dir_name.ends_with("ch")
                || dir_name.ends_with("-C")
                || dir_name.ends_with("CH")
                || dir_name.ends_with("C_X1080X")
            {
                output_path.join("CHINESE").join(&new_dir_name)
            } else {
                PathBuf::new() // 如果不匹配任何条件，则不移动
            };

            if !new_path.as_os_str().is_empty() && !new_path.as_path().exists() {
                info!("Moving {:?} to {:?}", path, new_path);
                fs::rename(path, &new_path).await?;

                // 如果需要对移动到 new_path 的文件进行进一步处理
                if new_path.starts_with(output_path.join("UNCENSORED")) {
                    let mut entries = fs::read_dir(&new_path).await?;
                    while let Some(entry) = entries.next_entry().await? {
                        let entry_path = entry.path();
                        println!("Renaming in UNCENSORED dir: {:?}", entry_path);
                        // 注意：调用异步函数
                        rename_files_removing_uncensored(&entry_path)?;
                    }
                }
            } else {
                info!("Directory {:?} already exists", new_path);
            }
        }
    }
    Ok(())
}
