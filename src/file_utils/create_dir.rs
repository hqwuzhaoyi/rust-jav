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
