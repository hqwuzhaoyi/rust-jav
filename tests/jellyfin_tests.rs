use std::{collections::BTreeMap, time::Duration};

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use rust_jav::jellyfin::{
    associate, AssociationConfidence, JellyfinClient, JellyfinConfig, JellyfinItem,
    JellyfinLibrary, RefreshOutcome, RetryPolicy,
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
            {"Id":"exact","Name":"Blue Room","Path":"/media/jav/ABC-123.mp4","ProviderIds":{"Jav":"ABC-123"},"UserData":{"Played":true,"PlayCount":2,"PlaybackPositionTicks":0}},
            {"Id":"fallback","Name":"XYZ-999","Path":"/other/XYZ-999.mkv","ProviderIds":{},"UserData":{"Played":false,"PlayCount":0,"PlaybackPositionTicks":42}}
        ],"TotalRecordCount":2}))
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
        .route("/Library/Refresh", post(refresh))
        .with_state(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), state)
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
