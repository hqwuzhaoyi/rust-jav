use std::{
    collections::HashMap,
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{header, HeaderMap, HeaderValue, Response, StatusCode, Uri},
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Sse,
    },
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use futures::stream;
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::convert::Infallible;

use crate::{
    application::{ApplicationServices, OperationsRequest},
    asset_index::{AssetIndex, AssetQuery, AssetState, ScanMode},
    management_tasks::{NewTask, TaskCoordinator, TaskKind, TaskStore},
    tui::state::OperationType,
};

const PASSWORD_ENV: &str = "RUST_JAV_ADMIN_PASSWORD";
const COOKIE_NAME: &str = "rust_jav_session";
const INDEX_HTML: &[u8] = include_bytes!("../frontend/dist/index.html");
const APP_JS: &[u8] = include_bytes!("../frontend/dist/assets/app.js");
const APP_CSS: &[u8] = include_bytes!("../frontend/dist/assets/app.css");

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unable to read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unable to write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid YAML in {path}: {source}")]
    Yaml {
        path: PathBuf,
        source: serde_yaml::Error,
    },
    #[error("Administrator is already configured")]
    AlreadyConfigured,
    #[error("{PASSWORD_ENV} must contain at least 12 characters")]
    InvalidPassword,
    #[error("{PASSWORD_ENV} is required for a local password reset")]
    MissingPassword,
    #[error("password hashing failed")]
    PasswordHash,
    #[error("server failed: {0}")]
    Server(#[from] std::io::Error),
    #[error(transparent)]
    Tasks(#[from] crate::management_tasks::Error),
    #[error(transparent)]
    Assets(#[from] crate::asset_index::Error),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ManagementYaml {
    port: u16,
    container: bool,
    session_ttl_seconds: u64,
    secrets_file: PathBuf,
    media_roots: Vec<PathBuf>,
    actor_view_root: Option<PathBuf>,
}

impl Default for ManagementYaml {
    fn default() -> Self {
        Self {
            port: 9317,
            container: false,
            session_ttl_seconds: 43_200,
            secrets_file: PathBuf::from("management.secrets.yaml"),
            media_roots: Vec::new(),
            actor_view_root: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ManagementConfig {
    pub port: u16,
    pub container: bool,
    pub session_ttl: Duration,
    pub secrets_file: PathBuf,
    pub media_roots: Vec<PathBuf>,
    pub actor_view_root: Option<PathBuf>,
}

impl ManagementConfig {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let contents = fs::read_to_string(path).map_err(|source| Error::Read {
            path: path.to_owned(),
            source,
        })?;
        let raw: ManagementYaml =
            serde_yaml::from_str(&contents).map_err(|source| Error::Yaml {
                path: path.to_owned(),
                source,
            })?;
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let secrets_file = if raw.secrets_file.is_absolute() {
            raw.secrets_file
        } else {
            parent.join(raw.secrets_file)
        };
        let media_roots = raw
            .media_roots
            .into_iter()
            .map(|root| {
                if root.is_absolute() {
                    root
                } else {
                    parent.join(root)
                }
            })
            .collect();
        let actor_view_root = raw.actor_view_root.map(|root| {
            if root.is_absolute() {
                root
            } else {
                parent.join(root)
            }
        });
        Ok(Self {
            port: raw.port,
            container: raw.container,
            session_ttl: Duration::from_secs(raw.session_ttl_seconds),
            secrets_file,
            media_roots,
            actor_view_root,
        })
    }

    pub fn listen_addr(&self) -> SocketAddr {
        let ip = if self.container {
            Ipv4Addr::UNSPECIFIED
        } else {
            Ipv4Addr::LOCALHOST
        };
        SocketAddr::new(IpAddr::V4(ip), self.port)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Secrets {
    #[serde(skip_serializing_if = "Option::is_none")]
    password_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bootstrap_token_hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SecretsStore(PathBuf);

impl SecretsStore {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    fn load(&self) -> Result<Secrets, Error> {
        if !self.0.exists() {
            return Ok(Secrets::default());
        }
        let contents = fs::read_to_string(&self.0).map_err(|source| Error::Read {
            path: self.0.clone(),
            source,
        })?;
        serde_yaml::from_str(&contents).map_err(|source| Error::Yaml {
            path: self.0.clone(),
            source,
        })
    }

    fn save(&self, secrets: &Secrets) -> Result<(), Error> {
        if let Some(parent) = self.0.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::Write {
                path: parent.to_owned(),
                source,
            })?;
        }
        let yaml = serde_yaml::to_string(secrets).map_err(|source| Error::Yaml {
            path: self.0.clone(),
            source,
        })?;
        let temporary = self.0.with_extension("yaml.tmp");
        fs::write(&temporary, yaml).map_err(|source| Error::Write {
            path: temporary.clone(),
            source,
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600)).map_err(
                |source| Error::Write {
                    path: temporary.clone(),
                    source,
                },
            )?;
        }
        fs::rename(&temporary, &self.0).map_err(|source| Error::Write {
            path: self.0.clone(),
            source,
        })
    }
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn hash_password(password: &str) -> Result<String, Error> {
    if password.len() < 12 {
        return Err(Error::InvalidPassword);
    }
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| Error::PasswordHash)
}

pub fn init_administrator(store: &SecretsStore) -> Result<String, Error> {
    let mut secrets = store.load()?;
    if secrets.password_hash.is_some() {
        return Err(Error::AlreadyConfigured);
    }
    let token = random_token();
    secrets.bootstrap_token_hash = Some(token_hash(&token));
    store.save(&secrets)?;
    Ok(token)
}

pub fn password_secrets(store: &SecretsStore, password: &str) -> Result<(), Error> {
    let mut secrets = store.load()?;
    secrets.password_hash = Some(hash_password(password)?);
    secrets.bootstrap_token_hash = None;
    store.save(&secrets)
}

pub fn reset_password_from_env(store: &SecretsStore) -> Result<(), Error> {
    let password = std::env::var(PASSWORD_ENV).map_err(|_| Error::MissingPassword)?;
    password_secrets(store, &password)
}

pub trait Clock: Send + Sync + 'static {
    fn unix_seconds(&self) -> u64;
}

#[derive(Clone)]
struct SystemClock;
impl Clock for SystemClock {
    fn unix_seconds(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

#[derive(Clone)]
pub struct AppState {
    config: ManagementConfig,
    store: SecretsStore,
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    clock: Arc<RwLock<Box<dyn Clock>>>,
    tasks: TaskStore,
    coordinator: TaskCoordinator,
    assets: AssetIndex,
}

#[derive(Clone)]
struct Session {
    expires_at: u64,
    credential_version: String,
}

impl AppState {
    pub fn new(config: ManagementConfig, clock: impl Clock) -> Result<Self, Error> {
        let now = clock.unix_seconds();
        let database = config.secrets_file.with_file_name("management.sqlite3");
        let tasks = TaskStore::open(&database)?;
        let assets = AssetIndex::open(&database)?;
        // A missing or incorrectly-permissioned TrueNAS mount degrades the
        // rebuildable index; it must not prevent the diagnostic API starting.
        let _ = assets.reconcile(&config.media_roots, ScanMode::Startup, now);
        tasks.interrupt_running_destructive(now)?;
        Ok(Self {
            store: SecretsStore::new(config.secrets_file.clone()),
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            clock: Arc::new(RwLock::new(Box::new(clock))),
            tasks,
            coordinator: TaskCoordinator::new(),
            assets,
        })
    }
    pub fn set_clock(&self, clock: impl Clock) {
        *self.clock.write().unwrap() = Box::new(clock);
    }
    fn now(&self) -> u64 {
        self.clock.read().unwrap().unix_seconds()
    }
}

#[derive(Deserialize)]
struct InitializeRequest {
    token: String,
    password: String,
}
#[derive(Deserialize)]
struct LoginRequest {
    password: String,
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/auth/initialize", post(initialize))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/status", get(status))
        .route("/api/v1/openapi.json", get(openapi))
        .route("/api/v1/tasks", post(create_task).get(list_tasks))
        .route("/api/v1/tasks/:task_id", get(get_task))
        .route("/api/v1/tasks/:task_id/events", get(task_events))
        .route("/api/v1/assets", get(list_assets))
        .route("/api/v1/assets/health", get(asset_health))
        .route("/api/v1/assets/scan", post(scan_assets))
        .route("/api/v1/assets/:asset_id", get(asset_detail))
        .route("/api/v1/assets/:asset_id/artwork", get(indexed_artwork))
        .route("/api/v1/actors", get(list_actor_folders))
        .route(
            "/api/v1/actors/:actor_name",
            get(actor_folder_confirmation).delete(remove_actor_folder_task),
        )
        .route("/api/v1/actors/:actor_name/poster", get(actor_poster))
        .route("/api/v1/media-roots/health", get(media_root_health))
        .route(
            "/assets/app.js",
            get(|| async { asset("application/javascript; charset=utf-8", APP_JS) }),
        )
        .route(
            "/assets/app.css",
            get(|| async { asset("text/css; charset=utf-8", APP_CSS) }),
        )
        .fallback(spa_fallback)
        .with_state(state)
}

async fn initialize(
    State(state): State<AppState>,
    Json(input): Json<InitializeRequest>,
) -> StatusCode {
    let mut secrets = match state.store.load() {
        Ok(value) => value,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };
    if secrets.password_hash.is_some() {
        return StatusCode::CONFLICT;
    }
    let Some(expected) = secrets.bootstrap_token_hash.as_deref() else {
        return StatusCode::FORBIDDEN;
    };
    if token_hash(&input.token) != expected {
        return StatusCode::FORBIDDEN;
    }
    let Ok(password_hash) = hash_password(&input.password) else {
        return StatusCode::BAD_REQUEST;
    };
    secrets.password_hash = Some(password_hash);
    secrets.bootstrap_token_hash = None;
    match state.store.save(&secrets) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn login(
    State(state): State<AppState>,
    Json(input): Json<LoginRequest>,
) -> impl IntoResponse {
    let Ok(secrets) = state.store.load() else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Some(encoded) = secrets.password_hash else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let valid = PasswordHash::new(&encoded)
        .ok()
        .and_then(|hash| {
            Argon2::default()
                .verify_password(input.password.as_bytes(), &hash)
                .ok()
        })
        .is_some();
    if !valid {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let token = random_token();
    let expires = state
        .now()
        .saturating_add(state.config.session_ttl.as_secs());
    state.sessions.write().unwrap().insert(
        token_hash(&token),
        Session {
            expires_at: expires,
            credential_version: token_hash(&encoded),
        },
    );
    let cookie = format!(
        "{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}",
        state.config.session_ttl.as_secs()
    );
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());
    response
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(token) = session_cookie(&headers) {
        state.sessions.write().unwrap().remove(&token_hash(&token));
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("rust_jav_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"),
    );
    response
}

async fn status(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let Ok(secrets) = state.store.load() else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    if secrets.password_hash.is_none() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if !authenticated(&state, &headers, secrets.password_hash.as_deref().unwrap()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(serde_json::json!({"version": env!("CARGO_PKG_VERSION")})).into_response()
}

#[derive(Debug, Default, Deserialize)]
struct AssetQueryParams {
    q: Option<String>,
    state: Option<String>,
    page: Option<usize>,
    per_page: Option<usize>,
}

async fn list_assets(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(input): Query<AssetQueryParams>,
) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    let asset_state = match input.state.as_deref() {
        None => None,
        Some("normal") => Some(AssetState::Normal),
        Some("synchronizing") => Some(AssetState::Synchronizing),
        Some("exception") => Some(AssetState::Exception),
        Some(_) => {
            return (
                StatusCode::BAD_REQUEST,
                "state must be normal, synchronizing, or exception",
            )
                .into_response()
        }
    };
    match state.assets.search(AssetQuery {
        query: input.q,
        state: asset_state,
        page: input.page.unwrap_or(1),
        per_page: input.per_page.unwrap_or(48),
    }) {
        Ok(page) => Json(page).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn asset_health(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    match state.assets.health_json() {
        Ok(value) => Json(value).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct ScanAssetsRequest {
    mode: String,
    media_root: Option<PathBuf>,
    #[serde(default)]
    paths: Vec<PathBuf>,
}

async fn scan_assets(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ScanAssetsRequest>,
) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    let result = match input.mode.as_str() {
        "manual" => {
            state
                .assets
                .reconcile(&state.config.media_roots, ScanMode::Manual, state.now())
        }
        "incremental" => match input.media_root {
            Some(root)
                if state.config.media_roots.contains(&root)
                    && !input.paths.iter().any(|path| !path.starts_with(&root)) =>
            {
                state
                    .assets
                    .reconcile_paths(&root, &input.paths, state.now())
            }
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    "incremental paths must be inside a configured Media Root",
                )
                    .into_response()
            }
        },
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                "mode must be manual or incremental",
            )
                .into_response()
        }
    };
    match result {
        Ok(()) => Json(state.assets.health_json().unwrap_or_default()).into_response(),
        Err(error) => (StatusCode::UNPROCESSABLE_ENTITY, error.to_string()).into_response(),
    }
}

async fn media_root_health(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    Json(
        state
            .config
            .media_roots
            .iter()
            .map(|root| state.assets.root_health(root))
            .collect::<Vec<_>>(),
    )
    .into_response()
}

async fn indexed_artwork(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(asset_id): AxumPath<String>,
) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    let Ok(Some(path)) = state.assets.indexed_artwork(&asset_id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(bytes) = fs::read(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = match path
        .extension()
        .and_then(|v| v.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(bytes))
        .unwrap()
        .into_response()
}

async fn asset_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(asset_id): AxumPath<String>,
) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    match state.assets.detail(&asset_id) {
        Ok(Some(detail)) => Json(detail).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Serialize)]
struct ActorFolderResponse {
    name: String,
    movie_count: usize,
    hard_link_count: usize,
    logical_size: u64,
    reclaimable_space: u64,
    poster_url: Option<String>,
}

fn actor_response(folder: crate::actor_views::ActorFolder) -> ActorFolderResponse {
    let poster_url = folder
        .poster_path
        .map(|_| format!("/api/v1/actors/{}/poster", folder.name));
    ActorFolderResponse {
        name: folder.name,
        movie_count: folder.movie_count,
        hard_link_count: folder.hard_link_count,
        logical_size: folder.logical_size,
        reclaimable_space: folder.reclaimable_space,
        poster_url,
    }
}

async fn list_actor_folders(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    let Some(root) = state.config.actor_view_root.as_deref() else {
        return Json(Vec::<ActorFolderResponse>::new()).into_response();
    };
    match crate::actor_views::browse_actor_folders(root) {
        Ok(folders) => {
            Json(folders.into_iter().map(actor_response).collect::<Vec<_>>()).into_response()
        }
        Err(error) => (StatusCode::UNPROCESSABLE_ENTITY, error.to_string()).into_response(),
    }
}

async fn actor_folder_confirmation(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(actor_name): AxumPath<String>,
) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    let Some(root) = state.config.actor_view_root.as_deref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match crate::actor_views::browse_actor_folders(root) {
        Ok(folders) => folders
            .into_iter()
            .find(|folder| folder.name == actor_name)
            .map(|folder| Json(actor_response(folder)).into_response())
            .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response()),
        Err(error) => (StatusCode::UNPROCESSABLE_ENTITY, error.to_string()).into_response(),
    }
}

async fn actor_poster(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(actor_name): AxumPath<String>,
) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    let Some(root) = state.config.actor_view_root.as_deref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(Some(path)) = crate::actor_views::browse_actor_folders(root).map(|folders| {
        folders
            .into_iter()
            .find(|folder| folder.name == actor_name)
            .and_then(|folder| folder.poster_path)
    }) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(bytes) = fs::read(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = match path
        .extension()
        .and_then(|v| v.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(bytes))
        .unwrap()
        .into_response()
}

async fn remove_actor_folder_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(actor_name): AxumPath<String>,
) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    let Some(root) = state.config.actor_view_root.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let exists = crate::actor_views::browse_actor_folders(&root)
        .map(|folders| folders.iter().any(|folder| folder.name == actor_name))
        .unwrap_or(false);
    if !exists {
        return StatusCode::NOT_FOUND.into_response();
    }
    let task = match state.tasks.create(
        NewTask::mutation("remove_actor_folder", root.display().to_string()),
        state.now(),
    ) {
        Ok(task) => task,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let id = task.id.clone();
    tokio::spawn(run_actor_removal_task(state, id, root, actor_name));
    (StatusCode::ACCEPTED, Json(task)).into_response()
}

async fn run_actor_removal_task(
    state: AppState,
    task_id: String,
    root: PathBuf,
    actor_name: String,
) {
    let _lease = state
        .coordinator
        .mutation(&root.display().to_string())
        .await;
    if state.tasks.mark_running(&task_id, state.now()).is_err() {
        return;
    }
    match crate::actor_views::remove_actor_folder(&root, &actor_name) {
        Ok(outcomes) => {
            for outcome in outcomes {
                if state
                    .tasks
                    .finish_item(
                        &task_id,
                        &outcome.kind,
                        Some(&outcome.path.display().to_string()),
                        &outcome.status,
                        outcome.message.as_deref(),
                    )
                    .is_err()
                {
                    return;
                }
            }
            let _ = state.tasks.mark_completed(&task_id, state.now());
        }
        Err(error) => {
            let _ = state
                .tasks
                .mark_failed(&task_id, state.now(), &error.to_string());
        }
    }
}

fn authorized(state: &AppState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let secrets = state
        .store
        .load()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let password_hash = secrets
        .password_hash
        .as_deref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    if authenticated(state, headers, password_hash) {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[derive(Debug, Deserialize)]
struct CreateTaskRequest {
    task_type: String,
    media_root: PathBuf,
    mode: String,
    #[serde(default)]
    operations: Vec<String>,
}

async fn create_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateTaskRequest>,
) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    if input.task_type != "operations" || !matches!(input.mode.as_str(), "preview" | "apply") {
        return (
            StatusCode::BAD_REQUEST,
            "task_type must be operations and mode must be preview or apply",
        )
            .into_response();
    }
    let operations = match input
        .operations
        .iter()
        .map(|value| parse_operation(value))
        .collect::<Option<Vec<_>>>()
    {
        Some(operations) if !operations.is_empty() => operations,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                "operations must contain known operation names",
            )
                .into_response()
        }
    };
    let media_root = input.media_root.display().to_string();
    let kind = if input.mode == "apply" {
        TaskKind::Mutation
    } else {
        TaskKind::Preview
    };
    let new_task = if kind == TaskKind::Mutation {
        NewTask::mutation(&input.task_type, &media_root)
    } else {
        NewTask::preview(&input.task_type, &media_root)
    };
    let task = match state.tasks.create(new_task, state.now()) {
        Ok(task) => task,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let task_id = task.id.clone();
    tokio::spawn(run_operations_task(
        state,
        task_id,
        input.media_root,
        operations,
        kind,
    ));
    (StatusCode::ACCEPTED, Json(task)).into_response()
}

async fn run_operations_task(
    state: AppState,
    task_id: String,
    media_root: PathBuf,
    operations: Vec<OperationType>,
    kind: TaskKind,
) {
    let _lease = if kind == TaskKind::Mutation {
        Some(
            state
                .coordinator
                .mutation(&media_root.display().to_string())
                .await,
        )
    } else {
        state
            .coordinator
            .preview(&media_root.display().to_string())
            .await;
        None
    };
    if state.tasks.mark_running(&task_id, state.now()).is_err() {
        return;
    }
    let request = if kind == TaskKind::Mutation {
        OperationsRequest::apply(media_root, operations)
    } else {
        OperationsRequest::preview(media_root, operations)
    };
    let report = ApplicationServices::new().operations().run(request).await;
    for action in &report.actions {
        let path = action
            .source
            .as_ref()
            .or(action.target.as_ref())
            .map(|path| path.display().to_string());
        if state
            .tasks
            .finish_item(
                &task_id,
                &action.kind,
                path.as_deref(),
                action.status.as_str(),
                action.reason.as_deref(),
            )
            .is_err()
        {
            return;
        }
    }
    let now = state.now();
    if report.summary.failed_actions > 0 || !report.errors.is_empty() {
        let message = report
            .errors
            .first()
            .map(String::as_str)
            .unwrap_or("one or more task items failed");
        let _ = state.tasks.mark_failed(&task_id, now, message);
    } else {
        let _ = state.tasks.mark_completed(&task_id, now);
    }
}

async fn list_tasks(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    match state.tasks.list() {
        Ok(tasks) => Json(tasks).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn get_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(task_id): AxumPath<String>,
) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    match state.tasks.get(&task_id) {
        Ok(Some(task)) => Json(task).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn task_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(task_id): AxumPath<String>,
) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    if !matches!(state.tasks.get(&task_id), Ok(Some(_))) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let events = stream::unfold(
        Some((state.tasks.clone(), task_id, true)),
        |stream_state| async move {
            let (store, id, first) = stream_state?;
            if !first {
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            let task = store.get(&id).ok().flatten()?;
            let terminal = task.status.is_terminal();
            let data = serde_json::to_string(&task).ok()?;
            let event = Event::default()
                .event("task")
                .id(task.id.clone())
                .data(data);
            let next = (!terminal).then_some((store, id, false));
            Some((Ok::<_, Infallible>(event), next))
        },
    );
    Sse::new(events)
        .keep_alive(KeepAlive::default())
        .into_response()
}

fn parse_operation(value: &str) -> Option<OperationType> {
    Some(match value {
        "delete_ad_files" => OperationType::DeleteAdFiles,
        "organize_by_code" => OperationType::OrganizeByCode,
        "clean_empty_dirs" => OperationType::CleanEmptyDirs,
        "standardize_names" => OperationType::StandardizeNames,
        "extract_codes" => OperationType::ExtractCodes,
        "categorize_files" => OperationType::CategorizeFiles,
        "move_origin" => OperationType::MoveOrigin,
        "remove_duplicates" => OperationType::RemoveDuplicates,
        _ => return None,
    })
}

async fn openapi(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    Json(openapi_document()).into_response()
}

fn openapi_document() -> serde_json::Value {
    serde_json::json!({
        "openapi": "3.1.0",
        "info": {"title": "rust-jav Management API", "version": env!("CARGO_PKG_VERSION")},
        "paths": {
            "/api/v1/tasks": {
                "get": {"summary": "List Management Tasks", "responses": {"200": {"description": "Tasks", "content":{"application/json":{"schema":{"type":"array","items":{"$ref":"#/components/schemas/ManagementTask"}}}}}}},
                "post": {"summary": "Create a Management Task", "requestBody":{"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/CreateTaskRequest"}}}}, "responses": {"202": {"description": "Accepted", "content":{"application/json":{"schema":{"$ref":"#/components/schemas/ManagementTask"}}}}, "400": {"description": "Invalid task"}}}
            },
            "/api/v1/tasks/{task_id}": {"get": {"summary": "Get a Management Task", "parameters": [{"name":"task_id","in":"path","required":true,"schema":{"type":"string"}}], "responses":{"200":{"description":"Task","content":{"application/json":{"schema":{"$ref":"#/components/schemas/ManagementTask"}}}},"404":{"description":"Not found"}}}},
            "/api/v1/tasks/{task_id}/events": {"get": {"summary": "Stream Management Task lifecycle", "parameters": [{"name":"task_id","in":"path","required":true,"schema":{"type":"string"}}], "responses":{"200":{"description":"Task snapshots","content":{"text/event-stream":{"schema":{"type":"string"}}}},"404":{"description":"Not found"}}}},
            "/api/v1/assets": {"get":{"summary":"Search date-grouped Media Assets","parameters":[
                {"name":"q","in":"query","schema":{"type":"string"}},{"name":"state","in":"query","schema":{"type":"string","enum":["normal","synchronizing","exception"]}},{"name":"page","in":"query","schema":{"type":"integer","minimum":1}},{"name":"per_page","in":"query","schema":{"type":"integer","minimum":1,"maximum":200}}
            ],"responses":{"200":{"description":"Paginated assets","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AssetPage"}}}}}}},
            "/api/v1/assets/{asset_id}":{"get":{"summary":"Get parsed NFO and Actor information for a Media Asset","parameters":[{"name":"asset_id","in":"path","required":true,"schema":{"type":"string"}}],"responses":{"200":{"description":"Asset detail","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AssetDetail"}}}},"404":{"description":"Media Asset not found"}}}},
            "/api/v1/assets/health":{"get":{"summary":"Get Asset Index reconciliation health","responses":{"200":{"description":"Index health"}}}},
            "/api/v1/assets/scan":{"post":{"summary":"Run manual or incremental reconciliation","requestBody":{"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/ScanAssetsRequest"}}}},"responses":{"200":{"description":"Reconciled"},"422":{"description":"Filesystem scan failed"}}}},
            "/api/v1/assets/{asset_id}/artwork":{"get":{"summary":"Serve artwork belonging to an indexed Media Asset","parameters":[{"name":"asset_id","in":"path","required":true,"schema":{"type":"string"}}],"responses":{"200":{"description":"Indexed image","content":{"image/jpeg":{},"image/png":{},"image/webp":{}}},"404":{"description":"No indexed artwork"}}}},
            "/api/v1/actors":{"get":{"summary":"Browse derived Actor Folders with inode-aware storage metrics","responses":{"200":{"description":"Actor Folders","content":{"application/json":{"schema":{"type":"array","items":{"$ref":"#/components/schemas/ActorFolder"}}}}}}}},
            "/api/v1/actors/{actor_name}":{"get":{"summary":"Recompute Actor Folder removal confirmation","responses":{"200":{"description":"Fresh confirmation metrics"}}},"delete":{"summary":"Remove derived paths as a Management Task","responses":{"202":{"description":"Accepted Management Task"},"404":{"description":"Actor Folder not found"}}}},
            "/api/v1/media-roots/health":{"get":{"summary":"Report TrueNAS Host Path access and process UID/GID","responses":{"200":{"description":"Media Root permission reports"}}}}
        },
        "components": {"schemas": {
            "CreateTaskRequest": {"type":"object","required":["task_type","media_root","mode","operations"],"properties":{
                "task_type":{"type":"string","const":"operations"},"media_root":{"type":"string"},"mode":{"type":"string","enum":["preview","apply"]},"operations":{"type":"array","minItems":1,"items":{"type":"string"}}
            }},
            "TaskItem": {"type":"object","required":["id","kind","status"],"properties":{"id":{"type":"integer"},"kind":{"type":"string"},"path":{"type":["string","null"]},"status":{"type":"string"},"message":{"type":["string","null"]}}},
            "ScanAssetsRequest":{"type":"object","required":["mode"],"properties":{"mode":{"type":"string","enum":["manual","incremental"]},"media_root":{"type":"string"},"paths":{"type":"array","items":{"type":"string"}}}},
            "MediaAsset":{"type":"object","required":["id","media_root","path","device","inode","observed_at","captured_date","state"],"properties":{"id":{"type":"string"},"media_root":{"type":"string"},"path":{"type":"string"},"device":{"type":"integer"},"inode":{"type":"integer"},"jav_code":{"type":["string","null"]},"title":{"type":["string","null"]},"nfo_path":{"type":["string","null"]},"artwork_url":{"type":["string","null"]},"observed_at":{"type":"integer"},"captured_date":{"type":"string","format":"date"},"state":{"type":"string","enum":["normal","synchronizing","exception"]},"exception":{"type":["string","null"]}}},
            "AssetActor":{"type":"object","required":["name"],"properties":{"name":{"type":"string"},"poster_url":{"type":["string","null"]},"actor_folder_url":{"type":["string","null"]}}},
            "ActorFolder":{"type":"object","required":["name","movie_count","hard_link_count","logical_size","reclaimable_space"],"properties":{"name":{"type":"string"},"movie_count":{"type":"integer"},"hard_link_count":{"type":"integer"},"logical_size":{"type":"integer"},"reclaimable_space":{"type":"integer"},"poster_url":{"type":["string","null"]}}},
            "AssetDetail":{"type":"object","required":["id","path","actors","tags","parse_status","state"],"properties":{"id":{"type":"string"},"path":{"type":"string"},"title":{"type":["string","null"]},"actors":{"type":"array","items":{"$ref":"#/components/schemas/AssetActor"}},"studio":{"type":["string","null"]},"release_date":{"type":["string","null"],"format":"date"},"runtime_minutes":{"type":["integer","null"]},"director":{"type":["string","null"]},"tags":{"type":"array","items":{"type":"string"}},"plot":{"type":["string","null"]},"parse_status":{"type":"string","enum":["valid","missing","invalid"]},"source_path":{"type":["string","null"]},"state":{"type":"string","enum":["normal","synchronizing","exception"]},"exception":{"type":["string","null"]}}},
            "AssetPage":{"type":"object","required":["items","groups","page","per_page","total","total_pages"],"properties":{"items":{"type":"array","items":{"$ref":"#/components/schemas/MediaAsset"}},"groups":{"type":"array","items":{"type":"object","properties":{"date":{"type":"string","format":"date"},"count":{"type":"integer"}}}},"page":{"type":"integer"},"per_page":{"type":"integer"},"total":{"type":"integer"},"total_pages":{"type":"integer"}}},
            "ManagementTask": {"type":"object","required":["id","task_type","media_root","kind","status","created_at","items"],"properties":{
                "id":{"type":"string"},"task_type":{"type":"string"},"media_root":{"type":"string"},"kind":{"type":"string","enum":["preview","mutation"]},"status":{"type":"string","enum":["queued","running","completed","failed","interrupted"]},"created_at":{"type":"integer"},"started_at":{"type":["integer","null"]},"finished_at":{"type":["integer","null"]},"error":{"type":["string","null"]},"items":{"type":"array","items":{"$ref":"#/components/schemas/TaskItem"}}
            }}
        }}
    })
}

fn session_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            (name == COOKIE_NAME).then(|| value.to_owned())
        })
}

fn authenticated(state: &AppState, headers: &HeaderMap, password_hash: &str) -> bool {
    let Some(token) = session_cookie(headers) else {
        return false;
    };
    let hash = token_hash(&token);
    let now = state.now();
    let mut sessions = state.sessions.write().unwrap();
    sessions.retain(|_, session| session.expires_at > now);
    sessions.get(&hash).is_some_and(|session| {
        session.expires_at > now && session.credential_version == token_hash(password_hash)
    })
}

fn asset(content_type: &'static str, bytes: &'static [u8]) -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .body(Body::from(bytes))
        .unwrap()
}

async fn spa_fallback(uri: Uri) -> Response<Body> {
    if uri.path().starts_with("/api/") {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap();
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(INDEX_HTML))
        .unwrap()
}

pub async fn serve(config: ManagementConfig) -> Result<(), Error> {
    let store = SecretsStore::new(config.secrets_file.clone());
    if let Ok(password) = std::env::var(PASSWORD_ENV) {
        if store.load()?.password_hash.is_none() {
            password_secrets(&store, &password)?;
        }
    }
    let address = config.listen_addr();
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("Management Interface listening on http://{address}");
    axum::serve(listener, app(AppState::new(config, SystemClock)?))
        .await
        .map_err(Error::Server)
}
