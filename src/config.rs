use clap::{Parser, ValueEnum};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect};
use log::trace;
use log::{info, LevelFilter};
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;
use std::collections::HashSet;
use std::{path::PathBuf, sync::Mutex};

const PREFIXES: [&str; 12] = [
    "hhd800.com@",
    "zzpp01.com@",
    "第一會所新片@SIS001@",
    "zzpp05.com@",
    "RH2048.COM@",
    "[7sht.me]@",
    "[98t.tv]@",
    "[ThZu.Cc]@",
    "[99u.me]@",
    "[22sht.me]@",
    "AVAV66.XYZ@",
    "4k2.com@",
];

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

const PATTERNS: Lazy<HashSet<String>> = Lazy::new(|| {
    include_str!("../patterns.txt")
        .lines()
        .map(|s| s.to_string())
        .collect()
});

impl From<Cli> for CliConfig {
    fn from(cli: Cli) -> Self {
        CliConfig {
            dir: cli.dir,
            output_dir: cli.output_dir.map_or_else(PathBuf::new, PathBuf::from),
            delete_ad: cli.delete_ad,
            move_chinese: cli.move_chinese,
            move_uncensored: cli.move_uncensored,
            rename_upper_case: cli.rename_upper_case,
            remove_prefixes: cli.remove_prefixes,
            prefixes: PREFIXES.iter().map(|s| s.to_string()).collect(),
            patterns: PATTERNS.iter().cloned().collect(),
            delete_dir_with_no_video: cli.delete_dir_with_no_video,
        }
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

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Name of the person to greet
    #[arg(short = 'd', long)]
    pub dir: String,

    /// Organized target folder
    #[arg(short = 'o', long)]
    pub output_dir: Option<String>,

    /// Whether to delete ad files
    #[arg(long)]
    pub delete_ad: bool,

    /// Whether to delete directories with no video files
    /// e.g. directories with only nfo files
    /// e.g. directories with only trailers
    /// e.g. directories with only sample files
    #[arg(long)]
    pub delete_dir_with_no_video: bool,

    /// Whether to move chinese subtitle files
    #[arg(long)]
    pub move_chinese: bool,

    /// Whether to move UNCENSORED files
    #[arg(long)]
    pub move_uncensored: bool,

    /// Whether to use the --upper-case flag
    #[arg(long)]
    pub rename_upper_case: bool,

    /// Whether to use all options
    #[arg(short = 'a', long)]
    pub all: bool,

    /// Whether to use the --remove-prefixes flag
    /// Remove prefixes from file names
    /// e.g. [7sht.me]@ -> ""
    #[arg(long)]
    pub remove_prefixes: bool,

    #[arg(short = 'l', long, value_enum)]
    pub log_level: Option<LogLevel>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for LevelFilter {
    fn from(log_level: LogLevel) -> Self {
        match log_level {
            LogLevel::Error => LevelFilter::Error,
            LogLevel::Warn => LevelFilter::Warn,
            LogLevel::Info => LevelFilter::Info,
            LogLevel::Debug => LevelFilter::Debug,
            LogLevel::Trace => LevelFilter::Trace,
        }
    }
}

pub async fn interactive_config(mut cli: Cli) -> Result<Cli, Box<dyn std::error::Error>> {
    let theme = ColorfulTheme::default();

    if cli.all {
        cli.delete_ad = true;
        cli.delete_dir_with_no_video = true;
        cli.move_chinese = true;
        cli.move_uncensored = true;
        cli.rename_upper_case = true;
        cli.remove_prefixes = true;
    } else {
        let items = [
            "删除没有视频文件的文件夹",
            "移动中文字幕视频",
            "移动无码视频",
            "重命名文件夹名为大写",
            "删除文件名前缀，如 [7sht.me]@",
            "删除广告文件",
        ];
        let default_selections: Vec<bool> = vec![true; items.len()];


        let selections = MultiSelect::with_theme(&theme)
            .with_prompt("选择要使用的选项（按空格键选择）")
            .items(&items)
            // 使用 with_defaults 设置默认全选
            .defaults(&default_selections)
            .interact()?;

        for selection in selections {
            match selection {
                0 => cli.delete_dir_with_no_video = true,
                1 => cli.move_chinese = true,
                2 => cli.move_uncensored = true,
                3 => cli.rename_upper_case = true,
                4 => cli.remove_prefixes = true,
                5 => cli.delete_ad = true,
                _ => {}
            }
        }
    }

    if cli.output_dir.is_none() {
        let output_dir: String = Input::with_theme(&theme)
            .with_prompt("输入整理后的目标文件夹")
            .interact_text()?;
        cli.output_dir = Some(output_dir);
    }

    Ok(cli)
}
