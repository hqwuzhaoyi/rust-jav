use log::info;
use std::fs;
use std::io::{self};
use std::path::Path;

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
            if dir_name.ends_with("-UC") || dir_name.contains("UNCENSORED") {
                let new_path = output_path.join("UNCENSORED").join(dir_name);
                info!("Moving {:?} to {:?}", path, new_path);
                if !new_path.exists() {
                    fs::rename(&path, new_path)?;
                } else {
                    info!("Directory {:?} already exists", new_path);
                }
            } else if dir_name.ends_with("ch")
                || dir_name.ends_with("-C")
                || dir_name.ends_with("CH")
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
