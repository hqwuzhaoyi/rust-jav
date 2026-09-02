pub mod compare;
pub mod expected;
pub mod fs_scan;
pub mod report;
pub mod types;

use std::io;
use std::path::PathBuf;

use compare::compare_manifests;
use expected::{build_expected_for_actor_links, build_expected_for_ops};
use fs_scan::scan_scope;
use report::{default_report_path, write_report};
use types::{
    ApprovalStatus, MigrationAction, MigrationScope, ScopeConfig, VerificationPlan,
    VerificationReport, VerificationStatus, VerificationSummary,
};

pub use types::{
    ManifestEntry, MigrationActionKind, ScopeCountSummary, ScopeDiff, ScopeManifest,
    VerificationPlan as MigrationVerificationPlan,
};

pub fn verify_ops(
    source_dir: PathBuf,
    before_source: types::ScopeManifest,
    actions: Vec<MigrationAction>,
    requires_manual_confirm: bool,
    failed_actions: usize,
    warnings: Vec<String>,
    errors: Vec<String>,
) -> io::Result<(VerificationSummary, VerificationReport)> {
    let plan = VerificationPlan {
        command: "ops".to_string(),
        mode: "apply".to_string(),
        scopes: vec![ScopeConfig {
            scope: MigrationScope::Source,
            root_dir: source_dir.clone(),
            allow_missing: false,
        }],
        actions,
        requires_manual_confirm,
    };
    let (expected_source, expected_stats) =
        build_expected_for_ops(&before_source, &plan.actions, &source_dir);
    let after_source = scan_scope(&source_dir, MigrationScope::Source, false)?;

    let before = vec![before_source];
    let expected = vec![expected_source];
    let after = vec![after_source];

    let comparison = compare_manifests(
        &expected,
        &after,
        &before,
        plan.requires_manual_confirm,
        failed_actions,
        &errors,
        &expected_stats.plan_conflicts,
    );
    let exit_code = exit_code_for(comparison.verification_status, comparison.approval_status);
    let report_path = default_report_path(&plan.command);
    let report = VerificationReport {
        version: 1,
        command: plan.command.clone(),
        mode: plan.mode.clone(),
        verification_status: comparison.verification_status,
        approval_status: comparison.approval_status,
        exit_code,
        report_path: report_path.clone(),
        before,
        expected,
        after,
        scope_counts: comparison.scope_counts.clone(),
        scope_extension_counts: comparison.scope_extension_counts,
        diffs: comparison.diffs,
        failed_actions,
        errors,
        warnings,
        expected_stats,
    };
    write_report(&report)?;

    Ok((
        VerificationSummary {
            verification_status: report.verification_status,
            approval_status: report.approval_status,
            exit_code,
            report_path: Some(report_path),
            scopes: comparison.scope_counts,
        },
        report,
    ))
}

pub struct ActorLinkVerificationInput {
    pub source_dir: PathBuf,
    pub actors_root: PathBuf,
    pub before_source: types::ScopeManifest,
    pub before_actors: types::ScopeManifest,
    pub actions: Vec<MigrationAction>,
    pub failed_actions: usize,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

pub fn verify_actor_links(
    input: ActorLinkVerificationInput,
) -> io::Result<(VerificationSummary, VerificationReport)> {
    let ActorLinkVerificationInput {
        source_dir,
        actors_root,
        before_source,
        before_actors,
        actions,
        failed_actions,
        warnings,
        errors,
    } = input;
    let plan = VerificationPlan {
        command: "actor-links".to_string(),
        mode: "apply".to_string(),
        scopes: vec![
            ScopeConfig {
                scope: MigrationScope::Source,
                root_dir: source_dir.clone(),
                allow_missing: false,
            },
            ScopeConfig {
                scope: MigrationScope::ActorsRoot,
                root_dir: actors_root.clone(),
                allow_missing: true,
            },
        ],
        actions,
        requires_manual_confirm: false,
    };
    let (expected, expected_stats) = build_expected_for_actor_links(
        &before_source,
        &before_actors,
        &plan.actions,
        &source_dir,
        &actors_root,
    );
    let after = vec![
        scan_scope(&source_dir, MigrationScope::Source, false)?,
        scan_scope(&actors_root, MigrationScope::ActorsRoot, true)?,
    ];
    let before = vec![before_source, before_actors];

    let comparison = compare_manifests(
        &expected,
        &after,
        &before,
        false,
        failed_actions,
        &errors,
        &expected_stats.plan_conflicts,
    );
    let exit_code = exit_code_for(comparison.verification_status, comparison.approval_status);
    let report_path = default_report_path(&plan.command);
    let report = VerificationReport {
        version: 1,
        command: plan.command.clone(),
        mode: plan.mode.clone(),
        verification_status: comparison.verification_status,
        approval_status: comparison.approval_status,
        exit_code,
        report_path: report_path.clone(),
        before,
        expected,
        after,
        scope_counts: comparison.scope_counts.clone(),
        scope_extension_counts: comparison.scope_extension_counts,
        diffs: comparison.diffs,
        failed_actions,
        errors,
        warnings,
        expected_stats,
    };
    write_report(&report)?;

    Ok((
        VerificationSummary {
            verification_status: report.verification_status,
            approval_status: report.approval_status,
            exit_code,
            report_path: Some(report_path),
            scopes: comparison.scope_counts,
        },
        report,
    ))
}

pub fn summary_from_error() -> VerificationSummary {
    VerificationSummary {
        verification_status: VerificationStatus::Error,
        approval_status: ApprovalStatus::Blocked,
        exit_code: 30,
        report_path: None,
        scopes: Vec::new(),
    }
}

pub fn exit_code_for(
    verification_status: VerificationStatus,
    approval_status: ApprovalStatus,
) -> i32 {
    match (verification_status, approval_status) {
        (VerificationStatus::Ok, ApprovalStatus::AutoPass) => 0,
        (VerificationStatus::Ok, ApprovalStatus::ManualConfirmRequired) => 10,
        (VerificationStatus::Mismatch, _) => 20,
        (VerificationStatus::Error, _) => 30,
        (_, ApprovalStatus::Blocked) => 20,
    }
}
