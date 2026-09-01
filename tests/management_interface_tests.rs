use std::time::Duration;

use axum::body::{to_bytes, Body};
use http::{header, Request, StatusCode};
use rust_jav::management::{
    app, init_administrator, password_secrets, AppState, Clock, DownloadError, ManagementConfig,
    RuleDownloader, SecretsStore,
};
use std::{future::Future, pin::Pin};
use tempfile::TempDir;
use tower::ServiceExt;
use url::Url;

#[derive(Clone)]
struct TestClock(u64);

impl Clock for TestClock {
    fn unix_seconds(&self) -> u64 {
        self.0
    }
}

struct FakeDownloader(Result<String, DownloadError>);

impl RuleDownloader for FakeDownloader {
    fn download<'a>(
        &'a self,
        _url: &'a Url,
        _timeout: Duration,
        _max_bytes: usize,
    ) -> Pin<Box<dyn Future<Output = Result<String, DownloadError>> + Send + 'a>> {
        Box::pin(async move {
            match &self.0 {
                Ok(yaml) => Ok(yaml.clone()),
                Err(DownloadError::TooLarge) => Err(DownloadError::TooLarge),
                Err(DownloadError::InvalidText) => Err(DownloadError::InvalidText),
                Err(DownloadError::Request) => Err(DownloadError::Request),
            }
        })
    }
}

fn fixture() -> (TempDir, ManagementConfig) {
    let dir = tempfile::tempdir().unwrap();
    let config = ManagementConfig {
        port: 9317,
        container: false,
        session_ttl: Duration::from_secs(60),
        secrets_file: dir.path().join("management.secrets.yaml"),
        active_rule_set_file: dir.path().join("active-rules.yaml"),
        rule_source_hosts: vec!["raw.githubusercontent.com".to_owned()],
        rule_download_timeout: Duration::from_secs(5),
        rule_download_max_bytes: 1024,
        database_file: dir.path().join("management.sqlite3"),
        artwork_cache_root: None,
        media_roots: Vec::new(),
        actor_view_root: None,
    };
    (dir, config)
}

#[tokio::test]
async fn layered_health_keeps_local_readiness_independent_from_jellyfin() {
    let (_dir, config) = fixture();
    let state = AppState::new(config, TestClock(100)).unwrap();

    let live = json_request(app(state.clone()), "GET", "/health/live", "", None).await;
    assert_eq!(live.status(), StatusCode::OK);
    let ready = json_request(app(state.clone()), "GET", "/health/ready", "", None).await;
    assert_eq!(ready.status(), StatusCode::OK);
    let jellyfin = json_request(app(state), "GET", "/health/jellyfin", "", None).await;
    assert_eq!(jellyfin.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(jellyfin.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(body["available"], false);
    assert_eq!(body["affects_local_readiness"], false);
}

#[tokio::test]
async fn actor_folder_api_lists_confirmation_and_removes_via_management_task() {
    let (dir, mut config) = fixture();
    let media = dir.path().join("media");
    let actors = dir.path().join("actors");
    std::fs::create_dir_all(media.join("ABC-123")).unwrap();
    std::fs::create_dir_all(actors.join("Alice/ABC-123")).unwrap();
    std::fs::write(media.join("ABC-123/ABC-123.mp4"), b"movie").unwrap();
    std::fs::write(
        media.join("ABC-123/ABC-123.nfo"),
        "<movie><title>Blue Room</title><actor><name>Alice</name></actor></movie>",
    )
    .unwrap();
    std::fs::hard_link(
        media.join("ABC-123/ABC-123.mp4"),
        actors.join("Alice/ABC-123/ABC-123.mp4"),
    )
    .unwrap();
    std::fs::hard_link(
        media.join("ABC-123/ABC-123.mp4"),
        actors.join("Alice/ABC-123/ABC-123-copy.mp4"),
    )
    .unwrap();
    std::fs::hard_link(
        media.join("ABC-123/ABC-123.nfo"),
        actors.join("Alice/ABC-123/ABC-123.nfo"),
    )
    .unwrap();
    config.media_roots.push(media.clone());
    config.actor_view_root = Some(actors.clone());
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let login = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/auth/login",
        r#"{"password":"a strong password"}"#,
        None,
    )
    .await;
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();

    let listed = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/actors",
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(listed.status(), StatusCode::OK);
    let folders: serde_json::Value =
        serde_json::from_slice(&to_bytes(listed.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(folders[0]["name"], "Alice");
    assert_eq!(folders[0]["movie_count"], 1);
    assert_eq!(folders[0]["derived_file_count"], 3);
    assert_eq!(folders[0]["unique_inode_count"], 2);
    assert_eq!(folders[0]["hard_link_count"], 3);
    assert_eq!(folders[0]["reclaimable_space"], 0);

    let detail_response = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/actors/Alice",
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail: serde_json::Value = serde_json::from_slice(
        &to_bytes(detail_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(detail["name"], "Alice");
    assert_eq!(detail["linked_assets"].as_array().unwrap().len(), 1);
    assert_eq!(detail["linked_assets"][0]["jav_code"], "ABC-123");

    let asset_id = detail["linked_assets"][0]["id"].as_str().unwrap();
    let asset_response = json_request(
        app(state.clone()),
        "GET",
        &format!("/api/v1/assets/{asset_id}"),
        "",
        Some(&cookie),
    )
    .await;
    let asset_detail: serde_json::Value = serde_json::from_slice(
        &to_bytes(asset_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        asset_detail["actors"][0]["actor_folder_url"],
        "/actors/QWxpY2U"
    );

    let missing = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/actors/Missing",
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let removal = json_request(
        app(state.clone()),
        "DELETE",
        "/api/v1/actors/Alice",
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(removal.status(), StatusCode::ACCEPTED);
    let task: serde_json::Value =
        serde_json::from_slice(&to_bytes(removal.into_body(), usize::MAX).await.unwrap()).unwrap();
    let id = task["id"].as_str().unwrap();
    for _ in 0..100 {
        let response = json_request(
            app(state.clone()),
            "GET",
            &format!("/api/v1/tasks/{id}"),
            "",
            Some(&cookie),
        )
        .await;
        let task: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        if task["status"] == "completed" {
            assert!(!actors.join("Alice").exists());
            assert!(media.join("ABC-123/ABC-123.mp4").exists());
            assert!(!task["items"].as_array().unwrap().is_empty());
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("actor removal task did not complete");
}

#[tokio::test]
async fn actor_folder_removal_rejects_links_without_an_indexed_media_asset() {
    let (dir, mut config) = fixture();
    let actors = dir.path().join("actors");
    let unindexed = dir.path().join("unindexed");
    std::fs::create_dir_all(actors.join("Alice/ABC-123")).unwrap();
    std::fs::create_dir_all(&unindexed).unwrap();
    std::fs::write(unindexed.join("ABC-123.mp4"), b"only unindexed source").unwrap();
    std::fs::hard_link(
        unindexed.join("ABC-123.mp4"),
        actors.join("Alice/ABC-123/ABC-123.mp4"),
    )
    .unwrap();
    config.actor_view_root = Some(actors.clone());
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let login = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/auth/login",
        r#"{"password":"a strong password"}"#,
        None,
    )
    .await;
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();

    let removal = json_request(
        app(state.clone()),
        "DELETE",
        "/api/v1/actors/Alice",
        "",
        Some(&cookie),
    )
    .await;

    assert_eq!(removal.status(), StatusCode::CONFLICT);
    assert!(actors.join("Alice/ABC-123/ABC-123.mp4").exists());
    assert!(unindexed.join("ABC-123.mp4").exists());
}

#[tokio::test]
async fn startup_scan_and_versioned_asset_search_expose_grouped_paginated_states() {
    let (dir, mut config) = fixture();
    let root = dir.path().join("media");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("ABC-123.mp4"), b"video").unwrap();
    std::fs::write(
        root.join("ABC-123.nfo"),
        "<movie><title>Blue Room</title></movie>",
    )
    .unwrap();
    config.media_roots.push(root);
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let login = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/auth/login",
        r#"{"password":"a strong password"}"#,
        None,
    )
    .await;
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();

    let response = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/assets?q=blue&state=normal&page=1&per_page=12",
        "",
        Some(cookie),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["jav_code"], "ABC-123");
    assert!(body["groups"].as_array().unwrap().len() == 1);
    let health = json_request(app(state), "GET", "/api/v1/assets/health", "", Some(cookie)).await;
    assert_eq!(health.status(), StatusCode::OK);
}

#[tokio::test]
async fn authenticated_asset_detail_api_exposes_nfo_and_rejects_anonymous_access() {
    let (dir, mut config) = fixture();
    let root = dir.path().join("media");
    let actor_root = dir.path().join("actors");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir_all(actor_root.join("miru/ABC-123")).unwrap();
    std::fs::write(root.join("ABC-123.mp4"), b"video").unwrap();
    std::fs::write(root.join("ABC-123.jpg"), b"poster").unwrap();
    std::fs::write(root.join("ABC-123.nfo"), r#"<movie><title>Blue Room</title><studio>Example</studio><actor><name>miru</name></actor><plot>Local plot</plot></movie>"#).unwrap();
    std::fs::hard_link(
        root.join("ABC-123.mp4"),
        actor_root.join("miru/ABC-123/ABC-123.mp4"),
    )
    .unwrap();
    config.media_roots.push(root);
    config.actor_view_root = Some(actor_root);
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let login = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/auth/login",
        r#"{"password":"a strong password"}"#,
        None,
    )
    .await;
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let listed = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/assets",
        "",
        Some(&cookie),
    )
    .await;
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(listed.into_body(), usize::MAX).await.unwrap()).unwrap();
    let id = body["items"][0]["id"].as_str().unwrap();
    let listed_artwork_url = body["items"][0]["artwork_url"].clone();
    let listed_captured_date = body["items"][0]["captured_date"].clone();

    let anonymous = json_request(
        app(state.clone()),
        "GET",
        &format!("/api/v1/assets/{id}"),
        "",
        None,
    )
    .await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    let response = json_request(
        app(state.clone()),
        "GET",
        &format!("/api/v1/assets/{id}"),
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let detail: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(detail["title"], "Blue Room");
    assert_eq!(detail["studio"], "Example");
    assert_eq!(detail["parse_status"], "valid");
    assert_eq!(detail["artwork_url"], listed_artwork_url);
    assert_eq!(detail["captured_date"], listed_captured_date);
    assert_eq!(detail["actors"][0]["name"], "miru");
    assert!(detail["actors"][0]["poster_url"].is_null());
    assert_eq!(detail["actors"][0]["actor_folder_url"], "/actors/bWlydQ");
    assert_eq!(detail["jellyfin"]["status"], "not_configured");
}

#[tokio::test]
async fn manual_and_incremental_scan_endpoints_reconcile_and_report_root_permissions() {
    let (dir, mut config) = fixture();
    let root = dir.path().join("media");
    std::fs::create_dir(&root).unwrap();
    config.media_roots.push(root.clone());
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let login = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/auth/login",
        r#"{"password":"a strong password"}"#,
        None,
    )
    .await;
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    std::fs::write(root.join("NEW-777.mkv"), b"video").unwrap();

    let scan = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/assets/scan",
        r#"{"mode":"manual"}"#,
        Some(&cookie),
    )
    .await;
    assert_eq!(scan.status(), StatusCode::OK);
    std::fs::remove_file(root.join("NEW-777.mkv")).unwrap();
    let incremental = serde_json::json!({"mode":"incremental","media_root":root,"paths":[root.join("NEW-777.mkv")]}).to_string();
    assert_eq!(
        json_request(
            app(state.clone()),
            "POST",
            "/api/v1/assets/scan",
            &incremental,
            Some(&cookie)
        )
        .await
        .status(),
        StatusCode::OK
    );
    let roots = json_request(
        app(state),
        "GET",
        "/api/v1/media-roots/health",
        "",
        Some(&cookie),
    )
    .await;
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(roots.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(body[0]["readable"], true);
    assert!(body[0]["uid"].is_number());
}

#[tokio::test]
async fn artwork_route_serves_only_the_artwork_bound_to_an_indexed_asset() {
    let (dir, mut config) = fixture();
    let root = dir.path().join("media");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("ART-101.mp4"), b"secret video").unwrap();
    std::fs::write(root.join("ART-101.jpg"), b"jpeg artwork").unwrap();
    config.media_roots.push(root);
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let login = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/auth/login",
        r#"{"password":"a strong password"}"#,
        None,
    )
    .await;
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let listed = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/assets",
        "",
        Some(&cookie),
    )
    .await;
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(listed.into_body(), usize::MAX).await.unwrap()).unwrap();
    let url = body["items"][0]["artwork_url"].as_str().unwrap();
    let artwork = json_request(app(state.clone()), "GET", url, "", Some(&cookie)).await;
    assert_eq!(
        to_bytes(artwork.into_body(), usize::MAX).await.unwrap(),
        "jpeg artwork"
    );
    assert_eq!(
        json_request(
            app(state),
            "GET",
            "/api/v1/assets/../../etc/passwd/artwork",
            "",
            Some(&cookie)
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

async fn authenticated_fixture() -> (TempDir, AppState, String) {
    let (dir, config) = fixture();
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let login = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/auth/login",
        r#"{"password":"a strong password"}"#,
        None,
    )
    .await;
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    (dir, state, cookie)
}

async fn login_cookie(state: &AppState) -> String {
    let login = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/auth/login",
        r#"{"password":"a strong password"}"#,
        None,
    )
    .await;
    login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn authenticated_administrator_can_view_validate_and_atomically_activate_yaml() {
    let (dir, state, cookie) = authenticated_fixture().await;
    let current = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/rules/active",
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(current.status(), StatusCode::OK);
    let body = to_bytes(current.into_body(), usize::MAX).await.unwrap();
    assert!(std::str::from_utf8(&body).unwrap().contains("version: 1"));

    let invalid = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/rules/validate",
        r#"{"yaml":"version: 1\nrules:\n  - enabled: true\n"}"#,
        Some(&cookie),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let overreaching = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/rules/validate",
        r#"{"yaml":"version: 1\nroots: ['/media']\ndelete: true\nrules: []\n"}"#,
        Some(&cookie),
    )
    .await;
    assert_eq!(overreaching.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let unchanged = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/rules/active",
        "",
        Some(&cookie),
    )
    .await;
    let unchanged_body = to_bytes(unchanged.into_body(), usize::MAX).await.unwrap();
    assert!(std::str::from_utf8(&unchanged_body)
        .unwrap()
        .contains("version: 1"));

    let replacement = "version: 1\nrules:\n  - pattern: '*.tracker'\n";
    let save = json_request(
        app(state.clone()),
        "PUT",
        "/api/v1/rules/active",
        &serde_json::json!({"yaml": replacement, "confirm_empty": false}).to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(save.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("active-rules.yaml")).unwrap(),
        replacement
    );

    let active = json_request(app(state), "GET", "/api/v1/rules/active", "", Some(&cookie)).await;
    let active_body = to_bytes(active.into_body(), usize::MAX).await.unwrap();
    assert!(std::str::from_utf8(&active_body)
        .unwrap()
        .contains("*.tracker"));
}

#[tokio::test]
async fn empty_activation_requires_separate_confirmation() {
    let (_dir, state, cookie) = authenticated_fixture().await;
    let yaml = "version: 1\nrules: []\n";
    let rejected = json_request(
        app(state.clone()),
        "PUT",
        "/api/v1/rules/active",
        &serde_json::json!({"yaml": yaml, "confirm_empty": false}).to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::CONFLICT);
    let accepted = json_request(
        app(state),
        "PUT",
        "/api/v1/rules/active",
        &serde_json::json!({"yaml": yaml, "confirm_empty": true}).to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn download_rejects_non_https_and_hosts_outside_allowlist_without_changing_active_rules() {
    let (_dir, state, cookie) = authenticated_fixture().await;
    for url in [
        "http://raw.githubusercontent.com/org/repo/main/rules.yaml",
        "https://example.com/rules.yaml",
    ] {
        let response = json_request(
            app(state.clone()),
            "POST",
            "/api/v1/rules/download",
            &serde_json::json!({"url": url}).to_string(),
            Some(&cookie),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let active = json_request(app(state), "GET", "/api/v1/rules/active", "", Some(&cookie)).await;
    assert_eq!(active.status(), StatusCode::OK);
}

#[tokio::test]
async fn successful_download_returns_only_a_proposal_and_failed_download_preserves_active_rules() {
    let (dir, config) = fixture();
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let proposed = "version: 1\nrules:\n  - pattern: '*.proposal'\n";
    let state = AppState::with_downloader(
        config.clone(),
        TestClock(100),
        FakeDownloader(Ok(proposed.to_owned())),
    )
    .unwrap();
    let cookie = login_cookie(&state).await;
    let downloaded = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/rules/download",
        r#"{"url":"https://raw.githubusercontent.com/acme/rules/main/rules.yaml"}"#,
        Some(&cookie),
    )
    .await;
    assert_eq!(downloaded.status(), StatusCode::OK);
    let body = to_bytes(downloaded.into_body(), usize::MAX).await.unwrap();
    assert!(std::str::from_utf8(&body).unwrap().contains("*.proposal"));
    assert!(!config.active_rule_set_file.exists());

    let failing = AppState::with_downloader(
        config,
        TestClock(100),
        FakeDownloader(Err(DownloadError::Request)),
    )
    .unwrap();
    let failing_cookie = login_cookie(&failing).await;
    let failure = json_request(
        app(failing.clone()),
        "POST",
        "/api/v1/rules/download",
        r#"{"url":"https://raw.githubusercontent.com/acme/rules/main/rules.yaml"}"#,
        Some(&failing_cookie),
    )
    .await;
    assert_eq!(failure.status(), StatusCode::BAD_GATEWAY);
    let active = json_request(
        app(failing),
        "GET",
        "/api/v1/rules/active",
        "",
        Some(&failing_cookie),
    )
    .await;
    let active_body = to_bytes(active.into_body(), usize::MAX).await.unwrap();
    assert!(!std::str::from_utf8(&active_body)
        .unwrap()
        .contains("*.proposal"));
    drop(dir);
}

#[tokio::test]
async fn oversized_download_is_rejected_and_rule_endpoints_never_expose_secrets() {
    let (_dir, config) = fixture();
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::with_downloader(
        config,
        TestClock(100),
        FakeDownloader(Err(DownloadError::TooLarge)),
    )
    .unwrap();
    let cookie = login_cookie(&state).await;
    let response = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/rules/download",
        r#"{"url":"https://raw.githubusercontent.com/acme/rules/main/rules.yaml"}"#,
        Some(&cookie),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let active = json_request(app(state), "GET", "/api/v1/rules/active", "", Some(&cookie)).await;
    let body = String::from_utf8(
        to_bytes(active.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(!body.contains("password"));
    assert!(!body.contains("secret"));
    assert!(!body.contains("root"));
}

async fn json_request(
    router: axum::Router,
    method: &str,
    uri: &str,
    json: &str,
    cookie: Option<&str>,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    router
        .oneshot(builder.body(Body::from(json.to_owned())).unwrap())
        .await
        .unwrap()
}

#[test]
fn native_and_container_listen_addresses_are_safe_by_default() {
    let (_dir, mut config) = fixture();
    assert_eq!(config.listen_addr().to_string(), "127.0.0.1:9317");
    config.container = true;
    assert_eq!(config.listen_addr().to_string(), "0.0.0.0:9317");
}

#[test]
fn unavailable_truenas_host_path_starts_with_degraded_index_instead_of_crashing_service() {
    let (dir, mut config) = fixture();
    config.media_roots.push(dir.path().join("not-mounted"));
    assert!(AppState::new(config, TestClock(100)).is_ok());
}

#[test]
fn yaml_loads_only_ordinary_settings_and_resolves_secrets_file_relative_to_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("management.yaml");
    std::fs::write(
        &path,
        "port: 8080\ncontainer: true\nsession_ttl_seconds: 90\nsecrets_file: private/admin.yaml\n",
    )
    .unwrap();

    let config = ManagementConfig::load(&path).unwrap();
    assert_eq!(config.listen_addr().to_string(), "0.0.0.0:8080");
    assert_eq!(config.session_ttl, Duration::from_secs(90));
    assert_eq!(config.secrets_file, dir.path().join("private/admin.yaml"));
}

#[cfg(unix)]
#[test]
fn secrets_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let (_dir, config) = fixture();
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let mode = std::fs::metadata(config.secrets_file)
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[tokio::test]
async fn management_api_is_unavailable_before_initialization() {
    let (_dir, config) = fixture();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let response = app(state)
        .oneshot(
            Request::builder()
                .uri("/api/v1/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn one_time_initialization_configures_exactly_one_administrator() {
    let (_dir, config) = fixture();
    let store = SecretsStore::new(config.secrets_file.clone());
    let token = init_administrator(&store).unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();

    let response = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/auth/initialize",
        &format!(r#"{{"token":"{token}","password":"correct horse battery staple"}}"#),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let replay = json_request(
        app(state),
        "POST",
        "/api/v1/auth/initialize",
        &format!(r#"{{"token":"{token}","password":"another strong password"}}"#),
        None,
    )
    .await;
    assert_eq!(replay.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn administrator_password_accepts_four_characters_and_rejects_three() {
    let (_short_dir, short_config) = fixture();
    let short_store = SecretsStore::new(short_config.secrets_file.clone());
    let short_token = init_administrator(&short_store).unwrap();
    let rejected = json_request(
        app(AppState::new(short_config, TestClock(100)).unwrap()),
        "POST",
        "/api/v1/auth/initialize",
        &format!(r#"{{"token":"{short_token}","password":"123"}}"#),
        None,
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);

    let (_valid_dir, valid_config) = fixture();
    let valid_store = SecretsStore::new(valid_config.secrets_file.clone());
    let valid_token = init_administrator(&valid_store).unwrap();
    let accepted = json_request(
        app(AppState::new(valid_config, TestClock(100)).unwrap()),
        "POST",
        "/api/v1/auth/initialize",
        &format!(r#"{{"token":"{valid_token}","password":"1234"}}"#),
        None,
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn login_issues_secure_session_and_authorizes_versioned_api() {
    let (_dir, config) = fixture();
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let login = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/auth/login",
        r#"{"password":"a strong password"}"#,
        None,
    )
    .await;
    assert_eq!(login.status(), StatusCode::NO_CONTENT);
    let set_cookie = login.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Strict"));
    let cookie = set_cookie.split(';').next().unwrap();

    let response = json_request(app(state), "GET", "/api/v1/status", "", Some(cookie)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(std::str::from_utf8(&body)
        .unwrap()
        .contains(env!("CARGO_PKG_VERSION")));
}

#[tokio::test]
async fn expired_session_is_unauthorized() {
    let (_dir, config) = fixture();
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let login_state = AppState::new(config.clone(), TestClock(100)).unwrap();
    let login = json_request(
        app(login_state.clone()),
        "POST",
        "/api/v1/auth/login",
        r#"{"password":"a strong password"}"#,
        None,
    )
    .await;
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    login_state.set_clock(TestClock(161));

    let response = json_request(app(login_state), "GET", "/api/v1/status", "", Some(&cookie)).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn local_password_reset_revokes_existing_sessions() {
    let (_dir, config) = fixture();
    let store = SecretsStore::new(config.secrets_file.clone());
    password_secrets(&store, "the original strong password").unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let login = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/auth/login",
        r#"{"password":"the original strong password"}"#,
        None,
    )
    .await;
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();

    password_secrets(&store, "the replacement strong password").unwrap();
    let response = json_request(app(state), "GET", "/api/v1/status", "", Some(&cookie)).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unauthenticated_requests_cannot_access_management_api() {
    let (_dir, config) = fixture();
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let response = json_request(app(state), "GET", "/api/v1/status", "", None).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unknown_non_api_routes_fall_back_to_embedded_react_shell() {
    let (_dir, config) = fixture();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let response = app(state)
        .oneshot(
            Request::builder()
                .uri("/settings/profile")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/html; charset=utf-8"
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(std::str::from_utf8(&body)
        .unwrap()
        .contains("<div id=\"root\"></div>"));
}

#[tokio::test]
async fn embedded_shell_busts_legacy_asset_cache_and_static_assets_revalidate() {
    let (_dir, config) = fixture();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let shell = app(state.clone())
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let shell_body = to_bytes(shell.into_body(), usize::MAX).await.unwrap();
    let shell_html = std::str::from_utf8(&shell_body).unwrap();
    assert!(shell_html.contains("/assets/app.js?v=2"));
    assert!(shell_html.contains("/assets/app.css?v=2"));

    let javascript = app(state)
        .oneshot(
            Request::builder()
                .uri("/assets/app.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(javascript.headers()[header::CACHE_CONTROL], "no-cache");
}

#[tokio::test]
async fn embedded_management_interface_exposes_task_creation_and_live_lifecycle() {
    let (_dir, config) = fixture();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let response = app(state)
        .oneshot(
            Request::builder()
                .uri("/assets/app.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let javascript = std::str::from_utf8(&body).unwrap();
    assert!(javascript.contains("Management Tasks"));
    assert!(javascript.contains("/api/v1/tasks"));
    assert!(javascript.contains("EventSource"));
    assert!(javascript.contains("Media Root"));
    assert!(javascript.contains("Operation"));
    assert!(javascript.contains("Preview"));
    assert!(javascript.contains("Confirm and execute"));
    assert!(javascript.contains("Preview 15-minute plan"));
    assert!(javascript.contains("Lifecycle"));
    assert!(javascript.contains("Refresh"));
    assert!(javascript.contains("item outcome"));
    assert!(javascript.contains("All Assets"));
    assert!(javascript.contains("搜索番号、标题或路径"));
    assert!(javascript.contains("/api/v1/assets"));
}

#[tokio::test]
async fn embedded_browser_shell_has_asset_search_state_filters_and_responsive_navigation() {
    let (_dir, config) = fixture();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let javascript = app(state.clone())
        .oneshot(
            Request::builder()
                .uri("/assets/app.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let javascript = to_bytes(javascript.into_body(), usize::MAX).await.unwrap();
    let javascript = std::str::from_utf8(&javascript).unwrap();
    assert!(javascript.contains("All Assets"));
    assert!(javascript.contains("/api/v1/assets"));
    assert!(javascript.contains("Synchronizing"));
    assert!(javascript.contains("Overview"));
    assert!(javascript.contains("NFO"));
    assert!(javascript.contains("Actor Folder"));
    assert!(javascript.contains("/api/v1/actors"));
    assert!(javascript.contains("Logical Size"));
    assert!(javascript.contains("Reclaimable Space"));
    assert!(javascript.contains("Remove via Management Task"));
    assert!(javascript.contains("/api/v1/assets/"));
    assert!(javascript.contains("dialog"));
    let css = app(state)
        .oneshot(
            Request::builder()
                .uri("/assets/app.css")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let css = to_bytes(css.into_body(), usize::MAX).await.unwrap();
    let css = std::str::from_utf8(&css).unwrap();
    assert!(css.contains("grid-template-columns:repeat(2"));
    assert!(css.contains("bottom-nav"));
    assert!(css.contains("sidebar"));
    assert!(css.contains("asset-inspector"));
    assert!(css.contains("aspect-ratio:2/3"));
    assert!(css.contains("actor-folder-grid"));
    assert!(css.contains("grid-template-columns:repeat(3"));
    assert!(css.contains("grid-template-columns:repeat(4"));
    assert!(css.contains("max-height:86vh"));
    assert!(css.contains("width:360px"));
}

#[tokio::test]
async fn management_ui_exposes_every_operation_full_pipeline_and_plan_confirmation() {
    let (_dir, config) = fixture();
    let javascript = app(AppState::new(config, TestClock(100)).unwrap())
        .oneshot(
            Request::builder()
                .uri("/assets/app.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let javascript = to_bytes(javascript.into_body(), usize::MAX).await.unwrap();
    let javascript = std::str::from_utf8(&javascript).unwrap();
    for label in [
        "Delete ad files",
        "Organize by code",
        "Clean empty directories",
        "Standardize names",
        "Extract codes",
        "Categorize files",
        "Move to ORIGIN",
        "Remove duplicates",
        "Full pipeline",
    ] {
        assert!(javascript.contains(label), "missing {label}");
    }
    assert!(javascript.contains("Review final paths"));
    assert!(javascript.contains("Confirm and execute"));
    assert!(javascript.contains("plan_id"));
    assert!(javascript.contains("confirmed"));
}

#[tokio::test]
async fn authenticated_versioned_api_creates_and_recovers_a_preview_task() {
    let (dir, state, cookie) = authenticated_fixture().await;
    std::fs::write(dir.path().join("新片首发每天更新.txt"), b"ad").unwrap();
    let request = serde_json::json!({
        "task_type": "operations",
        "media_root": dir.path(),
        "mode": "preview",
        "operations": ["delete_ad_files"]
    });
    let created = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &request.to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(created.status(), StatusCode::ACCEPTED);
    let created: serde_json::Value =
        serde_json::from_slice(&to_bytes(created.into_body(), usize::MAX).await.unwrap()).unwrap();
    let id = created["id"].as_str().unwrap();

    let mut final_task = None;
    for _ in 0..100 {
        let response = json_request(
            app(state.clone()),
            "GET",
            &format!("/api/v1/tasks/{id}"),
            "",
            Some(&cookie),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let task: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        if task["status"] == "completed" {
            final_task = Some(task);
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let task = final_task.expect("preview task should finish");
    assert_eq!(task["kind"], "preview");
    assert_eq!(task["media_root"], dir.path().display().to_string());
    assert_eq!(task["items"][0]["status"], "planned");
    assert!(dir.path().join("新片首发每天更新.txt").exists());
}

#[tokio::test]
async fn task_list_supports_compatible_limit_offset_and_total_count() {
    let (dir, state, cookie) = authenticated_fixture().await;
    let store = rust_jav::management_tasks::TaskStore::open(&dir.path().join("management.sqlite3"))
        .unwrap();
    for index in 0..75 {
        store
            .create(
                rust_jav::management_tasks::NewTask::preview(
                    "operations",
                    format!("/media/{index:03}"),
                ),
                1_000 + index,
            )
            .unwrap();
    }
    let first = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/tasks?limit=20&offset=0",
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(first.headers()["x-total-count"], "75");
    let first: serde_json::Value =
        serde_json::from_slice(&to_bytes(first.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(first.as_array().unwrap().len(), 20);
    let second = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/tasks?limit=20&offset=20",
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(second.headers()["x-total-count"], "75");
    let second: serde_json::Value =
        serde_json::from_slice(&to_bytes(second.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(second.as_array().unwrap().len(), 20);
    assert_ne!(first[0]["id"], second[0]["id"]);
    let active = json_request(
        app(state),
        "GET",
        "/api/v1/tasks?active=true",
        "",
        Some(&cookie),
    )
    .await;
    let active: serde_json::Value =
        serde_json::from_slice(&to_bytes(active.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(active.as_array().unwrap().len(), 75);
}

#[tokio::test]
async fn operation_preview_is_a_fifteen_minute_plan_in_canonical_pipeline_order() {
    let (dir, state, cookie) = authenticated_fixture().await;
    std::fs::write(dir.path().join("新片首发每天更新.txt"), b"ad").unwrap();
    let request = serde_json::json!({
        "task_type": "operations",
        "media_root": dir.path(),
        "mode": "preview",
        "operations": ["standardize_names", "delete_ad_files", "organize_by_code"]
    });
    let created = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &request.to_string(),
        Some(&cookie),
    )
    .await;
    let created: serde_json::Value =
        serde_json::from_slice(&to_bytes(created.into_body(), usize::MAX).await.unwrap()).unwrap();
    let id = created["id"].as_str().unwrap();
    let task = wait_for_task(&state, &cookie, id, "completed").await;

    assert_eq!(task["plan_expires_at"], 1_000);
    assert_eq!(task["operation_plan"]["operations"][0], "delete_ad_files");
    assert_eq!(task["operation_plan"]["operations"][1], "organize_by_code");
    assert_eq!(task["operation_plan"]["operations"][2], "standardize_names");
    assert_eq!(task["operation_plan"]["requires_confirmation"], true);
    assert!(task["operation_plan"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|action| action["destructive"] == true
            && action["path"]
                .as_str()
                .unwrap()
                .ends_with("新片首发每天更新.txt")));
}

#[tokio::test]
async fn mutation_requires_explicit_confirmation_of_an_unexpired_preview_plan() {
    let (dir, state, cookie) = authenticated_fixture().await;
    let source = dir.path().join("[7sht.me]@ABP-123.mp4");
    std::fs::write(&source, b"video").unwrap();
    let preview_request = serde_json::json!({"task_type":"operations","media_root":dir.path(),"mode":"preview","operations":["standardize_names"]});
    let preview = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &preview_request.to_string(),
        Some(&cookie),
    )
    .await;
    let preview: serde_json::Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    let plan_id = preview["id"].as_str().unwrap();
    let _ = wait_for_task(&state, &cookie, plan_id, "completed").await;

    let rejected = serde_json::json!({"task_type":"operations","mode":"apply","plan_id":plan_id,"confirmed":false});
    assert_eq!(
        json_request(
            app(state.clone()),
            "POST",
            "/api/v1/tasks",
            &rejected.to_string(),
            Some(&cookie)
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert!(source.exists());

    let confirmed = serde_json::json!({"task_type":"operations","mode":"apply","plan_id":plan_id,"confirmed":true});
    let mutation = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &confirmed.to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(mutation.status(), StatusCode::ACCEPTED);
    let mutation: serde_json::Value =
        serde_json::from_slice(&to_bytes(mutation.into_body(), usize::MAX).await.unwrap()).unwrap();
    let task = wait_for_task(
        &state,
        &cookie,
        mutation["id"].as_str().unwrap(),
        "completed",
    )
    .await;
    assert!(!source.exists());
    assert!(dir.path().join("ABP-123.mp4").exists());
    assert!(task["report"]["verification"].is_object());
}

#[tokio::test]
async fn confirmed_operation_plan_is_bound_consumed_once_and_executes_only_its_snapshot() {
    let (dir, state, cookie) = authenticated_fixture().await;
    let planned_source = dir.path().join("[7sht.me]@ABP-123.mp4");
    std::fs::write(&planned_source, b"planned").unwrap();
    let preview_request = serde_json::json!({
        "task_type": "operations",
        "media_root": dir.path(),
        "mode": "preview",
        "operations": ["standardize_names"]
    });
    let preview = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &preview_request.to_string(),
        Some(&cookie),
    )
    .await;
    let preview: serde_json::Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    let plan_id = preview["id"].as_str().unwrap();
    let _ = wait_for_task(&state, &cookie, plan_id, "completed").await;

    let unconfirmed_source = dir.path().join("[7sht.me]@NEW-456.mp4");
    std::fs::write(&unconfirmed_source, b"late arrival").unwrap();
    let confirmed = serde_json::json!({
        "task_type": "operations",
        "mode": "apply",
        "plan_id": plan_id,
        "confirmed": true
    });
    let mutation = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &confirmed.to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(mutation.status(), StatusCode::ACCEPTED);
    let mutation: serde_json::Value =
        serde_json::from_slice(&to_bytes(mutation.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(mutation["source_plan_id"], plan_id);
    let _ = wait_for_task(
        &state,
        &cookie,
        mutation["id"].as_str().unwrap(),
        "completed",
    )
    .await;

    assert!(!planned_source.exists());
    assert!(dir.path().join("ABP-123.mp4").exists());
    assert!(unconfirmed_source.exists());
    assert!(!dir.path().join("NEW-456.mp4").exists());

    let duplicate = json_request(
        app(state),
        "POST",
        "/api/v1/tasks",
        &confirmed.to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn confirmed_multi_operation_snapshot_tracks_paths_changed_by_earlier_actions() {
    let (dir, state, cookie) = authenticated_fixture().await;
    let source = dir.path().join("[7sht.me]@ABP-123-UC.mp4");
    std::fs::write(&source, b"video").unwrap();
    let preview_request = serde_json::json!({
        "task_type":"operations", "media_root":dir.path(), "mode":"preview",
        "operations":["categorize_files", "standardize_names"]
    });
    let preview = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &preview_request.to_string(),
        Some(&cookie),
    )
    .await;
    let preview: serde_json::Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    let plan_id = preview["id"].as_str().unwrap();
    let plan = wait_for_task(&state, &cookie, plan_id, "completed").await;
    assert_eq!(
        plan["operation_plan"]["operations"],
        serde_json::json!(["standardize_names", "categorize_files"])
    );

    let confirmed = serde_json::json!({"task_type":"operations", "mode":"apply", "plan_id":plan_id, "confirmed":true});
    let mutation = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &confirmed.to_string(),
        Some(&cookie),
    )
    .await;
    let mutation: serde_json::Value =
        serde_json::from_slice(&to_bytes(mutation.into_body(), usize::MAX).await.unwrap()).unwrap();
    let task = wait_for_task(
        &state,
        &cookie,
        mutation["id"].as_str().unwrap(),
        "completed",
    )
    .await;

    assert!(dir.path().join("UNCENSORED/ABP-123-UC.mp4").exists());
    assert!(!source.exists());
    assert_eq!(task["items"].as_array().unwrap().len(), 2);
    assert!(task["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["status"] == "applied"));
}

#[tokio::test]
async fn canonical_snapshot_generates_actions_that_only_exist_after_two_prior_stages() {
    let (dir, state, cookie) = authenticated_fixture().await;
    let incoming = dir.path().join("incoming");
    std::fs::create_dir(&incoming).unwrap();
    let source = incoming.join("[7sht.me]@ABP-123-UC.mp4");
    std::fs::write(&source, b"video").unwrap();
    let preview_request = serde_json::json!({
        "task_type":"operations", "media_root":dir.path(), "mode":"preview",
        "operations":["standardize_names", "clean_empty_dirs", "organize_by_code"]
    });
    let preview = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &preview_request.to_string(),
        Some(&cookie),
    )
    .await;
    let preview: serde_json::Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    let plan_id = preview["id"].as_str().unwrap();
    let plan = wait_for_task(&state, &cookie, plan_id, "completed").await;
    assert_eq!(
        plan["operation_plan"]["actions"].as_array().unwrap().len(),
        3
    );
    assert!(plan["operation_plan"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|action| action["kind"] == "delete-dir"
            && action["source"].as_str().unwrap().ends_with("/incoming")));

    let confirmed = serde_json::json!({"task_type":"operations","mode":"apply","plan_id":plan_id,"confirmed":true});
    let mutation = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &confirmed.to_string(),
        Some(&cookie),
    )
    .await;
    let mutation: serde_json::Value =
        serde_json::from_slice(&to_bytes(mutation.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(mutation["planned_item_count"], 3);
    let task = wait_for_task(
        &state,
        &cookie,
        mutation["id"].as_str().unwrap(),
        "completed",
    )
    .await;
    assert!(!incoming.exists());
    assert!(dir.path().join("ABP-123/ABP-123-UC.mp4").exists());
    assert_eq!(task["items"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn confirmed_snapshot_rejects_a_source_replaced_at_the_same_path() {
    let (dir, state, cookie) = authenticated_fixture().await;
    let source = dir.path().join("新片首发每天更新.txt");
    std::fs::write(&source, b"original").unwrap();
    let preview_request = serde_json::json!({"task_type":"operations","media_root":dir.path(),"mode":"preview","operations":["delete_ad_files"]});
    let preview = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &preview_request.to_string(),
        Some(&cookie),
    )
    .await;
    let preview: serde_json::Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    let plan_id = preview["id"].as_str().unwrap();
    let plan = wait_for_task(&state, &cookie, plan_id, "completed").await;
    assert!(plan["operation_plan"]["actions"][0]["source_identity"]["inode"].is_number());
    std::fs::remove_file(&source).unwrap();
    std::fs::write(&source, b"replacement").unwrap();

    let confirmed = serde_json::json!({"task_type":"operations","mode":"apply","plan_id":plan_id,"confirmed":true});
    let mutation = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &confirmed.to_string(),
        Some(&cookie),
    )
    .await;
    let mutation: serde_json::Value =
        serde_json::from_slice(&to_bytes(mutation.into_body(), usize::MAX).await.unwrap()).unwrap();
    let task = wait_for_task(&state, &cookie, mutation["id"].as_str().unwrap(), "failed").await;
    assert!(source.exists());
    assert_eq!(std::fs::read(&source).unwrap(), b"replacement");
    assert!(task["items"][0]["message"]
        .as_str()
        .unwrap()
        .contains("identity"));
}

#[tokio::test]
async fn confirmed_snapshot_rejects_a_symlink_escape_in_the_target_parent_chain() {
    use std::os::unix::fs::symlink;
    let (dir, state, cookie) = authenticated_fixture().await;
    let source = dir.path().join("ABP-123-UC.mp4");
    std::fs::write(&source, b"video").unwrap();
    let preview_request = serde_json::json!({"task_type":"operations","media_root":dir.path(),"mode":"preview","operations":["categorize_files"]});
    let preview = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &preview_request.to_string(),
        Some(&cookie),
    )
    .await;
    let preview: serde_json::Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    let plan_id = preview["id"].as_str().unwrap();
    let _ = wait_for_task(&state, &cookie, plan_id, "completed").await;
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), dir.path().join("UNCENSORED")).unwrap();

    let confirmed = serde_json::json!({"task_type":"operations","mode":"apply","plan_id":plan_id,"confirmed":true});
    let mutation = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &confirmed.to_string(),
        Some(&cookie),
    )
    .await;
    let mutation: serde_json::Value =
        serde_json::from_slice(&to_bytes(mutation.into_body(), usize::MAX).await.unwrap()).unwrap();
    let task = wait_for_task(&state, &cookie, mutation["id"].as_str().unwrap(), "failed").await;
    assert!(source.exists());
    assert!(!outside.path().join("ABP-123-UC.mp4").exists());
    assert!(task["items"][0]["message"]
        .as_str()
        .unwrap()
        .contains("symlink"));
}

#[tokio::test]
async fn confirmed_snapshot_stops_after_outcome_persistence_fails() {
    let (dir, state, cookie) = authenticated_fixture().await;
    let sources = [
        dir.path().join("[7sht.me]@AAA-111.mp4"),
        dir.path().join("[7sht.me]@BBB-222.mp4"),
    ];
    for source in &sources {
        std::fs::write(source, b"video").unwrap();
    }
    let preview_request = serde_json::json!({"task_type":"operations","media_root":dir.path(),"mode":"preview","operations":["standardize_names"]});
    let preview = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &preview_request.to_string(),
        Some(&cookie),
    )
    .await;
    let preview: serde_json::Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    let plan_id = preview["id"].as_str().unwrap();
    let _ = wait_for_task(&state, &cookie, plan_id, "completed").await;
    let connection = rusqlite::Connection::open(dir.path().join("management.sqlite3")).unwrap();
    connection.execute_batch("CREATE TRIGGER injected_item_failure BEFORE UPDATE OF status ON management_task_items WHEN OLD.status = 'running' BEGIN SELECT RAISE(FAIL, 'injected outcome persistence failure'); END;").unwrap();
    drop(connection);

    let confirmed = serde_json::json!({"task_type":"operations","mode":"apply","plan_id":plan_id,"confirmed":true});
    let mutation = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &confirmed.to_string(),
        Some(&cookie),
    )
    .await;
    let mutation: serde_json::Value =
        serde_json::from_slice(&to_bytes(mutation.into_body(), usize::MAX).await.unwrap()).unwrap();
    let task = wait_for_task(&state, &cookie, mutation["id"].as_str().unwrap(), "failed").await;
    let originals = sources.iter().filter(|path| path.exists()).count();
    let renamed = [
        dir.path().join("AAA-111.mp4"),
        dir.path().join("BBB-222.mp4"),
    ]
    .iter()
    .filter(|path| path.exists())
    .count();
    assert_eq!((originals, renamed), (1, 1));
    assert_eq!(task["items"].as_array().unwrap().len(), 1);
    assert_eq!(task["items"][0]["status"], "running");
    assert!(task["error"].as_str().unwrap().contains("persist"));
}

#[test]
fn startup_recovers_a_durable_running_quarantine_and_marks_it_interrupted() {
    let (dir, mut config) = fixture();
    let media_root = dir.path().join("media");
    std::fs::create_dir(&media_root).unwrap();
    config.media_roots.push(media_root.clone());
    let source = media_root.join("captured.mp4");
    std::fs::write(&source, b"approved").unwrap();
    let store = rust_jav::management_tasks::TaskStore::open(&config.database_file).unwrap();
    let task = store
        .create(
            rust_jav::management_tasks::NewTask::mutation(
                "operations",
                media_root.display().to_string(),
            ),
            100,
        )
        .unwrap();
    store.mark_running(&task.id, 101).unwrap();
    let journal = store
        .start_item(
            &task.id,
            "delete-file",
            Some(source.to_str().unwrap()),
            Some(source.to_str().unwrap()),
        )
        .unwrap();
    let quarantine = media_root.join(&journal.quarantine_token);
    std::fs::rename(&source, &quarantine).unwrap();
    drop(store);

    let _state = AppState::new(config.clone(), TestClock(200)).unwrap();
    let reopened = rust_jav::management_tasks::TaskStore::open(&config.database_file).unwrap();
    let recovered = reopened.get(&task.id).unwrap().unwrap();
    assert_eq!(
        recovered.status,
        rust_jav::management_tasks::TaskStatus::Interrupted
    );
    assert_eq!(recovered.items[0].status, "interrupted");
    assert!(recovered.items[0]
        .message
        .as_deref()
        .unwrap()
        .contains("restored"));
    assert_eq!(std::fs::read(&source).unwrap(), b"approved");
    assert!(!quarantine.exists());
}

#[test]
fn startup_recovers_a_permanent_deletion_quarantine_from_its_source_root() {
    let (dir, mut config) = fixture();
    let media_root = dir.path().join("media");
    let actor_root = dir.path().join("actors");
    std::fs::create_dir(&media_root).unwrap();
    std::fs::create_dir(&actor_root).unwrap();
    config.media_roots.push(media_root.clone());
    config.actor_view_root = Some(actor_root.clone());
    let source = actor_root.join("captured-by-deletion.mp4");
    std::fs::write(&source, b"approved").unwrap();
    let store = rust_jav::management_tasks::TaskStore::open(&config.database_file).unwrap();
    let task = store
        .create(
            rust_jav::management_tasks::NewTask::mutation(
                "permanent_deletion",
                format!("{},{}", media_root.display(), actor_root.display()),
            ),
            100,
        )
        .unwrap();
    store.mark_running(&task.id, 101).unwrap();
    let journal = store
        .start_item(
            &task.id,
            "permanent_deletion",
            Some(source.to_str().unwrap()),
            Some(source.to_str().unwrap()),
        )
        .unwrap();
    let quarantine = actor_root.join(&journal.quarantine_token);
    std::fs::rename(&source, &quarantine).unwrap();
    drop(store);

    let _state = AppState::new(config.clone(), TestClock(200)).unwrap();
    let reopened = rust_jav::management_tasks::TaskStore::open(&config.database_file).unwrap();
    let recovered = reopened.get(&task.id).unwrap().unwrap();
    assert_eq!(
        recovered.status,
        rust_jav::management_tasks::TaskStatus::Interrupted
    );
    assert_eq!(recovered.items[0].status, "interrupted");
    assert!(recovered.items[0]
        .message
        .as_deref()
        .unwrap()
        .contains("restored"));
    assert_eq!(std::fs::read(&source).unwrap(), b"approved");
    assert!(!quarantine.exists());
}

#[test]
fn startup_retains_an_occupied_directory_and_its_deletion_quarantine_locator() {
    let (dir, mut config) = fixture();
    let media_root = dir.path().join("media");
    std::fs::create_dir(&media_root).unwrap();
    config.media_roots.push(media_root.clone());
    let source = media_root.join("approved-directory");
    std::fs::create_dir(&source).unwrap();
    let store = rust_jav::management_tasks::TaskStore::open(&config.database_file).unwrap();
    let task = store
        .create(
            rust_jav::management_tasks::NewTask::mutation(
                "permanent_deletion",
                media_root.display().to_string(),
            ),
            100,
        )
        .unwrap();
    store.mark_running(&task.id, 101).unwrap();
    let journal = store
        .start_item(
            &task.id,
            "permanent_deletion",
            Some(source.to_str().unwrap()),
            Some(source.to_str().unwrap()),
        )
        .unwrap();
    let quarantine = media_root.join(&journal.quarantine_token);
    std::fs::rename(&source, &quarantine).unwrap();
    std::fs::write(quarantine.join("arrived-after-capture.txt"), b"new").unwrap();
    std::fs::create_dir(&source).unwrap();
    drop(store);

    let _state = AppState::new(config.clone(), TestClock(200)).unwrap();
    let reopened = rust_jav::management_tasks::TaskStore::open(&config.database_file).unwrap();
    let recovered = reopened.get(&task.id).unwrap().unwrap();
    assert_eq!(
        recovered.status,
        rust_jav::management_tasks::TaskStatus::Interrupted
    );
    let message = recovered.items[0].message.as_deref().unwrap();
    assert!(message.contains("source is occupied"));
    assert!(message.contains(&journal.quarantine_token));
    assert!(source.is_dir());
    assert!(quarantine.join("arrived-after-capture.txt").exists());
}

async fn wait_for_task(
    state: &AppState,
    cookie: &str,
    id: &str,
    status: &str,
) -> serde_json::Value {
    for _ in 0..200 {
        let response = json_request(
            app(state.clone()),
            "GET",
            &format!("/api/v1/tasks/{id}"),
            "",
            Some(cookie),
        )
        .await;
        let task: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        if task["status"] == status {
            return task;
        }
        if matches!(task["status"].as_str(), Some("failed" | "interrupted")) {
            panic!("task ended unexpectedly: {task}");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("task did not reach {status}")
}

#[tokio::test]
async fn sse_reconnect_to_completed_task_emits_recoverable_final_snapshot() {
    let (dir, state, cookie) = authenticated_fixture().await;
    let store = rust_jav::management_tasks::TaskStore::open(&dir.path().join("management.sqlite3"))
        .unwrap();
    let task = store
        .create(
            rust_jav::management_tasks::NewTask::preview("operations", "/media/a"),
            100,
        )
        .unwrap();
    store.mark_running(&task.id, 101).unwrap();
    store.mark_completed(&task.id, 102).unwrap();

    let response = json_request(
        app(state),
        "GET",
        &format!("/api/v1/tasks/{}/events", task.id),
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/event-stream"
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let event = std::str::from_utf8(&body).unwrap();
    assert!(event.contains("event: task"));
    assert!(event.contains("\"status\":\"completed\""));
    assert!(event.contains(&task.id));
}

#[tokio::test]
async fn generated_openapi_describes_task_rest_and_sse_contracts() {
    let (_dir, state, cookie) = authenticated_fixture().await;
    let response = json_request(app(state), "GET", "/api/v1/openapi.json", "", Some(&cookie)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let document: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(document["openapi"], "3.1.0");
    assert!(document["paths"]["/api/v1/tasks"]["post"].is_object());
    assert_eq!(
        document["paths"]["/api/v1/tasks"]["post"]["requestBody"]["content"]["application/json"]
            ["schema"]["$ref"],
        "#/components/schemas/CreateTaskRequest"
    );
    assert!(document["paths"]["/api/v1/tasks/{task_id}"]["get"].is_object());
    assert_eq!(
        document["paths"]["/api/v1/tasks/{task_id}/events"]["get"]["responses"]["200"]["content"]
            ["text/event-stream"]["schema"]["type"],
        "string"
    );
    assert_eq!(
        document["components"]["schemas"]["ManagementTask"]["required"][0],
        "id"
    );
    assert!(document["paths"]["/api/v1/assets"]["get"].is_object());
    assert!(document["paths"]["/api/v1/assets/{asset_id}"]["get"].is_object());
    assert_eq!(
        document["components"]["schemas"]["AssetDetail"]["properties"]["parse_status"]["enum"][0],
        "valid"
    );
    assert_eq!(
        document["components"]["schemas"]["AssetDetail"]["properties"]["jellyfin"]["$ref"],
        "#/components/schemas/JellyfinAssociation"
    );
    assert!(document["components"]["schemas"]["AssetDetail"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field == "jellyfin"));
    assert_eq!(
        document["components"]["schemas"]["JellyfinAssociation"]["properties"]["status"]["enum"][5],
        "not_configured"
    );
    assert_eq!(
        document["components"]["schemas"]["JellyfinAssociation"]["properties"]
            ["playback_position_ticks"]["type"][0],
        "integer"
    );
    assert!(document["paths"]["/api/v1/assets/scan"]["post"].is_object());
    assert!(document["paths"]["/api/v1/assets/{asset_id}/artwork"]["get"].is_object());
    assert_eq!(
        document["components"]["schemas"]["MediaAsset"]["properties"]["state"]["enum"][2],
        "exception"
    );
    assert_eq!(
        document["components"]["schemas"]["ActorFolder"]["properties"]["linked_assets"]["items"]
            ["$ref"],
        "#/components/schemas/MediaAsset"
    );
}

#[tokio::test]
async fn authenticated_candidate_plan_executes_as_durable_task_and_keeps_audit() {
    let (dir, mut config) = fixture();
    let root = dir.path().join("media");
    std::fs::create_dir(&root).unwrap();
    let selected = root.join("delete-me.mp4");
    let related = root.join("related-copy.mp4");
    std::fs::write(&selected, b"video").unwrap();
    std::fs::hard_link(&selected, &related).unwrap();
    std::fs::write(
        &config.active_rule_set_file,
        "version: 1\nrules:\n  - pattern: 'delete-*'\n",
    )
    .unwrap();
    config.media_roots.push(root);
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let cookie = login_cookie(&state).await;

    let candidates = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/deletion-candidates",
        "",
        Some(&cookie),
    )
    .await;
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(candidates.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert_eq!(body["items"][0]["matching_rule"], "delete-*");
    assert_eq!(body["items"][0]["type"], "file");
    assert!(body["items"][0]["video_warning"].is_string());
    assert_eq!(body["items"][0]["logical_size"], 5);

    let request = serde_json::json!({"paths":[selected],"selection":"unified"}).to_string();
    let planned = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/deletion-plans",
        &request,
        Some(&cookie),
    )
    .await;
    assert_eq!(planned.status(), StatusCode::CREATED);
    let plan: serde_json::Value =
        serde_json::from_slice(&to_bytes(planned.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(plan["paths"].as_array().unwrap().len(), 2);
    assert!(plan["reclaimable_space"].as_u64().unwrap() > 0);
    let endpoint = format!(
        "/api/v1/deletion-plans/{}/execute",
        plan["id"].as_str().unwrap()
    );
    let executed = json_request(
        app(state.clone()),
        "POST",
        &endpoint,
        r#"{"irreversible":true,"confirmation":"PERMANENTLY DELETE"}"#,
        Some(&cookie),
    )
    .await;
    assert_eq!(executed.status(), StatusCode::ACCEPTED);
    let task: serde_json::Value =
        serde_json::from_slice(&to_bytes(executed.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(task["task_type"], "permanent_deletion");
    assert_eq!(task["items"].as_array().unwrap().len(), 2);
    assert!(!selected.exists() && !related.exists());
    let audits = json_request(
        app(state),
        "GET",
        "/api/v1/deletion-audits",
        "",
        Some(&cookie),
    )
    .await;
    let records: serde_json::Value =
        serde_json::from_slice(&to_bytes(audits.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(records[0]["administrator"], "Administrator");
    assert_eq!(records[0]["rolled_back"], false);
    assert_eq!(
        records[0]["operation_plan"]["paths"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn deletion_plan_searches_actor_view_without_making_it_a_client_approved_root() {
    let (dir, mut config) = fixture();
    let media = dir.path().join("media");
    let actors = dir.path().join("actors");
    let actor_folder = actors.join("Alice Aoki");
    std::fs::create_dir(&media).unwrap();
    std::fs::create_dir_all(&actor_folder).unwrap();
    let selected = media.join("delete-me.mp4");
    let actor_link = actor_folder.join("delete-me.mp4");
    std::fs::write(&selected, b"video").unwrap();
    std::fs::hard_link(&selected, &actor_link).unwrap();
    std::fs::write(
        &config.active_rule_set_file,
        "version: 1\nrules:\n  - pattern: 'delete-*'\n",
    )
    .unwrap();
    config.media_roots.push(media.clone());
    config.actor_view_root = Some(actors.clone());
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let cookie = login_cookie(&state).await;

    let selected_request = serde_json::json!({
        "paths": [selected],
        "selection": "selected"
    });
    let selected_plan = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/deletion-plans",
        &selected_request.to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(selected_plan.status(), StatusCode::CREATED);
    let selected_plan: serde_json::Value = serde_json::from_slice(
        &to_bytes(selected_plan.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(selected_plan["paths"].as_array().unwrap().len(), 1);
    assert_eq!(
        selected_plan["discovered_hard_links"][0]["path"],
        serde_json::json!(actor_link)
    );
    assert_eq!(selected_plan["reclaimable_space"], 0);
    assert!(selected_plan["hard_link_search_roots"]
        .as_array()
        .unwrap()
        .iter()
        .any(|root| root == &serde_json::json!(actors)));

    let direct_actor = serde_json::json!({
        "paths": [actor_link],
        "selection": "selected"
    });
    let rejected = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/deletion-plans",
        &direct_actor.to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let unified_request = serde_json::json!({
        "paths": [selected],
        "selection": "unified"
    });
    let unified = json_request(
        app(state),
        "POST",
        "/api/v1/deletion-plans",
        &unified_request.to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(unified.status(), StatusCode::CREATED);
    let unified: serde_json::Value =
        serde_json::from_slice(&to_bytes(unified.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(unified["paths"].as_array().unwrap().len(), 2);
    assert!(unified["reclaimable_space"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn deletion_api_rejects_expired_plan_and_reports_replaced_file_as_partial() {
    let (dir, mut config) = fixture();
    let root = dir.path().join("media");
    std::fs::create_dir(&root).unwrap();
    let replaced = root.join("delete-replaced.txt");
    let deletable = root.join("delete-ok.txt");
    std::fs::write(&replaced, b"old").unwrap();
    std::fs::write(&deletable, b"ok").unwrap();
    std::fs::write(
        &config.active_rule_set_file,
        "version: 1\nrules:\n  - pattern: 'delete-*'\n",
    )
    .unwrap();
    config.media_roots.push(root);
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let cookie = login_cookie(&state).await;

    let create = |paths: Vec<&std::path::Path>| {
        serde_json::json!({"paths":paths,"selection":"selected"}).to_string()
    };
    let planned = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/deletion-plans",
        &create(vec![&replaced, &deletable]),
        Some(&cookie),
    )
    .await;
    let plan: serde_json::Value =
        serde_json::from_slice(&to_bytes(planned.into_body(), usize::MAX).await.unwrap()).unwrap();
    std::fs::remove_file(&replaced).unwrap();
    std::fs::write(&replaced, b"replacement").unwrap();
    let endpoint = format!(
        "/api/v1/deletion-plans/{}/execute",
        plan["id"].as_str().unwrap()
    );
    let response = json_request(
        app(state.clone()),
        "POST",
        &endpoint,
        r#"{"irreversible":true,"confirmation":"PERMANENTLY DELETE"}"#,
        Some(&cookie),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let task: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(task["status"], "failed");
    assert!(task["error"].as_str().unwrap().contains("partial"));
    let statuses = task["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["status"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(statuses.contains(&"changed") && statuses.contains(&"deleted"));
    assert!(replaced.exists() && !deletable.exists());

    let expiring = replaced.clone();
    let planned = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/deletion-plans",
        &create(vec![&expiring]),
        Some(&cookie),
    )
    .await;
    let plan: serde_json::Value =
        serde_json::from_slice(&to_bytes(planned.into_body(), usize::MAX).await.unwrap()).unwrap();
    state.set_clock(TestClock(701));
    let cookie = login_cookie(&state).await;
    let endpoint = format!(
        "/api/v1/deletion-plans/{}/execute",
        plan["id"].as_str().unwrap()
    );
    let expired = json_request(
        app(state),
        "POST",
        &endpoint,
        r#"{"irreversible":true,"confirmation":"PERMANENTLY DELETE"}"#,
        Some(&cookie),
    )
    .await;
    assert_eq!(expired.status(), StatusCode::CONFLICT);
    assert!(replaced.exists());
}

#[tokio::test]
async fn deletion_audit_persistence_failure_returns_a_durable_failed_task_with_real_items() {
    let (dir, mut config) = fixture();
    let root = dir.path().join("media");
    std::fs::create_dir(&root).unwrap();
    let selected = root.join("delete-audit-failure.mp4");
    std::fs::write(&selected, b"video").unwrap();
    std::fs::write(
        &config.active_rule_set_file,
        "version: 1\nrules:\n  - pattern: 'delete-*'\n",
    )
    .unwrap();
    config.media_roots.push(root);
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let cookie = login_cookie(&state).await;
    let planned = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/deletion-plans",
        &serde_json::json!({"paths":[selected],"selection":"selected"}).to_string(),
        Some(&cookie),
    )
    .await;
    let plan: serde_json::Value =
        serde_json::from_slice(&to_bytes(planned.into_body(), usize::MAX).await.unwrap()).unwrap();
    let connection = rusqlite::Connection::open(dir.path().join("management.sqlite3")).unwrap();
    connection.execute_batch(
        "CREATE TRIGGER injected_deletion_audit_failure BEFORE INSERT ON deletion_audit_records BEGIN SELECT RAISE(FAIL, 'injected deletion audit persistence failure'); END;",
    ).unwrap();
    drop(connection);

    let executed = json_request(
        app(state.clone()),
        "POST",
        &format!(
            "/api/v1/deletion-plans/{}/execute",
            plan["id"].as_str().unwrap()
        ),
        r#"{"irreversible":true,"confirmation":"PERMANENTLY DELETE"}"#,
        Some(&cookie),
    )
    .await;
    assert_eq!(executed.status(), StatusCode::ACCEPTED);
    let task: serde_json::Value =
        serde_json::from_slice(&to_bytes(executed.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(task["status"], "failed");
    assert!(task["error"].as_str().unwrap().contains("audit"));
    assert_eq!(task["items"].as_array().unwrap().len(), 1);
    assert_eq!(task["items"][0]["status"], "deleted");
    assert!(!selected.exists());

    let durable = json_request(
        app(state),
        "GET",
        &format!("/api/v1/tasks/{}", task["id"].as_str().unwrap()),
        "",
        Some(&cookie),
    )
    .await;
    let durable: serde_json::Value =
        serde_json::from_slice(&to_bytes(durable.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(durable["status"], "failed");
    assert_eq!(durable["items"][0]["status"], "deleted");
}

#[tokio::test]
async fn deletion_journal_creation_failure_returns_5xx_before_filesystem_mutation() {
    let (dir, mut config) = fixture();
    let root = dir.path().join("media");
    std::fs::create_dir(&root).unwrap();
    let selected = root.join("delete-outcome-failure.mp4");
    std::fs::write(&selected, b"video").unwrap();
    std::fs::write(
        &config.active_rule_set_file,
        "version: 1\nrules:\n  - pattern: 'delete-*'\n",
    )
    .unwrap();
    config.media_roots.push(root);
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let cookie = login_cookie(&state).await;
    let planned = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/deletion-plans",
        &serde_json::json!({"paths":[selected],"selection":"selected"}).to_string(),
        Some(&cookie),
    )
    .await;
    let plan: serde_json::Value =
        serde_json::from_slice(&to_bytes(planned.into_body(), usize::MAX).await.unwrap()).unwrap();
    let connection = rusqlite::Connection::open(dir.path().join("management.sqlite3")).unwrap();
    connection.execute_batch("CREATE TRIGGER injected_deletion_outcome_failure BEFORE INSERT ON management_task_items WHEN NEW.kind = 'permanent_deletion' BEGIN SELECT RAISE(FAIL, 'injected deletion outcome persistence failure'); END;").unwrap();
    drop(connection);

    let executed = json_request(
        app(state.clone()),
        "POST",
        &format!(
            "/api/v1/deletion-plans/{}/execute",
            plan["id"].as_str().unwrap()
        ),
        r#"{"irreversible":true,"confirmation":"PERMANENTLY DELETE"}"#,
        Some(&cookie),
    )
    .await;
    assert_eq!(executed.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let error = to_bytes(executed.into_body(), usize::MAX).await.unwrap();
    assert!(std::str::from_utf8(&error)
        .unwrap()
        .contains("durable deletion journal"));
    assert!(selected.exists());
    let store = rust_jav::management_tasks::TaskStore::open(&dir.path().join("management.sqlite3"))
        .unwrap();
    let task = store
        .list()
        .unwrap()
        .into_iter()
        .find(|task| task.task_type == "permanent_deletion")
        .unwrap();
    assert_eq!(task.status, rust_jav::management_tasks::TaskStatus::Running);
    assert!(task.items.is_empty());
    let audits = json_request(
        app(state),
        "GET",
        "/api/v1/deletion-audits",
        "",
        Some(&cookie),
    )
    .await;
    let audits: serde_json::Value =
        serde_json::from_slice(&to_bytes(audits.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert!(audits.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn deletion_terminal_status_persistence_failure_falls_back_to_failed_with_real_items() {
    let (dir, mut config) = fixture();
    let root = dir.path().join("media");
    std::fs::create_dir(&root).unwrap();
    let selected = root.join("delete-terminal-failure.mp4");
    std::fs::write(&selected, b"video").unwrap();
    std::fs::write(
        &config.active_rule_set_file,
        "version: 1\nrules:\n  - pattern: 'delete-*'\n",
    )
    .unwrap();
    config.media_roots.push(root);
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let cookie = login_cookie(&state).await;
    let planned = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/deletion-plans",
        &serde_json::json!({"paths":[selected],"selection":"selected"}).to_string(),
        Some(&cookie),
    )
    .await;
    let plan: serde_json::Value =
        serde_json::from_slice(&to_bytes(planned.into_body(), usize::MAX).await.unwrap()).unwrap();
    let connection = rusqlite::Connection::open(dir.path().join("management.sqlite3")).unwrap();
    connection.execute_batch("CREATE TRIGGER injected_deletion_completed_failure BEFORE UPDATE OF status ON management_tasks WHEN NEW.status = 'completed' AND OLD.task_type = 'permanent_deletion' BEGIN SELECT RAISE(FAIL, 'injected deletion terminal persistence failure'); END;").unwrap();
    drop(connection);

    let executed = json_request(
        app(state),
        "POST",
        &format!(
            "/api/v1/deletion-plans/{}/execute",
            plan["id"].as_str().unwrap()
        ),
        r#"{"irreversible":true,"confirmation":"PERMANENTLY DELETE"}"#,
        Some(&cookie),
    )
    .await;
    assert_eq!(executed.status(), StatusCode::ACCEPTED);
    let task: serde_json::Value =
        serde_json::from_slice(&to_bytes(executed.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(task["status"], "failed");
    assert!(task["error"].as_str().unwrap().contains("terminal status"));
    assert_eq!(task["items"][0]["status"], "deleted");
    assert!(!selected.exists());
}

#[tokio::test]
async fn deletion_journal_failure_stops_later_paths_returns_5xx_and_recovers_on_restart() {
    let (dir, mut config) = fixture();
    let root = dir.path().join("media");
    std::fs::create_dir(&root).unwrap();
    let first = root.join("delete-first.mp4");
    let second = root.join("delete-second.mp4");
    std::fs::write(&first, b"first").unwrap();
    std::fs::write(&second, b"second").unwrap();
    std::fs::write(
        &config.active_rule_set_file,
        "version: 1\nrules:\n  - pattern: 'delete-*'\n",
    )
    .unwrap();
    config.media_roots.push(root);
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let restart_config = config.clone();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let cookie = login_cookie(&state).await;
    let planned = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/deletion-plans",
        &serde_json::json!({"paths":[first, second],"selection":"selected"}).to_string(),
        Some(&cookie),
    )
    .await;
    let plan: serde_json::Value =
        serde_json::from_slice(&to_bytes(planned.into_body(), usize::MAX).await.unwrap()).unwrap();
    let connection = rusqlite::Connection::open(dir.path().join("management.sqlite3")).unwrap();
    connection.execute_batch(
        "CREATE TRIGGER injected_deletion_item_update_failure BEFORE UPDATE OF status ON management_task_items WHEN OLD.status = 'running' AND OLD.kind = 'permanent_deletion' BEGIN SELECT RAISE(FAIL, 'injected deletion item update failure'); END;
         CREATE TRIGGER injected_deletion_audit_failure_all BEFORE INSERT ON deletion_audit_records BEGIN SELECT RAISE(FAIL, 'injected deletion audit failure'); END;
         CREATE TRIGGER injected_deletion_mark_failed_failure BEFORE UPDATE OF status ON management_tasks WHEN NEW.status = 'failed' AND OLD.task_type = 'permanent_deletion' BEGIN SELECT RAISE(FAIL, 'injected deletion mark failed failure'); END;",
    ).unwrap();
    drop(connection);

    let executed = json_request(
        app(state.clone()),
        "POST",
        &format!(
            "/api/v1/deletion-plans/{}/execute",
            plan["id"].as_str().unwrap()
        ),
        r#"{"irreversible":true,"confirmation":"PERMANENTLY DELETE"}"#,
        Some(&cookie),
    )
    .await;
    assert_eq!(executed.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let error = to_bytes(executed.into_body(), usize::MAX).await.unwrap();
    assert!(std::str::from_utf8(&error)
        .unwrap()
        .contains("durable deletion journal"));
    assert!(!first.exists());
    assert!(second.exists());

    let store = rust_jav::management_tasks::TaskStore::open(&restart_config.database_file).unwrap();
    let task = store
        .list()
        .unwrap()
        .into_iter()
        .find(|task| task.task_type == "permanent_deletion")
        .unwrap();
    assert_eq!(task.status, rust_jav::management_tasks::TaskStatus::Running);
    assert_eq!(task.items.len(), 1);
    assert_eq!(task.items[0].status, "running");
    assert!(task.items[0].quarantine_token.is_some());
    drop(store);
    drop(state);

    let connection = rusqlite::Connection::open(&restart_config.database_file).unwrap();
    connection
        .execute_batch(
            "DROP TRIGGER injected_deletion_item_update_failure;
         DROP TRIGGER injected_deletion_audit_failure_all;
         DROP TRIGGER injected_deletion_mark_failed_failure;",
        )
        .unwrap();
    drop(connection);
    let _restarted = AppState::new(restart_config.clone(), TestClock(200)).unwrap();
    let reopened =
        rust_jav::management_tasks::TaskStore::open(&restart_config.database_file).unwrap();
    let recovered = reopened.get(&task.id).unwrap().unwrap();
    assert_eq!(
        recovered.status,
        rust_jav::management_tasks::TaskStatus::Interrupted
    );
    assert_eq!(recovered.items[0].status, "interrupted");
    assert!(recovered.items[0]
        .message
        .as_deref()
        .unwrap()
        .contains("quarantine is absent"));
}

#[tokio::test]
async fn jellyfin_configuration_connection_association_and_manual_refresh_are_server_side() {
    use axum::{
        routing::{get, post},
        Json, Router,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let refreshes = Arc::new(AtomicUsize::new(0));
    let image_requests = Arc::new(AtomicUsize::new(0));
    let people_requests = Arc::new(AtomicUsize::new(0));
    let count = refreshes.clone();
    let image_count = image_requests.clone();
    let people_count = people_requests.clone();
    let jellyfin = Router::new()
        .route("/System/Info", get(|| async { Json(serde_json::json!({"ServerName":"TrueNAS Jellyfin","Version":"10.11","Id":"server"})) }))
        .route("/Library/MediaFolders", get(|| async { Json(serde_json::json!({"Items":[{"Id":"jav","Name":"JAV","Path":"/media/jav"}]})) }))
        .route("/Items", get(|| async { Json(serde_json::json!({"Items":[{"Id":"jf-1","Name":"ABC-123","Path":"/media/jav/ABC-123.mp4","ProviderIds":{},"UserData":{"Played":true,"PlayCount":1,"PlaybackPositionTicks":0}}]})) }))
        .route("/Persons", get(move || { let people_count=people_count.clone(); async move { people_count.fetch_add(1, Ordering::SeqCst); Json(serde_json::json!({"Items":[{"Id":"person-alice","Name":"Alice","ImageTags":{"Primary":"portrait-tag"}}]})) }}))
        .route("/Items/:id/Images/Primary", get(move |headers: axum::http::HeaderMap, axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String,String>>| { let image_count=image_count.clone(); async move {
            assert_eq!(headers["X-Emby-Token"], "server-only-secret");
            assert_eq!(query.get("maxWidth").map(String::as_str), Some("320"));
            assert_eq!(query.get("tag").map(String::as_str), Some("portrait-tag"));
            image_count.fetch_add(1, Ordering::SeqCst);
            ([(header::CONTENT_TYPE, "image/jpeg")], b"actor portrait".to_vec())
        }}))
        .route("/Library/Refresh", post(move || { let count=count.clone(); async move { count.fetch_add(1, Ordering::SeqCst); StatusCode::NO_CONTENT } }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let jellyfin_url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, jellyfin).await.unwrap() });

    let (dir, mut config) = fixture();
    let root = dir.path().join("media/jav");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("ABC-123.mp4"), b"video").unwrap();
    std::fs::write(root.join("ABC-123.jpg"), b"asset artwork").unwrap();
    std::fs::write(
        root.join("ABC-123.nfo"),
        "<movie><title>ABC-123</title><actor><name>Alice</name></actor></movie>",
    )
    .unwrap();
    let actors = dir.path().join("actors");
    std::fs::create_dir_all(actors.join("Alice/ABC-123")).unwrap();
    std::fs::hard_link(
        root.join("ABC-123.mp4"),
        actors.join("Alice/ABC-123/ABC-123.mp4"),
    )
    .unwrap();
    config.media_roots.push(root);
    config.actor_view_root = Some(actors);
    config.artwork_cache_root = Some(dir.path().join("artwork-cache"));
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config.clone(), TestClock(100)).unwrap();
    let login = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/auth/login",
        r#"{"password":"a strong password"}"#,
        None,
    )
    .await;
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();

    let configured = json_request(app(state.clone()), "PUT", "/api/v1/jellyfin/config", &serde_json::json!({"url":jellyfin_url,"library_ids":["jav"],"api_key":"server-only-secret"}).to_string(), Some(&cookie)).await;
    assert_eq!(configured.status(), StatusCode::NO_CONTENT);
    let returned = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/jellyfin/config",
        "",
        Some(&cookie),
    )
    .await;
    let returned: serde_json::Value =
        serde_json::from_slice(&to_bytes(returned.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(returned["url"], jellyfin_url);
    assert_eq!(returned["api_key_configured"], true);
    assert!(returned.get("api_key").is_none());

    let preserved = json_request(
        app(state.clone()),
        "PUT",
        "/api/v1/jellyfin/config",
        &serde_json::json!({"url":jellyfin_url,"library_ids":["jav"],"api_key":""}).to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(preserved.status(), StatusCode::NO_CONTENT);
    let secrets = std::fs::read_to_string(config.secrets_file).unwrap();
    assert!(secrets.contains("server-only-secret"));

    let connection = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/jellyfin/test",
        "{}",
        Some(&cookie),
    )
    .await;
    assert_eq!(connection.status(), StatusCode::OK);
    let listed = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/assets",
        "",
        Some(&cookie),
    )
    .await;
    let listed: serde_json::Value =
        serde_json::from_slice(&to_bytes(listed.into_body(), usize::MAX).await.unwrap()).unwrap();
    let id = listed["items"][0]["id"].as_str().unwrap();
    let listed_artwork_url = listed["items"][0]["artwork_url"].clone();
    let listed_captured_date = listed["items"][0]["captured_date"].clone();
    let detail = json_request(
        app(state.clone()),
        "GET",
        &format!("/api/v1/assets/{id}"),
        "",
        Some(&cookie),
    )
    .await;
    let detail: serde_json::Value =
        serde_json::from_slice(&to_bytes(detail.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(detail["artwork_url"], listed_artwork_url);
    assert_eq!(detail["captured_date"], listed_captured_date);
    assert_eq!(detail["parse_status"], "valid");
    assert_eq!(detail["jellyfin"]["status"], "played");
    assert_eq!(detail["jellyfin"]["confidence"], "certain_path");
    assert_eq!(detail["jellyfin"]["may_authorize_deletion"], true);
    assert!(detail["jellyfin"]["open_url"]
        .as_str()
        .unwrap()
        .contains("web/#/details?id=jf-1"));
    assert_eq!(
        detail["actors"][0]["poster_url"],
        "/api/v1/actors/Alice/poster"
    );
    assert_eq!(detail["actors"][0]["actor_folder_url"], "/actors/QWxpY2U");

    let actor_list = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/actors",
        "",
        Some(&cookie),
    )
    .await;
    let actor_list: serde_json::Value =
        serde_json::from_slice(&to_bytes(actor_list.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert_eq!(actor_list[0]["poster_url"], "/api/v1/actors/Alice/poster");

    let portrait = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/actors/Alice/poster",
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(portrait.status(), StatusCode::OK);
    assert_eq!(portrait.headers()[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(
        to_bytes(portrait.into_body(), usize::MAX)
            .await
            .unwrap()
            .as_ref(),
        b"actor portrait"
    );
    assert_eq!(image_requests.load(Ordering::SeqCst), 1);
    let cached_portrait = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/actors/Alice/poster",
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(cached_portrait.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(cached_portrait.into_body(), usize::MAX)
            .await
            .unwrap()
            .as_ref(),
        b"actor portrait"
    );
    assert_eq!(
        image_requests.load(Ordering::SeqCst),
        1,
        "person ID + image tag cache hit"
    );
    assert_eq!(
        people_requests.load(Ordering::SeqCst),
        1,
        "asset detail, Actor list and portrait requests share one People snapshot"
    );

    let refresh = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/jellyfin/refresh",
        "{}",
        Some(&cookie),
    )
    .await;
    assert_eq!(refresh.status(), StatusCode::OK);
    assert_eq!(refreshes.load(Ordering::SeqCst), 1);
    let status = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/jellyfin/refresh",
        "",
        Some(&cookie),
    )
    .await;
    let status: serde_json::Value =
        serde_json::from_slice(&to_bytes(status.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(status["status"], "completed");
    assert_eq!(status["attempts"], 1);

    let preview = serde_json::json!({
        "task_type":"operations",
        "media_root":dir.path().join("media/jav"),
        "mode":"preview",
        "operations":["delete_ad_files"]
    });
    let planned = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &preview.to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(planned.status(), StatusCode::ACCEPTED);
    let planned: serde_json::Value =
        serde_json::from_slice(&to_bytes(planned.into_body(), usize::MAX).await.unwrap()).unwrap();
    let plan_id = planned["id"].as_str().unwrap();
    let _ = wait_for_task(&state, &cookie, plan_id, "completed").await;
    let task = serde_json::json!({
        "task_type":"operations",
        "mode":"apply",
        "plan_id":plan_id,
        "confirmed":true
    });
    let created = json_request(
        app(state.clone()),
        "POST",
        "/api/v1/tasks",
        &task.to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(created.status(), StatusCode::ACCEPTED);
    for _ in 0..100 {
        if refreshes.load(Ordering::SeqCst) == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        refreshes.load(Ordering::SeqCst),
        2,
        "one automatic refresh for the whole applied batch"
    );
}

#[tokio::test]
async fn jellyfin_configuration_rejects_url_userinfo_and_get_exposes_only_safe_fields() {
    let (_dir, config) = fixture();
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config.clone(), TestClock(100)).unwrap();
    let cookie = login_cookie(&state).await;
    let rejected = json_request(
        app(state.clone()),
        "PUT",
        "/api/v1/jellyfin/config",
        &serde_json::json!({
            "url": "http://embedded-user:embedded-password@jellyfin:8096",
            "library_ids": ["jav"],
            "api_key": "server-only-api-key"
        })
        .to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);

    let connection = rusqlite::Connection::open(&config.database_file).unwrap();
    connection
        .execute(
            "INSERT INTO jellyfin_config(singleton,url,library_ids) VALUES(1,?1,?2)",
            rusqlite::params![
                "http://legacy-user:legacy-password@jellyfin:8096",
                "[\"jav\"]"
            ],
        )
        .unwrap();

    let returned = json_request(
        app(state),
        "GET",
        "/api/v1/jellyfin/config",
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(returned.status(), StatusCode::OK);
    let body = to_bytes(returned.into_body(), usize::MAX).await.unwrap();
    let text = std::str::from_utf8(&body).unwrap();
    let document: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let mut fields = document
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    fields.sort_unstable();
    assert_eq!(fields, ["api_key_configured", "library_ids", "url"]);
    for credential in [
        "embedded-user",
        "embedded-password",
        "legacy-user",
        "legacy-password",
        "server-only-api-key",
        "api_key\"",
        "server_key",
        "credential",
    ] {
        assert!(!text.contains(credential), "GET leaked {credential}");
    }
}

#[tokio::test]
async fn jellyfin_server_url_change_requires_a_new_api_key() {
    let (_dir, state, cookie) = authenticated_fixture().await;
    let original_url = "http://jellyfin-a:8096";
    let configured = json_request(
        app(state.clone()),
        "PUT",
        "/api/v1/jellyfin/config",
        &serde_json::json!({
            "url": original_url,
            "library_ids": ["jav"],
            "api_key": "server-a-key"
        })
        .to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(configured.status(), StatusCode::NO_CONTENT);

    let rejected = json_request(
        app(state.clone()),
        "PUT",
        "/api/v1/jellyfin/config",
        &serde_json::json!({
            "url": "http://jellyfin-b:8096",
            "library_ids": ["jav"],
            "api_key": ""
        })
        .to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);

    let returned = json_request(
        app(state.clone()),
        "GET",
        "/api/v1/jellyfin/config",
        "",
        Some(&cookie),
    )
    .await;
    let returned: serde_json::Value =
        serde_json::from_slice(&to_bytes(returned.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(returned["url"], original_url);

    let preserved = json_request(
        app(state),
        "PUT",
        "/api/v1/jellyfin/config",
        &serde_json::json!({
            "url": original_url,
            "library_ids": ["jav", "movies"],
            "api_key": ""
        })
        .to_string(),
        Some(&cookie),
    )
    .await;
    assert_eq!(preserved.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn embedded_ui_preserves_library_and_tasks_while_adding_jellyfin_controls() {
    let (_dir, config) = fixture();
    let javascript = app(AppState::new(config, TestClock(100)).unwrap())
        .oneshot(
            Request::builder()
                .uri("/assets/app.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let javascript = to_bytes(javascript.into_body(), usize::MAX).await.unwrap();
    let javascript = std::str::from_utf8(&javascript).unwrap();
    for text in [
        "All Assets",
        "Management Tasks",
        "Overview",
        "NFO",
        "Jellyfin",
        "Open in Jellyfin",
        "Test connection",
        "Refresh Jellyfin",
    ] {
        assert!(
            javascript.contains(text),
            "missing preserved or new UI text: {text}"
        );
    }
}
