use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;

use rust_jav::cli::Cli;
use rust_jav::report::{OutputFormat, OutputMode};
use rust_jav::runtime::{resolve_run_request, RunRequest};

fn unique_temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rust-jav-runtime-{label}-{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(path: &Path, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

#[tokio::test]
async fn runtime_routes_tui_command_without_report() {
    let cli = Cli::try_parse_from(["rust-jav", "tui", "--dir", "./examples/test"]).unwrap();
    let request = resolve_run_request(cli).await.unwrap();

    match request {
        RunRequest::Tui { dir } => assert_eq!(dir.to_string_lossy(), "./examples/test"),
        other => panic!("expected Tui request, got {other:?}"),
    }
}

#[tokio::test]
async fn runtime_routes_ops_preview_with_zero_exit_code() {
    let cli = Cli::try_parse_from([
        "rust-jav",
        "ops",
        "--dir",
        "./examples/test",
        "--op",
        "extract-codes",
        "--json",
    ])
    .unwrap();
    let request = resolve_run_request(cli).await.unwrap();

    match request {
        RunRequest::Report {
            report,
            format,
            exit_code,
        } => {
            assert_eq!(format, OutputFormat::Json);
            assert_eq!(exit_code, 0);
            assert_eq!(report.mode, OutputMode::Preview);
        }
        other => panic!("expected report request, got {other:?}"),
    }
}

#[tokio::test]
async fn runtime_routes_actor_links_preview_with_zero_exit_code() {
    let source_dir = unique_temp_dir("actor-preview-source");
    let actors_root = unique_temp_dir("actor-preview-target");
    write_file(&source_dir.join("REBD-615.mp4"), b"video");
    write_file(
        &source_dir.join("REBD-615.nfo"),
        include_str!("../REBD-615.nfo").as_bytes(),
    );

    let cli = Cli::try_parse_from([
        "rust-jav",
        "actor-links",
        "--source",
        source_dir.to_str().unwrap(),
        "--actors-root",
        actors_root.to_str().unwrap(),
    ])
    .unwrap();
    let request = resolve_run_request(cli).await.unwrap();

    match request {
        RunRequest::Report {
            report,
            format,
            exit_code,
        } => {
            assert_eq!(format, OutputFormat::Text);
            assert_eq!(exit_code, 0);
            assert_eq!(report.mode, OutputMode::Preview);
            assert!(!actors_root.join("miru").exists());
        }
        other => panic!("expected report request, got {other:?}"),
    }

    fs::remove_dir_all(source_dir).unwrap();
    fs::remove_dir_all(actors_root).unwrap();
}
