use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    body::{to_bytes, Body},
    extract::{Path as AxumPath, Query, State},
    http::{header, HeaderMap, Request, Response, StatusCode},
    routing::get,
    Json, Router,
};
use futures::future::join_all;
use rust_jav::management::{
    app, password_secrets, AppState, Clock, ManagementConfig, SecretsStore,
};
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::{net::TcpListener, task::JoinHandle};
use tower::ServiceExt;

#[path = "support/artwork_fixtures.rs"]
mod artwork_fixtures;

#[derive(Clone)]
struct TestClock(u64);

impl Clock for TestClock {
    fn unix_seconds(&self) -> u64 {
        self.0
    }
}

#[derive(Clone)]
struct MockItem {
    id: String,
    name: String,
    path: String,
    jav_code: Option<String>,
    image_tag: String,
}

impl MockItem {
    fn certain(id: &str, path: &str, jav_code: &str, image_tag: &str) -> Self {
        Self {
            id: id.to_owned(),
            name: jav_code.to_owned(),
            path: path.to_owned(),
            jav_code: Some(jav_code.to_owned()),
            image_tag: image_tag.to_owned(),
        }
    }

    fn uncertain(id: &str, jav_code: &str, image_tag: &str) -> Self {
        Self {
            id: id.to_owned(),
            name: jav_code.to_owned(),
            path: format!("/unrelated/{jav_code}.mkv"),
            jav_code: Some(jav_code.to_owned()),
            image_tag: image_tag.to_owned(),
        }
    }

    fn json(&self) -> Value {
        json!({
            "Id": self.id,
            "Name": self.name,
            "Path": self.path,
            "ProviderIds": self.jav_code.as_ref().map_or_else(
                || json!({}),
                |code| json!({"Jav": code}),
            ),
            "ImageTags": {"Primary": self.image_tag},
            "UserData": {
                "Played": false,
                "PlayCount": 0,
                "PlaybackPositionTicks": 0
            }
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ImageRequest {
    item_id: String,
    image_tag: String,
    max_width: String,
    api_key: String,
}

#[derive(Clone)]
struct MockJellyfin {
    state: Arc<Mutex<MockJellyfinData>>,
    url: String,
    _server: Arc<JoinHandle<()>>,
}

struct MockJellyfinData {
    items: Vec<MockItem>,
    people: Vec<MockItem>,
    item_queries: Vec<BTreeMap<String, String>>,
    image_requests: Vec<ImageRequest>,
    image_status: StatusCode,
    image_content_type: String,
    image_bytes: Vec<u8>,
    image_delay: Duration,
    items_delay: Duration,
    items_padding_bytes: usize,
}

impl MockJellyfin {
    async fn start(items: Vec<MockItem>, content_type: &str, bytes: Vec<u8>) -> Self {
        async fn items_handler(
            State(state): State<Arc<Mutex<MockJellyfinData>>>,
            Query(query): Query<BTreeMap<String, String>>,
        ) -> Json<Value> {
            let (items, delay, padding) = {
                let mut data = state.lock().unwrap();
                data.item_queries.push(query);
                (
                    data.items.clone(),
                    data.items_delay,
                    data.items_padding_bytes,
                )
            };
            tokio::time::sleep(delay).await;
            let mut body = json!({
                "Items": items.iter().map(MockItem::json).collect::<Vec<_>>()
            });
            if padding > 0 {
                body["Padding"] = Value::String("x".repeat(padding));
            }
            Json(body)
        }

        async fn people(State(state): State<Arc<Mutex<MockJellyfinData>>>) -> Json<Value> {
            let data = state.lock().unwrap();
            Json(json!({
                "Items": data.people.iter().map(|person| json!({
                    "Id": person.id,
                    "Name": person.name,
                    "ImageTags": {"Primary": person.image_tag}
                })).collect::<Vec<_>>()
            }))
        }

        async fn image(
            State(state): State<Arc<Mutex<MockJellyfinData>>>,
            AxumPath(item_id): AxumPath<String>,
            Query(query): Query<BTreeMap<String, String>>,
            headers: HeaderMap,
        ) -> Response<Body> {
            let (status, content_type, bytes, delay) = {
                let mut data = state.lock().unwrap();
                data.image_requests.push(ImageRequest {
                    item_id,
                    image_tag: query.get("tag").cloned().unwrap_or_default(),
                    max_width: query.get("maxWidth").cloned().unwrap_or_default(),
                    api_key: headers
                        .get("X-Emby-Token")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_owned(),
                });
                (
                    data.image_status,
                    data.image_content_type.clone(),
                    data.image_bytes.clone(),
                    data.image_delay,
                )
            };
            tokio::time::sleep(delay).await;
            Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(bytes))
                .unwrap()
        }

        let state = Arc::new(Mutex::new(MockJellyfinData {
            items,
            people: Vec::new(),
            item_queries: Vec::new(),
            image_requests: Vec::new(),
            image_status: StatusCode::OK,
            image_content_type: content_type.to_owned(),
            image_bytes: bytes,
            image_delay: Duration::ZERO,
            items_delay: Duration::ZERO,
            items_padding_bytes: 0,
        }));
        let router = Router::new()
            .route("/Items", get(items_handler))
            .route("/Persons", get(people))
            .route("/Items/:id/Images/Primary", get(image))
            .with_state(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self {
            state,
            url,
            _server: Arc::new(server),
        }
    }

    fn set_items(&self, items: Vec<MockItem>) {
        self.state.lock().unwrap().items = items;
    }

    fn set_people(&self, people: Vec<MockItem>) {
        self.state.lock().unwrap().people = people;
    }

    fn set_image_delay(&self, delay: Duration) {
        self.state.lock().unwrap().image_delay = delay;
    }

    fn set_items_delay(&self, delay: Duration) {
        self.state.lock().unwrap().items_delay = delay;
    }

    fn set_items_padding(&self, bytes: usize) {
        self.state.lock().unwrap().items_padding_bytes = bytes;
    }

    fn image_requests(&self) -> Vec<ImageRequest> {
        self.state.lock().unwrap().image_requests.clone()
    }

    fn item_queries(&self) -> Vec<BTreeMap<String, String>> {
        self.state.lock().unwrap().item_queries.clone()
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
        artwork_cache_root: Some(dir.path().join("artwork-cache")),
        media_roots: Vec::new(),
        actor_view_root: None,
    };
    (dir, config)
}

fn add_asset(root: &std::path::Path, code: &str, local_artwork: Option<&[u8]>) -> String {
    let directory = root.join(code);
    std::fs::create_dir_all(&directory).unwrap();
    let video = directory.join(format!("{code}.mp4"));
    std::fs::write(&video, b"video").unwrap();
    std::fs::write(
        directory.join(format!("{code}.nfo")),
        format!("<movie><title>{code}</title></movie>"),
    )
    .unwrap();
    if let Some(bytes) = local_artwork {
        std::fs::write(directory.join("folder.jpg"), bytes).unwrap();
    }
    video.display().to_string()
}

async fn authenticated_state(mut config: ManagementConfig) -> (AppState, String) {
    password_secrets(
        &SecretsStore::new(config.secrets_file.clone()),
        "a strong password",
    )
    .unwrap();
    config.media_roots.sort();
    let state = AppState::new(config, TestClock(100)).unwrap();
    let login = request(
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
    (state, cookie)
}

async fn configure_jellyfin(
    state: &AppState,
    cookie: &str,
    url: &str,
    library_ids: &[&str],
    api_key: &str,
) {
    let response = request(
        app(state.clone()),
        "PUT",
        "/api/v1/jellyfin/config",
        &json!({
            "url": url,
            "library_ids": library_ids,
            "api_key": api_key
        })
        .to_string(),
        Some(cookie),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

async fn asset(state: &AppState, cookie: &str, code: &str) -> Value {
    let response = request(
        app(state.clone()),
        "GET",
        "/api/v1/assets?per_page=100",
        "",
        Some(cookie),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["jav_code"] == code)
        .unwrap()
        .clone()
}

fn artwork_url(item: &Value) -> String {
    format!("/api/v1/assets/{}/artwork", item["id"].as_str().unwrap())
}

async fn request(
    router: Router,
    method: &str,
    uri: &str,
    body: &str,
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
        .oneshot(builder.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap()
}

async fn response_bytes(response: axum::response::Response) -> Vec<u8> {
    to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

fn padded_jpeg(target_bytes: usize) -> Vec<u8> {
    let source = artwork_fixtures::valid_jpeg();
    assert_eq!(&source[..2], &[0xff, 0xd8]);
    let mut output = Vec::with_capacity(target_bytes + source.len());
    output.extend_from_slice(&source[..2]);
    let mut remaining = target_bytes.saturating_sub(source.len());
    while remaining > 4 {
        let payload = remaining.saturating_sub(4).min(u16::MAX as usize - 2);
        output.extend_from_slice(&[0xff, 0xfe]);
        output.extend_from_slice(&((payload + 2) as u16).to_be_bytes());
        output.resize(output.len() + payload, b'x');
        remaining -= payload + 4;
    }
    output.extend_from_slice(&source[2..]);
    output
}

#[cfg(unix)]
fn set_modified_seconds(path: &std::path::Path, seconds: i64) {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    let path = CString::new(path.as_os_str().as_bytes()).unwrap();
    let times = [
        libc::timespec {
            tv_sec: seconds,
            tv_nsec: 0,
        },
        libc::timespec {
            tv_sec: seconds,
            tv_nsec: 0,
        },
    ];
    assert_eq!(
        unsafe { libc::utimensat(libc::AT_FDCWD, path.as_ptr(), times.as_ptr(), 0) },
        0
    );
}

#[tokio::test]
async fn valid_local_artwork_has_priority_and_the_rust_jav_route_requires_authentication() {
    let (dir, mut config) = fixture();
    let media = dir.path().join("media");
    let local = artwork_fixtures::valid_jpeg();
    let path = add_asset(&media, "LOCAL-100", Some(&local));
    config.media_roots.push(media);
    let jellyfin = MockJellyfin::start(
        vec![MockItem::certain(
            "jf-local",
            &path,
            "LOCAL-100",
            "tag-local",
        )],
        "image/png",
        artwork_fixtures::valid_png(),
    )
    .await;
    let (state, cookie) = authenticated_state(config).await;
    configure_jellyfin(
        &state,
        &cookie,
        &jellyfin.url,
        &["jav"],
        "server-only-secret",
    )
    .await;
    let item = asset(&state, &cookie, "LOCAL-100").await;
    let url = artwork_url(&item);

    let unauthenticated = request(app(state.clone()), "GET", &url, "", None).await;
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
    let authenticated = request(app(state), "GET", &url, "", Some(&cookie)).await;
    assert_eq!(authenticated.status(), StatusCode::OK);
    assert_eq!(authenticated.headers()[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(response_bytes(authenticated).await, local);
    assert!(jellyfin.image_requests().is_empty());
}

#[tokio::test]
async fn certain_association_falls_back_for_dass_591_but_uncertain_metadata_is_forbidden() {
    let (dir, mut config) = fixture();
    let media = dir.path().join("media");
    let dass_path = add_asset(&media, "DASS-591", Some(&[]));
    add_asset(&media, "META-200", None);
    config.media_roots.push(media);
    let expected = artwork_fixtures::valid_jpeg();
    let jellyfin = MockJellyfin::start(
        vec![
            MockItem::certain("jf-dass", &dass_path, "DASS-591", "dass-tag"),
            MockItem::uncertain("jf-meta", "META-200", "meta-tag"),
        ],
        "image/jpeg",
        expected.clone(),
    )
    .await;
    let (state, cookie) = authenticated_state(config).await;
    configure_jellyfin(
        &state,
        &cookie,
        &jellyfin.url,
        &["jav"],
        "server-only-secret",
    )
    .await;

    let dass = asset(&state, &cookie, "DASS-591").await;
    assert_eq!(
        dass["artwork_url"],
        artwork_url(&dass),
        "the browser must receive a rust-jav fallback URL"
    );
    let dass_response = request(
        app(state.clone()),
        "GET",
        &artwork_url(&dass),
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(dass_response.status(), StatusCode::OK);
    assert_eq!(response_bytes(dass_response).await, expected);

    let uncertain = asset(&state, &cookie, "META-200").await;
    let uncertain_response = request(
        app(state),
        "GET",
        &artwork_url(&uncertain),
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(uncertain_response.status(), StatusCode::NOT_FOUND);
    let image_requests = jellyfin.image_requests();
    assert_eq!(image_requests.len(), 1);
    assert_eq!(image_requests[0].item_id, "jf-dass");
    assert_eq!(image_requests[0].image_tag, "dass-tag");
    assert_eq!(image_requests[0].api_key, "server-only-secret");
    assert!(!image_requests[0].max_width.is_empty());
    assert!(jellyfin.item_queries().iter().all(|query| query
        .get("fields")
        .is_some_and(|fields| fields.contains("ImageTags"))));
}

#[tokio::test]
async fn cache_hit_uses_item_tag_and_size_identity_and_tag_changes_miss() {
    let (dir, mut config) = fixture();
    let media = dir.path().join("media");
    let first_path = add_asset(&media, "CACHE-301", None);
    let second_path = add_asset(&media, "CACHE-302", None);
    config.media_roots.push(media);
    let first = MockItem::certain("item-a", &first_path, "CACHE-301", "shared-tag");
    let second = MockItem::certain("item-b", &second_path, "CACHE-302", "shared-tag");
    let jellyfin = MockJellyfin::start(
        vec![first.clone(), second.clone()],
        "image/jpeg",
        artwork_fixtures::valid_jpeg(),
    )
    .await;
    let (state, cookie) = authenticated_state(config).await;
    configure_jellyfin(&state, &cookie, &jellyfin.url, &["jav"], "cache-secret").await;
    let first_asset = asset(&state, &cookie, "CACHE-301").await;
    let second_asset = asset(&state, &cookie, "CACHE-302").await;

    for _ in 0..2 {
        let response = request(
            app(state.clone()),
            "GET",
            &artwork_url(&first_asset),
            "",
            Some(&cookie),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    let other_item = request(
        app(state.clone()),
        "GET",
        &artwork_url(&second_asset),
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(other_item.status(), StatusCode::OK);
    assert_eq!(
        jellyfin.image_requests().len(),
        2,
        "one hit and two item keys"
    );

    jellyfin.set_items(vec![
        MockItem {
            image_tag: "changed-tag".to_owned(),
            ..first
        },
        second,
    ]);
    let changed_tag = request(
        app(state),
        "GET",
        &artwork_url(&first_asset),
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(changed_tag.status(), StatusCode::OK);
    let requests = jellyfin.image_requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].item_id, "item-a");
    assert_eq!(requests[1].item_id, "item-b");
    assert_eq!(requests[2].image_tag, "changed-tag");
    assert!(requests.iter().all(|request| !request.max_width.is_empty()));
}

#[tokio::test]
async fn concurrent_cache_misses_for_the_same_image_are_coalesced() {
    let (dir, mut config) = fixture();
    let media = dir.path().join("media");
    let path = add_asset(&media, "BURST-401", None);
    config.media_roots.push(media);
    let jellyfin = MockJellyfin::start(
        vec![MockItem::certain(
            "item-burst",
            &path,
            "BURST-401",
            "burst-tag",
        )],
        "image/jpeg",
        artwork_fixtures::valid_jpeg(),
    )
    .await;
    jellyfin.set_image_delay(Duration::from_millis(100));
    jellyfin.set_items_delay(Duration::from_millis(100));
    let (state, cookie) = authenticated_state(config).await;
    configure_jellyfin(&state, &cookie, &jellyfin.url, &["jav"], "burst-secret").await;
    let item = asset(&state, &cookie, "BURST-401").await;
    let url = artwork_url(&item);
    let item_queries_before_burst = jellyfin.item_queries().len();

    let responses = join_all((0..8).map(|_| {
        let state = state.clone();
        let cookie = cookie.clone();
        let url = url.clone();
        async move { request(app(state), "GET", &url, "", Some(&cookie)).await }
    }))
    .await;

    assert!(responses
        .iter()
        .all(|response| response.status() == StatusCode::OK));
    assert_eq!(
        jellyfin.image_requests().len(),
        1,
        "one upstream request must serve the entire concurrent burst"
    );
    assert_eq!(
        jellyfin.item_queries().len() - item_queries_before_burst,
        1,
        "association discovery must be coalesced before the image cache key is known"
    );
}

#[tokio::test]
async fn actor_and_asset_callers_share_cache_without_aliasing_different_sizes() {
    let (dir, mut config) = fixture();
    let media = dir.path().join("media");
    let path = add_asset(&media, "SHARED-501", None);
    let actors = dir.path().join("actors");
    std::fs::create_dir_all(actors.join("Alice/SHARED-501")).unwrap();
    std::fs::hard_link(&path, actors.join("Alice/SHARED-501/SHARED-501.mp4")).unwrap();
    config.media_roots.push(media);
    config.actor_view_root = Some(actors);
    let shared = MockItem::certain("shared-item", &path, "SHARED-501", "shared-tag");
    let jellyfin = MockJellyfin::start(
        vec![shared.clone()],
        "image/jpeg",
        artwork_fixtures::valid_jpeg(),
    )
    .await;
    jellyfin.set_people(vec![MockItem {
        name: "Alice".to_owned(),
        ..shared
    }]);
    let (state, cookie) = authenticated_state(config).await;
    configure_jellyfin(&state, &cookie, &jellyfin.url, &["jav"], "shared-secret").await;
    let item = asset(&state, &cookie, "SHARED-501").await;

    let asset_image = request(
        app(state.clone()),
        "GET",
        &artwork_url(&item),
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(asset_image.status(), StatusCode::OK);
    let actor_image = request(
        app(state),
        "GET",
        "/api/v1/actors/Alice/poster",
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(actor_image.status(), StatusCode::OK);

    let requests = jellyfin.image_requests();
    assert_eq!(requests.len(), 2);
    assert!(requests
        .iter()
        .all(|request| request.item_id == "shared-item" && request.image_tag == "shared-tag"));
    assert_eq!(
        requests
            .iter()
            .map(|request| request.max_width.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        2,
        "requested size must participate in the shared cache identity"
    );
    assert!(requests.iter().any(|request| request.max_width == "320"));
}

#[tokio::test]
async fn jellyfin_configuration_change_invalidates_cached_artwork() {
    let (dir, mut config) = fixture();
    let media = dir.path().join("media");
    let path = add_asset(&media, "CONFIG-601", None);
    config.media_roots.push(media);
    let item = MockItem::certain("config-item", &path, "CONFIG-601", "same-tag");
    let first_bytes = artwork_fixtures::valid_jpeg();
    let second_bytes = artwork_fixtures::valid_png();
    let first = MockJellyfin::start(vec![item.clone()], "image/jpeg", first_bytes.clone()).await;
    let second = MockJellyfin::start(vec![item], "image/png", second_bytes.clone()).await;
    let (state, cookie) = authenticated_state(config).await;
    configure_jellyfin(&state, &cookie, &first.url, &["jav"], "first-secret").await;
    let listed = asset(&state, &cookie, "CONFIG-601").await;
    let initial = request(
        app(state.clone()),
        "GET",
        &artwork_url(&listed),
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(initial.status(), StatusCode::OK);
    assert_eq!(response_bytes(initial).await, first_bytes);

    configure_jellyfin(
        &state,
        &cookie,
        &second.url,
        &["jav", "other"],
        "second-secret",
    )
    .await;
    let changed = request(
        app(state.clone()),
        "GET",
        &artwork_url(&listed),
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(changed.status(), StatusCode::OK);
    assert_eq!(changed.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(response_bytes(changed).await, second_bytes);
    assert_eq!(first.image_requests().len(), 1);
    assert_eq!(second.image_requests().len(), 1);

    let public_config = request(
        app(state),
        "GET",
        "/api/v1/jellyfin/config",
        "",
        Some(&cookie),
    )
    .await;
    let public_config = response_bytes(public_config).await;
    let public_config = std::str::from_utf8(&public_config).unwrap();
    assert!(!public_config.contains("first-secret"));
    assert!(!public_config.contains("second-secret"));
}

#[tokio::test]
async fn offline_or_invalid_jellyfin_responses_never_become_cached_artwork() {
    let (dir, mut config) = fixture();
    let media = dir.path().join("media");
    let path = add_asset(&media, "INVALID-701", None);
    config.media_roots.push(media);
    let jellyfin = MockJellyfin::start(
        vec![MockItem::certain(
            "invalid-item",
            &path,
            "INVALID-701",
            "invalid-tag",
        )],
        "text/html",
        b"<html>not an image</html>".to_vec(),
    )
    .await;
    let (state, cookie) = authenticated_state(config).await;
    configure_jellyfin(&state, &cookie, &jellyfin.url, &["jav"], "invalid-secret").await;
    let item = asset(&state, &cookie, "INVALID-701").await;
    let url = artwork_url(&item);

    for _ in 0..2 {
        let invalid = request(app(state.clone()), "GET", &url, "", Some(&cookie)).await;
        assert_eq!(invalid.status(), StatusCode::BAD_GATEWAY);
    }
    assert_eq!(
        jellyfin.image_requests().len(),
        2,
        "an invalid body must not turn the next request into a cache hit"
    );

    let unused_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let offline_url = format!("http://{}", unused_listener.local_addr().unwrap());
    drop(unused_listener);
    configure_jellyfin(&state, &cookie, &offline_url, &["jav"], "offline-secret").await;
    let offline = request(app(state), "GET", &url, "", Some(&cookie)).await;
    assert_eq!(offline.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn memory_cache_enforces_a_total_encoded_byte_budget_with_lru_eviction() {
    let (dir, mut config) = fixture();
    config.artwork_cache_root = None;
    let media = dir.path().join("media");
    let mut mock_items = Vec::new();
    for code in ["MEM-801", "MEM-802", "MEM-803"] {
        let path = add_asset(&media, code, None);
        mock_items.push(MockItem::certain(
            &format!("item-{code}"),
            &path,
            code,
            &format!("tag-{code}"),
        ));
    }
    config.media_roots.push(media);
    let jellyfin =
        MockJellyfin::start(mock_items, "image/jpeg", padded_jpeg(3 * 1024 * 1024)).await;
    let (state, cookie) = authenticated_state(config).await;
    configure_jellyfin(&state, &cookie, &jellyfin.url, &["jav"], "memory-secret").await;
    let first = asset(&state, &cookie, "MEM-801").await;
    let second = asset(&state, &cookie, "MEM-802").await;
    let third = asset(&state, &cookie, "MEM-803").await;

    for item in [&first, &second, &first, &third, &second] {
        let response = request(
            app(state.clone()),
            "GET",
            &artwork_url(item),
            "",
            Some(&cookie),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    assert_eq!(
        jellyfin.image_requests().len(),
        4,
        "three 3 MiB entries cannot all remain in the bounded memory cache"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn disk_cache_evicts_oldest_regular_entries_without_following_symlinks() {
    use std::os::unix::fs::symlink;

    let (dir, mut config) = fixture();
    let cache_root = config.artwork_cache_root.clone().unwrap();
    let cache_dir = cache_root.join("jellyfin-images");
    std::fs::create_dir_all(&cache_dir).unwrap();
    let oldest = cache_dir.join(format!("{}.image", "a".repeat(64)));
    let newest = cache_dir.join(format!("{}.image", "b".repeat(64)));
    std::fs::File::create(&oldest)
        .unwrap()
        .set_len(40 * 1024 * 1024)
        .unwrap();
    std::fs::File::create(&newest)
        .unwrap()
        .set_len(40 * 1024 * 1024)
        .unwrap();
    set_modified_seconds(&oldest, 10);
    set_modified_seconds(&newest, 20);
    let outside = dir.path().join("outside-do-not-touch");
    std::fs::write(&outside, b"outside").unwrap();
    symlink(
        &outside,
        cache_dir.join(format!("{}.image", "c".repeat(64))),
    )
    .unwrap();

    let media = dir.path().join("media");
    let path = add_asset(&media, "DISK-901", None);
    config.media_roots.push(media);
    let jellyfin = MockJellyfin::start(
        vec![MockItem::certain(
            "disk-item",
            &path,
            "DISK-901",
            "disk-tag",
        )],
        "image/jpeg",
        artwork_fixtures::valid_jpeg(),
    )
    .await;
    let (state, cookie) = authenticated_state(config).await;
    configure_jellyfin(&state, &cookie, &jellyfin.url, &["jav"], "disk-secret").await;
    let item = asset(&state, &cookie, "DISK-901").await;
    let response = request(app(state), "GET", &artwork_url(&item), "", Some(&cookie)).await;
    assert_eq!(response.status(), StatusCode::OK);

    let total_regular_bytes = std::fs::read_dir(&cache_dir)
        .unwrap()
        .map(|entry| std::fs::symlink_metadata(entry.unwrap().path()).unwrap())
        .filter(|metadata| metadata.file_type().is_file())
        .map(|metadata| metadata.len())
        .sum::<u64>();
    assert!(total_regular_bytes <= 64 * 1024 * 1024);
    assert!(
        !oldest.exists(),
        "the oldest cache entry must be evicted first"
    );
    assert!(newest.exists());
    assert_eq!(std::fs::read(&outside).unwrap(), b"outside");
}

#[tokio::test]
async fn oversized_items_json_is_rejected_before_association_deserialization() {
    let (dir, mut config) = fixture();
    let media = dir.path().join("media");
    let path = add_asset(&media, "ITEMS-999", None);
    config.media_roots.push(media);
    let jellyfin = MockJellyfin::start(
        vec![MockItem::certain(
            "items-item",
            &path,
            "ITEMS-999",
            "items-tag",
        )],
        "image/jpeg",
        artwork_fixtures::valid_jpeg(),
    )
    .await;
    jellyfin.set_items_padding(9 * 1024 * 1024);
    let (state, cookie) = authenticated_state(config).await;
    configure_jellyfin(&state, &cookie, &jellyfin.url, &["jav"], "items-secret").await;
    let item = asset(&state, &cookie, "ITEMS-999").await;
    let response = request(app(state), "GET", &artwork_url(&item), "", Some(&cookie)).await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert!(jellyfin.image_requests().is_empty());
}

#[tokio::test]
async fn asset_and_actor_routes_share_a_global_in_flight_response_byte_budget() {
    let (dir, mut config) = fixture();
    config.artwork_cache_root = None;
    let media = dir.path().join("media");
    let path = add_asset(&media, "FLOW-1001", None);
    let actors = dir.path().join("actors");
    std::fs::create_dir_all(actors.join("Alice/FLOW-1001")).unwrap();
    std::fs::hard_link(&path, actors.join("Alice/FLOW-1001/FLOW-1001.mp4")).unwrap();
    config.media_roots.push(media);
    config.actor_view_root = Some(actors);
    let shared = MockItem::certain("flow-item", &path, "FLOW-1001", "flow-tag");
    let jellyfin = MockJellyfin::start(
        vec![shared.clone()],
        "image/jpeg",
        padded_jpeg(24 * 1024 * 1024),
    )
    .await;
    jellyfin.set_people(vec![MockItem {
        name: "Alice".to_owned(),
        ..shared
    }]);
    let (state, cookie) = authenticated_state(config).await;
    configure_jellyfin(&state, &cookie, &jellyfin.url, &["jav"], "flow-secret").await;
    let item = asset(&state, &cookie, "FLOW-1001").await;
    let url = artwork_url(&item);

    let held_asset = request(app(state.clone()), "GET", &url, "", Some(&cookie)).await;
    assert_eq!(held_asset.status(), StatusCode::OK);
    let held_actor = request(
        app(state.clone()),
        "GET",
        "/api/v1/actors/Alice/poster",
        "",
        Some(&cookie),
    )
    .await;
    assert_eq!(held_actor.status(), StatusCode::OK);

    let rejected = request(app(state.clone()), "GET", &url, "", Some(&cookie)).await;
    assert_eq!(rejected.status(), StatusCode::SERVICE_UNAVAILABLE);

    drop(held_asset);
    let recovered = request(app(state), "GET", &url, "", Some(&cookie)).await;
    assert_eq!(recovered.status(), StatusCode::OK);
    drop(held_actor);
    drop(recovered);
}

#[tokio::test]
async fn list_detail_and_actor_linked_assets_share_bounded_item_discovery() {
    let (dir, mut config) = fixture();
    let media = dir.path().join("media");
    let path = add_asset(&media, "DISC-1101", None);
    let actors = dir.path().join("actors");
    std::fs::create_dir_all(actors.join("Alice/DISC-1101")).unwrap();
    std::fs::hard_link(&path, actors.join("Alice/DISC-1101/DISC-1101.mp4")).unwrap();
    config.media_roots.push(media);
    config.actor_view_root = Some(actors);
    let item = MockItem::certain("discovery-item", &path, "DISC-1101", "discovery-tag");
    let jellyfin = MockJellyfin::start(
        vec![item.clone()],
        "image/jpeg",
        artwork_fixtures::valid_jpeg(),
    )
    .await;
    jellyfin.set_people(vec![MockItem {
        name: "Alice".to_owned(),
        ..item
    }]);
    let (state, cookie) = authenticated_state(config).await;
    configure_jellyfin(&state, &cookie, &jellyfin.url, &["jav"], "discovery-secret").await;
    let listed = asset(&state, &cookie, "DISC-1101").await;
    let id = listed["id"].as_str().unwrap().to_owned();
    tokio::time::sleep(Duration::from_millis(300)).await;
    jellyfin.set_items_delay(Duration::from_millis(100));
    let before = jellyfin.item_queries().len();

    let responses = join_all([
        request(
            app(state.clone()),
            "GET",
            "/api/v1/assets?per_page=100",
            "",
            Some(&cookie),
        ),
        request(
            app(state.clone()),
            "GET",
            &format!("/api/v1/assets/{id}"),
            "",
            Some(&cookie),
        ),
        request(app(state), "GET", "/api/v1/actors/Alice", "", Some(&cookie)),
    ])
    .await;

    assert!(responses
        .iter()
        .all(|response| response.status() == StatusCode::OK));
    assert_eq!(
        jellyfin.item_queries().len() - before,
        1,
        "all management discovery callers must share one config-bound snapshot"
    );
}
