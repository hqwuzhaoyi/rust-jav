use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn run_script(args: &[&str]) -> std::process::Output {
    Command::new("bash")
        .arg("scripts/verify_migration_counts.sh")
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap()
}

#[test]
fn migration_count_script_compare_accepts_relayout_when_counts_match() {
    let before_dir = unique_temp_dir("migration-count-before");
    let after_dir = unique_temp_dir("migration-count-after");
    let manifest = unique_temp_dir("migration-count-manifest").join("before.txt");

    write_file(&before_dir.join("movie/ABP-123.mp4"), b"video");
    write_file(&before_dir.join("movie/ABP-123.nfo"), b"nfo");
    write_file(&before_dir.join("movie/ABP-123-poster.jpg"), b"poster");
    write_file(&before_dir.join("movie/ABP-123-backdrop.jpg"), b"fanart");

    let snapshot = run_script(&[
        "snapshot",
        "--dir",
        before_dir.to_str().unwrap(),
        "--output",
        manifest.to_str().unwrap(),
    ]);
    assert!(
        snapshot.status.success(),
        "snapshot should succeed: stderr={}",
        String::from_utf8_lossy(&snapshot.stderr)
    );

    write_file(&after_dir.join("ABP-123/ABP-123.mp4"), b"video");
    write_file(&after_dir.join("ABP-123/ABP-123.nfo"), b"nfo");
    write_file(&after_dir.join("ABP-123/cover/poster.jpg"), b"poster");
    write_file(&after_dir.join("ABP-123/cover/backdrop.jpg"), b"fanart");

    let compare = run_script(&[
        "compare",
        "--before",
        manifest.to_str().unwrap(),
        "--after-dir",
        after_dir.to_str().unwrap(),
    ]);
    assert!(
        compare.status.success(),
        "compare should succeed when counts match: stdout={} stderr={}",
        String::from_utf8_lossy(&compare.stdout),
        String::from_utf8_lossy(&compare.stderr)
    );

    let stdout = String::from_utf8_lossy(&compare.stdout);
    assert!(stdout.contains("before_total=4"));
    assert!(stdout.contains("after_total=4"));
    assert!(stdout.contains("status=ok"));
    assert!(stdout.contains("mp4"));
    assert!(stdout.contains("jpg"));
    assert!(stdout.contains("nfo"));

    fs::remove_dir_all(before_dir).unwrap();
    fs::remove_dir_all(after_dir).unwrap();
    fs::remove_file(manifest).unwrap();
}

#[test]
fn migration_count_script_compare_reports_mismatch_when_counts_drift() {
    let before_dir = unique_temp_dir("migration-count-mismatch-before");
    let after_dir = unique_temp_dir("migration-count-mismatch-after");
    let manifest = unique_temp_dir("migration-count-mismatch-manifest").join("before.txt");

    write_file(&before_dir.join("movie/SSIS-001.mp4"), b"video");
    write_file(&before_dir.join("movie/SSIS-001.nfo"), b"nfo");
    write_file(&before_dir.join("movie/SSIS-001-poster.jpg"), b"poster");

    let snapshot = run_script(&[
        "snapshot",
        "--dir",
        before_dir.to_str().unwrap(),
        "--output",
        manifest.to_str().unwrap(),
    ]);
    assert!(
        snapshot.status.success(),
        "snapshot should succeed: stderr={}",
        String::from_utf8_lossy(&snapshot.stderr)
    );

    write_file(&after_dir.join("SSIS-001/SSIS-001.mp4"), b"video");
    write_file(&after_dir.join("SSIS-001/SSIS-001.nfo"), b"nfo");
    write_file(&after_dir.join("SSIS-001/readme.txt"), b"unexpected");

    let compare = run_script(&[
        "compare",
        "--before",
        manifest.to_str().unwrap(),
        "--after-dir",
        after_dir.to_str().unwrap(),
    ]);
    assert!(
        !compare.status.success(),
        "compare should fail when counts drift: stdout={} stderr={}",
        String::from_utf8_lossy(&compare.stdout),
        String::from_utf8_lossy(&compare.stderr)
    );

    let stdout = String::from_utf8_lossy(&compare.stdout);
    assert!(stdout.contains("before_total=3"));
    assert!(stdout.contains("after_total=3"));
    assert!(stdout.contains("status=mismatch"));
    assert!(stdout.contains("jpg"));
    assert!(stdout.contains("txt"));

    fs::remove_dir_all(before_dir).unwrap();
    fs::remove_dir_all(after_dir).unwrap();
    fs::remove_file(manifest).unwrap();
}
