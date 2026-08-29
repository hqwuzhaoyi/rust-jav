use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;
use rust_jav::active_rules::ActiveRuleSet;
use rust_jav::actor_links::execute_actor_links_command;
use rust_jav::application::{ApplicationServices, OperationsRequest};
use rust_jav::cli::Cli;
use rust_jav::migration_verifier::types::{ApprovalStatus, MigrationScope, VerificationStatus};
use rust_jav::operations::execute_operations_command;
use rust_jav::report::{ActionStatus, OutputMode};
use rust_jav::runtime::resolve_run_request;
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

#[tokio::test]
async fn ops_apply_adds_verification_summary_and_report_path() {
    let source_dir = unique_temp_dir("ops-verify");
    let source_file = source_dir.join("[7sht.me]@ABP-123.mp4");
    write_file(&source_file, b"video");

    let report = execute_operations_command(
        source_dir.clone(),
        vec![OperationType::StandardizeNames],
        true,
    )
    .await;

    let verification = report
        .verification
        .as_ref()
        .expect("apply should include verification summary");
    assert_eq!(verification.verification_status, VerificationStatus::Ok);
    assert_eq!(verification.approval_status, ApprovalStatus::AutoPass);
    assert_eq!(verification.exit_code, 0);
    assert!(verification
        .report_path
        .as_ref()
        .is_some_and(|path| path.exists()));
    assert!(verification.scopes.iter().any(|scope| {
        scope.scope == MigrationScope::Source
            && scope.before_count == 1
            && scope.expected_count == 1
            && scope.after_count == 1
    }));

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

#[test]
fn actor_links_conventional_movie_nfo_preserves_recursive_movie_layout() {
    let source_dir = unique_temp_dir("actor-conventional-source");
    let actors_root = unique_temp_dir("actor-conventional-target");
    let movie_dir = source_dir.join("MIAB-492-C");
    let source_video = movie_dir.join("MIAB-492-C.mp4");
    let source_trickplay = movie_dir.join("MIAB-492-C.trickplay/320 - 10x10/0.jpg");
    write_file(&source_video, b"video");
    write_file(&source_trickplay, b"trickplay");
    write_file(&movie_dir.join("folder.jpg"), b"poster");
    write_file(
        &movie_dir.join("movie.nfo"),
        b"<movie><actor><name>AIKA</name></actor></movie>",
    );
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        source_dir.join("outside.jpg"),
        movie_dir.join("MIAB-492-C.trickplay/ignored.jpg"),
    )
    .unwrap();

    let report =
        execute_actor_links_command(source_dir.clone(), actors_root.clone(), true).unwrap();
    let target_movie = actors_root.join("AIKA/MIAB-492-C");

    assert_eq!(report.summary.failed_actions, 0);
    assert!(target_movie.join("MIAB-492-C.mp4").exists());
    assert!(target_movie.join("movie.nfo").exists());
    assert!(target_movie.join("folder.jpg").exists());
    assert!(target_movie
        .join("MIAB-492-C.trickplay/320 - 10x10/0.jpg")
        .exists());
    assert!(!target_movie
        .join("MIAB-492-C.trickplay/ignored.jpg")
        .exists());

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            fs::metadata(&source_trickplay).unwrap().ino(),
            fs::metadata(target_movie.join("MIAB-492-C.trickplay/320 - 10x10/0.jpg"))
                .unwrap()
                .ino()
        );
    }

    fs::remove_dir_all(source_dir).unwrap();
    fs::remove_dir_all(actors_root).unwrap();
}

#[test]
fn actor_links_apply_adds_dual_scope_verification_summary() {
    let source_dir = unique_temp_dir("actor-verify-source");
    let actors_root = unique_temp_dir("actor-verify-target");
    write_file(&source_dir.join("REBD-615.mp4"), b"video");
    write_file(
        &source_dir.join("REBD-615.nfo"),
        include_str!("../REBD-615.nfo").as_bytes(),
    );
    write_file(&source_dir.join("REBD-615-poster.jpg"), b"poster");
    write_file(&source_dir.join("REBD-615-backdrop.jpg"), b"fanart");

    let report =
        execute_actor_links_command(source_dir.clone(), actors_root.clone(), true).unwrap();
    let verification = report
        .verification
        .as_ref()
        .expect("apply should include verification summary");

    assert_eq!(verification.verification_status, VerificationStatus::Ok);
    assert_eq!(verification.approval_status, ApprovalStatus::AutoPass);
    assert_eq!(verification.exit_code, 0);
    assert_eq!(verification.scopes.len(), 2);
    assert!(verification.scopes.iter().any(|scope| {
        scope.scope == MigrationScope::Source
            && scope.before_count == 4
            && scope.expected_count == 4
            && scope.after_count == 4
    }));
    assert!(verification.scopes.iter().any(|scope| {
        scope.scope == MigrationScope::ActorsRoot
            && scope.before_count == 0
            && scope.expected_count == 4
            && scope.after_count == 4
    }));
    assert!(verification
        .report_path
        .as_ref()
        .is_some_and(|path| path.exists()));

    fs::remove_dir_all(source_dir).unwrap();
    fs::remove_dir_all(actors_root).unwrap();
}

#[test]
fn actor_links_apply_detects_wrong_preexisting_targets() {
    let source_dir = unique_temp_dir("actor-verify-mismatch-source");
    let actors_root = unique_temp_dir("actor-verify-mismatch-target");
    write_file(&source_dir.join("REBD-615.mp4"), b"video");
    write_file(
        &source_dir.join("REBD-615.nfo"),
        include_str!("../REBD-615.nfo").as_bytes(),
    );
    write_file(&source_dir.join("REBD-615-poster.jpg"), b"poster");
    write_file(&source_dir.join("REBD-615-backdrop.jpg"), b"fanart");

    let target_dir = actors_root.join("miru").join("REBD-615");
    write_file(&target_dir.join("REBD-615.mp4"), b"video");
    write_file(
        &target_dir.join("REBD-615.nfo"),
        include_str!("../REBD-615.nfo").as_bytes(),
    );
    write_file(&target_dir.join("REBD-615-poster.jpg"), b"poster");
    write_file(&target_dir.join("REBD-615-backdrop.jpg"), b"fanart");

    let report =
        execute_actor_links_command(source_dir.clone(), actors_root.clone(), true).unwrap();
    let verification = report
        .verification
        .as_ref()
        .expect("apply should include verification summary");

    assert_eq!(
        verification.verification_status,
        VerificationStatus::Mismatch
    );
    assert_eq!(verification.approval_status, ApprovalStatus::Blocked);
    assert_eq!(verification.exit_code, 20);
    let report_path = verification
        .report_path
        .as_ref()
        .expect("mismatch should still write a report");
    let detailed = fs::read_to_string(report_path).unwrap();
    assert!(detailed.contains("\"expected_existing_links\":0"));
    assert!(detailed.contains("\"expected_new_links\":4"));

    fs::remove_dir_all(source_dir).unwrap();
    fs::remove_dir_all(actors_root).unwrap();
}

#[test]
fn actor_links_rerun_report_tracks_existing_links() {
    let source_dir = unique_temp_dir("actor-verify-rerun-source");
    let actors_root = unique_temp_dir("actor-verify-rerun-target");
    write_file(&source_dir.join("REBD-615.mp4"), b"video");
    write_file(
        &source_dir.join("REBD-615.nfo"),
        include_str!("../REBD-615.nfo").as_bytes(),
    );
    write_file(&source_dir.join("REBD-615-poster.jpg"), b"poster");
    write_file(&source_dir.join("REBD-615-backdrop.jpg"), b"fanart");

    let first = execute_actor_links_command(source_dir.clone(), actors_root.clone(), true).unwrap();
    let first_report_path = first
        .verification
        .as_ref()
        .and_then(|verification| verification.report_path.as_ref())
        .expect("first apply should write a report")
        .clone();
    let first_report = fs::read_to_string(&first_report_path).unwrap();
    assert!(first_report.contains("\"expected_new_links\":4"));
    assert!(first_report.contains("\"expected_existing_links\":0"));

    let second =
        execute_actor_links_command(source_dir.clone(), actors_root.clone(), true).unwrap();
    let second_report_path = second
        .verification
        .as_ref()
        .and_then(|verification| verification.report_path.as_ref())
        .expect("second apply should write a report")
        .clone();
    let second_report = fs::read_to_string(&second_report_path).unwrap();
    assert!(second_report.contains("\"expected_new_links\":0"));
    assert!(second_report.contains("\"expected_existing_links\":4"));

    fs::remove_dir_all(source_dir).unwrap();
    fs::remove_dir_all(actors_root).unwrap();
}

#[test]
fn actor_links_report_records_duplicate_target_plan_conflicts() {
    let source_dir = unique_temp_dir("actor-verify-conflict-source");
    let actors_root = unique_temp_dir("actor-verify-conflict-target");

    for subdir in ["disc-a", "disc-b"] {
        let movie_dir = source_dir.join(subdir);
        write_file(&movie_dir.join("REBD-615.mp4"), b"video");
        write_file(
            &movie_dir.join("REBD-615.nfo"),
            include_str!("../REBD-615.nfo").as_bytes(),
        );
        write_file(&movie_dir.join("REBD-615-poster.jpg"), b"poster");
        write_file(&movie_dir.join("REBD-615-backdrop.jpg"), b"fanart");
    }

    let report =
        execute_actor_links_command(source_dir.clone(), actors_root.clone(), true).unwrap();
    let verification = report
        .verification
        .as_ref()
        .expect("apply should include verification summary");
    assert_eq!(
        verification.verification_status,
        VerificationStatus::Mismatch
    );
    let report_path = verification
        .report_path
        .as_ref()
        .expect("conflict should still write a report");
    let detailed = fs::read_to_string(report_path).unwrap();
    assert!(detailed.contains("duplicate actor-link target"));

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
            && action
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("target already exists"))
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

// ── delete-ad-files tests ────────────────────────────────────────────────────

#[tokio::test]
async fn yaml_rules_drive_compatible_preview_and_apply_matching() {
    let yaml = r#"
version: 1
rules:
  - pattern: "offer.*"
  - pattern: "disabled*"
    enabled: false
"#;
    let active_rules = ActiveRuleSet::from_yaml(yaml, false).unwrap();
    let source_dir = unique_temp_dir("yaml-rules-preview-apply");
    write_file(&source_dir.join("OFFER.HTML"), b"ad");
    write_file(&source_dir.join("offerXhtml"), b"literal-dot");
    write_file(&source_dir.join("disabled-offer.html"), b"disabled");

    let preview = ApplicationServices::new()
        .operations()
        .run(OperationsRequest::preview_with_rules(
            source_dir.clone(),
            vec![OperationType::DeleteAdFiles],
            active_rules.clone(),
        ))
        .await;
    assert_eq!(preview.summary.planned_actions, 1);
    assert!(source_dir.join("OFFER.HTML").exists());

    let apply = ApplicationServices::new()
        .operations()
        .run(OperationsRequest::apply_with_rules(
            source_dir.clone(),
            vec![OperationType::DeleteAdFiles],
            active_rules,
        ))
        .await;
    assert_eq!(apply.summary.applied_actions, 1);
    assert!(!source_dir.join("OFFER.HTML").exists());
    assert!(source_dir.join("offerXhtml").exists());
    assert!(source_dir.join("disabled-offer.html").exists());

    fs::remove_dir_all(source_dir).unwrap();
}

#[tokio::test]
async fn explicitly_confirmed_empty_yaml_rule_set_is_a_safe_noop() {
    let active_rules = ActiveRuleSet::from_yaml("version: 1\nrules: []\n", true).unwrap();
    let source_dir = unique_temp_dir("yaml-rules-empty");
    write_file(&source_dir.join("新片首发每天更新.txt"), b"ad");

    let report = ApplicationServices::new()
        .operations()
        .run(OperationsRequest::apply_with_rules(
            source_dir.clone(),
            vec![OperationType::DeleteAdFiles],
            active_rules,
        ))
        .await;

    assert_eq!(report.summary.applied_actions, 0);
    assert!(source_dir.join("新片首发每天更新.txt").exists());
    fs::remove_dir_all(source_dir).unwrap();
}

#[tokio::test]
async fn cli_rejects_invalid_yaml_before_preview_or_apply() {
    let source_dir = unique_temp_dir("yaml-rules-invalid-source");
    let rules_dir = unique_temp_dir("yaml-rules-invalid-config");
    let rules_path = rules_dir.join("rules.yaml");
    write_file(&source_dir.join("offer.html"), b"keep");
    write_file(&rules_path, b"version: 1\nrules: [not valid");

    for apply in [false, true] {
        let mut argv = vec![
            "rust-jav".to_string(),
            "ops".to_string(),
            "--dir".to_string(),
            source_dir.display().to_string(),
            "--rules".to_string(),
            rules_path.display().to_string(),
            "--op".to_string(),
            "delete-ad-files".to_string(),
        ];
        if apply {
            argv.push("--apply".to_string());
        }
        let error = resolve_run_request(Cli::try_parse_from(argv).unwrap())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("invalid rule set YAML"));
        assert!(source_dir.join("offer.html").exists());
    }

    fs::remove_dir_all(source_dir).unwrap();
    fs::remove_dir_all(rules_dir).unwrap();
}

#[tokio::test]
async fn cli_requires_confirmation_for_empty_yaml_rule_set() {
    let source_dir = unique_temp_dir("yaml-rules-empty-cli-source");
    let rules_dir = unique_temp_dir("yaml-rules-empty-cli-config");
    let rules_path = rules_dir.join("rules.yaml");
    write_file(&rules_path, b"version: 1\nrules: []\n");

    let cli = Cli::try_parse_from([
        "rust-jav",
        "ops",
        "--dir",
        source_dir.to_str().unwrap(),
        "--rules",
        rules_path.to_str().unwrap(),
        "--op",
        "delete-ad-files",
    ])
    .unwrap();
    let error = resolve_run_request(cli).await.unwrap_err();
    assert!(error.to_string().contains("--confirm-empty-rules"));

    fs::remove_dir_all(source_dir).unwrap();
    fs::remove_dir_all(rules_dir).unwrap();
}

#[tokio::test]
async fn delete_ad_files_preview_plans_matched_files_without_deleting() {
    let source_dir = unique_temp_dir("delete-ad-preview");
    write_file(&source_dir.join("新片首发每天更新.txt"), b"ad");
    write_file(&source_dir.join("大平台真人荷官.html"), b"ad");
    write_file(&source_dir.join("STAR-123.mp4"), b"video");

    let report = execute_operations_command(
        source_dir.clone(),
        vec![OperationType::DeleteAdFiles],
        false,
    )
    .await;

    assert_eq!(report.mode, OutputMode::Preview);
    assert_eq!(
        report.summary.planned_actions, 2,
        "should plan exactly 2 ad files"
    );
    assert_eq!(report.summary.applied_actions, 0, "preview must not delete");
    assert!(source_dir.join("新片首发每天更新.txt").exists());
    assert!(source_dir.join("大平台真人荷官.html").exists());
    assert!(source_dir.join("STAR-123.mp4").exists());

    fs::remove_dir_all(source_dir).unwrap();
}

#[tokio::test]
async fn delete_ad_files_apply_deletes_matched_and_spares_unmatched() {
    let source_dir = unique_temp_dir("delete-ad-apply");
    write_file(&source_dir.join("新片首发每天更新.txt"), b"ad");
    write_file(&source_dir.join("大平台真人荷官.html"), b"ad");
    write_file(&source_dir.join("STAR-123.mp4"), b"video");

    let report =
        execute_operations_command(source_dir.clone(), vec![OperationType::DeleteAdFiles], true)
            .await;

    assert_eq!(report.mode, OutputMode::Apply);
    let applied: Vec<_> = report
        .actions
        .iter()
        .filter(|a| a.status == ActionStatus::Applied)
        .collect();
    assert_eq!(applied.len(), 2, "exactly 2 files should be deleted");
    assert!(!source_dir.join("新片首发每天更新.txt").exists());
    assert!(!source_dir.join("大平台真人荷官.html").exists());
    assert!(source_dir.join("STAR-123.mp4").exists());

    fs::remove_dir_all(source_dir).unwrap();
}

#[tokio::test]
async fn delete_ad_files_apply_deletes_matching_video_file() {
    let source_dir = unique_temp_dir("delete-ad-video");
    write_file(&source_dir.join("新片首发每天更新.mp4"), b"video-ad");
    write_file(&source_dir.join("PRED-456.mp4"), b"real-video");

    let report =
        execute_operations_command(source_dir.clone(), vec![OperationType::DeleteAdFiles], true)
            .await;

    assert!(
        report.summary.warning_count > 0,
        "should warn about matched video files"
    );
    assert!(
        !source_dir.join("新片首发每天更新.mp4").exists(),
        "ad video deleted"
    );
    assert!(source_dir.join("PRED-456.mp4").exists(), "real video kept");

    fs::remove_dir_all(source_dir).unwrap();
}

#[tokio::test]
async fn delete_ad_files_apply_requires_manual_confirmation_when_verified() {
    let source_dir = unique_temp_dir("delete-ad-manual-confirm");
    write_file(&source_dir.join("新片首发每天更新.txt"), b"ad");
    write_file(&source_dir.join("PRED-456.mp4"), b"real-video");

    let report =
        execute_operations_command(source_dir.clone(), vec![OperationType::DeleteAdFiles], true)
            .await;

    let verification = report
        .verification
        .as_ref()
        .expect("destructive apply should include verification summary");
    assert_eq!(verification.verification_status, VerificationStatus::Ok);
    assert_eq!(
        verification.approval_status,
        ApprovalStatus::ManualConfirmRequired
    );
    assert_eq!(verification.exit_code, 10);

    fs::remove_dir_all(source_dir).unwrap();
}

#[tokio::test]
async fn delete_ad_files_preview_on_empty_dir_produces_no_actions() {
    let source_dir = unique_temp_dir("delete-ad-empty");

    let report = execute_operations_command(
        source_dir.clone(),
        vec![OperationType::DeleteAdFiles],
        false,
    )
    .await;

    assert_eq!(report.summary.planned_actions, 0);
    assert_eq!(report.summary.warning_count, 0);

    fs::remove_dir_all(source_dir).unwrap();
}

#[tokio::test]
async fn delete_ad_files_runs_before_other_ops_in_full_pipeline() {
    let all = OperationType::all();
    assert_eq!(
        all[0],
        OperationType::DeleteAdFiles,
        "DeleteAdFiles must be first so ad files are removed before rename/move ops run"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn delete_ad_files_does_not_follow_symlinked_directories_outside_root() {
    use std::os::unix::fs as unix_fs;

    let source_dir = unique_temp_dir("delete-ad-symlink-root");
    let outside_dir = unique_temp_dir("delete-ad-symlink-outside");
    let outside_file = outside_dir.join("新片首发每天更新.txt");
    write_file(&outside_file, b"outside-ad");

    let escape_link = source_dir.join("escape");
    unix_fs::symlink(&outside_dir, &escape_link).unwrap();

    let preview = execute_operations_command(
        source_dir.clone(),
        vec![OperationType::DeleteAdFiles],
        false,
    )
    .await;
    assert!(
        preview.actions.is_empty(),
        "preview must not traverse symlinked directories outside the requested root"
    );

    let apply =
        execute_operations_command(source_dir.clone(), vec![OperationType::DeleteAdFiles], true)
            .await;
    assert!(apply.actions.is_empty());
    assert!(
        outside_file.exists(),
        "apply must not delete files reachable only through an external symlink"
    );

    fs::remove_dir_all(source_dir).unwrap();
    fs::remove_dir_all(outside_dir).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn delete_ad_files_apply_reports_failures_when_directory_is_not_writable() {
    use std::os::unix::fs::PermissionsExt;

    let source_dir = unique_temp_dir("delete-ad-permission");
    let locked_dir = source_dir.join("locked");
    fs::create_dir_all(&locked_dir).unwrap();
    let ad_file = locked_dir.join("新片首发每天更新.txt");
    write_file(&ad_file, b"ad");

    let original_mode = fs::metadata(&locked_dir).unwrap().permissions().mode();
    let mut perms = fs::metadata(&locked_dir).unwrap().permissions();
    perms.set_mode(0o555);
    fs::set_permissions(&locked_dir, perms).unwrap();

    let report =
        execute_operations_command(source_dir.clone(), vec![OperationType::DeleteAdFiles], true)
            .await;

    let mut cleanup_perms = fs::metadata(&locked_dir).unwrap().permissions();
    cleanup_perms.set_mode(original_mode);
    fs::set_permissions(&locked_dir, cleanup_perms).unwrap();

    assert_eq!(report.summary.failed_actions, 1);
    assert_eq!(report.summary.applied_actions, 0);
    assert!(report.actions.iter().any(|action| {
        action.status == ActionStatus::Failed && action.source.as_ref() == Some(&ad_file)
    }));
    assert!(
        ad_file.exists(),
        "failed delete should keep the original file in place"
    );

    fs::remove_dir_all(source_dir).unwrap();
}

#[tokio::test]
async fn delete_ad_files_full_pipeline_deletes_before_later_ops_can_move_or_rename() {
    let source_dir = unique_temp_dir("delete-ad-full-pipeline");
    let ad_video = source_dir.join("新片首发每天更新-C.mp4");
    write_file(&ad_video, b"ad-video");

    let report = execute_operations_command(source_dir.clone(), OperationType::all(), true).await;

    assert!(report.actions.iter().any(|action| {
        action.kind == "delete-file"
            && action.status == ActionStatus::Applied
            && action.source.as_ref() == Some(&ad_video)
    }));
    assert!(
        !report.actions.iter().any(|action| {
            action.source.as_ref() == Some(&ad_video) && action.kind != "delete-file"
        }),
        "once delete-ad-files removes the file, no later op should reference it"
    );
    assert!(
        !ad_video.exists(),
        "full pipeline should delete the ad file before later ops run"
    );

    fs::remove_dir_all(source_dir).unwrap();
}
