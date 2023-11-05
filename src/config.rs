use log::trace;
use once_cell::sync::OnceCell;
use std::{path::PathBuf, sync::Mutex};

#[derive(Debug, Clone)]
pub struct CliConfig {
    pub dir: String,
    pub output_dir: PathBuf,
    pub delete_ad: bool,
    pub move_chinese: bool,
    pub move_uncensored: bool,
    pub rename_upper_case: bool,
    pub remove_prefixes: bool,
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
