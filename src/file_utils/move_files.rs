use log::info;
use std::fs;
use std::io::{self};
use std::path::Path;

use crate::file_utils::rename_files::rename_files_removing_uncensored;

pub fn create_category_directories<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let output_path = path.as_ref();

    // 创建 "UNCENSORED" 文件夹
    let uncensored_path = output_path.join("UNCENSORED");
    if !uncensored_path.exists() {
        info!("Creating directory {:?}", uncensored_path);
        fs::create_dir(&uncensored_path)?;
    }

    // 创建 "CHINESE" 文件夹
    let chinese_path = output_path.join("CHINESE");
    if !chinese_path.exists() {
        info!("Creating directory {:?}", chinese_path);
        fs::create_dir(&chinese_path)?;
    }

    Ok(())
}

pub fn move_directories<P: AsRef<Path>>(file_path: P, output_dir_path: P) -> io::Result<()> {
    let path = file_path.as_ref();
    let output_path = output_dir_path.as_ref();
    if path.is_dir() {
        if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
            if dir_name.ends_with("-UC")
                || dir_name.contains("UNCENSORED")
                || dir_name.contains("uncensored")
            {
                // new_path 去掉后缀 -UC、UNCENSORED、uncensored
                let new_path = output_path
                    .join("UNCENSORED")
                    .join(dir_name.replace("-UC", ""));
                info!("Moving {:?} to {:?}", path, new_path);
                if !new_path.exists() {
                    fs::rename(&path, new_path.clone())?;

                    // 去new_path 内的所有文件调用 rename_files_removing_uncensored
                    for entry in fs::read_dir(&new_path)? {
                        let entry = entry?;
                        let path = entry.path();
                        println!("rename UNCENSORED dir: {:?}", path);
                        rename_files_removing_uncensored(&path)?;
                    }
                } else {
                    info!("Directory {:?} already exists", new_path);
                }
            } else if dir_name.ends_with("ch")
                || dir_name.ends_with("-C")
                || dir_name.ends_with("CH")
                || dir_name.ends_with("C_X1080X")
            {
                let new_path = output_path.join("CHINESE").join(dir_name);
                info!("Moving {:?} to {:?}", path, new_path);
                if !new_path.exists() {
                    fs::rename(&path, new_path)?;
                } else {
                    info!("Directory {:?} already exists", new_path);
                }
            }
        }
    }
    Ok(())
}
