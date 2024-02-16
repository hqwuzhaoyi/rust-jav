use log::trace;
use once_cell::sync::OnceCell;
use std::{path::PathBuf, sync::Mutex};

#[derive(Debug, Clone)]
pub struct CliConfig {
    pub dir: String,
    pub output_dir: PathBuf,
    pub delete_ad: bool,
    pub delete_dir_with_no_video: bool,
    pub remove_prefixes: bool,
    pub move_chinese: bool,
    pub move_uncensored: bool,
    pub rename_upper_case: bool,
    pub prefixes: Vec<String>,
    pub patterns: Vec<String>,
}

impl CliConfig {
    // 添加一个方法来封装你的逻辑
    pub fn should_create_directories(&self) -> bool {
        trace!("should_create_directories is called");
        trace!("output_dir.exists(): {}", self.output_dir.exists());
        self.output_dir.exists() && (self.move_chinese || self.move_uncensored)
    }

    pub fn should_remove_prefixes(&self) -> bool {
        trace!("should_remove_prefixes is called");
        self.remove_prefixes
    }

    pub fn should_delete_ad_files(&self) -> bool {
        trace!("should_delete_ad_files is called");
        self.delete_ad
    }

    pub fn should_delete_dir_with_no_video(&self) -> bool {
        trace!("should_delete_dir_with_no_video is called");
        self.delete_dir_with_no_video
    }

    pub fn should_rename_upper_case(&self) -> bool {
        trace!("should_rename_upper_case is called");
        self.rename_upper_case
    }

    pub fn should_move_chinese(&self) -> bool {
        trace!("should_move_chinese is called");
        self.move_chinese
    }

    pub fn should_move_uncensored(&self) -> bool {
        trace!("should_move_uncensored is called");
        self.move_uncensored
    }

    pub fn should_move_dir(&self) -> bool {
        trace!("should_move_dir  is called");
        self.move_chinese || self.move_uncensored
    }

    pub fn should_use_all_options(&self) -> bool {
        trace!("should_use_all_options is called");
        self.all_options()
    }

    pub fn all_options(&self) -> bool {
        trace!("all_options is called");
        self.move_chinese
            && self.move_uncensored
            && self.delete_ad
            && self.rename_upper_case
            && self.remove_prefixes
    }
}

pub static CONFIG: OnceCell<Mutex<CliConfig>> = OnceCell::new();

// 提供一个设置全局配置的公共函数
pub fn set_config(config: CliConfig) -> Result<(), &'static str> {
    CONFIG
        .set(Mutex::new(config))
        .map_err(|_| "Configuration has already been set")
}

// 提供一个获取全局配置的公共函数
pub fn get_config() -> Option<std::sync::MutexGuard<'static, CliConfig>> {
    CONFIG.get().map(|mutex| mutex.lock().unwrap())
}
