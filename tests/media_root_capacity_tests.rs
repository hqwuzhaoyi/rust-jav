use std::time::Duration;

use axum::body::{to_bytes, Body};
use http::{header, Request, StatusCode};
use rust_jav::management::{
    app, password_secrets, AppState, Clock, ManagementConfig, SecretsStore,
};
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

#[derive(Clone)]
struct TestClock;

impl Clock for TestClock {
    fn unix_seconds(&self) -> u64 {
        100
    }
}

fn fixture(media_roots: Vec<std::path::PathBuf>) -> (TempDir, ManagementConfig) {
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
        media_roots,
        actor_view_root: None,
    };
    (dir, config)
}

async fn authenticated_state(config: ManagementConfig) -> (AppState, String) {
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    let state = AppState::new(config, TestClock).unwrap();
    let response = request(state.clone(), None, "/api/v1/media-roots/storage").await;
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    (state, cookie)
}

async fn request(state: AppState, cookie: Option<&str>, uri: &str) -> axum::response::Response {
    let (method, uri, body) = if cookie.is_some() {
        ("GET", uri, "")
    } else {
        (
            "POST",
            "/api/v1/auth/login",
            r#"{"password":"a strong password"}"#,
        )
    };
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    app(state)
        .oneshot(builder.body(Body::from(body)).unwrap())
        .await
        .unwrap()
}

async fn json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[tokio::test]
async fn 当媒体根目录可访问时_应通过真实端点返回容量且保留权限字段() {
    let sandbox = tempfile::tempdir().unwrap();
    let root = sandbox.path().join("media");
    std::fs::create_dir(&root).unwrap();
    let (_state_dir, config) = fixture(vec![root.clone()]);
    let (state, cookie) = authenticated_state(config).await;

    let response = request(state, Some(&cookie), "/api/v1/media-roots/storage").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    let report = &body["roots"][0];

    assert_eq!(report["path"], root.display().to_string());
    assert_eq!(report["readable"], true);
    assert_eq!(report["writable"], true);
    assert!(report["action"].is_null());
    assert_eq!(report["capacity"]["status"], "healthy");
    let total = report["capacity"]["total_bytes"].as_u64().unwrap();
    let used = report["capacity"]["used_bytes"].as_u64().unwrap();
    let available = report["capacity"]["available_bytes"].as_u64().unwrap();
    assert!(total > 0);
    assert!(used <= total);
    assert!(available <= total);
}

#[tokio::test]
async fn 当多个媒体根目录位于同一文件系统时_聚合容量应只计算一次() {
    let sandbox = tempfile::tempdir().unwrap();
    let first = sandbox.path().join("movies");
    let second = sandbox.path().join("series");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    let (_state_dir, config) = fixture(vec![first, second]);
    let (state, cookie) = authenticated_state(config).await;

    let body = json(request(state, Some(&cookie), "/api/v1/media-roots/storage").await).await;
    let roots = body["roots"].as_array().unwrap();
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0]["capacity"], roots[1]["capacity"]);
    assert_eq!(body["aggregate"]["status"], "healthy");
    assert_eq!(body["aggregate"]["filesystem_count"], 1);
    assert_eq!(
        body["aggregate"]["total_bytes"],
        roots[0]["capacity"]["total_bytes"]
    );
    assert_eq!(
        body["aggregate"]["used_bytes"],
        roots[0]["capacity"]["used_bytes"]
    );
    assert_eq!(
        body["aggregate"]["available_bytes"],
        roots[0]["capacity"]["available_bytes"]
    );
}

#[tokio::test]
async fn 当媒体根目录缺失时_应明确降级并让容量保持为空() {
    let sandbox = tempfile::tempdir().unwrap();
    let missing = sandbox.path().join("not-mounted");
    let (_state_dir, config) = fixture(vec![missing.clone()]);
    let (state, cookie) = authenticated_state(config).await;

    let response = request(state, Some(&cookie), "/api/v1/media-roots/storage").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    let report = &body["roots"][0];

    assert_eq!(report["path"], missing.display().to_string());
    assert_eq!(report["readable"], false);
    assert_eq!(report["writable"], false);
    assert!(report["action"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert_eq!(report["capacity"]["status"], "degraded");
    assert!(report["capacity"]["total_bytes"].is_null());
    assert!(report["capacity"]["used_bytes"].is_null());
    assert!(report["capacity"]["available_bytes"].is_null());
    assert_eq!(body["aggregate"]["status"], "degraded");
    assert_eq!(body["aggregate"]["filesystem_count"], 0);
    assert!(body["aggregate"]["total_bytes"].is_null());
    assert!(body["aggregate"]["used_bytes"].is_null());
    assert!(body["aggregate"]["available_bytes"].is_null());
}

#[tokio::test]
async fn openapi_应描述兼容的权限接口与容量聚合接口() {
    let sandbox = tempfile::tempdir().unwrap();
    let root = sandbox.path().join("media");
    std::fs::create_dir(&root).unwrap();
    let (_state_dir, config) = fixture(vec![root]);
    let (state, cookie) = authenticated_state(config).await;

    let document = json(request(state, Some(&cookie), "/api/v1/openapi.json").await).await;
    assert_eq!(
        document["paths"]["/api/v1/media-roots/health"]["get"]["responses"]["200"]["content"]
            ["application/json"]["schema"]["type"],
        "array"
    );
    assert_eq!(
        document["paths"]["/api/v1/media-roots/storage"]["get"]["responses"]["200"]["content"]
            ["application/json"]["schema"]["$ref"],
        "#/components/schemas/MediaRootStorage"
    );
    assert_eq!(
        document["components"]["schemas"]["RootCapacity"]["properties"]["total_bytes"]["type"][1],
        "null"
    );
}
