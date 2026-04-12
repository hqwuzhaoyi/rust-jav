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

fn run_fixture_script(output_dir: &Path) -> std::process::Output {
    Command::new("bash")
        .arg("examples/create_test_files.sh")
        .arg(output_dir)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap()
}

#[test]
fn create_test_files_script_generates_expected_scenarios_and_samples() {
    let output_dir = unique_temp_dir("fixture-script");

    let output = run_fixture_script(&output_dir);
    assert!(
        output.status.success(),
        "fixture script should succeed: stderr={} ",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("delete-ad-files (8 files)"));
    assert!(stdout.contains("actor-links (4 files)"));
    assert!(stdout.contains("cargo run -- ops --dir"));
    assert!(stdout.contains("--op delete-ad-files --json"));

    let expected_paths = [
        output_dir.join("delete-ad-files/新片首发每天更新.txt"),
        output_dir.join("delete-ad-files/大平台真人荷官.html"),
        output_dir.join("delete-ad-files/新片首发每天更新.mp4"),
        output_dir.join("delete-ad-files/STAR-123.mp4"),
        output_dir.join("standardize-names/[7sht.me]@MIDE-001.mp4"),
        output_dir.join("extract-codes/sample__abp123-C.mp4"),
        output_dir.join("categorize-files/UUSS-456-UC.mp4"),
        output_dir.join("move-origin/JUFE-333.mp4"),
        output_dir.join("organize-by-code/PRED-456-cd2.mp4"),
        output_dir.join("clean-empty-dirs/KEEP/video.mp4"),
        output_dir.join("actor-links/REBD-615.nfo"),
    ];

    for path in expected_paths {
        assert!(
            path.exists(),
            "expected fixture path to exist: {}",
            path.display()
        );
    }

    fs::remove_dir_all(output_dir).unwrap();
}

#[test]
fn create_test_files_script_resets_existing_scenarios_on_rerun() {
    let output_dir = unique_temp_dir("fixture-script-rerun");

    let first = run_fixture_script(&output_dir);
    assert!(
        first.status.success(),
        "first fixture generation should succeed: stderr={}",
        String::from_utf8_lossy(&first.stderr)
    );

    let stale_file = output_dir.join("delete-ad-files/stale.tmp");
    fs::write(&stale_file, b"stale").unwrap();
    assert!(stale_file.exists());

    let second = run_fixture_script(&output_dir);
    assert!(
        second.status.success(),
        "second fixture generation should succeed: stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );

    assert!(
        !stale_file.exists(),
        "rerunning the fixture script should reset scenario directories"
    );
    assert!(output_dir
        .join("delete-ad-files/新片首发每天更新.txt")
        .exists());
    assert!(output_dir.join("actor-links/REBD-615.mp4").exists());

    fs::remove_dir_all(output_dir).unwrap();
}
