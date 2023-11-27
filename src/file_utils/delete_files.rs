use log::{info, trace};
use regex::Regex;
use std::fs;
use std::io::{self};
use std::path::Path;

const VIDEO_PATTERNS: [&str; 4] = ["mp4", "mkv", "avi", "wmv"];

pub fn delete_files_matching_patterns<P: AsRef<Path>>(
    file_path: P,
    patterns: &[String],
) -> io::Result<()> {
    let path = file_path.as_ref();
    if path.is_file() {
        let file_name = match path.file_name() {
            Some(name) => name.to_string_lossy(),
            None => return Ok(()),
        };
        for pattern in patterns {
            let regex_pattern = format!("^{}.*$", pattern.replace("*", ".*"));
            let re = Regex::new(&regex_pattern).unwrap();
            if re.is_match(&file_name) {
                info!("will delete file: {:?}", path);
                // 取消下面这行注释以启用删除功能
                fs::remove_file(path)?;
                break;
            }
        }
    }
    Ok(())
}

// 删除目录下有nfo文件但是没有视频的目录
pub fn delete_dir_with_no_video<P: AsRef<Path>>(path: P) -> io::Result<()>
where
    P: AsRef<Path>,
{
    if path.as_ref().is_dir() {
        let mut has_video = false;
        let mut has_nfo = false;
        for entry in fs::read_dir(&path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                trace!("path.is_dir: {:?}", path);
                delete_dir_with_no_video(&path)?;
            } else if path.is_file() {
                trace!("path.is_file: {:?}", path);
                let file_name = match path.file_name() {
                    Some(name) => name.to_string_lossy(),
                    None => return Ok(()),
                };

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
        trace!("path: {:?}", path.as_ref());

        if has_nfo && !has_video {
            let file_path = path.as_ref();
            info!("will delete dir: {:?}", file_path);
            // 取消下面这行注释以启用删除功能
            fs::remove_dir_all(path)?;
        }
    } else {
        let file_path = path.as_ref();
        trace!("is not dir: {:?}", file_path);
    }
    Ok(())
}
