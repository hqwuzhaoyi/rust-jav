use std::collections::BTreeMap;

use super::types::{
    ApprovalStatus, MismatchedFile, ScopeCountSummary, ScopeDiff, ScopeExtensionCounts,
    ScopeManifest, VerificationStatus,
};

pub struct ComparisonResult {
    pub verification_status: VerificationStatus,
    pub approval_status: ApprovalStatus,
    pub scope_counts: Vec<ScopeCountSummary>,
    pub scope_extension_counts: Vec<ScopeExtensionCounts>,
    pub diffs: Vec<ScopeDiff>,
}

pub fn compare_manifests(
    expected: &[ScopeManifest],
    after: &[ScopeManifest],
    before: &[ScopeManifest],
    requires_manual_confirm: bool,
    failed_actions: usize,
    extra_errors: &[String],
    plan_conflicts: &[String],
) -> ComparisonResult {
    let mut scope_counts = Vec::new();
    let mut scope_extension_counts = Vec::new();
    let mut diffs = Vec::new();
    let mut has_mismatch = failed_actions > 0 || !extra_errors.is_empty() || !plan_conflicts.is_empty();

    let after_by_scope = after
        .iter()
        .map(|scope| (scope.scope, scope))
        .collect::<BTreeMap<_, _>>();

    for expected_scope in expected {
        let after_scope = after
            .iter()
            .find(|scope| scope.scope == expected_scope.scope)
            .expect("after scope missing");
        let before_scope = before
            .iter()
            .find(|scope| scope.scope == expected_scope.scope)
            .expect("before scope missing");

        scope_counts.push(ScopeCountSummary {
            scope: expected_scope.scope,
            before_count: before_scope.entries.len(),
            expected_count: expected_scope.entries.len(),
            after_count: after_scope.entries.len(),
        });
        scope_extension_counts.push(ScopeExtensionCounts {
            scope: expected_scope.scope,
            before: count_extensions(before_scope),
            expected: count_extensions(expected_scope),
            after: count_extensions(after_scope),
        });

        let diff = diff_scope(expected_scope, after_scope, &after_by_scope);
        if !diff.missing_files.is_empty()
            || !diff.unexpected_files.is_empty()
            || !diff.mismatched_files.is_empty()
        {
            has_mismatch = true;
        }
        diffs.push(diff);
    }

    let verification_status = if has_mismatch {
        VerificationStatus::Mismatch
    } else {
        VerificationStatus::Ok
    };
    let approval_status = match verification_status {
        VerificationStatus::Ok if requires_manual_confirm => ApprovalStatus::ManualConfirmRequired,
        VerificationStatus::Ok => ApprovalStatus::AutoPass,
        _ => ApprovalStatus::Blocked,
    };

    ComparisonResult {
        verification_status,
        approval_status,
        scope_counts,
        scope_extension_counts,
        diffs,
    }
}

fn diff_scope(
    expected: &ScopeManifest,
    after: &ScopeManifest,
    after_by_scope: &BTreeMap<super::types::MigrationScope, &ScopeManifest>,
) -> ScopeDiff {
    let expected_map = expected
        .entries
        .iter()
        .map(|entry| (entry.relative_path.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let after_map = after
        .entries
        .iter()
        .map(|entry| (entry.relative_path.clone(), entry))
        .collect::<BTreeMap<_, _>>();

    let mut missing_files = Vec::new();
    let mut mismatched_files = Vec::new();
    for (path, expected_entry) in &expected_map {
        match after_map.get(path) {
            None => missing_files.push(path.clone()),
            Some(actual) => {
                let mut mismatch_fields = Vec::new();
                if expected_entry.extension != actual.extension {
                    mismatch_fields.push("extension".to_string());
                }
                if expected_entry.size != actual.size {
                    mismatch_fields.push("size".to_string());
                }
                if expected_entry.link_type == "hardlink" {
                    let source_identity = expected_entry
                        .origin_before_relative_path
                        .as_ref()
                        .and_then(|source_rel| {
                            after_by_scope
                                .get(&super::types::MigrationScope::Source)
                                .and_then(|scope| {
                                    scope.entries.iter().find(|entry| {
                                        entry.relative_path.as_str() == source_rel.as_str()
                                    })
                                })
                        })
                        .and_then(|entry| entry.file_identity.as_ref());
                    if source_identity.is_none() || actual.file_identity.as_ref().is_none() {
                        mismatch_fields.push("hardlink_identity_unavailable".to_string());
                    } else if actual.file_identity.as_ref() != source_identity {
                        mismatch_fields.push("hardlink_identity".to_string());
                    }
                }
                if !mismatch_fields.is_empty() {
                    mismatched_files.push(MismatchedFile {
                        relative_path: path.clone(),
                        mismatch_fields,
                    });
                }
            }
        }
    }

    let mut unexpected_files = Vec::new();
    for path in after_map.keys() {
        if !expected_map.contains_key(path) {
            unexpected_files.push(path.clone());
        }
    }

    ScopeDiff {
        scope: expected.scope,
        missing_files,
        unexpected_files,
        mismatched_files,
    }
}

fn count_extensions(scope: &ScopeManifest) -> Vec<(String, usize)> {
    let mut counts = BTreeMap::<String, usize>::new();
    for entry in &scope.entries {
        *counts.entry(entry.extension.clone()).or_default() += 1;
    }
    counts.into_iter().collect()
}
