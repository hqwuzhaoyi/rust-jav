use clap::{Parser, ValueEnum};
use env_logger;
use log::{info, LevelFilter};
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::path::PathBuf;

mod config;
mod file_utils;

use config::{set_config, CliConfig};

const PATTERNS: Lazy<HashSet<String>> = Lazy::new(|| {
    include_str!("../patterns.txt")
        .lines()
        .map(|s| s.to_string())
        .collect()
});

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Name of the person to greet
    #[arg(short = 'd', long)]
    dir: String,

    /// Organized target folder
    #[arg(short = 'o', long)]
    output_dir: Option<String>,

    /// Whether to delete ad files
    #[arg(long)]
    delete_ad: bool,

    /// Whether to move chinese subtitle files
    #[arg(long)]
    move_chinese: bool,

    /// Whether to move UNCENSORED files
    #[arg(long)]
    move_uncensored: bool,

    /// Whether to use the --upper-case flag
    #[arg(long)]
    rename_upper_case: bool,

    /// Whether to use all options
    #[arg(short = 'a', long)]
    all: bool,

    /// Whether to use the --remove-prefixes flag
    /// Remove prefixes from file names
    /// e.g. [7sht.me]@ -> ""
    #[arg(long)]
    remove_prefixes: bool,

    #[arg(short = 'l', long, value_enum)]
    log_level: Option<LogLevel>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum LogLevel {
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let log_level = cli.log_level.unwrap_or(LogLevel::Info);
    env_logger::Builder::new()
        .filter(None, LevelFilter::from(log_level))
        .init();

    info!("Start to organize files...");

    // let dir = &cli.dir; // 获取 dir 参数的值
    let pattern_slice: Vec<String> = PATTERNS.iter().cloned().collect();
    info!("all pattern {:?}", pattern_slice);

    let delete_ad = cli.delete_ad || cli.all;
    let move_chinese = cli.move_chinese || cli.all;
    let move_uncensored = cli.move_uncensored || cli.all;
    let rename_upper_case = cli.rename_upper_case || cli.all;
    let remove_prefixes = cli.remove_prefixes || cli.all;

    let prefixes = [
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
    ];

    let dir = cli.dir.clone();

    let config = CliConfig {
        dir: cli.dir,
        output_dir: cli.output_dir.map_or_else(PathBuf::new, PathBuf::from),
        delete_ad,
        move_chinese,
        move_uncensored,
        rename_upper_case,
        remove_prefixes,
        prefixes: prefixes.iter().map(|s| s.to_string()).collect(),
        patterns: pattern_slice,
    };

    let output_dir = config.output_dir.clone();
    let should_create_directories = config.should_create_directories();
    set_config(config)?;

    info!("should_create_directories: {}", should_create_directories);

    if should_create_directories {
        info!("Creating category directories...");
        let path = output_dir.as_path();
        file_utils::move_files::create_category_directories(path)?;
    }

    file_utils::traverse_directory(true, dir)?;
    Ok(())
}
