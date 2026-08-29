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
        media_roots: Vec::new(),
        actor_view_root: None,
    };
    (dir, config)
}

#[tokio::test]
async fn actor_folder_api_lists_confirmation_and_removes_via_management_task() {
    let (dir, mut config) = fixture();
    let media = dir.path().join("media");
    let actors = dir.path().join("actors");
    std::fs::create_dir_all(media.join("ABC-123")).unwrap();
    std::fs::create_dir_all(actors.join("Alice/ABC-123")).unwrap();
    std::fs::write(media.join("ABC-123/ABC-123.mp4"), b"movie").unwrap();
    std::fs::hard_link(
        media.join("ABC-123/ABC-123.mp4"),
        actors.join("Alice/ABC-123/ABC-123.mp4"),
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
    assert_eq!(folders[0]["hard_link_count"], 1);
    assert_eq!(folders[0]["logical_size"], 5);
    assert_eq!(folders[0]["reclaimable_space"], 0);

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
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("ABC-123.mp4"), b"video").unwrap();
    std::fs::write(root.join("ABC-123.jpg"), b"poster").unwrap();
    std::fs::write(root.join("ABC-123.nfo"), r#"<movie><title>Blue Room</title><studio>Example</studio><actor><name>miru</name></actor><plot>Local plot</plot></movie>"#).unwrap();
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
    let id = body["items"][0]["id"].as_str().unwrap();

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
    assert_eq!(detail["actors"][0]["name"], "miru");
    assert!(detail["actors"][0]["actor_folder_url"]
        .as_str()
        .unwrap()
        .starts_with("/actors/"));
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
    assert!(javascript.contains("Search code, title, or path"));
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
    assert!(document["paths"]["/api/v1/assets/scan"]["post"].is_object());
    assert!(document["paths"]["/api/v1/assets/{asset_id}/artwork"]["get"].is_object());
    assert_eq!(
        document["components"]["schemas"]["MediaAsset"]["properties"]["state"]["enum"][2],
        "exception"
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
    let count = refreshes.clone();
    let jellyfin = Router::new()
        .route("/System/Info", get(|| async { Json(serde_json::json!({"ServerName":"TrueNAS Jellyfin","Version":"10.11","Id":"server"})) }))
        .route("/Library/MediaFolders", get(|| async { Json(serde_json::json!({"Items":[{"Id":"jav","Name":"JAV","Path":"/media/jav"}]})) }))
        .route("/Items", get(|| async { Json(serde_json::json!({"Items":[{"Id":"jf-1","Name":"ABC-123","Path":"/media/jav/ABC-123.mp4","ProviderIds":{},"UserData":{"Played":true,"PlayCount":1,"PlaybackPositionTicks":0}}]})) }))
        .route("/Library/Refresh", post(move || { let count=count.clone(); async move { count.fetch_add(1, Ordering::SeqCst); StatusCode::NO_CONTENT } }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let jellyfin_url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, jellyfin).await.unwrap() });

    let (dir, mut config) = fixture();
    let root = dir.path().join("media/jav");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("ABC-123.mp4"), b"video").unwrap();
    std::fs::write(
        root.join("ABC-123.nfo"),
        "<movie><title>ABC-123</title></movie>",
    )
    .unwrap();
    config.media_roots.push(root);
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
    assert_eq!(detail["jellyfin"]["status"], "played");
    assert_eq!(detail["jellyfin"]["confidence"], "uncertain_metadata");
    assert_eq!(detail["jellyfin"]["may_authorize_deletion"], false);
    assert!(detail["jellyfin"]["open_url"]
        .as_str()
        .unwrap()
        .contains("web/#/details?id=jf-1"));

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
