use log::error;
use log::info;
use log::trace;
use std::fs;
use std::io;
use std::path::Path;
use async_recursion::async_recursion;

pub mod delete_files;
pub mod move_files;
pub mod rename_files;

#[async_recursion]
pub async fn traverse_directory<P: AsRef<Path> + Send + Sync + 'static>(is_root: bool, sub_path: P) -> io::Result<()> {
    trace!("traverse_directory is called");
    let config = {
        let guard = crate::config::get_config().unwrap(); // 假设这个函数返回一个鎖的保護者
        guard.clone() // 複製數據，保護者在這個大括號結束時釋放鎖
    }; // 鎖在此被釋放，因為保護者 guard 離開了作用域
    trace!("config: {:?}", config);
    //  从默认值没有的话，就从 config 中获取
    let path = if sub_path.as_ref().to_str().unwrap() == "" {
        Path::new(&config.dir)
    } else {
        sub_path.as_ref()
    };
    let output_dir_path = &config.output_dir;
    let prefixes = &config.prefixes;
    let patterns = &config.patterns;

    trace!("traverse_directory: {:?}", path);
    trace!("output_dir_path: {:?}", output_dir_path);
    trace!("prefixes: {:?}", prefixes);
    trace!("patterns: {:?}", patterns);

    let path = sub_path.as_ref();
    // 处理 fs::read_dir 的结果
    match fs::read_dir(path) {
        Ok(entries) => {
            for entry in entries {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        error!("Failed to access entry in directory {:?}: {}", path, e);
                        continue; // 跳过这个项目，继续下一个
                    }
                };
                let path = entry.path();

                // 对每个文件执行删除操作
                trace!("Before delete files111: {:?}", path);
                let _ = delete_files::delete_files_matching_patterns(&path, patterns).await?;
                trace!("delete files end");

                trace!("Before delete directories: {:?}", path);
                let _ = delete_files::delete_dir_with_no_video(path.clone()).await?;
                trace!("delete directories end");

                // 对每个文件执行重命名操作
                trace!("Before rename files: {:?}", path);
                rename_files::rename_files_removing_prefixes(&path, prefixes)?;
                trace!("rename files end");

                // 如果是目录，则递归调用
                if path.is_dir() {
                    trace!("traverse_directory: {:?}", path);
                    traverse_directory(false, path.clone()).await?;
                }

                if is_root {
                    trace!("Before rename directories: {:?}", path);
                    rename_files::rename_directories_to_uppercase(&path)?;
                    trace!("rename directories end");
                    if output_dir_path.exists() {
                        trace!("move files: {:?}", path);
                        move_files::move_directories(&path, &output_dir_path)?;
                        trace!("move files end");
                    }
                }
            }
        }
        Err(e) => {
            error!("Failed to read directory {:?}: {}", path, e);
            return Err(e);
        }
    }
    Ok(())
}
