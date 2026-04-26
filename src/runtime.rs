use std::path::PathBuf;

use color_eyre::Result;

use crate::actor_links::execute_actor_links_command;
use crate::cli::{Cli, Command};
use crate::nfo_check::execute_nfo_check_command;
use crate::operations::execute_operations_command;
use crate::report::{CommandReport, OutputFormat};

#[derive(Debug, Clone)]
pub enum RunRequest {
    Tui {
        dir: PathBuf,
    },
    Report {
        report: CommandReport,
        format: OutputFormat,
        exit_code: i32,
    },
}

pub async fn resolve_run_request(cli: Cli) -> Result<RunRequest> {
    match cli.command {
        Command::Tui(args) => Ok(RunRequest::Tui { dir: args.dir }),
        Command::Ops(args) => {
            let selected_operations = args.selected_operations();
            let report =
                execute_operations_command(args.dir, selected_operations, args.apply).await;
            Ok(RunRequest::Report {
                exit_code: report_exit_code(&report),
                format: if args.json {
                    OutputFormat::Json
                } else {
                    OutputFormat::Text
                },
                report,
            })
        }
        Command::ActorLinks(args) => {
            let report = execute_actor_links_command(args.source, args.actors_root, args.apply)?;
            Ok(RunRequest::Report {
                exit_code: report_exit_code(&report),
                format: if args.json {
                    OutputFormat::Json
                } else {
                    OutputFormat::Text
                },
                report,
            })
        }
        Command::NfoCheck(args) => {
            if args.codes_only {
                // Special mode: just print movie codes, one per line
                match crate::nfo_check::missing_codes_only(
                    &args.dir,
                    args.max_depth,
                    &args.skip,
                ) {
                    Ok(codes) => {
                        println!("{codes}");
                        std::process::exit(0);
                    }
                    Err(e) => {
                        eprintln!("Error: {e}");
                        std::process::exit(1);
                    }
                }
            }
            let report =
                execute_nfo_check_command(args.dir, args.max_depth, args.skip);
            Ok(RunRequest::Report {
                exit_code: report_exit_code(&report),
                format: if args.json {
                    OutputFormat::Json
                } else {
                    OutputFormat::Text
                },
                report,
            })
        }
    }
}

fn report_exit_code(report: &CommandReport) -> i32 {
    if let Some(verification) = report.verification.as_ref() {
        return verification.exit_code;
    }

    if report.summary.failed_actions > 0 || report.summary.error_count > 0 {
        1
    } else {
        0
    }
}
