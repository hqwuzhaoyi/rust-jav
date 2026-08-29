//! Shared application-service boundary for every user interface.
//!
//! The CLI, TUI, and future management interface should depend on these
//! services instead of assembling filesystem operations themselves.

use std::io;
use std::path::PathBuf;

use crate::migration_verifier::types::{
    MigrationAction, MigrationScope, ScopeManifest, VerificationReport, VerificationSummary,
};
use crate::report::{CommandReport, OutputFormat};
use crate::tui::executor::{OperationExecutor, OperationPlan, OperationResult};
use crate::tui::state::{Operation, OperationType};

#[derive(Debug, Clone, Copy, Default)]
pub struct ApplicationServices;

impl ApplicationServices {
    pub fn new() -> Self {
        Self
    }

    pub fn operations(&self) -> OperationsService {
        OperationsService
    }

    pub fn actor_view(&self) -> ActorViewService {
        ActorViewService
    }

    pub fn nfo(&self) -> NfoService {
        NfoService
    }

    pub fn verification(&self) -> VerificationService {
        VerificationService
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VerificationService;

impl VerificationService {
    pub fn scan(
        &self,
        root: &std::path::Path,
        scope: MigrationScope,
        allow_missing: bool,
    ) -> io::Result<ScopeManifest> {
        crate::migration_verifier::fs_scan::scan_scope(root, scope, allow_missing)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn verify_operations(
        &self,
        source_dir: PathBuf,
        before_source: ScopeManifest,
        actions: Vec<MigrationAction>,
        requires_manual_confirm: bool,
        failed_actions: usize,
        warnings: Vec<String>,
        errors: Vec<String>,
    ) -> io::Result<(VerificationSummary, VerificationReport)> {
        crate::migration_verifier::verify_ops(
            source_dir,
            before_source,
            actions,
            requires_manual_confirm,
            failed_actions,
            warnings,
            errors,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn verify_actor_view(
        &self,
        source_dir: PathBuf,
        actors_root: PathBuf,
        before_source: ScopeManifest,
        before_actors: ScopeManifest,
        actions: Vec<MigrationAction>,
        failed_actions: usize,
        warnings: Vec<String>,
        errors: Vec<String>,
    ) -> io::Result<(VerificationSummary, VerificationReport)> {
        crate::migration_verifier::verify_actor_links(
            source_dir,
            actors_root,
            before_source,
            before_actors,
            actions,
            failed_actions,
            warnings,
            errors,
        )
    }

    pub fn error_summary(&self) -> VerificationSummary {
        crate::migration_verifier::summary_from_error()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationsRequest {
    pub source_dir: PathBuf,
    pub selected_ops: Vec<OperationType>,
    pub apply: bool,
}

impl OperationsRequest {
    pub fn preview(source_dir: PathBuf, selected_ops: Vec<OperationType>) -> Self {
        Self {
            source_dir,
            selected_ops,
            apply: false,
        }
    }

    pub fn apply(source_dir: PathBuf, selected_ops: Vec<OperationType>) -> Self {
        Self {
            source_dir,
            selected_ops,
            apply: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OperationsService;

impl OperationsService {
    pub async fn run(&self, request: OperationsRequest) -> CommandReport {
        crate::operations::run_operations_command(
            request.source_dir,
            request.selected_ops,
            request.apply,
        )
        .await
    }

    pub async fn analyze(&self, source_dir: PathBuf) -> Vec<(OperationType, Vec<PathBuf>)> {
        OperationExecutor::new(source_dir, true)
            .analyze_operations()
            .await
    }

    pub async fn plan(&self, source_dir: PathBuf, op_type: OperationType) -> OperationPlan {
        OperationExecutor::new(source_dir, true)
            .plan_operation(op_type)
            .await
    }

    pub async fn execute(&self, source_dir: PathBuf, op_type: OperationType) -> OperationResult {
        OperationExecutor::new(source_dir, false)
            .execute(&Operation::new(op_type))
            .await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorViewRequest {
    pub source_dir: PathBuf,
    pub actors_root: PathBuf,
    pub apply: bool,
}

impl ActorViewRequest {
    pub fn preview(source_dir: PathBuf, actors_root: PathBuf) -> Self {
        Self {
            source_dir,
            actors_root,
            apply: false,
        }
    }

    pub fn apply(source_dir: PathBuf, actors_root: PathBuf) -> Self {
        Self {
            source_dir,
            actors_root,
            apply: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ActorViewService;

impl ActorViewService {
    pub fn run(&self, request: ActorViewRequest) -> io::Result<CommandReport> {
        crate::actor_links::run_actor_links_command(
            request.source_dir,
            request.actors_root,
            request.apply,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NfoCheckRequest {
    pub source_dir: PathBuf,
    pub max_depth: usize,
    pub skip: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NfoService;

impl NfoService {
    pub fn check(&self, request: NfoCheckRequest) -> CommandReport {
        crate::nfo_check::run_nfo_check_command(request.source_dir, request.max_depth, request.skip)
    }

    pub fn missing_codes(&self, request: &NfoCheckRequest) -> io::Result<String> {
        crate::nfo_check::missing_codes_only(&request.source_dir, request.max_depth, &request.skip)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReportingService;

impl ReportingService {
    pub fn render(report: &CommandReport, format: OutputFormat) -> String {
        match format {
            OutputFormat::Text => report.to_text(),
            OutputFormat::Json => report.to_json(),
        }
    }

    pub fn exit_code(report: &CommandReport) -> i32 {
        if let Some(verification) = report.verification.as_ref() {
            verification.exit_code
        } else if report.summary.failed_actions > 0 || report.summary.error_count > 0 {
            1
        } else {
            0
        }
    }
}
