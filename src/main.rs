use clap::Parser;
use env_logger;
use log::{info, LevelFilter};
use std::path::PathBuf;

mod config;
mod file_utils;

use config::{interactive_config, set_config, Cli, CliConfig, LogLevel};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args = Cli::parse();

    let log_level: LogLevel = cli_args.log_level.unwrap_or(LogLevel::Info);
    env_logger::Builder::new()
        .filter(None, LevelFilter::from(log_level))
        .init();

    info!("Start to organize files...");

    let dir = cli_args.dir.clone();
    let cli = interactive_config(cli_args).await?;

    let config = CliConfig {
        ..cli.into()
    };

    let output_dir = config.output_dir.clone();
    let should_create_directories = config.should_create_directories();
    set_config(config)?;

    info!("should_create_directories: {}", should_create_directories);

    if should_create_directories {
        info!("Creating category directories...");
        let path = output_dir.as_path();
        file_utils::create_dir::create_category_directories(path)?;
    }

    file_utils::traverse_directory(true, dir).await?;
    Ok(())
}
