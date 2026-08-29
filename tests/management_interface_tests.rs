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
    };
    (dir, config)
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
