use std::{
    collections::HashMap,
    ffi::CString,
    fs::{File, OpenOptions},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::PathBuf,
};

use crate::active_rules::ActiveRuleSet;
use crate::migration_verifier::types::{MigrationAction, MigrationActionKind, MigrationScope};
use crate::report::{ActionItem, ActionStatus, CommandReport, OutputMode};
use crate::tui::executor::{OperationExecutor, OperationPlan, PlannedAction};
use crate::tui::state::{Operation, OperationType};

pub(crate) async fn run_operations_command(
    source_dir: PathBuf,
    selected_ops: Vec<OperationType>,
    apply: bool,
    active_rules: ActiveRuleSet,
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

    let executor = OperationExecutor::with_rules(source_dir.clone(), !apply, active_rules);
    let verification = crate::application::ApplicationServices::new().verification();
    let mut report = CommandReport::new("ops", mode, source_dir, op_names);
    let mut migration_actions = Vec::new();
    let mut action_counter = 0usize;
    let mut path_evolution = PathEvolution::default();
    let before_source = if apply {
        match verification.scan(&report.source_dir, MigrationScope::Source, false) {
            Ok(scope) => Some(scope),
            Err(error) => {
                report
                    .errors
                    .push(format!("verification pre-scan failed: {error}"));
                report.verification = Some(verification.error_summary());
                report.finalize();
                return report;
            }
        }
    } else {
        None
    };

    for op_type in selected_ops {
        let mut plan = executor.plan_operation(op_type).await;
        if !apply {
            plan.actions = plan
                .actions
                .into_iter()
                .filter_map(|action| path_evolution.rewrite(action))
                .collect();
        }
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
        match verification.verify_operations(
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
                report.verification = Some(verification.error_summary());
            }
        }
    }
    report
}

pub(crate) async fn plan_canonical_operation_snapshot(
    source_dir: PathBuf,
    selected_ops: Vec<OperationType>,
    active_rules: ActiveRuleSet,
) -> CommandReport {
    let op_names = selected_ops
        .iter()
        .map(|operation| operation.name().to_string())
        .collect::<Vec<_>>();
    let mut report = CommandReport::new("ops", OutputMode::Preview, source_dir.clone(), op_names);
    let canonical_root = match std::fs::canonicalize(&source_dir) {
        Ok(root) => root,
        Err(error) => {
            report
                .errors
                .push(format!("cannot canonicalize preview root: {error}"));
            report.finalize();
            return report;
        }
    };
    let shadow = match ShadowTree::create(&canonical_root) {
        Ok(shadow) => shadow,
        Err(error) => {
            report
                .errors
                .push(format!("cannot create canonical preview shadow: {error}"));
            report.finalize();
            return report;
        }
    };
    let executor = OperationExecutor::with_rules(shadow.root.clone(), false, active_rules);
    for op_type in selected_ops {
        let plan = executor.plan_operation(op_type).await;
        report.warnings.extend(plan.warnings);
        report
            .actions
            .extend(plan.actions.iter().cloned().map(|action| {
                planned_action_to_item(
                    map_shadow_action(action, &shadow.root, &canonical_root),
                    ActionStatus::Planned,
                )
            }));
        let operation = Operation {
            op_type,
            enabled: true,
            affected_count: 0,
            affected_size: 0,
            affected_files: Vec::new(),
        };
        let result = executor.execute(&operation).await;
        if let Some(error) = result.error {
            report.errors.push(format!(
                "shadow planning failed for {}: {error}",
                op_type.name()
            ));
            break;
        }
        if !result.failed_files.is_empty() {
            report
                .errors
                .extend(result.failed_files.into_iter().map(|(path, error)| {
                    format!("shadow planning failed for {}: {error}", path.display())
                }));
            break;
        }
    }
    report.finalize();
    report
}

struct ShadowTree {
    root: PathBuf,
}

impl ShadowTree {
    fn create(source: &std::path::Path) -> std::io::Result<Self> {
        let base = std::env::temp_dir();
        for _ in 0..16 {
            let root = base.join(format!(
                "rust-jav-plan-{}-{:016x}",
                std::process::id(),
                rand::random::<u64>()
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => {
                    mirror_tree(source, &root)?;
                    return Ok(Self { root });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate unique preview shadow",
        ))
    }
}

impl Drop for ShadowTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn mirror_tree(source: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = std::fs::symlink_metadata(&source_path)?;
        if metadata.file_type().is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(&source_path)?, target_path)?;
        } else if metadata.is_dir() {
            std::fs::create_dir(&target_path)?;
            mirror_tree(&source_path, &target_path)?;
        } else if metadata.is_file() {
            let file = std::fs::File::create(target_path)?;
            file.set_len(metadata.len())?;
        }
    }
    Ok(())
}

fn map_shadow_action(
    mut action: PlannedAction,
    shadow_root: &std::path::Path,
    real_root: &std::path::Path,
) -> PlannedAction {
    let map = |path: PathBuf| {
        path.strip_prefix(shadow_root)
            .map(|relative| real_root.join(relative))
            .unwrap_or(path)
    };
    action.source = action.source.map(&map);
    action.target = action.target.map(map);
    action
}

#[derive(Default)]
struct PathEvolution {
    moved: Vec<(PathBuf, PathBuf)>,
    deleted: Vec<(PathBuf, bool)>,
}

impl PathEvolution {
    fn resolve(&self, path: &std::path::Path) -> PathBuf {
        let mut resolved = path.to_path_buf();
        for (source, target) in &self.moved {
            if resolved == *source {
                resolved = target.clone();
            } else if let Ok(suffix) = resolved.strip_prefix(source) {
                resolved = target.join(suffix);
            }
        }
        resolved
    }

    fn rewrite(&mut self, mut action: PlannedAction) -> Option<PlannedAction> {
        let original_source = action.source.clone();
        if let Some(source) = original_source.as_deref() {
            if self.deleted.iter().any(|(deleted, directory)| {
                source == deleted || (*directory && source.starts_with(deleted))
            }) {
                return None;
            }
            action.source = Some(self.resolve(source));
        }
        if let Some(original_target) = action.target.clone() {
            let mut target = self.resolve(&original_target);
            if let (Some(original_source), Some(current_source)) =
                (original_source.as_deref(), action.source.as_deref())
            {
                if original_target.parent() == original_source.parent() {
                    if let (Some(parent), Some(name)) =
                        (current_source.parent(), original_target.file_name())
                    {
                        target = parent.join(name);
                    }
                } else if original_target.file_name() == original_source.file_name() {
                    if let Some(name) = current_source.file_name() {
                        target.set_file_name(name);
                    }
                }
            }
            action.target = Some(target);
        }
        if let Some(original_source) = original_source {
            match action.kind.as_str() {
                "delete-file" => self.deleted.push((original_source, false)),
                "delete-dir" => self.deleted.push((original_source, true)),
                "move" | "rename" | "extract-code" => {
                    if let Some(target) = action.target.clone() {
                        self.moved.push((original_source, target));
                    }
                }
                _ => {}
            }
        }
        Some(action)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceIdentity {
    pub device: u64,
    pub inode: u64,
    pub modified_seconds: i64,
    pub modified_nanoseconds: i64,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct ConfirmedAction {
    pub action: PlannedAction,
    pub source_identity: Option<SourceIdentity>,
}

pub(crate) fn run_confirmed_operation_plan(
    source_dir: PathBuf,
    canonical_media_root: PathBuf,
    selected_ops: Vec<OperationType>,
    actions: Vec<ConfirmedAction>,
    warnings: Vec<String>,
    mut start_outcome: impl FnMut(&ConfirmedAction) -> Result<(i64, String), String>,
    mut finish_outcome: impl FnMut(i64, &ActionItem) -> Result<(), String>,
) -> CommandReport {
    let op_names = selected_ops
        .iter()
        .map(|operation| operation.name().to_string())
        .collect::<Vec<_>>();
    let requires_manual_confirm = selected_ops.iter().any(|operation| {
        matches!(
            operation,
            OperationType::DeleteAdFiles | OperationType::RemoveDuplicates
        )
    });
    let verification = crate::application::ApplicationServices::new().verification();
    let mut report = CommandReport::new("ops", OutputMode::Apply, source_dir.clone(), op_names);
    report.warnings = warnings;
    let before_source = match verification.scan(&report.source_dir, MigrationScope::Source, false) {
        Ok(scope) => scope,
        Err(error) => {
            report
                .errors
                .push(format!("verification pre-scan failed: {error}"));
            report.verification = Some(verification.error_summary());
            report.finalize();
            return report;
        }
    };
    let migration_actions = actions
        .iter()
        .enumerate()
        .filter_map(|(index, action)| planned_action_to_migration_action(&action.action, index + 1))
        .collect::<Vec<_>>();
    let safe_root = match SafeMutationRoot::open(&canonical_media_root) {
        Ok(root) => root,
        Err(error) => {
            report
                .errors
                .push(format!("cannot open canonical mutation root: {error}"));
            report.finalize();
            return report;
        }
    };

    let mut trusted_updates = HashMap::<(u64, u64), SourceIdentity>::new();
    for action in actions {
        let (item_id, quarantine_token) = match start_outcome(&action) {
            Ok(journal) => journal,
            Err(error) => {
                report.errors.push(format!(
                    "cannot persist pending task outcome; no action was executed: {error}"
                ));
                break;
            }
        };
        let outcome = match validate_confirmed_action(
            &source_dir,
            &canonical_media_root,
            &action,
            &trusted_updates,
        ) {
            Ok(()) => {
                let expected = action.source_identity.as_ref().map(|identity| {
                    trusted_updates
                        .get(&(identity.device, identity.inode))
                        .cloned()
                        .unwrap_or_else(|| identity.clone())
                });
                safe_root.execute(action.action, expected.as_ref(), &quarantine_token)
            }
            Err(error) => ActionItem {
                kind: action.action.kind,
                source: action.action.source,
                target: action.action.target,
                status: ActionStatus::Failed,
                reason: Some(error),
            },
        };
        if outcome.status == ActionStatus::Applied {
            refresh_parent_identities(&outcome, &mut trusted_updates);
        }
        if let Err(error) = finish_outcome(item_id, &outcome) {
            report.actions.push(outcome);
            report.errors.push(format!(
                "cannot persist task outcome; remaining actions were stopped: {error}"
            ));
            break;
        }
        report.actions.push(outcome);
    }
    report.finalize();
    match verification.verify_operations(
        report.source_dir.clone(),
        before_source,
        migration_actions,
        requires_manual_confirm,
        report.summary.failed_actions,
        report.warnings.clone(),
        report.errors.clone(),
    ) {
        Ok((summary, _detailed)) => report.verification = Some(summary),
        Err(error) => {
            report.errors.push(format!("verification failed: {error}"));
            report.verification = Some(verification.error_summary());
        }
    }
    report
}

fn validate_confirmed_action(
    source_dir: &std::path::Path,
    canonical_media_root: &std::path::Path,
    confirmed: &ConfirmedAction,
    trusted_updates: &HashMap<(u64, u64), SourceIdentity>,
) -> Result<(), String> {
    let current_root = std::fs::canonicalize(source_dir)
        .map_err(|error| format!("cannot canonicalize media root: {error}"))?;
    if current_root != canonical_media_root {
        return Err("canonical media root changed after preview".to_string());
    }
    for path in [
        confirmed.action.source.as_deref(),
        confirmed.action.target.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if !path.starts_with(canonical_media_root) {
            return Err(format!(
                "stored path is outside canonical media root: {}",
                path.display()
            ));
        }
        let relative = path
            .strip_prefix(canonical_media_root)
            .map_err(|_| "stored path is outside canonical media root".to_string())?;
        if relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(format!("stored path is not normalized: {}", path.display()));
        }
        validate_parent_chain(canonical_media_root, path)?;
    }
    if let Some(source) = confirmed.action.source.as_deref() {
        let expected = confirmed
            .source_identity
            .as_ref()
            .ok_or_else(|| "stored source identity is missing".to_string())?;
        let metadata = std::fs::symlink_metadata(source)
            .map_err(|error| format!("stored source identity cannot be verified: {error}"))?;
        let actual = source_identity(&metadata);
        let expected = trusted_updates
            .get(&(expected.device, expected.inode))
            .unwrap_or(expected);
        if &actual != expected {
            return Err("stored source identity changed after preview".to_string());
        }
    }
    Ok(())
}

fn source_identity(metadata: &std::fs::Metadata) -> SourceIdentity {
    SourceIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        size: metadata.size(),
    }
}

fn refresh_parent_identities(
    outcome: &ActionItem,
    trusted: &mut HashMap<(u64, u64), SourceIdentity>,
) {
    for parent in [outcome.source.as_deref(), outcome.target.as_deref()]
        .into_iter()
        .flatten()
        .filter_map(std::path::Path::parent)
    {
        if let Ok(metadata) = std::fs::symlink_metadata(parent) {
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                let identity = source_identity(&metadata);
                trusted.insert((identity.device, identity.inode), identity);
            }
        }
    }
}

fn validate_parent_chain(root: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "stored path has no parent".to_string())?;
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| "stored path parent is outside canonical media root".to_string())?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "stored path parent is a symlink: {}",
                    current.display()
                ))
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(format!(
                    "stored path parent is not a directory: {}",
                    current.display()
                ))
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(format!("cannot inspect stored path parent: {error}")),
        }
    }
    Ok(())
}

struct SafeMutationRoot {
    root: PathBuf,
    directory: File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MutationPhase {
    SourceCaptured,
    BeforeCommit,
}

impl SafeMutationRoot {
    fn open(root: &std::path::Path) -> Result<Self, String> {
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            root: root.to_path_buf(),
            directory,
        })
    }

    fn execute(
        &self,
        action: PlannedAction,
        expected: Option<&SourceIdentity>,
        quarantine_token: &str,
    ) -> ActionItem {
        self.execute_with_hook(action, expected, quarantine_token, |_| {})
    }

    fn execute_with_hook(
        &self,
        action: PlannedAction,
        expected: Option<&SourceIdentity>,
        quarantine_token: &str,
        mut hook: impl FnMut(MutationPhase),
    ) -> ActionItem {
        let result = self.execute_inner(&action, expected, quarantine_token, &mut hook);
        ActionItem {
            kind: action.kind,
            source: action.source,
            target: action.target,
            status: if result.is_ok() {
                ActionStatus::Applied
            } else {
                ActionStatus::Failed
            },
            reason: result.err().or(action.reason),
        }
    }

    fn execute_inner(
        &self,
        action: &PlannedAction,
        expected: Option<&SourceIdentity>,
        quarantine_token: &str,
        hook: &mut impl FnMut(MutationPhase),
    ) -> Result<(), String> {
        let source = action
            .source
            .as_deref()
            .ok_or_else(|| "confirmed action has no source".to_string())?;
        let (source_parent, source_name) = self.parent_and_name(source, false)?;
        let quarantine_name =
            self.capture_source(&source_parent, &source_name, quarantine_token)?;
        hook(MutationPhase::SourceCaptured);
        let actual = fstatat_identity(&source_parent, &quarantine_name).map_err(|error| {
            self.rollback_error(
                &source_parent,
                &quarantine_name,
                &source_name,
                error.to_string(),
            )
        })?;
        if expected.is_none_or(|expected| expected != &actual) {
            return Err(self.rollback_error(
                &source_parent,
                &quarantine_name,
                &source_name,
                "stored source identity changed after preview".to_string(),
            ));
        }
        match action.kind.as_str() {
            "delete-file" | "delete-dir" => {
                let flags = if action.kind == "delete-dir" {
                    libc::AT_REMOVEDIR
                } else {
                    0
                };
                let result = unsafe {
                    libc::unlinkat(source_parent.as_raw_fd(), quarantine_name.as_ptr(), flags)
                };
                if result == 0 {
                    Ok(())
                } else {
                    Err(self.rollback_error(
                        &source_parent,
                        &quarantine_name,
                        &source_name,
                        std::io::Error::last_os_error().to_string(),
                    ))
                }
            }
            "move" | "rename" | "extract-code" => {
                let target = action
                    .target
                    .as_deref()
                    .ok_or_else(|| "confirmed move action has no target".to_string())?;
                let (target_parent, target_name) = match self.parent_and_name(target, true) {
                    Ok(target) => target,
                    Err(error) => {
                        return Err(self.rollback_error(
                            &source_parent,
                            &quarantine_name,
                            &source_name,
                            error,
                        ))
                    }
                };
                hook(MutationPhase::BeforeCommit);
                match rename_noreplace(
                    &source_parent,
                    &quarantine_name,
                    &target_parent,
                    &target_name,
                ) {
                    Ok(()) => Ok(()),
                    Err(error) => Err(self.rollback_error(
                        &source_parent,
                        &quarantine_name,
                        &source_name,
                        format!("target commit refused without replacement: {error}"),
                    )),
                }
            }
            other => Err(self.rollback_error(
                &source_parent,
                &quarantine_name,
                &source_name,
                format!("unsupported confirmed action kind: {other}"),
            )),
        }
    }

    fn capture_source(
        &self,
        parent: &File,
        source: &CString,
        quarantine_token: &str,
    ) -> Result<CString, String> {
        let quarantine = quarantine_name(quarantine_token)?;
        rename_noreplace(parent, source, parent, &quarantine)
            .map_err(|error| format!("cannot atomically capture source: {error}"))?;
        Ok(quarantine)
    }

    fn rollback_error(
        &self,
        parent: &File,
        quarantine: &CString,
        source: &CString,
        error: String,
    ) -> String {
        match rename_noreplace(parent, quarantine, parent, source) {
            Ok(()) => error,
            Err(rollback) => format!(
                "{error}; source rollback refused without replacement: {rollback}; approved entry remains quarantined as {}",
                quarantine.to_string_lossy()
            ),
        }
    }

    fn parent_and_name(
        &self,
        path: &std::path::Path,
        create: bool,
    ) -> Result<(File, CString), String> {
        let relative = path
            .strip_prefix(&self.root)
            .map_err(|_| "stored path is outside canonical media root".to_string())?;
        let mut parts = relative
            .components()
            .map(|component| match component {
                std::path::Component::Normal(part) => CString::new(part.as_bytes())
                    .map_err(|_| "stored path contains NUL".to_string()),
                _ => Err("stored path is not normalized".to_string()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let name = parts
            .pop()
            .ok_or_else(|| "stored path does not name an entry".to_string())?;
        let mut current = self
            .directory
            .try_clone()
            .map_err(|error| error.to_string())?;
        for part in parts {
            let mut fd = unsafe {
                libc::openat(
                    current.as_raw_fd(),
                    part.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0
                && create
                && std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound
            {
                let made = unsafe { libc::mkdirat(current.as_raw_fd(), part.as_ptr(), 0o755) };
                if made < 0
                    && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
                {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                fd = unsafe {
                    libc::openat(
                        current.as_raw_fd(),
                        part.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
            }
            if fd < 0 {
                return Err(format!(
                    "stored path parent cannot be opened without following symlinks: {}",
                    std::io::Error::last_os_error()
                ));
            }
            current = unsafe { File::from_raw_fd(fd) };
        }
        Ok((current, name))
    }
}

fn rename_noreplace(
    source_parent: &File,
    source: &CString,
    target_parent: &File,
    target: &CString,
) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            source_parent.as_raw_fd(),
            source.as_ptr(),
            target_parent.as_raw_fd(),
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        ) as libc::c_int
    };
    #[cfg(target_os = "macos")]
    let result = unsafe {
        libc::renameatx_np(
            source_parent.as_raw_fd(),
            source.as_ptr(),
            target_parent.as_raw_fd(),
            target.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let result = {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "atomic no-replace rename is unavailable on this platform",
        ));
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn quarantine_name(token: &str) -> Result<CString, String> {
    let suffix = token
        .strip_prefix(".rust-jav-quarantine-item-")
        .ok_or_else(|| "invalid durable quarantine token prefix".to_string())?;
    if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("invalid durable quarantine token id".to_string());
    }
    CString::new(token.as_bytes()).map_err(|_| "durable quarantine token contains NUL".to_string())
}

pub(crate) fn recover_durable_quarantine(
    media_root: &std::path::Path,
    source_path: &std::path::Path,
    quarantine_token: &str,
) -> String {
    let canonical_root = match std::fs::canonicalize(media_root) {
        Ok(root) => root,
        Err(error) => return format!(
            "interrupted mutation: cannot canonicalize media root for quarantine recovery: {error}; inspect token {quarantine_token} manually"
        ),
    };
    let canonical_source = source_path
        .strip_prefix(media_root)
        .map(|relative| canonical_root.join(relative))
        .unwrap_or_else(|_| source_path.to_path_buf());
    let root = match SafeMutationRoot::open(&canonical_root) {
        Ok(root) => root,
        Err(error) => return format!(
            "interrupted mutation: cannot open media root for quarantine recovery: {error}; inspect token {quarantine_token} manually"
        ),
    };
    let (parent, source_name) = match root.parent_and_name(&canonical_source, false) {
        Ok(entry) => entry,
        Err(error) => return format!(
            "interrupted mutation: unsafe source path prevented quarantine recovery: {error}; inspect token {quarantine_token} manually"
        ),
    };
    let quarantine_name = match quarantine_name(quarantine_token) {
        Ok(name) => name,
        Err(error) => return format!("interrupted mutation: {error}"),
    };
    let quarantine_path = canonical_source
        .parent()
        .unwrap_or(&canonical_root)
        .join(quarantine_token);
    match fstatat_identity(&parent, &quarantine_name) {
        Ok(_) => {}
        Err(error) if error.raw_os_error() == Some(libc::ENOENT) => return format!(
            "interrupted mutation: durable quarantine is absent at {}; inspect source {} and the planned target to determine whether the operation completed",
            quarantine_path.display(), canonical_source.display()
        ),
        Err(error) => return format!(
            "interrupted mutation: cannot safely inspect quarantine {}: {error}",
            quarantine_path.display()
        ),
    }
    match fstatat_identity(&parent, &source_name) {
        Ok(_) => return format!(
            "interrupted mutation: source is occupied; quarantine retained at {}. Compare it with {} and restore manually",
            quarantine_path.display(), canonical_source.display()
        ),
        Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {}
        Err(error) => return format!(
            "interrupted mutation: cannot safely inspect source before quarantine restore: {error}; quarantine retained at {}",
            quarantine_path.display()
        ),
    }
    match rename_noreplace(&parent, &quarantine_name, &parent, &source_name) {
        Ok(()) => format!(
            "interrupted mutation: restored quarantined source to {} after service restart",
            canonical_source.display()
        ),
        Err(error) => format!(
            "interrupted mutation: automatic restore refused without replacement: {error}; quarantine retained at {}",
            quarantine_path.display()
        ),
    }
}

fn fstatat_identity(parent: &File, name: &CString) -> std::io::Result<SourceIdentity> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    let result = unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let stat = unsafe { stat.assume_init() };
    let modified_nanoseconds = stat.st_mtime_nsec;
    Ok(SourceIdentity {
        device: stat.st_dev as u64,
        inode: stat.st_ino,
        modified_seconds: stat.st_mtime,
        modified_nanoseconds,
        size: stat.st_size as u64,
    })
}

pub async fn execute_operations_command(
    source_dir: PathBuf,
    selected_ops: Vec<OperationType>,
    apply: bool,
) -> CommandReport {
    let request = if apply {
        crate::application::OperationsRequest::apply(source_dir, selected_ops)
    } else {
        crate::application::OperationsRequest::preview(source_dir, selected_ops)
    };
    crate::application::ApplicationServices::new()
        .operations()
        .run(request)
        .await
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

#[cfg(test)]
mod safe_mutation_tests {
    use super::*;

    fn identity(path: &std::path::Path) -> SourceIdentity {
        source_identity(&std::fs::symlink_metadata(path).unwrap())
    }

    #[test]
    fn quarantine_delete_never_deletes_a_same_name_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("delete.txt");
        std::fs::write(&source, b"approved").unwrap();
        let expected = identity(&source);
        let root = SafeMutationRoot::open(directory.path()).unwrap();
        let outcome = root.execute_with_hook(
            PlannedAction {
                kind: "delete-file".into(),
                source: Some(source.clone()),
                target: None,
                reason: None,
            },
            Some(&expected),
            ".rust-jav-quarantine-item-1",
            |phase| {
                if phase == MutationPhase::SourceCaptured {
                    std::fs::write(&source, b"replacement").unwrap();
                }
            },
        );
        assert_eq!(outcome.status, ActionStatus::Applied);
        assert_eq!(std::fs::read(source).unwrap(), b"replacement");
    }

    #[test]
    fn quarantine_move_never_overwrites_a_racing_target_and_rolls_back() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.txt");
        let target = directory.path().join("target.txt");
        std::fs::write(&source, b"approved").unwrap();
        let expected = identity(&source);
        let root = SafeMutationRoot::open(directory.path()).unwrap();
        let outcome = root.execute_with_hook(
            PlannedAction {
                kind: "move".into(),
                source: Some(source.clone()),
                target: Some(target.clone()),
                reason: None,
            },
            Some(&expected),
            ".rust-jav-quarantine-item-2",
            |phase| {
                if phase == MutationPhase::BeforeCommit {
                    std::fs::write(&target, b"competitor").unwrap();
                }
            },
        );
        assert_eq!(outcome.status, ActionStatus::Failed);
        assert_eq!(std::fs::read(target).unwrap(), b"competitor");
        assert_eq!(std::fs::read(source).unwrap(), b"approved");
    }
}
