use std::fs;

use rust_jav::application::{
    ActorViewRequest, ApplicationServices, OperationsRequest, ReportingService,
};
use rust_jav::migration_verifier::types::{ApprovalStatus, VerificationStatus};
use rust_jav::report::{OutputFormat, OutputMode};
use rust_jav::tui::state::OperationType;

#[tokio::test]
async fn operations_service_preserves_preview_default_and_report_formats() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().to_path_buf();
    fs::write(source.join("prefix-PRED-123.mp4"), b"video").unwrap();

    let report = ApplicationServices::new()
        .operations()
        .run(OperationsRequest::preview(
            source.clone(),
            vec![OperationType::StandardizeNames],
        ))
        .await;

    assert_eq!(report.mode, OutputMode::Preview);
    assert!(source.join("prefix-PRED-123.mp4").exists());
    assert!(report.verification.is_none());
    assert_eq!(
        ReportingService::render(&report, OutputFormat::Text),
        report.to_text()
    );
    assert_eq!(
        ReportingService::render(&report, OutputFormat::Json),
        report.to_json()
    );
}

#[test]
fn actor_view_service_preserves_apply_verification() {
    let source = tempfile::tempdir().unwrap();
    let actors = tempfile::tempdir().unwrap();
    fs::write(source.path().join("TEST-001.mp4"), b"video").unwrap();
    fs::write(
        source.path().join("TEST-001.nfo"),
        b"<movie><actor><name>Test Actor</name></actor></movie>",
    )
    .unwrap();

    let report = ApplicationServices::new()
        .actor_view()
        .run(ActorViewRequest::apply(
            source.path().to_path_buf(),
            actors.path().to_path_buf(),
        ))
        .unwrap();

    assert_eq!(report.mode, OutputMode::Apply);
    assert!(actors
        .path()
        .join("Test Actor/TEST-001/TEST-001.mp4")
        .exists());
    let verification = report.verification.as_ref().unwrap();
    assert_eq!(verification.verification_status, VerificationStatus::Ok);
    assert_eq!(verification.approval_status, ApprovalStatus::AutoPass);
    assert_eq!(ReportingService::exit_code(&report), 0);
}
