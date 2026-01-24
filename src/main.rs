use std::path::PathBuf;

use clap::Parser;
use color_eyre::Result;
use tokio::sync::mpsc;

use rust_jav::config::Cli;
use rust_jav::tui;

#[tokio::main]
async fn main() -> Result<()> {
    // Install color_eyre panic and error hooks
    color_eyre::install()?;

    let cli_args = Cli::parse();

    // Get the source directory as PathBuf
    let source_dir = PathBuf::from(&cli_args.dir);

    // Initialize the TUI
    let mut terminal = tui::init_terminal()?;

    // Create action channel
    let (action_tx, _action_rx) = mpsc::unbounded_channel();

    // Create the app
    let app = tui::App::new(source_dir, action_tx);

    // Run the TUI event loop
    let result = tui::run_app(&mut terminal, app).await;

    // Restore terminal on exit
    tui::restore_terminal(&mut terminal)?;

    // Return any error from the event loop
    result
}
