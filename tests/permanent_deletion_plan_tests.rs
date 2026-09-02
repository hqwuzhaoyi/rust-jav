#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use rust_jav::deletion_plan::{
    DeletionOutcomeStatus, FileType, PermanentDeletionPlan, PermanentDeletionPlanner,
    PlanExecutionError,
};
use rust_jav::management_tasks::TaskStore;

struct TestJournal {
    _directory: tempfile::TempDir,
    database: PathBuf,
    store: TaskStore,
    task_id: String,
}

fn durable_journal(plan: &PermanentDeletionPlan) -> TestJournal {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("tasks.sqlite3");
    let store = TaskStore::open(&database).unwrap();
    let task = store
        .create_deletion_mutation(
            "/test",
            1,
            &serde_json::json!({
                "id": "test-plan",
                "created_at": plan.created_at.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                "expires_at": plan.expires_at.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                "selection": "selected",
                "hard_link_search_roots": plan.hard_link_search_roots,
                "paths": plan.approved_paths.iter().map(|path| serde_json::json!({
                    "path": path.path,
                    "filesystem_identity": {
                        "device": path.identity.device,
                        "inode": path.identity.inode
                    }
                })).collect::<Vec<_>>(),
                "rule_set_version": 1,
                "rules": ["test-*"]
            }),
        )
        .unwrap();
    store.mark_running(&task.id, 2).unwrap();
    TestJournal {
        _directory: directory,
        database,
        store,
        task_id: task.id,
    }
}

#[test]
fn preview_reports_hard_links_sizes_types_and_video_warning() {
    let temp = tempfile::tempdir().unwrap();
    let media = temp.path().join("media");
    let actors = temp.path().join("actors");
    fs::create_dir_all(&media).unwrap();
    fs::create_dir_all(&actors).unwrap();
    let source = media.join("MOVIE-001.mp4");
    let actor_link = actors.join("MOVIE-001.mp4");
    fs::write(&source, vec![7_u8; 8193]).unwrap();
    fs::hard_link(&source, &actor_link).unwrap();
    let allocation = fs::metadata(&source).unwrap().blocks() * 512;

    let plan = PermanentDeletionPlanner::new(vec![media, actors])
        .create_plan(
            vec![source.clone(), actor_link.clone()],
            Duration::from_secs(60),
            SystemTime::UNIX_EPOCH + Duration::from_secs(100),
        )
        .unwrap();

    assert_eq!(plan.logical_size, 8193);
    assert_eq!(plan.reclaimable_space, allocation);
    assert_eq!(plan.approved_paths.len(), 2);
    assert_eq!(plan.approved_paths[0].file_type, FileType::RegularFile);
    assert!(plan.related_hard_links.is_empty());
    assert!(plan
        .video_warnings
        .iter()
        .any(|warning| warning.path == source));
}

#[test]
fn preview_discovers_but_does_not_approve_related_links() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let selected = root.join("selected.mkv");
    let related = root.join("related.mkv");
    fs::write(&selected, b"content").unwrap();
    fs::hard_link(&selected, &related).unwrap();

    let plan = PermanentDeletionPlanner::new(vec![root.to_path_buf()])
        .create_plan(vec![selected], Duration::from_secs(60), SystemTime::now())
        .unwrap();

    assert_eq!(plan.reclaimable_space, 0);
    assert_eq!(plan.related_hard_links.len(), 1);
    assert_eq!(plan.related_hard_links[0].path, related);
    assert_eq!(plan.related_hard_links[0].file_type, FileType::RegularFile);
}

#[test]
fn symlink_directories_are_not_followed_and_deletion_unlinks_only_the_symlink() {
    let temp = tempfile::tempdir().unwrap();
    let approved_root = temp.path().join("approved");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&approved_root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let target = outside.join("keep.mp4");
    fs::write(&target, b"keep").unwrap();
    let link = approved_root.join("linked-dir");
    symlink(&outside, &link).unwrap();

    let now = SystemTime::now();
    let planner = PermanentDeletionPlanner::new(vec![approved_root]);
    let plan = planner
        .create_plan(vec![link.clone()], Duration::from_secs(60), now)
        .unwrap();
    assert_eq!(plan.approved_paths[0].file_type, FileType::Symlink);
    assert!(plan.related_hard_links.is_empty());

    let journal = durable_journal(&plan);
    let result = planner
        .execute(
            &plan,
            now + Duration::from_secs(1),
            &journal.store,
            &journal.task_id,
        )
        .unwrap();
    assert_eq!(result.outcomes[0].status, DeletionOutcomeStatus::Deleted);
    let durable = journal.store.get(&journal.task_id).unwrap().unwrap();
    assert_eq!(durable.items[0].status, "deleted");
    assert_eq!(durable.items[0].mutation_phase.as_deref(), Some("finished"));
    assert_eq!(
        durable.items[0].identity_inode,
        Some(plan.approved_paths[0].identity.inode)
    );
    assert!(!link.exists());
    assert!(target.exists());
}

#[test]
fn execution_rejects_expired_and_replaced_files_without_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first.txt");
    fs::write(&first, b"original").unwrap();
    let created = SystemTime::UNIX_EPOCH + Duration::from_secs(500);
    let planner = PermanentDeletionPlanner::new(vec![temp.path().to_path_buf()]);
    let expired = planner
        .create_plan(vec![first.clone()], Duration::from_secs(5), created)
        .unwrap();
    let expired_journal = durable_journal(&expired);
    assert_eq!(
        planner.execute(
            &expired,
            created + Duration::from_secs(6),
            &expired_journal.store,
            &expired_journal.task_id,
        ),
        Err(PlanExecutionError::Expired)
    );
    assert!(first.exists());

    let current = planner
        .create_plan(
            vec![first.clone()],
            Duration::from_secs(60),
            SystemTime::now(),
        )
        .unwrap();
    fs::remove_file(&first).unwrap();
    fs::write(&first, b"replacement with another size").unwrap();
    let current_journal = durable_journal(&current);
    let result = planner
        .execute(
            &current,
            SystemTime::now(),
            &current_journal.store,
            &current_journal.task_id,
        )
        .unwrap();
    assert_eq!(result.outcomes[0].status, DeletionOutcomeStatus::Changed);
    assert!(first.exists());
}

#[test]
fn public_execute_rejects_a_task_whose_authority_does_not_match_the_plan() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("approved.mp4");
    fs::write(&path, b"approved").unwrap();
    let planner = PermanentDeletionPlanner::new(vec![temp.path().to_path_buf()]);
    let now = SystemTime::now();
    let plan = planner
        .create_plan(vec![path.clone()], Duration::from_secs(60), now)
        .unwrap();
    let journal = durable_journal(&plan);
    journal
        .store
        .save_operation_plan(
            &journal.task_id,
            plan.expires_at
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            r#"{"id":"different-plan","expires_at":0,"hard_link_search_roots":[],"paths":[]}"#,
        )
        .unwrap();

    let error = planner
        .execute(&plan, now, &journal.store, &journal.task_id)
        .unwrap_err();
    assert!(error.to_string().contains("does not match"));
    assert!(path.exists());
    assert!(journal
        .store
        .get(&journal.task_id)
        .unwrap()
        .unwrap()
        .items
        .is_empty());
}

#[test]
fn atomic_unlink_preserves_a_replacement_injected_after_batch_preflight() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("racing-replacement.mp4");
    let moved_approved_inode = temp.path().join("approved-inode-moved-by-racer.mp4");
    fs::write(&path, b"approved inode").unwrap();
    let planner = PermanentDeletionPlanner::new(vec![temp.path().to_path_buf()]);
    let now = SystemTime::now();
    let plan = planner
        .create_plan(vec![path.clone()], Duration::from_secs(60), now)
        .unwrap();

    let journal = durable_journal(&plan);
    let result = planner
        .execute_with_pre_unlink_hook(&plan, now, &journal.store, &journal.task_id, |planned| {
            if planned.path == path {
                fs::rename(&path, &moved_approved_inode).unwrap();
                fs::write(&path, b"replacement must survive").unwrap();
            }
        })
        .unwrap();

    assert_eq!(result.outcomes[0].status, DeletionOutcomeStatus::Changed);
    assert_eq!(fs::read(&path).unwrap(), b"replacement must survive");
    assert_eq!(fs::read(&moved_approved_inode).unwrap(), b"approved inode");
}

#[test]
fn ancestor_symlink_swap_cannot_redirect_unlink_outside_the_approved_root() {
    let temp = tempfile::tempdir().unwrap();
    let approved = temp.path().join("approved");
    let ancestor = approved.join("library");
    let original_ancestor = approved.join("library-original");
    let outside = temp.path().join("outside");
    let path = ancestor.join("movie/approved.mp4");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::create_dir_all(outside.join("movie")).unwrap();
    fs::write(&path, b"approved inode").unwrap();
    let planner = PermanentDeletionPlanner::new(vec![approved.clone()]);
    let now = SystemTime::now();
    let plan = planner
        .create_plan(vec![path.clone()], Duration::from_secs(60), now)
        .unwrap();
    let journal = durable_journal(&plan);
    let outside_path = outside.join("movie/approved.mp4");

    let result = planner
        .execute_with_pre_unlink_hook(&plan, now, &journal.store, &journal.task_id, |_| {
            fs::rename(&ancestor, &original_ancestor).unwrap();
            fs::hard_link(original_ancestor.join("movie/approved.mp4"), &outside_path).unwrap();
            symlink(&outside, &ancestor).unwrap();
        })
        .unwrap();

    assert_ne!(result.outcomes[0].status, DeletionOutcomeStatus::Deleted);
    assert!(outside_path.exists());
    assert!(original_ancestor.join("movie/approved.mp4").exists());
}

#[test]
fn approved_root_ancestor_symlink_cannot_redirect_the_root_anchor() {
    let temp = tempfile::tempdir().unwrap();
    let container = temp.path().join("container");
    let original_container = temp.path().join("container-original");
    let approved_root = container.join("approved");
    let approved_path = approved_root.join("movie/approved.mp4");
    let outside = temp.path().join("outside");
    let outside_path = outside.join("approved/movie/approved.mp4");
    fs::create_dir_all(approved_path.parent().unwrap()).unwrap();
    fs::create_dir_all(outside_path.parent().unwrap()).unwrap();
    fs::write(&approved_path, b"approved inode").unwrap();
    // Exist before planning so the stored link count remains unchanged after
    // the ancestor swap and cannot mask the root-anchor vulnerability.
    fs::hard_link(&approved_path, &outside_path).unwrap();
    let planner = PermanentDeletionPlanner::new(vec![approved_root]);
    let now = SystemTime::now();
    let plan = planner
        .create_plan(vec![approved_path], Duration::from_secs(60), now)
        .unwrap();
    let journal = durable_journal(&plan);

    fs::rename(&container, &original_container).unwrap();
    symlink(&outside, &container).unwrap();
    let result = planner.execute(&plan, now, &journal.store, &journal.task_id);

    assert!(result.is_err());
    assert!(outside_path.exists());
    assert!(original_container
        .join("approved/movie/approved.mp4")
        .exists());
}

#[test]
fn restored_replacement_phase_survives_outcome_persistence_failure() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("replacement.mp4");
    let approved_inode = temp.path().join("approved-inode.mp4");
    fs::write(&path, b"approved").unwrap();
    let planner = PermanentDeletionPlanner::new(vec![temp.path().to_path_buf()]);
    let now = SystemTime::now();
    let plan = planner
        .create_plan(vec![path.clone()], Duration::from_secs(60), now)
        .unwrap();
    let journal = durable_journal(&plan);
    let connection = rusqlite::Connection::open(&journal.database).unwrap();
    connection.execute_batch(
        "CREATE TRIGGER fail_replacement_outcome BEFORE UPDATE OF status ON management_task_items WHEN OLD.status='running' BEGIN SELECT RAISE(FAIL, 'injected outcome failure'); END;",
    ).unwrap();
    drop(connection);

    let error = planner
        .execute_journaled_with_capture_hook(
            &plan,
            now,
            &journal.store,
            &journal.task_id,
            |planned, token| {
                let quarantine = planned.path.parent().unwrap().join(token);
                fs::rename(&quarantine, &approved_inode).unwrap();
                fs::write(&quarantine, b"replacement").unwrap();
            },
        )
        .unwrap_err();

    assert!(error.to_string().contains("outcome"));
    assert_eq!(fs::read(&path).unwrap(), b"replacement");
    assert_eq!(fs::read(&approved_inode).unwrap(), b"approved");
    let item = &journal.store.get(&journal.task_id).unwrap().unwrap().items[0];
    assert_eq!(item.status, "running");
    assert_eq!(item.mutation_phase.as_deref(), Some("restored"));
}

#[test]
fn execution_attempts_every_path_and_reports_partial_outcomes() {
    let temp = tempfile::tempdir().unwrap();
    let deletable = temp.path().join("deletable.txt");
    let changed = temp.path().join("changed.txt");
    fs::write(&deletable, b"delete me").unwrap();
    fs::write(&changed, b"change me").unwrap();
    let planner = PermanentDeletionPlanner::new(vec![temp.path().to_path_buf()]);
    let plan = planner
        .create_plan(
            vec![deletable.clone(), changed.clone()],
            Duration::from_secs(60),
            SystemTime::now(),
        )
        .unwrap();
    fs::remove_file(&changed).unwrap();
    fs::write(&changed, b"replacement is different").unwrap();

    let journal = durable_journal(&plan);
    let result = planner
        .execute(&plan, SystemTime::now(), &journal.store, &journal.task_id)
        .unwrap();
    assert_eq!(result.outcomes.len(), 2);
    assert_eq!(result.outcomes[0].path, deletable);
    assert_eq!(result.outcomes[0].status, DeletionOutcomeStatus::Deleted);
    assert_eq!(result.outcomes[1].path, changed);
    assert_eq!(result.outcomes[1].status, DeletionOutcomeStatus::Changed);
    assert!(result.partial);
    assert!(!result.rolled_back);
}

#[test]
fn deletion_failure_does_not_prevent_later_paths_from_being_attempted() {
    let temp = tempfile::tempdir().unwrap();
    let locked_dir = temp.path().join("locked");
    fs::create_dir(&locked_dir).unwrap();
    let blocked = locked_dir.join("blocked.txt");
    let later = temp.path().join("later.txt");
    fs::write(&blocked, b"blocked").unwrap();
    fs::write(&later, b"later").unwrap();
    let planner = PermanentDeletionPlanner::new(vec![temp.path().to_path_buf()]);
    let plan = planner
        .create_plan(
            vec![blocked.clone(), later.clone()],
            Duration::from_secs(60),
            SystemTime::now(),
        )
        .unwrap();

    let original_mode = fs::metadata(&locked_dir).unwrap().permissions().mode();
    fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o500)).unwrap();
    let journal = durable_journal(&plan);
    let result = planner
        .execute(&plan, SystemTime::now(), &journal.store, &journal.task_id)
        .unwrap();
    fs::set_permissions(&locked_dir, fs::Permissions::from_mode(original_mode)).unwrap();

    // Root may bypass directory permissions; when it does, the changed-file test above
    // still covers partial completion. On normal macOS/Linux user runs this is Failed.
    assert!(matches!(
        result.outcomes[0].status,
        DeletionOutcomeStatus::Failed | DeletionOutcomeStatus::Deleted
    ));
    assert_eq!(result.outcomes[1].status, DeletionOutcomeStatus::Deleted);
}

#[test]
fn paths_outside_approved_roots_and_symlink_root_escapes_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let outside_file = outside.join("outside.txt");
    fs::write(&outside_file, b"outside").unwrap();
    symlink(&outside, root.join("escape")).unwrap();

    let planner = PermanentDeletionPlanner::new(vec![root]);
    assert!(planner
        .create_plan(
            vec![outside_file],
            Duration::from_secs(60),
            SystemTime::now()
        )
        .is_err());
    assert!(planner
        .create_plan(
            vec![PathBuf::from(temp.path()).join("root/escape/outside.txt")],
            Duration::from_secs(60),
            SystemTime::now()
        )
        .is_err());
}

#[test]
fn approving_a_directory_enumerates_children_and_deletes_child_first() {
    let temp = tempfile::tempdir().unwrap();
    let asset = temp.path().join("MOVIE-002");
    let nested = asset.join("art");
    fs::create_dir_all(&nested).unwrap();
    let video = asset.join("MOVIE-002.mp4");
    let poster = nested.join("poster.jpg");
    fs::write(&video, b"video").unwrap();
    fs::write(&poster, b"poster").unwrap();

    let planner = PermanentDeletionPlanner::new(vec![temp.path().to_path_buf()]);
    let plan = planner
        .create_plan(
            vec![asset.clone()],
            Duration::from_secs(60),
            SystemTime::now(),
        )
        .unwrap();

    assert_eq!(plan.approved_paths.len(), 4);
    assert_eq!(plan.logical_size, 11);
    assert_eq!(plan.approved_paths.last().unwrap().path, asset);
    let journal = durable_journal(&plan);
    let result = planner
        .execute(&plan, SystemTime::now(), &journal.store, &journal.task_id)
        .unwrap();
    assert!(result
        .outcomes
        .iter()
        .all(|outcome| outcome.status == DeletionOutcomeStatus::Deleted));
    assert!(!asset.exists());
}

#[test]
fn directory_rollback_failure_retains_a_durable_quarantine_locator() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("approved-directory");
    fs::create_dir(&directory).unwrap();
    let planner = PermanentDeletionPlanner::new(vec![temp.path().to_path_buf()]);
    let now = SystemTime::now();
    let plan = planner
        .create_plan(vec![directory.clone()], Duration::from_secs(60), now)
        .unwrap();
    let token = ".rust-jav-quarantine-item-1";

    let journal = durable_journal(&plan);
    let result = planner
        .execute_journaled_with_capture_hook(
            &plan,
            now,
            &journal.store,
            &journal.task_id,
            |planned, quarantine_token| {
                if planned.path == directory {
                    let quarantine = temp.path().join(quarantine_token);
                    fs::write(quarantine.join("arrived-after-plan.txt"), b"new").unwrap();
                    fs::create_dir(&directory).unwrap();
                }
            },
        )
        .unwrap();

    assert_eq!(result.outcomes[0].status, DeletionOutcomeStatus::Failed);
    let message = result.outcomes[0].message.as_deref().unwrap();
    assert!(message.contains(token));
    assert!(message.contains("rollback refused"));
    assert!(directory.exists());
    assert!(temp
        .path()
        .join(token)
        .join("arrived-after-plan.txt")
        .exists());
}

#[test]
fn unlink_failure_does_not_rollback_when_restoring_intent_cannot_be_persisted() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("approved-directory");
    fs::create_dir(&directory).unwrap();
    let planner = PermanentDeletionPlanner::new(vec![temp.path().to_path_buf()]);
    let now = SystemTime::now();
    let plan = planner
        .create_plan(vec![directory.clone()], Duration::from_secs(60), now)
        .unwrap();
    let journal = durable_journal(&plan);
    let connection = rusqlite::Connection::open(&journal.database).unwrap();
    connection.execute_batch(
        "CREATE TRIGGER fail_restoring_intent BEFORE UPDATE OF mutation_phase ON management_task_items WHEN OLD.mutation_phase='unlinking' AND NEW.mutation_phase='restoring_replacement' BEGIN SELECT RAISE(FAIL, 'injected restoring intent failure'); END;",
    ).unwrap();
    drop(connection);

    let error = planner
        .execute_journaled_with_capture_hook(
            &plan,
            now,
            &journal.store,
            &journal.task_id,
            |planned, token| {
                fs::write(
                    planned.path.parent().unwrap().join(token).join("new.txt"),
                    b"new",
                )
                .unwrap();
            },
        )
        .unwrap_err();

    assert!(error.to_string().contains("restoring intent"));
    let task = journal.store.get(&journal.task_id).unwrap().unwrap();
    let item = &task.items[0];
    assert_eq!(item.mutation_phase.as_deref(), Some("unlinking"));
    assert!(!directory.exists());
    assert!(temp
        .path()
        .join(item.quarantine_token.as_deref().unwrap())
        .join("new.txt")
        .exists());
}

#[test]
fn unlink_failure_rollback_is_recoverable_when_restored_phase_persistence_fails() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("approved-directory");
    fs::create_dir(&directory).unwrap();
    let planner = PermanentDeletionPlanner::new(vec![temp.path().to_path_buf()]);
    let now = SystemTime::now();
    let plan = planner
        .create_plan(vec![directory.clone()], Duration::from_secs(60), now)
        .unwrap();
    let journal = durable_journal(&plan);
    let connection = rusqlite::Connection::open(&journal.database).unwrap();
    connection.execute_batch(
        "CREATE TRIGGER fail_restored_phase BEFORE UPDATE OF mutation_phase ON management_task_items WHEN OLD.mutation_phase='restoring_replacement' AND NEW.mutation_phase='restored' BEGIN SELECT RAISE(FAIL, 'injected restored phase failure'); END;",
    ).unwrap();
    drop(connection);

    let error = planner
        .execute_journaled_with_capture_hook(
            &plan,
            now,
            &journal.store,
            &journal.task_id,
            |planned, token| {
                fs::write(
                    planned.path.parent().unwrap().join(token).join("new.txt"),
                    b"new",
                )
                .unwrap();
            },
        )
        .unwrap_err();

    assert!(error.to_string().contains("restored phase"));
    let task = journal.store.get(&journal.task_id).unwrap().unwrap();
    let item = &task.items[0];
    assert_eq!(
        item.mutation_phase.as_deref(),
        Some("restoring_replacement")
    );
    assert!(directory.join("new.txt").exists());
    assert!(!temp
        .path()
        .join(item.quarantine_token.as_deref().unwrap())
        .exists());
}
