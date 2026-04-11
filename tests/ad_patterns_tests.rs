use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rust_jav::file_utils::delete_files::delete_files_matching_patterns;
use rust_jav::operations::execute_operations_command;
use rust_jav::report::ActionStatus;
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

#[test]
fn embedded_patterns_include_known_entries_and_match_dynamic_filenames() {
    let patterns = rust_jav::file_utils::ad_patterns::embedded_patterns();
    assert!(patterns.iter().any(|p| p == "新片首发*"));
    assert!(rust_jav::file_utils::ad_patterns::filename_matches_any(
        "新片首发每天更新.mp4",
        &patterns,
    ));
    assert!(!rust_jav::file_utils::ad_patterns::filename_matches_any(
        "SSNI-888.mkv",
        &patterns,
    ));
}

#[tokio::test]
async fn legacy_delete_helper_uses_same_matching_rules_as_cli_path() {
    let patterns = rust_jav::file_utils::ad_patterns::embedded_patterns();

    let legacy_dir = unique_temp_dir("legacy-parity-legacy");
    let legacy_matched = legacy_dir.join("聚 合 全 網 H 直 播.html");
    let legacy_unmatched = legacy_dir.join("聚 合 全 網 H 直 播Xhtml");
    write_file(&legacy_matched, b"ad");
    write_file(&legacy_unmatched, b"not-ad");

    let deleted = delete_files_matching_patterns(&legacy_matched, &patterns)
        .await
        .unwrap();
    assert!(
        deleted,
        "literal .html filename should be deleted by legacy helper"
    );
    assert!(!legacy_matched.exists());

    let deleted = delete_files_matching_patterns(&legacy_unmatched, &patterns)
        .await
        .unwrap();
    assert!(
        !deleted,
        "legacy helper must not delete filenames that only match because '.' was treated as regex"
    );
    assert!(legacy_unmatched.exists());

    let cli_dir = unique_temp_dir("legacy-parity-cli");
    let cli_matched = cli_dir.join("聚 合 全 網 H 直 播.html");
    let cli_unmatched = cli_dir.join("聚 合 全 網 H 直 播Xhtml");
    write_file(&cli_matched, b"ad");
    write_file(&cli_unmatched, b"not-ad");

    let report =
        execute_operations_command(cli_dir.clone(), vec![OperationType::DeleteAdFiles], true).await;
    assert!(report
        .actions
        .iter()
        .any(|action| action.status == ActionStatus::Applied
            && action.source.as_ref() == Some(&cli_matched)));
    assert!(
        !report
            .actions
            .iter()
            .any(|action| action.source.as_ref() == Some(&cli_unmatched)),
        "CLI path must not treat Xhtml as matching the .html pattern"
    );
    assert!(!cli_matched.exists());
    assert!(cli_unmatched.exists());

    fs::remove_dir_all(legacy_dir).unwrap();
    fs::remove_dir_all(cli_dir).unwrap();
}
