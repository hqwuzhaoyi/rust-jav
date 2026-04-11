use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rust_jav::actor_links::execute_actor_links_command;
use rust_jav::operations::execute_operations_command;
use rust_jav::report::{ActionStatus, OutputMode};
use rust_jav::tui::state::OperationType;

fn unique_temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rust-jav-{label}-{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(path: &Path, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn count_files(dir: &Path) -> usize {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .count()
}

#[tokio::test]
async fn ops_preview_is_default_and_does_not_mutate_files() {
    let source_dir = unique_temp_dir("ops-preview");
    let source_file = source_dir.join("[7sht.me]@ABP-123.mp4");
    write_file(&source_file, b"video");

    let report = execute_operations_command(
        source_dir.clone(),
        vec![OperationType::StandardizeNames],
        false,
    )
    .await;

    assert_eq!(report.mode, OutputMode::Preview);
    assert!(
        source_file.exists(),
        "preview must not rename original files"
    );
    assert!(!source_dir.join("ABP-123.mp4").exists());
    assert!(report
        .actions
        .iter()
        .all(|action| action.status == ActionStatus::Planned));
    assert!(report.to_json().contains("\"mode\":\"preview\""));

    fs::remove_dir_all(source_dir).unwrap();
}

#[tokio::test]
async fn ops_apply_mutates_files_when_explicit() {
    let source_dir = unique_temp_dir("ops-apply");
    let source_file = source_dir.join("[7sht.me]@ABP-123.mp4");
    write_file(&source_file, b"video");

    let report = execute_operations_command(
        source_dir.clone(),
        vec![OperationType::StandardizeNames],
        true,
    )
    .await;

    assert_eq!(report.mode, OutputMode::Apply);
    assert!(
        !source_file.exists(),
        "apply should rename the original file"
    );
    assert!(source_dir.join("ABP-123.mp4").exists());
    assert!(report
        .actions
        .iter()
        .any(|action| action.status == ActionStatus::Applied));

    fs::remove_dir_all(source_dir).unwrap();
}

#[test]
fn actor_links_preview_does_not_create_targets() {
    let source_dir = unique_temp_dir("actor-preview-source");
    let actors_root = unique_temp_dir("actor-preview-target");
    write_file(&source_dir.join("REBD-615.mp4"), b"video");
    write_file(
        &source_dir.join("REBD-615.nfo"),
        include_str!("../REBD-615.nfo").as_bytes(),
    );
    write_file(&source_dir.join("REBD-615-poster.jpg"), b"poster");
    write_file(&source_dir.join("REBD-615-backdrop.jpg"), b"fanart");

    let report =
        execute_actor_links_command(source_dir.clone(), actors_root.clone(), false).unwrap();

    assert_eq!(report.mode, OutputMode::Preview);
    assert!(report.actions.iter().any(|action| action
        .target
        .as_ref()
        .is_some_and(|path| path.ends_with("miru/REBD-615/REBD-615.mp4"))));
    assert!(
        !actors_root.join("miru").exists(),
        "preview must not create actor directories"
    );

    fs::remove_dir_all(source_dir).unwrap();
    fs::remove_dir_all(actors_root).unwrap();
}

#[test]
fn actor_links_apply_creates_directory_style_links() {
    let source_dir = unique_temp_dir("actor-apply-source");
    let actors_root = unique_temp_dir("actor-apply-target");
    let source_video = source_dir.join("REBD-615.mp4");
    let source_nfo = source_dir.join("REBD-615.nfo");
    write_file(&source_video, b"video");
    write_file(&source_nfo, include_str!("../REBD-615.nfo").as_bytes());
    write_file(&source_dir.join("REBD-615-poster.jpg"), b"poster");
    write_file(&source_dir.join("REBD-615-backdrop.jpg"), b"fanart");

    let report =
        execute_actor_links_command(source_dir.clone(), actors_root.clone(), true).unwrap();
    let linked_video = actors_root
        .join("miru")
        .join("REBD-615")
        .join("REBD-615.mp4");

    assert_eq!(report.mode, OutputMode::Apply);
    assert!(linked_video.exists());
    assert!(report
        .actions
        .iter()
        .any(|action| action.status == ActionStatus::Applied));

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            fs::metadata(&source_video).unwrap().ino(),
            fs::metadata(&linked_video).unwrap().ino()
        );
    }

    fs::remove_dir_all(source_dir).unwrap();
    fs::remove_dir_all(actors_root).unwrap();
}

#[tokio::test]
async fn organize_by_code_preview_and_apply_work() {
    let source_dir = unique_temp_dir("organize-by-code");
    let source_file = source_dir.join("ABP-123.mp4");
    write_file(&source_file, b"video");

    let preview = execute_operations_command(
        source_dir.clone(),
        vec![OperationType::OrganizeByCode],
        false,
    )
    .await;
    assert!(preview.actions.iter().any(|action| {
        action
            .target
            .as_ref()
            .is_some_and(|path| path.ends_with("ABP-123/ABP-123.mp4"))
    }));
    assert!(source_file.exists());

    let apply = execute_operations_command(
        source_dir.clone(),
        vec![OperationType::OrganizeByCode],
        true,
    )
    .await;
    assert!(apply
        .actions
        .iter()
        .any(|action| action.status == ActionStatus::Applied));
    assert!(!source_file.exists());
    assert!(source_dir.join("ABP-123").join("ABP-123.mp4").exists());

    fs::remove_dir_all(source_dir).unwrap();
}

#[tokio::test]
async fn clean_empty_dirs_preview_and_apply_work() {
    let source_dir = unique_temp_dir("clean-empty-dirs");
    let empty_dir = source_dir.join("EMPTY");
    fs::create_dir_all(&empty_dir).unwrap();

    let preview = execute_operations_command(
        source_dir.clone(),
        vec![OperationType::CleanEmptyDirs],
        false,
    )
    .await;
    assert!(preview
        .actions
        .iter()
        .any(|action| action.source.as_ref() == Some(&empty_dir)));
    assert!(empty_dir.exists());

    let apply = execute_operations_command(
        source_dir.clone(),
        vec![OperationType::CleanEmptyDirs],
        true,
    )
    .await;
    assert!(apply
        .actions
        .iter()
        .any(|action| action.status == ActionStatus::Applied));
    assert!(!empty_dir.exists());

    fs::remove_dir_all(source_dir).unwrap();
}

#[tokio::test]
async fn extract_codes_preview_and_apply_work() {
    let source_dir = unique_temp_dir("extract-codes");
    let source_file = source_dir.join("sample__abp123-C.mp4");
    write_file(&source_file, b"video");

    let preview =
        execute_operations_command(source_dir.clone(), vec![OperationType::ExtractCodes], false)
            .await;
    assert!(preview.actions.iter().any(|action| {
        action.kind == "extract-code"
            && action
                .target
                .as_ref()
                .is_some_and(|path| path.ends_with("ABP-123-C.mp4"))
    }));
    assert!(source_file.exists());

    let apply =
        execute_operations_command(source_dir.clone(), vec![OperationType::ExtractCodes], true)
            .await;
    assert!(apply
        .actions
        .iter()
        .any(|action| action.status == ActionStatus::Applied));
    assert!(
        !source_file.exists(),
        "extract-codes apply should rename the original file"
    );
    assert!(source_dir.join("ABP-123-C.mp4").exists());

    fs::remove_dir_all(source_dir).unwrap();
}

#[tokio::test]
async fn extract_codes_apply_fails_safely_when_target_exists() {
    let source_dir = unique_temp_dir("extract-codes-conflict");
    let source_file = source_dir.join("sample__abp123-C.mp4");
    let target_file = source_dir.join("ABP-123-C.mp4");
    write_file(&source_file, b"video-new");
    write_file(&target_file, b"video-old");

    let apply =
        execute_operations_command(source_dir.clone(), vec![OperationType::ExtractCodes], true)
            .await;

    assert_eq!(apply.mode, OutputMode::Apply);
    assert_eq!(apply.summary.applied_actions, 0);
    assert_eq!(apply.summary.failed_actions, 1);
    assert!(apply.actions.iter().any(|action| {
        action.status == ActionStatus::Failed
            && action.reason.as_deref().is_some_and(|reason| reason.contains("target already exists"))
    }));
    assert!(
        source_file.exists(),
        "extract-codes should preserve the original file when target exists"
    );
    assert!(
        target_file.exists(),
        "extract-codes should preserve the existing target file"
    );
    assert_eq!(fs::read(&target_file).unwrap(), b"video-old");

    fs::remove_dir_all(source_dir).unwrap();
}

#[tokio::test]
async fn categorize_files_preview_and_apply_work() {
    let source_dir = unique_temp_dir("categorize-files");
    let source_file = source_dir.join("ABP-123-C.mp4");
    write_file(&source_file, b"video");

    let preview = execute_operations_command(
        source_dir.clone(),
        vec![OperationType::CategorizeFiles],
        false,
    )
    .await;
    assert!(preview.actions.iter().any(|action| {
        action
            .target
            .as_ref()
            .is_some_and(|path| path.ends_with("CHINESE/ABP-123-C.mp4"))
    }));
    assert!(source_file.exists());

    let apply = execute_operations_command(
        source_dir.clone(),
        vec![OperationType::CategorizeFiles],
        true,
    )
    .await;
    assert!(apply
        .actions
        .iter()
        .any(|action| action.status == ActionStatus::Applied));
    assert!(source_dir.join("CHINESE").join("ABP-123-C.mp4").exists());

    fs::remove_dir_all(source_dir).unwrap();
}

#[tokio::test]
async fn move_origin_preview_and_apply_work() {
    let source_dir = unique_temp_dir("move-origin");
    let source_file = source_dir.join("ABP-123.mp4");
    write_file(&source_file, b"video");

    let preview =
        execute_operations_command(source_dir.clone(), vec![OperationType::MoveOrigin], false)
            .await;
    assert!(preview.actions.iter().any(|action| {
        action
            .target
            .as_ref()
            .is_some_and(|path| path.ends_with("ORIGIN/ABP-123.mp4"))
    }));
    assert!(source_file.exists());

    let apply =
        execute_operations_command(source_dir.clone(), vec![OperationType::MoveOrigin], true).await;
    assert!(apply
        .actions
        .iter()
        .any(|action| action.status == ActionStatus::Applied));
    assert!(source_dir.join("ORIGIN").join("ABP-123.mp4").exists());

    fs::remove_dir_all(source_dir).unwrap();
}

#[tokio::test]
async fn remove_duplicates_preview_and_apply_work() {
    let source_dir = unique_temp_dir("remove-duplicates");
    let first = source_dir.join("AA-001.mp4");
    let second = source_dir.join("AA-002.mp4");
    let payload = vec![0u8; 1_100_000];
    write_file(&first, &payload);
    write_file(&second, &payload);

    let preview = execute_operations_command(
        source_dir.clone(),
        vec![OperationType::RemoveDuplicates],
        false,
    )
    .await;
    assert_eq!(preview.summary.warning_count, 1);
    assert_eq!(preview.actions.len(), 1);

    let apply = execute_operations_command(
        source_dir.clone(),
        vec![OperationType::RemoveDuplicates],
        true,
    )
    .await;
    assert!(apply.summary.warning_count >= 1);
    assert_eq!(count_files(&source_dir), 1);

    fs::remove_dir_all(source_dir).unwrap();
}

#[test]
fn actor_links_apply_is_idempotent_for_existing_targets() {
    let source_dir = unique_temp_dir("actor-idempotent-source");
    let actors_root = unique_temp_dir("actor-idempotent-target");
    write_file(&source_dir.join("REBD-615.mp4"), b"video");
    write_file(
        &source_dir.join("REBD-615.nfo"),
        include_str!("../REBD-615.nfo").as_bytes(),
    );
    write_file(&source_dir.join("REBD-615-poster.jpg"), b"poster");

    let first = execute_actor_links_command(source_dir.clone(), actors_root.clone(), true).unwrap();
    assert!(first
        .actions
        .iter()
        .any(|action| action.status == ActionStatus::Applied));

    let second =
        execute_actor_links_command(source_dir.clone(), actors_root.clone(), true).unwrap();
    assert!(second
        .actions
        .iter()
        .any(|action| action.status == ActionStatus::Skipped));

    fs::remove_dir_all(source_dir).unwrap();
    fs::remove_dir_all(actors_root).unwrap();
}

#[test]
fn actor_links_preview_warns_when_nfo_has_no_actor() {
    let source_dir = unique_temp_dir("actor-malformed-source");
    let actors_root = unique_temp_dir("actor-malformed-target");
    write_file(&source_dir.join("NO-ACTOR.mp4"), b"video");
    write_file(
        &source_dir.join("NO-ACTOR.nfo"),
        br#"<?xml version="1.0"?><movie><title>No Actor</title></movie>"#,
    );

    let report =
        execute_actor_links_command(source_dir.clone(), actors_root.clone(), false).unwrap();
    assert_eq!(report.actions.len(), 0);
    assert_eq!(report.summary.warning_count, 1);

    fs::remove_dir_all(source_dir).unwrap();
    fs::remove_dir_all(actors_root).unwrap();
}
