use std::{collections::BTreeMap, time::Duration};

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use rust_jav::jellyfin::{
    associate, match_person, AssociationConfidence, JellyfinClient, JellyfinConfig, JellyfinItem,
    JellyfinLibrary, JellyfinPerson, RefreshOutcome, RetryPolicy,
};
use serde_json::{json, Value};
use tokio::net::TcpListener;

#[derive(Clone, Default)]
struct MockState(std::sync::Arc<std::sync::Mutex<MockData>>);

#[derive(Default)]
struct MockData {
    api_keys: Vec<String>,
    item_queries: Vec<BTreeMap<String, String>>,
    refreshes: usize,
    refresh_failures: usize,
    image_requests: Vec<String>,
}

async fn mock_server(refresh_failures: usize) -> (String, MockState) {
    async fn info(State(state): State<MockState>, headers: HeaderMap) -> Json<Value> {
        state.0.lock().unwrap().api_keys.push(
            headers
                .get("X-Emby-Token")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_owned(),
        );
        Json(json!({"ServerName":"TrueNAS Jellyfin","Version":"10.11.0","Id":"server-1"}))
    }
    async fn libraries() -> Json<Value> {
        Json(json!({"Items":[
            {"Id":"movies","Name":"Movies","Path":"/media/movies"},
            {"Id":"jav","Name":"JAV","Path":"/media/jav"}
        ]}))
    }
    async fn items(
        State(state): State<MockState>,
        Query(query): Query<BTreeMap<String, String>>,
    ) -> Json<Value> {
        state.0.lock().unwrap().item_queries.push(query);
        Json(json!({"Items":[
            {"Id":"exact","Name":"Blue Room","Path":"/media/jav/ABC-123.mp4","ProviderIds":{"Jav":"ABC-123"},"UserData":{"Played":true,"PlayCount":2,"PlaybackPositionTicks":0},"People":[{"Name":"架乃ゆら","Type":"Actor"},{"Name":"导演","Type":"Director"}]},
            {"Id":"fallback","Name":"XYZ-999","Path":"/other/XYZ-999.mkv","ProviderIds":{},"UserData":{"Played":false,"PlayCount":0,"PlaybackPositionTicks":42}}
        ],"TotalRecordCount":2}))
    }
    async fn people() -> Json<Value> {
        Json(json!({"Items":[
            {"Id":"alice","Name":"ＡＬＩＣＥ","ImageTags":{"Primary":"tag-a"}},
            {"Id":"duplicate-1","Name":"森沢かな","ImageTags":{"Primary":"tag-1"}},
            {"Id":"duplicate-2","Name":"森沢かな","ImageTags":{"Primary":"tag-2"}},
            {"Id":"no-image","Name":"Tiny Lu","ImageTags":{}}
        ]}))
    }
    async fn image(
        State(state): State<MockState>,
        axum::extract::Path(id): axum::extract::Path<String>,
        Query(query): Query<BTreeMap<String, String>>,
    ) -> ([(&'static str, &'static str); 1], Vec<u8>) {
        state.0.lock().unwrap().image_requests.push(format!(
            "{id}:{}:{}",
            query.get("maxWidth").map(String::as_str).unwrap_or(""),
            query.get("tag").map(String::as_str).unwrap_or("")
        ));
        ([("content-type", "image/jpeg")], b"portrait".to_vec())
    }
    async fn refresh(State(state): State<MockState>) -> StatusCode {
        let mut data = state.0.lock().unwrap();
        data.refreshes += 1;
        if data.refreshes <= data.refresh_failures {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            StatusCode::NO_CONTENT
        }
    }
    let state = MockState::default();
    state.0.lock().unwrap().refresh_failures = refresh_failures;
    let app = Router::new()
        .route("/System/Info", get(info))
        .route("/Library/MediaFolders", get(libraries))
        .route("/Items", get(items))
        .route("/Persons", get(people))
        .route("/Items/:id/Images/Primary", get(image))
        .route("/Library/Refresh", post(refresh))
        .with_state(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), state)
}

#[test]
fn person_match_requires_one_unicode_normalized_name_with_primary_image() {
    let people = vec![
        JellyfinPerson::fixture("alice", "ＡＬＩＣＥ", Some("tag-a")),
        JellyfinPerson::fixture("duplicate-1", "森沢かな", Some("tag-1")),
        JellyfinPerson::fixture("duplicate-2", "森沢かな", None),
        JellyfinPerson::fixture("no-image", "Tiny Lu", None),
    ];

    assert_eq!(match_person(" alice ", &people).unwrap().id, "alice");
    assert!(match_person("森沢かな", &people).is_none());
    assert!(match_person("Tiny Lu", &people).is_none());
    assert!(match_person("Missing", &people).is_none());
}

#[tokio::test]
async fn person_discovery_and_primary_portrait_are_server_side() {
    let (url, state) = mock_server(0).await;
    let client = JellyfinClient::new(
        JellyfinConfig {
            url,
            library_ids: vec!["jav".into()],
        },
        "server-secret".into(),
    )
    .unwrap();

    let people = client.people().await.unwrap();
    let person = match_person("alice", &people).unwrap();
    let image = client.primary_image(person, 320).await.unwrap();

    assert_eq!(image.content_type, "image/jpeg");
    assert_eq!(image.bytes, b"portrait");
    assert_eq!(state.0.lock().unwrap().image_requests, ["alice:320:tag-a"]);
}

#[tokio::test]
async fn truenas_connection_test_uses_server_only_key_and_returns_selected_libraries() {
    let (url, state) = mock_server(0).await;
    let client = JellyfinClient::new(
        JellyfinConfig {
            url,
            library_ids: vec!["jav".into()],
        },
        "server-secret".into(),
    )
    .unwrap();

    let connection = client.test_connection().await.unwrap();

    assert_eq!(connection.server_name, "TrueNAS Jellyfin");
    assert_eq!(
        connection.libraries,
        vec![JellyfinLibrary {
            id: "jav".into(),
            name: "JAV".into(),
            path: Some("/media/jav".into())
        }]
    );
    assert_eq!(state.0.lock().unwrap().api_keys, ["server-secret"]);
}

#[tokio::test]
async fn item_discovery_is_scoped_to_each_selected_library_and_requests_association_fields() {
    let (url, state) = mock_server(0).await;
    let client = JellyfinClient::new(
        JellyfinConfig {
            url,
            library_ids: vec!["movies".into(), "jav".into()],
        },
        "key".into(),
    )
    .unwrap();

    let items = client.selected_items().await.unwrap();

    assert_eq!(items.len(), 4);
    let queries = &state.0.lock().unwrap().item_queries;
    assert_eq!(queries.len(), 2);
    assert_eq!(queries[0]["recursive"], "true");
    assert!(queries.iter().any(|q| q["parentId"] == "movies"));
    assert!(queries.iter().any(|q| q["parentId"] == "jav"));
    assert!(queries[0]["fields"].contains("Path"));
    assert!(queries[0]["fields"].contains("ProviderIds"));
    assert!(queries[0]["fields"].contains("UserData"));
    assert!(queries[0]["fields"].contains("People"));
    assert!(items[0].has_actor("架乃ゆら"));
    assert!(!items[0].has_actor("导演"));
}

#[test]
fn normalized_path_wins_and_metadata_fallback_is_explicitly_uncertain() {
    let exact = JellyfinItem::fixture(
        "one",
        "Blue Room",
        Some(r"C:\Media\JAV\ABC-123.mp4"),
        Some("ABC-123"),
    );
    let fallback = JellyfinItem::fixture(
        "two",
        "XYZ-999",
        Some("/unrelated/file.mkv"),
        Some("XYZ-999"),
    );

    let by_path = associate(
        r"c:/media/jav/./ABC-123.mp4",
        Some("WRONG-1"),
        None,
        &[exact.clone(), fallback.clone()],
    )
    .unwrap();
    assert_eq!(by_path.item_id, "one");
    assert_eq!(by_path.confidence, AssociationConfidence::CertainPath);
    assert!(by_path.may_authorize_deletion());

    let by_metadata = associate(
        "/media/missing.mp4",
        Some("XYZ-999"),
        None,
        &[exact, fallback],
    )
    .unwrap();
    assert_eq!(by_metadata.item_id, "two");
    assert_eq!(
        by_metadata.confidence,
        AssociationConfidence::UncertainMetadata
    );
    assert!(!by_metadata.may_authorize_deletion());
}

#[test]
fn unique_media_relative_path_suffix_matches_different_mount_prefixes() {
    let matching = JellyfinItem::fixture(
        "midv-821",
        "MIDV-821 full title",
        Some("/media/jav/CHINESE/MIDV-821-C/MIDV-821-C.mp4"),
        None,
    );
    let unrelated = JellyfinItem::fixture(
        "other",
        "Other",
        Some("/media/jav/OTHER/OTHER-001.mp4"),
        None,
    );

    let association = associate(
        "/media/CHINESE/MIDV-821-C/MIDV-821-C.mp4",
        Some("MIDV-821"),
        None,
        &[matching, unrelated],
    )
    .unwrap();

    assert_eq!(association.item_id, "midv-821");
    assert_eq!(association.confidence, AssociationConfidence::CertainPath);
    assert!(association.reason.contains("relative path suffix"));
}

#[tokio::test]
async fn batch_refresh_is_one_request_and_retries_at_most_five_attempts() {
    let (url, state) = mock_server(4).await;
    let client = JellyfinClient::new(
        JellyfinConfig {
            url,
            library_ids: vec!["jav".into()],
        },
        "key".into(),
    )
    .unwrap();
    let policy = RetryPolicy {
        max_attempts: 5,
        base_delay: Duration::ZERO,
        max_delay: Duration::ZERO,
    };

    assert_eq!(
        client.refresh_batch(policy).await,
        RefreshOutcome::Completed { attempts: 5 }
    );
    assert_eq!(state.0.lock().unwrap().refreshes, 5);

    let (url, state) = mock_server(99).await;
    let client = JellyfinClient::new(
        JellyfinConfig {
            url,
            library_ids: vec!["jav".into()],
        },
        "key".into(),
    )
    .unwrap();
    assert_eq!(
        client.refresh_batch(policy).await,
        RefreshOutcome::ManualRetryRequired { attempts: 5 }
    );
    assert_eq!(state.0.lock().unwrap().refreshes, 5);
}
