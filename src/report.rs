use std::path::PathBuf;

use crate::migration_verifier::types::{ScopeCountSummary, VerificationSummary};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Preview,
    Apply,
}

impl OutputMode {
    pub fn as_str(self) -> &'static str {
        match self {
            OutputMode::Preview => "preview",
            OutputMode::Apply => "apply",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionStatus {
    Planned,
    Applied,
    Skipped,
    Failed,
}

impl ActionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ActionStatus::Planned => "planned",
            ActionStatus::Applied => "applied",
            ActionStatus::Skipped => "skipped",
            ActionStatus::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionItem {
    pub kind: String,
    pub source: Option<PathBuf>,
    pub target: Option<PathBuf>,
    pub status: ActionStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub planned_actions: usize,
    pub applied_actions: usize,
    pub skipped_actions: usize,
    pub failed_actions: usize,
    pub warning_count: usize,
    pub error_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandReport {
    pub command: String,
    pub mode: OutputMode,
    pub source_dir: PathBuf,
    pub selected_ops: Vec<String>,
    pub summary: Summary,
    pub verification: Option<VerificationSummary>,
    pub actions: Vec<ActionItem>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl CommandReport {
    pub fn new(
        command: impl Into<String>,
        mode: OutputMode,
        source_dir: PathBuf,
        selected_ops: Vec<String>,
    ) -> Self {
        Self {
            command: command.into(),
            mode,
            source_dir,
            selected_ops,
            summary: Summary::default(),
            verification: None,
            actions: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn finalize(&mut self) {
        self.summary.planned_actions = self
            .actions
            .iter()
            .filter(|action| action.status == ActionStatus::Planned)
            .count();
        self.summary.applied_actions = self
            .actions
            .iter()
            .filter(|action| action.status == ActionStatus::Applied)
            .count();
        self.summary.skipped_actions = self
            .actions
            .iter()
            .filter(|action| action.status == ActionStatus::Skipped)
            .count();
        self.summary.failed_actions = self
            .actions
            .iter()
            .filter(|action| action.status == ActionStatus::Failed)
            .count();
        self.summary.warning_count = self.warnings.len();
        self.summary.error_count = self.errors.len();
    }

    pub fn to_text(&self) -> String {
        let mut lines = vec![
            format!("command: {}", self.command),
            format!("mode: {}", self.mode.as_str()),
            format!("source: {}", self.source_dir.display()),
        ];

        if !self.selected_ops.is_empty() {
            lines.push(format!("selected: {}", self.selected_ops.join(", ")));
        }

        lines.push(format!(
            "summary: planned={}, applied={}, skipped={}, failed={}, warnings={}, errors={}",
            self.summary.planned_actions,
            self.summary.applied_actions,
            self.summary.skipped_actions,
            self.summary.failed_actions,
            self.summary.warning_count,
            self.summary.error_count
        ));

        if !self.actions.is_empty() {
            lines.push("actions:".to_string());
            for action in &self.actions {
                let source = action
                    .source
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "-".to_string());
                let target = action
                    .target
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "-".to_string());
                let reason = action.reason.clone().unwrap_or_default();
                lines.push(format!(
                    "  - [{}] {} | source={} | target={}{}",
                    action.status.as_str(),
                    action.kind,
                    source,
                    target,
                    if reason.is_empty() {
                        String::new()
                    } else {
                        format!(" | reason={reason}")
                    }
                ));
            }
        }

        if !self.warnings.is_empty() {
            lines.push("warnings:".to_string());
            for warning in &self.warnings {
                lines.push(format!("  - {warning}"));
            }
        }

        if !self.errors.is_empty() {
            lines.push("errors:".to_string());
            for error in &self.errors {
                lines.push(format!("  - {error}"));
            }
        }

        if let Some(verification) = &self.verification {
            lines.push(format!(
                "verification: {}",
                verification.verification_status.as_str()
            ));
            lines.push(format!(
                "approval: {}",
                verification.approval_status.as_str()
            ));
            lines.push(format!("verification_exit_code: {}", verification.exit_code));
            for scope in &verification.scopes {
                lines.push(format!(
                    "  scope {}: before={}, expected={}, after={}",
                    scope.scope.as_str(),
                    scope.before_count,
                    scope.expected_count,
                    scope.after_count
                ));
            }
            if let Some(path) = verification.report_path.as_ref() {
                lines.push(format!("report_path: {}", path.display()));
            }
        }

        lines.join("\n")
    }

    pub fn to_json(&self) -> String {
        let selected_ops = json_array(self.selected_ops.iter().map(json_string));
        let warnings = json_array(self.warnings.iter().map(json_string));
        let errors = json_array(self.errors.iter().map(json_string));
        let verification = self
            .verification
            .as_ref()
            .map(verification_json)
            .unwrap_or_else(|| "null".to_string());
        let actions = json_array(self.actions.iter().map(|action| {
            format!(
                "{{\"kind\":{},\"source\":{},\"target\":{},\"status\":{},\"reason\":{}}}",
                json_string(&action.kind),
                json_path(action.source.as_ref()),
                json_path(action.target.as_ref()),
                json_string(action.status.as_str()),
                json_optional_string(action.reason.as_deref())
            )
        }));

        format!(
            concat!(
                "{{",
                "\"command\":{},",
                "\"mode\":{},",
                "\"source_dir\":{},",
                "\"selected_ops\":{},",
                "\"summary\":{{",
                "\"planned_actions\":{},",
                "\"applied_actions\":{},",
                "\"skipped_actions\":{},",
                "\"failed_actions\":{},",
                "\"warning_count\":{},",
                "\"error_count\":{}",
                "}},",
                "\"verification\":{},",
                "\"actions\":{},",
                "\"warnings\":{},",
                "\"errors\":{}",
                "}}"
            ),
            json_string(&self.command),
            json_string(self.mode.as_str()),
            json_string(self.source_dir.display().to_string()),
            selected_ops,
            self.summary.planned_actions,
            self.summary.applied_actions,
            self.summary.skipped_actions,
            self.summary.failed_actions,
            self.summary.warning_count,
            self.summary.error_count,
            verification,
            actions,
            warnings,
            errors
        )
    }
}

fn json_array(items: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", items.into_iter().collect::<Vec<_>>().join(","))
}

fn json_optional_string(value: Option<&str>) -> String {
    value.map(json_string).unwrap_or_else(|| "null".to_string())
}

fn json_path(path: Option<&PathBuf>) -> String {
    path.map(|value| json_string(value.display().to_string()))
        .unwrap_or_else(|| "null".to_string())
}

fn json_string(value: impl AsRef<str>) -> String {
    let escaped = value
        .as_ref()
        .chars()
        .flat_map(|ch| match ch {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect(),
            '\n' => "\\n".chars().collect(),
            '\r' => "\\r".chars().collect(),
            '\t' => "\\t".chars().collect(),
            other => vec![other],
        })
        .collect::<String>();

    format!("\"{}\"", escaped)
}

fn verification_json(verification: &VerificationSummary) -> String {
    format!(
        concat!(
            "{{",
            "\"verification_status\":{},",
            "\"approval_status\":{},",
            "\"exit_code\":{},",
            "\"report_path\":{},",
            "\"scopes\":{}",
            "}}"
        ),
        json_string(verification.verification_status.as_str()),
        json_string(verification.approval_status.as_str()),
        verification.exit_code,
        verification
            .report_path
            .as_ref()
            .map(|path| json_string(path.display().to_string()))
            .unwrap_or_else(|| "null".to_string()),
        json_array(verification.scopes.iter().map(scope_count_json))
    )
}

fn scope_count_json(summary: &ScopeCountSummary) -> String {
    format!(
        "{{\"scope\":{},\"before_count\":{},\"expected_count\":{},\"after_count\":{}}}",
        json_string(summary.scope.as_str()),
        summary.before_count,
        summary.expected_count,
        summary.after_count
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_report(mode: OutputMode) -> CommandReport {
        CommandReport::new(
            "ops",
            mode,
            PathBuf::from("/source"),
            vec!["standardize-names".to_string()],
        )
    }

    // ── ActionStatus ──────────────────────────────────────────────────────────

    #[test]
    fn action_status_as_str_round_trips() {
        assert_eq!(ActionStatus::Planned.as_str(), "planned");
        assert_eq!(ActionStatus::Applied.as_str(), "applied");
        assert_eq!(ActionStatus::Skipped.as_str(), "skipped");
        assert_eq!(ActionStatus::Failed.as_str(), "failed");
    }

    #[test]
    fn output_mode_as_str_round_trips() {
        assert_eq!(OutputMode::Preview.as_str(), "preview");
        assert_eq!(OutputMode::Apply.as_str(), "apply");
    }

    // ── CommandReport::finalize ───────────────────────────────────────────────

    #[test]
    fn finalize_counts_planned_actions() {
        let mut report = make_report(OutputMode::Preview);
        report.actions.push(ActionItem {
            kind: "rename".to_string(),
            source: Some(PathBuf::from("/a.mp4")),
            target: Some(PathBuf::from("/b.mp4")),
            status: ActionStatus::Planned,
            reason: None,
        });
        report.actions.push(ActionItem {
            kind: "rename".to_string(),
            source: Some(PathBuf::from("/c.mp4")),
            target: Some(PathBuf::from("/d.mp4")),
            status: ActionStatus::Planned,
            reason: None,
        });
        report.finalize();

        assert_eq!(report.summary.planned_actions, 2);
        assert_eq!(report.summary.applied_actions, 0);
        assert_eq!(report.summary.skipped_actions, 0);
        assert_eq!(report.summary.failed_actions, 0);
    }

    #[test]
    fn finalize_counts_mixed_statuses() {
        let mut report = make_report(OutputMode::Apply);
        let statuses = [
            ActionStatus::Applied,
            ActionStatus::Applied,
            ActionStatus::Skipped,
            ActionStatus::Failed,
        ];
        for status in statuses {
            report.actions.push(ActionItem {
                kind: "rename".to_string(),
                source: Some(PathBuf::from("/x.mp4")),
                target: None,
                status,
                reason: None,
            });
        }
        report.warnings.push("a warning".to_string());
        report.errors.push("an error".to_string());
        report.finalize();

        assert_eq!(report.summary.applied_actions, 2);
        assert_eq!(report.summary.skipped_actions, 1);
        assert_eq!(report.summary.failed_actions, 1);
        assert_eq!(report.summary.warning_count, 1);
        assert_eq!(report.summary.error_count, 1);
    }

    // ── CommandReport::to_text ────────────────────────────────────────────────

    #[test]
    fn to_text_contains_command_and_mode() {
        let mut report = make_report(OutputMode::Preview);
        report.finalize();
        let text = report.to_text();

        assert!(text.contains("command: ops"), "should include command name");
        assert!(text.contains("mode: preview"), "should include mode");
        assert!(
            text.contains("source: /source"),
            "should include source dir"
        );
        assert!(
            text.contains("standardize-names"),
            "should include selected ops"
        );
    }

    #[test]
    fn to_text_includes_actions() {
        let mut report = make_report(OutputMode::Preview);
        report.actions.push(ActionItem {
            kind: "rename".to_string(),
            source: Some(PathBuf::from("/a.mp4")),
            target: Some(PathBuf::from("/b.mp4")),
            status: ActionStatus::Planned,
            reason: None,
        });
        report.finalize();
        let text = report.to_text();

        assert!(text.contains("[planned]"), "action status must appear");
        assert!(text.contains("rename"), "action kind must appear");
        assert!(text.contains("source="), "source path must appear");
        assert!(text.contains("target="), "target path must appear");
    }

    #[test]
    fn to_text_includes_warnings_and_errors() {
        let mut report = make_report(OutputMode::Apply);
        report.warnings.push("disk full".to_string());
        report.errors.push("permission denied".to_string());
        report.finalize();
        let text = report.to_text();

        assert!(text.contains("warnings:"), "warnings header must appear");
        assert!(text.contains("disk full"));
        assert!(text.contains("errors:"), "errors header must appear");
        assert!(text.contains("permission denied"));
    }

    #[test]
    fn to_text_includes_reason_when_present() {
        let mut report = make_report(OutputMode::Apply);
        report.actions.push(ActionItem {
            kind: "rename".to_string(),
            source: Some(PathBuf::from("/a.mp4")),
            target: Some(PathBuf::from("/b.mp4")),
            status: ActionStatus::Failed,
            reason: Some("target already exists".to_string()),
        });
        report.finalize();
        let text = report.to_text();

        assert!(
            text.contains("reason=target already exists"),
            "failure reason must appear"
        );
    }

    // ── CommandReport::to_json ────────────────────────────────────────────────

    #[test]
    fn to_json_is_valid_structure() {
        let mut report = make_report(OutputMode::Preview);
        report.finalize();
        let json = report.to_json();

        assert!(json.starts_with('{') && json.ends_with('}'));
        assert!(json.contains("\"command\":\"ops\""));
        assert!(json.contains("\"mode\":\"preview\""));
        assert!(json.contains("\"planned_actions\":0"));
        assert!(json.contains("\"actions\":[]"));
        assert!(json.contains("\"warnings\":[]"));
        assert!(json.contains("\"errors\":[]"));
    }

    #[test]
    fn to_json_contains_action_fields() {
        let mut report = make_report(OutputMode::Apply);
        report.actions.push(ActionItem {
            kind: "rename".to_string(),
            source: Some(PathBuf::from("/a.mp4")),
            target: Some(PathBuf::from("/b.mp4")),
            status: ActionStatus::Applied,
            reason: None,
        });
        report.finalize();
        let json = report.to_json();

        assert!(json.contains("\"kind\":\"rename\""));
        assert!(json.contains("\"status\":\"applied\""));
        assert!(json.contains("\"reason\":null"));
    }

    #[test]
    fn to_json_escapes_special_characters_in_strings() {
        let mut report = CommandReport::new(
            "ops",
            OutputMode::Preview,
            PathBuf::from("/path/with\"quote"),
            vec![],
        );
        report.warnings.push("msg with\nnewline".to_string());
        report.finalize();
        let json = report.to_json();

        assert!(
            json.contains("\\\"quote"),
            "double quotes must be escaped in JSON"
        );
        assert!(
            json.contains("\\n"),
            "newlines must be escaped in JSON strings"
        );
    }

    #[test]
    fn to_json_null_source_and_target_when_absent() {
        let mut report = make_report(OutputMode::Apply);
        report.actions.push(ActionItem {
            kind: "clean-dir".to_string(),
            source: None,
            target: None,
            status: ActionStatus::Applied,
            reason: None,
        });
        report.finalize();
        let json = report.to_json();

        assert!(json.contains("\"source\":null"));
        assert!(json.contains("\"target\":null"));
    }
}
