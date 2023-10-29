use std::fs;
use std::io::{self};
use std::path::Path;

pub fn create_category_directories<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let parent_path = path.as_ref().parent().unwrap();

    // 创建 "UNCENSORED" 文件夹
    let uncensored_path = parent_path.join("UNCENSORED");
    if !uncensored_path.exists() {
        fs::create_dir(&uncensored_path)?;
    }

    // 创建 "CHINESE" 文件夹
    let chinese_path = parent_path.join("CHINESE");
    if !chinese_path.exists() {
        fs::create_dir(&chinese_path)?;
    }

    Ok(())
}

pub fn move_directories<P: AsRef<Path>>(file_path: P) -> io::Result<()> {
    let path = file_path.as_ref();

    if path.is_dir() {
        if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
            let grandparent_path = path.parent().unwrap().parent().unwrap();
            if dir_name.ends_with("-UC") || dir_name.contains("UNCENSORED") {
                let new_path = grandparent_path.join("UNCENSORED").join(dir_name);
                println!("Moving {:?} to {:?}", path, new_path);
                fs::rename(&path, new_path)?;
            } else if dir_name.ends_with("ch")
                || dir_name.ends_with("-C")
                || dir_name.ends_with("CH")
            {
                let new_path = grandparent_path.join("CHINESE").join(dir_name);
                println!("Moving {:?} to {:?}", path, new_path);
                fs::rename(&path, new_path)?;
            }
        }
    }
    Ok(())
}
