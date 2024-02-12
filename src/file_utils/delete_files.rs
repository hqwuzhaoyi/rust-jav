use log::{info, trace};
use regex::Regex;
// use std::fs;
// use std::io::{self};
use async_recursion::async_recursion;
use std::path::Path;
use tokio::fs;
use tokio::io;

const VIDEO_PATTERNS: [&str; 4] = ["mp4", "mkv", "avi", "wmv"];

pub async fn delete_files_matching_patterns<P: AsRef<Path>>(
    file_path: P,
    patterns: &[String],
) -> io::Result<()> {
    let path = file_path.as_ref();
    let is_file: bool = fs::metadata(&path)
        .await
        .map(|m| m.is_file())
        .unwrap_or(false);

    if is_file {
        let file_name = match path.file_name() {
            Some(name) => name.to_string_lossy().into_owned(),
            None => return Ok(()),
        };
        for pattern in patterns {
            let regex_pattern = format!("^{}.*$", pattern.replace("*", ".*"));
            let re = Regex::new(&regex_pattern).unwrap();
            if re.is_match(&file_name) {
                info!("will delete file: {:?}", path);
                // 使用 tokio 的异步删除文件功能
                fs::remove_file(path).await?;
                break;
            }
        }
    }
    Ok(())
}

// 删除目录下有nfo文件但是没有视频的目录
#[async_recursion]
pub async fn delete_dir_with_no_video<P: AsRef<Path> + Send + Sync + 'static>(
    path: P,
) -> io::Result<()> {
    let path_ref = path.as_ref();
    if path_ref.is_dir() {
        let mut has_video = false;
        let mut has_nfo = false;
        let mut only_has_trailers_dir = true;

        let mut entries = fs::read_dir(path_ref).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                trace!("path.is_dir: {:?}", path);
                delete_dir_with_no_video(path.clone()).await?;
                let dir_name = path.file_name().unwrap().to_string_lossy();

                if dir_name != "trailers" {
                    only_has_trailers_dir = false;
                }
            } else {
                let file_name = path.file_name().unwrap().to_string_lossy();

                for pattern in VIDEO_PATTERNS.iter() {
                    if file_name.ends_with(pattern) {
                        has_video = true;
                        break;
                    }
                }

                if file_name.ends_with("nfo") {
                    has_nfo = true;
                }
            }
        }

        trace!("has_nfo: {:?}", has_nfo);
        trace!("has_video: {:?}", has_video);
        trace!("only_has_trailers_dir: {:?}", only_has_trailers_dir);
        trace!("path: {:?}", path_ref);

        if !has_video && (has_nfo || !only_has_trailers_dir) {
            info!("will delete dir: {:?}", path_ref);
            // 取消下面这行注释以启用删除功能
            fs::remove_dir_all(path_ref).await?;
        }
    } else {
        trace!("is not dir: {:?}", path_ref);
    }
    Ok(())
}
