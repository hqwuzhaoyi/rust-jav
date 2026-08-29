use std::path::PathBuf;

use color_eyre::Result;

use crate::application::{
    ActorViewRequest, ApplicationServices, NfoCheckRequest, OperationsRequest, ReportingService,
};
use crate::cli::{Cli, Command};
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
    let services = ApplicationServices::new();
    match cli.command {
        Command::Tui(args) => Ok(RunRequest::Tui { dir: args.dir }),
        Command::Ops(args) => {
            let selected_operations = args.selected_operations();
            let request = if args.apply {
                OperationsRequest::apply(args.dir, selected_operations)
            } else {
                OperationsRequest::preview(args.dir, selected_operations)
            };
            let report = services.operations().run(request).await;
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
            let request = if args.apply {
                ActorViewRequest::apply(args.source, args.actors_root)
            } else {
                ActorViewRequest::preview(args.source, args.actors_root)
            };
            let report = services.actor_view().run(request)?;
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
                let request = NfoCheckRequest {
                    source_dir: args.dir.clone(),
                    max_depth: args.max_depth,
                    skip: args.skip.clone(),
                };
                match services.nfo().missing_codes(&request) {
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
            let report = services.nfo().check(NfoCheckRequest {
                source_dir: args.dir,
                max_depth: args.max_depth,
                skip: args.skip,
            });
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
    ReportingService::exit_code(report)
}
