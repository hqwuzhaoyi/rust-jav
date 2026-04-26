use std::path::PathBuf;

use crate::migration_verifier::types::{
    MigrationAction, MigrationActionKind, MigrationScope,
};
use crate::migration_verifier::fs_scan::scan_scope;
use crate::migration_verifier::{summary_from_error, verify_ops};
use crate::report::{ActionItem, ActionStatus, CommandReport, OutputMode};
use crate::tui::executor::{OperationExecutor, OperationPlan, PlannedAction};
use crate::tui::state::{Operation, OperationType};

pub async fn execute_operations_command(
    source_dir: PathBuf,
    selected_ops: Vec<OperationType>,
    apply: bool,
) -> CommandReport {
    let mode = if apply {
        OutputMode::Apply
    } else {
        OutputMode::Preview
    };
    let op_names = selected_ops
        .iter()
        .map(|op| op.name().to_string())
        .collect::<Vec<_>>();
    let requires_manual_confirm = selected_ops.iter().any(|op| {
        matches!(
            op,
            OperationType::DeleteAdFiles | OperationType::RemoveDuplicates
        )
    });

    let executor = OperationExecutor::new(source_dir.clone(), !apply);
    let mut report = CommandReport::new("ops", mode, source_dir, op_names);
    let mut migration_actions = Vec::new();
    let mut action_counter = 0usize;
    let before_source = if apply {
        match scan_scope(&report.source_dir, MigrationScope::Source, false) {
            Ok(scope) => Some(scope),
            Err(error) => {
                report.errors.push(format!("verification pre-scan failed: {error}"));
                report.verification = Some(summary_from_error());
                report.finalize();
                return report;
            }
        }
    } else {
        None
    };

    for op_type in selected_ops {
        let plan = executor.plan_operation(op_type).await;
        for action in &plan.actions {
            action_counter += 1;
            if let Some(migration_action) =
                planned_action_to_migration_action(action, action_counter)
            {
                migration_actions.push(migration_action);
            }
        }

        if !apply {
            report.actions.extend(
                plan.actions
                    .into_iter()
                    .map(|action| planned_action_to_item(action, ActionStatus::Planned)),
            );
            report.warnings.extend(plan.warnings);
            continue;
        }

        let temp_operation = Operation {
            op_type,
            enabled: true,
            affected_count: 0,
            affected_size: 0,
            affected_files: Vec::new(),
        };
        let result = executor.execute(&temp_operation).await;
        let failed_sources = result
            .failed_files
            .iter()
            .map(|(path, error)| (path.clone(), error.clone()))
            .collect::<Vec<_>>();

        if result.error.is_some() && result.affected_count == 0 && failed_sources.is_empty() {
            report.errors.push(format!(
                "{} failed: {}",
                op_type.name(),
                result.error.unwrap_or_else(|| "unknown error".to_string())
            ));
        }

        report.warnings.extend(plan.warnings);
        report
            .actions
            .extend(plan.actions.into_iter().map(|action| {
                let mut item = planned_action_to_item(action, ActionStatus::Applied);
                if let Some(source) = item.source.as_ref() {
                    if let Some((_, error)) = failed_sources.iter().find(|(path, _)| path == source)
                    {
                        item.status = ActionStatus::Failed;
                        item.reason = Some(error.clone());
                    }
                }
                item
            }));
    }

    report.finalize();
    if apply {
        match verify_ops(
            report.source_dir.clone(),
            before_source.expect("apply flow must have pre-scanned source"),
            migration_actions,
            requires_manual_confirm,
            report.summary.failed_actions,
            report.warnings.clone(),
            report.errors.clone(),
        ) {
            Ok((summary, _detailed)) => report.verification = Some(summary),
            Err(error) => {
                report.errors.push(format!("verification failed: {error}"));
                report.verification = Some(summary_from_error());
            }
        }
    }
    report
}

fn planned_action_to_item(action: PlannedAction, status: ActionStatus) -> ActionItem {
    ActionItem {
        kind: action.kind,
        source: action.source,
        target: action.target,
        status,
        reason: action.reason,
    }
}

#[allow(dead_code)]
fn _keep_public_types(_: &OperationPlan) {}

fn planned_action_to_migration_action(
    action: &PlannedAction,
    counter: usize,
) -> Option<MigrationAction> {
    let kind = match action.kind.as_str() {
        "move" => MigrationActionKind::Move,
        "rename" | "extract-code" => MigrationActionKind::Rename,
        "delete-file" => MigrationActionKind::DeleteFile,
        _ => return None,
    };

    Some(MigrationAction {
        action_id: format!("act-{counter:04}"),
        kind,
        scope: MigrationScope::Source,
        source: action.source.clone(),
        target: action.target.clone(),
        reason: action.reason.clone(),
    })
}
