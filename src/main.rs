use clap::Parser;
use color_eyre::Result;
use tokio::sync::mpsc;

use rust_jav::cli::Cli;
use rust_jav::report::OutputFormat;
use rust_jav::runtime::{resolve_run_request, RunRequest};
use rust_jav::tui;

#[tokio::main]
async fn main() -> Result<()> {
    // Install color_eyre panic and error hooks
    color_eyre::install()?;

    let cli_args = Cli::parse();

    match resolve_run_request(cli_args).await? {
        RunRequest::Tui { dir } => {
            let mut terminal = tui::init_terminal()?;
            let (action_tx, _action_rx) = mpsc::unbounded_channel();
            let app = tui::App::new(dir, action_tx);
            let result = tui::run_app(&mut terminal, app).await;
            tui::restore_terminal(&mut terminal)?;
            result
        }
        RunRequest::Report {
            report,
            format,
            exit_code,
        } => {
            print_report(&report, format);
            if exit_code != 0 {
                std::process::exit(exit_code);
            }
            Ok(())
        }
    }
}

fn print_report(report: &rust_jav::report::CommandReport, format: OutputFormat) {
    let output = match format {
        OutputFormat::Text => report.to_text(),
        OutputFormat::Json => report.to_json(),
    };
    println!("{output}");
}
