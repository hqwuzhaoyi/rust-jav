use async_recursion::async_recursion;
use log::error;
use log::trace;
use std::fs;
use std::io;
use std::path::Path;

pub mod create_dir;
pub mod delete_files;
pub mod move_files;
pub mod rename_files_async;

#[async_recursion]
pub async fn traverse_directory<P: AsRef<Path> + Send + Sync + 'static>(
    is_root: bool,
    sub_path: P,
) -> io::Result<()> {
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
                        continue;
                    }
                };
                let path = entry.path();

                // 对每个文件执行删除操作
                match delete_files::delete_files_matching_patterns(&path, patterns).await {
                    Ok(dir_deleted) => {
                        // if dir_deleted {
                        //     // 目录被删除，可能不需要继续后续的重命名或其他操作
                        //     trace!("Directory deleted, skipping further actions for this path.");
                        //     continue; // 跳过当前迭代
                        // }
                    }
                    Err(e) => {
                        error!("Error deleting files: {}", e);
                        continue;
                    }
                }

                // 删除没有视频的目录，并根据返回值决定是否继续
                match delete_files::delete_dir_with_no_video(path.clone()).await {
                    Ok(dir_deleted) => {
                        if dir_deleted {
                            // 目录被删除，可能不需要继续后续的重命名或其他操作
                            trace!("Directory deleted, skipping further actions for this path.");
                            continue; // 跳过当前迭代
                        }
                    }
                    Err(e) => {
                        error!("Error deleting directories: {}", e);
                        continue;
                    }
                }

                if fs::metadata(&path).is_ok() {
                    // 对每个文件执行重命名操作
                    if let Err(e) =
                        rename_files_async::rename_files_removing_prefixes(&path, prefixes).await
                    {
                        error!("Error renaming files: {}", e);
                        continue;
                    }
                } else {
                    trace!("File {:?} does not exist", path);
                }

                // 如果是目录，则递归调用
                if path.is_dir() {
                    if let Err(e) = traverse_directory(false, path.clone()).await {
                        error!("Error traversing directory: {}", e);
                        continue;
                    }
                }

                // 对于根目录特有的操作
                if is_root {
                    let should_rename_upper_case = config.should_rename_upper_case();
                    if should_rename_upper_case {
                        if let Err(e) =
                            rename_files_async::rename_directories_to_uppercase(&path).await
                        {
                            error!("Error renaming directories to uppercase: {}", e);
                            continue;
                        }
                    }
                    if output_dir_path.exists() {
                        if let Err(e) = move_files::move_directories(&path, &output_dir_path).await
                        {
                            error!("Error moving directories: {}", e);
                            continue;
                        }
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
