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

    // 创建 "ORIGIN" 文件夹
    let origin_path = output_path.join("ORIGIN");
    if !origin_path.exists() {
        info!("Creating directory {:?}", origin_path);
        fs::create_dir(&origin_path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("rust-jav-createdir-{label}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn creates_uncensored_chinese_origin_dirs() {
        let dir = temp_dir("cats");

        create_category_directories(&dir).unwrap();

        assert!(dir.join("UNCENSORED").is_dir());
        assert!(dir.join("CHINESE").is_dir());
        assert!(dir.join("ORIGIN").is_dir());

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn is_idempotent_when_dirs_already_exist() {
        let dir = temp_dir("cats-idempotent");
        std::fs::create_dir_all(dir.join("UNCENSORED")).unwrap();
        std::fs::create_dir_all(dir.join("CHINESE")).unwrap();
        std::fs::create_dir_all(dir.join("ORIGIN")).unwrap();

        // Should not error when directories already exist
        create_category_directories(&dir).unwrap();

        std::fs::remove_dir_all(dir).unwrap();
    }
}
