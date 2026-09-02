use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::types::{
    file_extension, ExpectedStats, ManifestEntry, MigrationAction, MigrationActionKind,
    MigrationScope, ScopeManifest,
};

pub fn build_expected_for_ops(
    before: &ScopeManifest,
    actions: &[MigrationAction],
    source_root: &Path,
) -> (ScopeManifest, ExpectedStats) {
    let mut by_path = before
        .entries
        .iter()
        .cloned()
        .map(|entry| (entry.relative_path.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut conflicts = Vec::new();

    for action in actions {
        match action.kind {
            MigrationActionKind::DeleteFile => {
                let Some(source) = action.source.as_ref() else {
                    conflicts.push(format!("{} missing source path", action.action_id));
                    continue;
                };
                let source_rel = relative_string(source_root, source);
                if by_path.remove(&source_rel).is_none() {
                    conflicts.push(format!(
                        "{} source missing from expected state: {}",
                        action.action_id, source_rel
                    ));
                }
            }
            MigrationActionKind::Move | MigrationActionKind::Rename => {
                let (Some(source), Some(target)) = (action.source.as_ref(), action.target.as_ref())
                else {
                    conflicts.push(format!(
                        "{} missing source or target path",
                        action.action_id
                    ));
                    continue;
                };
                let source_rel = relative_string(source_root, source);
                let target_rel = relative_string(source_root, target);
                let Some(mut entry) = by_path.remove(&source_rel) else {
                    conflicts.push(format!(
                        "{} source missing from expected state: {}",
                        action.action_id, source_rel
                    ));
                    continue;
                };

                if let Some(existing) = by_path.get(&target_rel) {
                    conflicts.push(format!(
                        "{} target conflict at {} (existing entry {})",
                        action.action_id, target_rel, existing.entry_id
                    ));
                    by_path.insert(source_rel, entry);
                    continue;
                }

                entry.relative_path = target_rel.clone();
                entry.file_name = Path::new(&target_rel)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or(target_rel.as_str())
                    .to_string();
                entry.extension = file_extension(&entry.file_name);
                entry.action_ids.push(action.action_id.clone());
                by_path.insert(target_rel, entry);
            }
            MigrationActionKind::HardLink => {
                conflicts.push(format!(
                    "{} unexpected hard-link action in ops expected builder",
                    action.action_id
                ));
            }
        }
    }

    (
        ScopeManifest {
            scope: MigrationScope::Source,
            root_dir: before.root_dir.clone(),
            entries: by_path.into_values().collect(),
        },
        ExpectedStats {
            expected_new_links: 0,
            expected_existing_links: 0,
            plan_conflicts: conflicts,
        },
    )
}

pub fn build_expected_for_actor_links(
    source_before: &ScopeManifest,
    actors_before: &ScopeManifest,
    actions: &[MigrationAction],
    source_root: &Path,
    actors_root: &Path,
) -> (Vec<ScopeManifest>, ExpectedStats) {
    let source_expected = source_before.clone();
    let mut actors_by_path = actors_before
        .entries
        .iter()
        .cloned()
        .map(|entry| (entry.relative_path.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let source_by_path = source_before
        .entries
        .iter()
        .cloned()
        .map(|entry| (entry.relative_path.clone(), entry))
        .collect::<BTreeMap<_, _>>();

    let mut expected_new_links = 0usize;
    let mut expected_existing_links = 0usize;
    let mut conflicts = Vec::new();
    let mut seen_targets = BTreeSet::new();

    for action in actions {
        let (Some(source), Some(target)) = (action.source.as_ref(), action.target.as_ref()) else {
            conflicts.push(format!(
                "{} missing source or target path",
                action.action_id
            ));
            continue;
        };

        let source_rel = relative_string(source_root, source);
        let target_rel = relative_string(actors_root, target);
        if !seen_targets.insert(target_rel.clone()) {
            conflicts.push(format!(
                "{} duplicate actor-link target {}",
                action.action_id, target_rel
            ));
            continue;
        }
        let Some(source_entry) = source_by_path.get(&source_rel) else {
            conflicts.push(format!(
                "{} source missing from source manifest: {}",
                action.action_id, source_rel
            ));
            continue;
        };

        let existed_before = actors_by_path
            .get(&target_rel)
            .map(|entry| {
                entry.link_type == "hardlink"
                    && entry.file_identity.is_some()
                    && entry.file_identity == source_entry.file_identity
            })
            .unwrap_or(false);
        if existed_before {
            expected_existing_links += 1;
        } else {
            expected_new_links += 1;
        }

        let file_name = Path::new(&target_rel)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(target_rel.as_str())
            .to_string();
        let expected_entry = ManifestEntry {
            entry_id: format!("actor-exp-{}", action.action_id),
            scope: MigrationScope::ActorsRoot,
            relative_path: target_rel.clone(),
            file_name: file_name.clone(),
            extension: file_extension(&file_name),
            size: source_entry.size,
            file_identity: None,
            origin_before_entry_id: Some(source_entry.entry_id.clone()),
            origin_before_relative_path: Some(source_entry.relative_path.clone()),
            action_ids: vec![action.action_id.clone()],
            link_type: "hardlink".to_string(),
            link_source_entry_id: Some(source_entry.entry_id.clone()),
        };
        actors_by_path.insert(target_rel, expected_entry);
    }

    (
        vec![
            source_expected,
            ScopeManifest {
                scope: MigrationScope::ActorsRoot,
                root_dir: actors_before.root_dir.clone(),
                entries: actors_by_path.into_values().collect(),
            },
        ],
        ExpectedStats {
            expected_new_links,
            expected_existing_links,
            plan_conflicts: conflicts,
        },
    )
}

fn relative_string(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
