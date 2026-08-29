#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use rust_jav::deletion_plan::{
    DeletionOutcomeStatus, FileType, PermanentDeletionPlanner, PlanExecutionError,
};

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

    let result = planner
        .execute(&plan, now + Duration::from_secs(1))
        .unwrap();
    assert_eq!(result.outcomes[0].status, DeletionOutcomeStatus::Deleted);
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
    assert_eq!(
        planner.execute(&expired, created + Duration::from_secs(6)),
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
    let result = planner.execute(&current, SystemTime::now()).unwrap();
    assert_eq!(result.outcomes[0].status, DeletionOutcomeStatus::Changed);
    assert!(first.exists());
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

    let result = planner.execute(&plan, SystemTime::now()).unwrap();
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
    let result = planner.execute(&plan, SystemTime::now()).unwrap();
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
    let result = planner.execute(&plan, SystemTime::now()).unwrap();
    assert!(result
        .outcomes
        .iter()
        .all(|outcome| outcome.status == DeletionOutcomeStatus::Deleted));
    assert!(!asset.exists());
}
