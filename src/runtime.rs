use std::path::PathBuf;

use color_eyre::Result;

use crate::application::{
    ActorViewRequest, ApplicationServices, NfoCheckRequest, OperationsRequest, ReportingService,
};
use crate::cli::{AdministratorAction, Cli, Command};
use crate::report::{CommandReport, OutputFormat};

#[derive(Debug, Clone)]
pub enum RunRequest {
    Tui {
        dir: PathBuf,
    },
    Report {
        report: Box<CommandReport>,
        format: OutputFormat,
        exit_code: i32,
    },
    Serve {
        config: PathBuf,
    },
    Administrator {
        action: AdministratorAction,
    },
}

pub async fn resolve_run_request(cli: Cli) -> Result<RunRequest> {
    let services = ApplicationServices::new();
    match cli.command {
        Command::Tui(args) => Ok(RunRequest::Tui { dir: args.dir }),
        Command::Ops(args) => {
            let selected_operations = args.selected_operations();
            let active_rules = match args.rules.as_deref() {
                Some(path) => {
                    crate::active_rules::ActiveRuleSet::load(path, args.confirm_empty_rules)?
                }
                None => crate::active_rules::ActiveRuleSet::embedded(),
            };
            let request = if args.apply {
                OperationsRequest::apply_with_rules(args.dir, selected_operations, active_rules)
            } else {
                OperationsRequest::preview_with_rules(args.dir, selected_operations, active_rules)
            };
            let report = services.operations().run(request).await;
            Ok(RunRequest::Report {
                exit_code: report_exit_code(&report),
                format: if args.json {
                    OutputFormat::Json
                } else {
                    OutputFormat::Text
                },
                report: Box::new(report),
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
                report: Box::new(report),
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
                report: Box::new(report),
            })
        }
        Command::Serve(args) => Ok(RunRequest::Serve {
            config: args.config,
        }),
        Command::Administrator(args) => Ok(RunRequest::Administrator {
            action: args.action,
        }),
    }
}

fn report_exit_code(report: &CommandReport) -> i32 {
    ReportingService::exit_code(report)
}
