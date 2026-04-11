use std::path::PathBuf;

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

    let executor = OperationExecutor::new(source_dir.clone(), !apply);
    let mut report = CommandReport::new("ops", mode, source_dir, op_names);

    for op_type in selected_ops {
        let plan = executor.plan_operation(op_type).await;

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
