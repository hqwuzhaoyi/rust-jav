use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    collections::{HashMap, HashSet},
    fs,
    future::Future,
    io::Read,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex, RwLock},
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
use futures::{stream, StreamExt};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

const MIN_PASSWORD_LENGTH: usize = 4;

use crate::active_rules::{ActiveRuleSet, ActiveRuleSetError};
use std::convert::Infallible;

use crate::{
    application::{ApplicationServices, OperationsRequest},
    asset_index::{AssetIndex, AssetQuery, AssetState, MediaRootHealth, ScanMode},
    deletion_plan::{
        DeletionOutcomeStatus, FileType as DeletionFileType, PermanentDeletionPlan,
        PermanentDeletionPlanner, RelatedHardLink,
    },
    jellyfin::{
        associate, match_person, JellyfinClient, JellyfinConfig, JellyfinImage, JellyfinPerson,
        RefreshOutcome, RetryPolicy,
    },
    management_tasks::{NewTask, TaskCoordinator, TaskKind, TaskStore},
    operations::{ConfirmedAction, SourceIdentity},
    tui::{executor::PlannedAction, state::OperationType},
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
    #[error("{PASSWORD_ENV} must contain at least 4 characters")]
    InvalidPassword,
    #[error("{PASSWORD_ENV} is required for a local password reset")]
    MissingPassword,
    #[error("password hashing failed")]
    PasswordHash,
    #[error("invalid Active Rule Set in {path}: {message}")]
    ActiveRules { path: PathBuf, message: String },
    #[error("server failed: {0}")]
    Server(#[from] std::io::Error),
    #[error(transparent)]
    Tasks(#[from] crate::management_tasks::Error),
    #[error(transparent)]
    Assets(#[from] crate::asset_index::Error),
    #[error("TrueNAS deployment validation failed: {0}")]
    Deployment(String),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ManagementYaml {
    port: u16,
    container: bool,
    session_ttl_seconds: u64,
    secrets_file: PathBuf,
    active_rule_set_file: PathBuf,
    rule_source_hosts: Vec<String>,
    rule_download_timeout_seconds: u64,
    rule_download_max_bytes: usize,
    database_file: PathBuf,
    artwork_cache_root: Option<PathBuf>,
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
            active_rule_set_file: PathBuf::from("active-rules.yaml"),
            rule_source_hosts: vec!["raw.githubusercontent.com".to_owned()],
            rule_download_timeout_seconds: 10,
            rule_download_max_bytes: 1_048_576,
            database_file: PathBuf::from("management.sqlite3"),
            artwork_cache_root: None,
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
    pub active_rule_set_file: PathBuf,
    pub rule_source_hosts: Vec<String>,
    pub rule_download_timeout: Duration,
    pub rule_download_max_bytes: usize,
    pub database_file: PathBuf,
    pub artwork_cache_root: Option<PathBuf>,
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
        let active_rule_set_file = if raw.active_rule_set_file.is_absolute() {
            raw.active_rule_set_file
        } else {
            parent.join(raw.active_rule_set_file)
        };
        let database_file = if raw.database_file.is_absolute() {
            raw.database_file
        } else {
            parent.join(raw.database_file)
        };
        let artwork_cache_root = raw.artwork_cache_root.map(|root| {
            if root.is_absolute() {
                root
            } else {
                parent.join(root)
            }
        });
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
            active_rule_set_file,
            rule_source_hosts: raw.rule_source_hosts,
            rule_download_timeout: Duration::from_secs(raw.rule_download_timeout_seconds),
            rule_download_max_bytes: raw.rule_download_max_bytes,
            database_file,
            artwork_cache_root,
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

    pub fn validate_truenas_mounts(&self) -> Result<DeploymentReport, Error> {
        let config_root = self.secrets_file.parent().unwrap_or_else(|| Path::new("."));
        let state_root = self
            .database_file
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let cache_root = self.artwork_cache_root.as_deref().ok_or_else(|| {
            Error::Deployment("artwork_cache_root must use a distinct Host Path".into())
        })?;
        let actor_root = self.actor_view_root.as_deref().ok_or_else(|| {
            Error::Deployment("actor_view_root must use a distinct Host Path".into())
        })?;
        if self.media_roots.is_empty() {
            return Err(Error::Deployment(
                "at least one Media Root is required".into(),
            ));
        }
        let mut paths = vec![config_root, state_root, cache_root, actor_root];
        paths.extend(self.media_roots.iter().map(PathBuf::as_path));
        for (index, left) in paths.iter().enumerate() {
            for right in paths.iter().skip(index + 1) {
                if left == right || left.starts_with(right) || right.starts_with(left) {
                    return Err(Error::Deployment(format!("configuration, SQLite, artwork/cache, Media Roots, and Actor View must use distinct Host Paths; '{}' overlaps '{}'", left.display(), right.display())));
                }
            }
        }
        let media_mounts = self
            .media_roots
            .iter()
            .map(|path| inspect_mount(path, MountRole::MediaRoot))
            .collect::<Result<Vec<_>, _>>()?;
        let actor_device = fs::metadata(actor_root)
            .map_err(|error| mount_error(actor_root, &error))?
            .dev();
        if !media_mounts
            .iter()
            .all(|mount| mount.device == actor_device)
        {
            return Err(Error::Deployment("Actor View and every Media Root must be on the same filesystem/ZFS dataset for hard links".into()));
        }
        let mut mounts = vec![
            inspect_mount(config_root, MountRole::Configuration)?,
            inspect_mount(state_root, MountRole::Sqlite)?,
            inspect_mount(cache_root, MountRole::ArtworkCache)?,
        ];
        mounts.extend(media_mounts);
        let mut actor = inspect_mount(actor_root, MountRole::ActorView)?;
        actor.same_filesystem_as_media = Some(true);
        mounts.push(actor);
        rusqlite::Connection::open(&self.database_file).map_err(|error| {
            Error::Deployment(format!(
                "SQLite state '{}' is not ready for UID {} / GID {}: {error}",
                self.database_file.display(),
                unsafe { libc::geteuid() },
                unsafe { libc::getegid() }
            ))
        })?;
        Ok(DeploymentReport {
            uid: unsafe { libc::geteuid() },
            gid: unsafe { libc::getegid() },
            database_ready: true,
            mounts,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MountRole {
    Configuration,
    Sqlite,
    ArtworkCache,
    MediaRoot,
    ActorView,
}

#[derive(Debug, Clone, Serialize)]
pub struct MountReport {
    pub role: MountRole,
    pub path: PathBuf,
    pub readable: bool,
    pub writable: bool,
    pub device: u64,
    pub same_filesystem_as_media: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeploymentReport {
    pub uid: u32,
    pub gid: u32,
    pub database_ready: bool,
    pub mounts: Vec<MountReport>,
}

fn inspect_mount(path: &Path, role: MountRole) -> Result<MountReport, Error> {
    let metadata = fs::metadata(path).map_err(|error| mount_error(path, &error))?;
    if !metadata.is_dir() {
        return Err(Error::Deployment(format!(
            "Host Path '{}' for {role:?} is not a directory",
            path.display()
        )));
    }
    let readable = fs::read_dir(path).is_ok();
    let probe = path.join(format!(".rust-jav-write-check-{}", std::process::id()));
    let writable = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(probe);
            true
        }
        Err(_) => false,
    };
    if !readable || !writable {
        return Err(Error::Deployment(format!(
            "Host Path '{}' is not {} for container UID {} / GID {}; update its TrueNAS ACL",
            path.display(),
            if !readable { "readable" } else { "writable" },
            unsafe { libc::geteuid() },
            unsafe { libc::getegid() }
        )));
    }
    Ok(MountReport {
        role,
        path: path.to_owned(),
        readable,
        writable,
        device: metadata.dev(),
        same_filesystem_as_media: None,
    })
}

fn mount_error(path: &Path, error: &std::io::Error) -> Error {
    Error::Deployment(format!(
        "Host Path '{}' is unavailable to container UID {} / GID {}: {error}",
        path.display(),
        unsafe { libc::geteuid() },
        unsafe { libc::getegid() }
    ))
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Secrets {
    #[serde(skip_serializing_if = "Option::is_none")]
    password_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bootstrap_token_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jellyfin_api_key: Option<String>,
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
    if password.len() < MIN_PASSWORD_LENGTH {
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
    rules: Arc<RwLock<RuleState>>,
    downloader: Arc<dyn RuleDownloader>,
    tasks: TaskStore,
    coordinator: TaskCoordinator,
    assets: AssetIndex,
    deletion_plans: Arc<Mutex<HashMap<String, StoredDeletionPlan>>>,
    database: PathBuf,
}

#[derive(Clone)]
struct StoredDeletionPlan {
    plan: PermanentDeletionPlan,
    selection: String,
    rule_version: u32,
    rules: Vec<String>,
    discovered_hard_links: Vec<RelatedHardLink>,
}

#[derive(Clone)]
struct RuleState {
    yaml: String,
    active: ActiveRuleSet,
}

#[derive(Debug)]
pub enum DownloadError {
    Request,
    TooLarge,
    InvalidText,
}

pub trait RuleDownloader: Send + Sync + 'static {
    fn download<'a>(
        &'a self,
        url: &'a Url,
        timeout: Duration,
        max_bytes: usize,
    ) -> Pin<Box<dyn Future<Output = Result<String, DownloadError>> + Send + 'a>>;
}

struct HttpRuleDownloader;

impl RuleDownloader for HttpRuleDownloader {
    fn download<'a>(
        &'a self,
        url: &'a Url,
        timeout: Duration,
        max_bytes: usize,
    ) -> Pin<Box<dyn Future<Output = Result<String, DownloadError>> + Send + 'a>> {
        Box::pin(async move {
            let response = reqwest::Client::builder()
                .timeout(timeout)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| DownloadError::Request)?
                .get(url.clone())
                .send()
                .await
                .map_err(|_| DownloadError::Request)?
                .error_for_status()
                .map_err(|_| DownloadError::Request)?;
            if response
                .content_length()
                .is_some_and(|length| length > max_bytes as u64)
            {
                return Err(DownloadError::TooLarge);
            }
            let mut bytes = Vec::new();
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|_| DownloadError::Request)?;
                if bytes.len().saturating_add(chunk.len()) > max_bytes {
                    return Err(DownloadError::TooLarge);
                }
                bytes.extend_from_slice(&chunk);
            }
            String::from_utf8(bytes).map_err(|_| DownloadError::InvalidText)
        })
    }
}

#[derive(Clone)]
struct Session {
    expires_at: u64,
    credential_version: String,
}

impl AppState {
    pub fn new(config: ManagementConfig, clock: impl Clock) -> Result<Self, Error> {
        Self::with_downloader(config, clock, HttpRuleDownloader)
    }
    pub fn with_downloader(
        config: ManagementConfig,
        clock: impl Clock,
        downloader: impl RuleDownloader,
    ) -> Result<Self, Error> {
        let yaml = if config.active_rule_set_file.exists() {
            fs::read_to_string(&config.active_rule_set_file).map_err(|source| Error::Read {
                path: config.active_rule_set_file.clone(),
                source,
            })?
        } else {
            ActiveRuleSet::embedded_yaml().to_owned()
        };
        let active =
            ActiveRuleSet::from_yaml(&yaml, true).map_err(|source| Error::ActiveRules {
                path: config.active_rule_set_file.clone(),
                message: source.to_string(),
            })?;
        let now = clock.unix_seconds();
        let database = config.database_file.clone();
        let tasks = TaskStore::open(&database)?;
        let assets = AssetIndex::open(&database)?;
        let integration =
            rusqlite::Connection::open(&database).map_err(crate::asset_index::Error::from)?;
        integration.execute_batch("CREATE TABLE IF NOT EXISTS jellyfin_config (
            singleton INTEGER PRIMARY KEY CHECK(singleton=1), url TEXT NOT NULL, library_ids TEXT NOT NULL
          );
          CREATE TABLE IF NOT EXISTS jellyfin_refresh (
            singleton INTEGER PRIMARY KEY CHECK(singleton=1), status TEXT NOT NULL, attempts INTEGER NOT NULL, error TEXT
          );").map_err(crate::asset_index::Error::from)?;
        for item in tasks.running_mutation_items()? {
            let message = match (item.source_path.as_deref(), item.quarantine_token.as_deref()) {
                (Some(source), Some(token)) => crate::operations::recover_durable_quarantine(
                    Path::new(&item.media_root),
                    Path::new(source),
                    token,
                ),
                _ => "interrupted mutation: running item has no durable quarantine locator; inspect its source and planned target manually".to_string(),
            };
            tasks.interrupt_item(item.id, &message)?;
        }
        // A missing or incorrectly-permissioned TrueNAS mount degrades the
        // rebuildable index; it must not prevent the diagnostic API starting.
        let _ = assets.reconcile(&config.media_roots, ScanMode::Startup, now);
        tasks.interrupt_running_destructive(now)?;
        Ok(Self {
            store: SecretsStore::new(config.secrets_file.clone()),
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            clock: Arc::new(RwLock::new(Box::new(clock))),
            rules: Arc::new(RwLock::new(RuleState { yaml, active })),
            downloader: Arc::new(downloader),
            tasks,
            coordinator: TaskCoordinator::new(),
            assets,
            deletion_plans: Arc::new(Mutex::new(HashMap::new())),
            database,
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
#[derive(Deserialize)]
struct RuleYamlRequest {
    yaml: String,
}
#[derive(Deserialize)]
struct ActivateRulesRequest {
    yaml: String,
    #[serde(default)]
    confirm_empty: bool,
}
#[derive(Deserialize)]
struct DownloadRulesRequest {
    url: String,
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(health_live))
        .route("/health/ready", get(health_ready))
        .route("/health/mounts", get(health_mounts))
        .route("/health/jellyfin", get(health_jellyfin))
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
        .route("/api/v1/deletion-candidates", get(deletion_candidates))
        .route("/api/v1/deletion-plans", post(create_deletion_plan))
        .route(
            "/api/v1/deletion-plans/:plan_id/execute",
            post(execute_deletion_plan),
        )
        .route("/api/v1/deletion-audits", get(deletion_audits))
        .route("/api/v1/actors", get(list_actor_folders))
        .route(
            "/api/v1/actors/:actor_name",
            get(actor_folder_confirmation).delete(remove_actor_folder_task),
        )
        .route("/api/v1/actors/:actor_name/poster", get(actor_poster))
        .route("/api/v1/media-roots/health", get(media_root_health))
        .route("/api/v1/media-roots/storage", get(media_root_storage))
        .route(
            "/api/v1/rules/active",
            get(active_rules).put(activate_rules),
        )
        .route("/api/v1/rules/validate", post(validate_rules))
        .route("/api/v1/rules/download", post(download_rules))
        .route(
            "/api/v1/jellyfin/config",
            get(get_jellyfin_config).put(put_jellyfin_config),
        )
        .route("/api/v1/jellyfin/test", post(test_jellyfin))
        .route(
            "/api/v1/jellyfin/refresh",
            get(jellyfin_refresh_status).post(refresh_jellyfin),
        )
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

async fn health_live() -> impl IntoResponse {
    Json(serde_json::json!({"process":"alive"}))
}

async fn health_ready(State(state): State<AppState>) -> impl IntoResponse {
    match rusqlite::Connection::open(&state.database)
        .and_then(|connection| connection.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0)))
    {
        Ok(result) if result == "ok" => Json(serde_json::json!({"process":"alive","database":"ready"})).into_response(),
        Ok(result) => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"process":"alive","database":"degraded","detail":result}))).into_response(),
        Err(error) => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"process":"alive","database":"unavailable","detail":error.to_string()}))).into_response(),
    }
}

async fn health_mounts(State(state): State<AppState>) -> impl IntoResponse {
    match state.config.validate_truenas_mounts() {
        Ok(report) => Json(serde_json::json!({"ready":true,"report":report})).into_response(),
        Err(error) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"ready":false,"detail":error.to_string()})),
        )
            .into_response(),
    }
}

async fn health_jellyfin(State(state): State<AppState>) -> impl IntoResponse {
    let (available, detail) = match jellyfin_client(&state) {
        Ok(Some(client)) => match client.test_connection().await {
            Ok(status) => (true, Some(status.server_name)),
            Err(error) => (false, Some(error.to_string())),
        },
        Ok(None) => (false, Some("not configured".into())),
        Err(status) => (false, Some(status.to_string())),
    };
    Json(serde_json::json!({"available":available,"detail":detail,"affects_local_readiness":false}))
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

fn authorize(state: &AppState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let secrets = state
        .store
        .load()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let password_hash = secrets
        .password_hash
        .as_deref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    authenticated(state, headers, password_hash)
        .then_some(())
        .ok_or(StatusCode::UNAUTHORIZED)
}

async fn active_rules(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(status) = authorize(&state, &headers) {
        return status.into_response();
    }
    let rules = state.rules.read().unwrap();
    Json(serde_json::json!({
        "yaml": rules.yaml,
        "version": rules.active.version(),
        "enabled_rule_count": rules.active.enabled_patterns().len()
    }))
    .into_response()
}

async fn validate_rules(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RuleYamlRequest>,
) -> impl IntoResponse {
    if let Err(status) = authorize(&state, &headers) {
        return status.into_response();
    }
    match ActiveRuleSet::from_yaml(&input.yaml, true) {
        Ok(rules) => {
            Json(serde_json::json!({"valid": true, "empty": rules.enabled_patterns().is_empty()}))
                .into_response()
        }
        Err(error) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

async fn activate_rules(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ActivateRulesRequest>,
) -> impl IntoResponse {
    if let Err(status) = authorize(&state, &headers) {
        return status.into_response();
    }
    let candidate = match ActiveRuleSet::from_yaml(&input.yaml, input.confirm_empty) {
        Ok(candidate) => candidate,
        Err(ActiveRuleSetError::UnconfirmedEmpty) => return (StatusCode::CONFLICT, Json(serde_json::json!({"error": "empty Active Rule Set requires confirmation", "requires_empty_confirmation": true}))).into_response(),
        Err(error) => return (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({"error": error.to_string()}))).into_response(),
    };
    let mut active = state.rules.write().unwrap();
    if persist_atomically(&state.config.active_rule_set_file, input.yaml.as_bytes()).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    *active = RuleState {
        yaml: input.yaml,
        active: candidate,
    };
    StatusCode::NO_CONTENT.into_response()
}

async fn download_rules(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<DownloadRulesRequest>,
) -> impl IntoResponse {
    if let Err(status) = authorize(&state, &headers) {
        return status.into_response();
    }
    let url = match Url::parse(&input.url) {
        Ok(url) => url,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let allowed = url.scheme() == "https"
        && url.host_str().is_some_and(|host| {
            state
                .config
                .rule_source_hosts
                .iter()
                .any(|allowed| host.eq_ignore_ascii_case(allowed))
        })
        && url.username().is_empty()
        && url.password().is_none();
    if !allowed {
        return StatusCode::BAD_REQUEST.into_response();
    }
    match state
        .downloader
        .download(
            &url,
            state.config.rule_download_timeout,
            state.config.rule_download_max_bytes,
        )
        .await
    {
        Ok(yaml) => Json(serde_json::json!({"yaml": yaml})).into_response(),
        Err(DownloadError::TooLarge) => (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(
                serde_json::json!({"error": "Rule Source response exceeds configured size limit"}),
            ),
        )
            .into_response(),
        Err(_) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": "Rule Source download failed"})),
        )
            .into_response(),
    }
}

fn persist_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("yaml.tmp");
    {
        use std::io::Write;
        let mut file = fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(temporary, path)
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

async fn media_root_storage(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    let roots = state
        .config
        .media_roots
        .iter()
        .map(|root| state.assets.root_health(root))
        .collect();
    Json(MediaRootHealth::from_roots(roots)).into_response()
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
    let Ok(mut file) = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return StatusCode::NOT_FOUND.into_response();
    }
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

fn deletion_file_type(value: DeletionFileType) -> &'static str {
    match value {
        DeletionFileType::RegularFile => "file",
        DeletionFileType::Directory => "directory",
        DeletionFileType::Symlink => "symlink",
        DeletionFileType::Other => "other",
    }
}

fn system_time_seconds(value: SystemTime) -> u64 {
    value
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn plan_json(id: &str, stored: &StoredDeletionPlan) -> serde_json::Value {
    let plan = &stored.plan;
    serde_json::json!({
        "id": id,
        "selection": stored.selection,
        "rule_set_version": stored.rule_version,
        "rules": stored.rules,
        "created_at": system_time_seconds(plan.created_at),
        "expires_at": system_time_seconds(plan.expires_at),
        "logical_size": plan.logical_size,
        "reclaimable_space": plan.reclaimable_space,
        "hard_link_search_roots": plan.hard_link_search_roots,
        "paths": plan.approved_paths.iter().map(|path| serde_json::json!({
            "path": path.path,
            "type": deletion_file_type(path.file_type),
            "filesystem_identity": {"device": path.identity.device, "inode": path.identity.inode},
            "logical_size": path.logical_size,
            "allocated_size": path.allocated_size,
            "observed_link_count": path.observed_link_count,
            "video_warning": plan.video_warnings.iter().find(|warning| warning.path == path.path).map(|warning| warning.message.clone())
        })).collect::<Vec<_>>(),
        "discovered_hard_links": stored.discovered_hard_links.iter().map(|link| serde_json::json!({
            "path": link.path,
            "type": deletion_file_type(link.file_type),
            "filesystem_identity": {"device": link.identity.device, "inode": link.identity.inode}
        })).collect::<Vec<_>>()
    })
}

fn discover_candidates(root: &Path, rules: &ActiveRuleSet, found: &mut Vec<(PathBuf, String)>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            discover_candidates(&path, rules, found);
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if let Some(pattern) = rules.matching_pattern(name) {
            found.push((path, pattern.to_owned()));
        }
    }
}

async fn deletion_candidates(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(status) = authorize(&state, &headers) {
        return status.into_response();
    }
    let rules = state.rules.read().unwrap().active.clone();
    let mut found = Vec::new();
    for root in &state.config.media_roots {
        discover_candidates(root, &rules, &mut found);
    }
    found.sort_by(|left, right| left.0.cmp(&right.0));
    let planner = PermanentDeletionPlanner::new(state.config.media_roots.clone());
    let now = UNIX_EPOCH + Duration::from_secs(state.now());
    let candidates = found
        .into_iter()
        .filter_map(|(path, rule)| {
            let plan = planner
                .create_plan(vec![path.clone()], Duration::from_secs(600), now)
                .ok()?;
            let item = plan.approved_paths.iter().find(|item| item.path == path)?;
            Some(serde_json::json!({
                "path": path,
                "matching_rule": rule,
                "type": deletion_file_type(item.file_type),
                "video_warning": plan.video_warnings.first().map(|warning| warning.message.clone()),
                "logical_size": item.logical_size,
                "reclaimable_space": plan.reclaimable_space
            }))
        })
        .collect::<Vec<_>>();
    Json(serde_json::json!({"rule_set_version": rules.version(), "items": candidates}))
        .into_response()
}

#[derive(Deserialize)]
struct CreateDeletionPlanRequest {
    paths: Vec<PathBuf>,
    selection: String,
}

async fn create_deletion_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateDeletionPlanRequest>,
) -> impl IntoResponse {
    if let Err(status) = authorize(&state, &headers) {
        return status.into_response();
    }
    if input.paths.is_empty() || !matches!(input.selection.as_str(), "selected" | "unified") {
        return (
            StatusCode::BAD_REQUEST,
            "paths are required and selection must be selected or unified",
        )
            .into_response();
    }
    let rule_state = state.rules.read().unwrap().clone();
    if input.paths.iter().any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| rule_state.active.matching_pattern(name))
            .is_none()
    }) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            "every selected path must match the Active Rule Set",
        )
            .into_response();
    }
    let planner = PermanentDeletionPlanner::new(state.config.media_roots.clone());
    let now = UNIX_EPOCH + Duration::from_secs(state.now());
    let preview = match planner.create_plan(input.paths, Duration::from_secs(600), now) {
        Ok(plan) => plan,
        Err(error) => return (StatusCode::UNPROCESSABLE_ENTITY, error.to_string()).into_response(),
    };
    let discovered_hard_links = preview.related_hard_links.clone();
    let plan = if input.selection == "unified" {
        let mut paths = preview
            .approved_paths
            .iter()
            .map(|item| item.path.clone())
            .collect::<Vec<_>>();
        paths.extend(
            preview
                .related_hard_links
                .iter()
                .map(|item| item.path.clone()),
        );
        match planner.create_plan(paths, Duration::from_secs(600), now) {
            Ok(plan) => plan,
            Err(error) => {
                return (StatusCode::UNPROCESSABLE_ENTITY, error.to_string()).into_response()
            }
        }
    } else {
        preview
    };
    let id = random_token();
    let stored = StoredDeletionPlan {
        plan,
        selection: input.selection,
        rule_version: rule_state.active.version(),
        rules: rule_state.active.enabled_patterns(),
        discovered_hard_links,
    };
    let response = plan_json(&id, &stored);
    state.deletion_plans.lock().unwrap().insert(id, stored);
    (StatusCode::CREATED, Json(response)).into_response()
}

#[derive(Deserialize)]
struct ExecuteDeletionPlanRequest {
    irreversible: bool,
    confirmation: String,
}

async fn execute_deletion_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(plan_id): AxumPath<String>,
    Json(input): Json<ExecuteDeletionPlanRequest>,
) -> impl IntoResponse {
    if let Err(status) = authorize(&state, &headers) {
        return status.into_response();
    }
    if !input.irreversible || input.confirmation != "PERMANENTLY DELETE" {
        return (
            StatusCode::CONFLICT,
            "explicit irreversible confirmation is required",
        )
            .into_response();
    }
    let Some(stored) = state.deletion_plans.lock().unwrap().remove(&plan_id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let task = match state.tasks.create(
        NewTask::mutation(
            "permanent_deletion",
            stored
                .plan
                .hard_link_search_roots
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(","),
        ),
        state.now(),
    ) {
        Ok(task) => task,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let _lease = state.coordinator.mutation(&task.media_root).await;
    if state.tasks.mark_running(&task.id, state.now()).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let planner = PermanentDeletionPlanner::new(stored.plan.hard_link_search_roots.clone());
    let result = match planner.execute(&stored.plan, UNIX_EPOCH + Duration::from_secs(state.now()))
    {
        Ok(result) => result,
        Err(error) => {
            let _ = state
                .tasks
                .mark_failed(&task.id, state.now(), &error.to_string());
            return (StatusCode::CONFLICT, error.to_string()).into_response();
        }
    };
    for outcome in &result.outcomes {
        let status = match outcome.status {
            DeletionOutcomeStatus::Deleted => "deleted",
            DeletionOutcomeStatus::Changed => "changed",
            DeletionOutcomeStatus::Failed => "failed",
        };
        let _ = state.tasks.finish_item(
            &task.id,
            "permanent_deletion",
            Some(&outcome.path.display().to_string()),
            status,
            outcome.message.as_deref(),
        );
    }
    let audit = serde_json::json!({
        "administrator": "Administrator", "time": state.now(), "task_id": task.id,
        "active_rule_set": {"version": stored.rule_version, "rules": stored.rules},
        "operation_plan": plan_json(&plan_id, &stored),
        "outcomes": result.outcomes.iter().map(|outcome| serde_json::json!({"path":outcome.path,"status":format!("{:?}", outcome.status).to_ascii_lowercase(),"message":outcome.message})).collect::<Vec<_>>(),
        "partial": result.partial, "rolled_back": false
    });
    let _ = state
        .tasks
        .record_deletion_audit(&task.id, state.now(), &audit);
    if result
        .outcomes
        .iter()
        .any(|outcome| outcome.status != DeletionOutcomeStatus::Deleted)
    {
        let _ = state.tasks.mark_failed(
            &task.id,
            state.now(),
            "permanent deletion completed with partial failures",
        );
    } else {
        let _ = state.tasks.mark_completed(&task.id, state.now());
    }
    match state.tasks.get(&task.id) {
        Ok(Some(task)) => (StatusCode::ACCEPTED, Json(task)).into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn deletion_audits(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(status) = authorize(&state, &headers) {
        return status.into_response();
    }
    match state.tasks.deletion_audits() {
        Ok(records) => Json(records).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
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
        Ok(Some(detail)) => {
            let mut value = serde_json::to_value(&detail).unwrap_or_default();
            for (index, actor) in detail.actors.iter().enumerate() {
                value["actors"][index]["poster_url"] = serde_json::Value::Null;
                value["actors"][index]["actor_folder_url"] = canonical_actor_folder_url(
                    state.config.actor_view_root.as_deref(),
                    &actor.name,
                )
                .map_or(serde_json::Value::Null, serde_json::Value::String);
            }
            if let Ok(Some(client)) = jellyfin_client(&state) {
                if let Ok(people) = client.people().await {
                    for (index, actor) in detail.actors.iter().enumerate() {
                        value["actors"][index]["poster_url"] = match_person(&actor.name, &people)
                            .map(|_| actor_poster_url(&actor.name))
                            .map_or(serde_json::Value::Null, serde_json::Value::String);
                    }
                }
                if let Ok(items) = client.selected_items().await {
                    if let Some(association) = associate(
                        &detail.path,
                        detail.jav_code.as_deref(),
                        detail.title.as_deref(),
                        &items,
                    ) {
                        let status = if association.played {
                            "played"
                        } else if association.playback_position_ticks > 0 {
                            "in_progress"
                        } else {
                            "unplayed"
                        };
                        value["jellyfin"] = serde_json::json!({
                            "status": status,
                            "confidence": association.confidence,
                            "reason": association.reason,
                            "play_count": association.play_count,
                            "playback_position_ticks": association.playback_position_ticks,
                            "open_url": client.open_url(&association.item_id),
                            "may_authorize_deletion": association.may_authorize_deletion()
                        });
                    } else {
                        value["jellyfin"] = serde_json::json!({"status":"not_found"});
                    }
                } else {
                    value["jellyfin"] = serde_json::json!({"status":"offline"});
                }
            } else {
                value["jellyfin"] = serde_json::json!({"status":"not_configured"});
            }
            Json(value).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn canonical_actor_folder_url(root: Option<&Path>, actor_name: &str) -> Option<String> {
    let folder = crate::actor_views::actor_folder_detail(root?, actor_name)
        .ok()??
        .folder;
    Some(format!(
        "/actors/{}",
        URL_SAFE_NO_PAD.encode(folder.name.as_bytes())
    ))
}

#[derive(Serialize)]
struct ActorFolderResponse {
    name: String,
    movie_count: usize,
    derived_file_count: usize,
    unique_inode_count: usize,
    hard_link_count: usize,
    logical_size: u64,
    reclaimable_space: u64,
    poster_url: Option<String>,
    linked_assets: Vec<crate::asset_index::MediaAsset>,
}

fn actor_response(
    folder: crate::actor_views::ActorFolder,
    linked_assets: Vec<crate::asset_index::MediaAsset>,
    has_jellyfin_portrait: bool,
) -> ActorFolderResponse {
    let poster_url = has_jellyfin_portrait.then(|| actor_poster_url(&folder.name));
    ActorFolderResponse {
        name: folder.name,
        movie_count: folder.movie_count,
        derived_file_count: folder.derived_file_count,
        unique_inode_count: folder.unique_inode_count,
        hard_link_count: folder.hard_link_count,
        logical_size: folder.logical_size,
        reclaimable_space: folder.reclaimable_space,
        poster_url,
        linked_assets,
    }
}

fn actor_poster_url(actor_name: &str) -> String {
    let mut url = Url::parse("http://localhost/api/v1/actors/").expect("static URL is valid");
    let mut segments = url
        .path_segments_mut()
        .expect("HTTP URL supports path segments");
    segments.pop_if_empty().push(actor_name).push("poster");
    drop(segments);
    url.path().to_owned()
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
    let people = match jellyfin_client(&state) {
        Ok(Some(client)) => client.people().await.unwrap_or_default(),
        _ => Vec::new(),
    };
    match crate::actor_views::browse_actor_folders(root) {
        Ok(folders) => Json(
            folders
                .into_iter()
                .map(|folder| {
                    let has_portrait = match_person(&folder.name, &people).is_some();
                    actor_response(folder, Vec::new(), has_portrait)
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
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
    match crate::actor_views::actor_folder_detail(root, &actor_name) {
        Ok(Some(detail)) => {
            let identities = detail
                .file_identities
                .iter()
                .map(|identity| (identity.device, identity.inode))
                .collect::<Vec<_>>();
            let has_portrait = match jellyfin_client(&state) {
                Ok(Some(client)) => client
                    .people()
                    .await
                    .ok()
                    .and_then(|people| match_person(&actor_name, &people).map(|_| ()))
                    .is_some(),
                _ => false,
            };
            match state.assets.assets_by_identities(&identities) {
                Ok(assets) => {
                    Json(actor_response(detail.folder, assets, has_portrait)).into_response()
                }
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            }
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
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
    let Ok(Some(folder)) = crate::actor_views::actor_folder_detail(root, &actor_name) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(Some(client)) = jellyfin_client(&state) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(people) = client.people().await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let Some(person) = match_person(&folder.folder.name, &people) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(image) = cached_person_image(&state, &client, person).await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, image.content_type)
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(image.bytes))
        .unwrap()
        .into_response()
}

async fn cached_person_image(
    state: &AppState,
    client: &JellyfinClient,
    person: &JellyfinPerson,
) -> Result<JellyfinImage, crate::jellyfin::Error> {
    let cache_paths = state.config.artwork_cache_root.as_ref().map(|root| {
        let mut hasher = Sha256::new();
        hasher.update(person.id.as_bytes());
        hasher.update(b"\0");
        hasher.update(person.primary_image_tag().unwrap_or_default().as_bytes());
        hasher.update(b"\0width=320");
        let key = format!("{:x}", hasher.finalize());
        let directory = root.join("jellyfin-people");
        (
            directory.join(format!("{key}.image")),
            directory.join(format!("{key}.type")),
        )
    });
    if let Some((image_path, type_path)) = &cache_paths {
        if let (Ok(bytes), Ok(content_type)) = (fs::read(image_path), fs::read_to_string(type_path))
        {
            return Ok(JellyfinImage {
                bytes,
                content_type,
            });
        }
    }
    let image = client.primary_image(person, 320).await?;
    if let Some((image_path, type_path)) = cache_paths {
        if let Some(directory) = image_path.parent() {
            if fs::create_dir_all(directory).is_ok() {
                let _ = fs::write(image_path, &image.bytes);
                let _ = fs::write(type_path, image.content_type.as_bytes());
            }
        }
    }
    Ok(image)
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
    let detail = match crate::actor_views::actor_folder_detail(&root, &actor_name) {
        Ok(Some(detail)) => detail,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) => return (StatusCode::UNPROCESSABLE_ENTITY, error.to_string()).into_response(),
    };
    if !actor_removal_is_trusted(&state, &detail).unwrap_or(false) {
        return (
            StatusCode::CONFLICT,
            "Actor Folder contains a file that is not backed by an indexed Media Asset",
        )
            .into_response();
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
    let trusted = crate::actor_views::actor_folder_detail(&root, &actor_name)
        .ok()
        .flatten()
        .is_some_and(|detail| actor_removal_is_trusted(&state, &detail).unwrap_or(false));
    if !trusted {
        let _ = state.tasks.mark_failed(
            &task_id,
            state.now(),
            "Actor Folder changed or is no longer backed by an indexed Media Asset",
        );
        return;
    }
    match crate::actor_views::remove_actor_folder(&root, &actor_name) {
        Ok(outcomes) => {
            let failed = outcomes.iter().any(|outcome| outcome.status == "failed");
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
            if failed {
                let _ = state.tasks.mark_failed(
                    &task_id,
                    state.now(),
                    "one or more Actor View paths could not be removed",
                );
            } else {
                let _ = state.tasks.mark_completed(&task_id, state.now());
            }
        }
        Err(error) => {
            let _ = state
                .tasks
                .mark_failed(&task_id, state.now(), &error.to_string());
        }
    }
}

fn actor_removal_is_trusted(
    state: &AppState,
    detail: &crate::actor_views::ActorFolderDetail,
) -> Result<bool, crate::asset_index::Error> {
    let identities = detail
        .file_identities
        .iter()
        .map(|identity| (identity.device, identity.inode))
        .collect::<Vec<_>>();
    let assets = state.assets.assets_by_identities(&identities)?;
    let indexed = assets
        .iter()
        .map(|asset| (asset.device, asset.inode))
        .collect::<HashSet<_>>();
    let indexed_directories = assets
        .iter()
        .filter_map(|asset| Path::new(&asset.path).parent().map(Path::to_owned))
        .collect::<HashSet<_>>();
    for (actor_path, identity) in &detail.files {
        if indexed.contains(&(identity.device, identity.inode)) {
            continue;
        }
        let Ok(relative) = actor_path.strip_prefix(&detail.folder.path) else {
            return Ok(false);
        };
        let mut matched = false;
        for media_root in &state.config.media_roots {
            let source = media_root.join(relative);
            let Ok(metadata) = fs::symlink_metadata(&source) else {
                continue;
            };
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || (metadata.dev(), metadata.ino()) != (identity.device, identity.inode)
            {
                continue;
            }
            if source
                .parent()
                .is_some_and(|directory| indexed_directories.contains(directory))
            {
                matched = true;
                break;
            }
        }
        if !matched {
            return Ok(false);
        }
    }
    Ok(true)
}
#[derive(Debug, Deserialize)]
struct PutJellyfinConfig {
    url: String,
    library_ids: Vec<String>,
    api_key: String,
}

fn load_jellyfin_config(state: &AppState) -> Result<Option<JellyfinConfig>, StatusCode> {
    use rusqlite::OptionalExtension;
    let connection = rusqlite::Connection::open(&state.database)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    connection
        .query_row(
            "SELECT url,library_ids FROM jellyfin_config WHERE singleton=1",
            [],
            |row| {
                let ids: String = row.get(1)?;
                let library_ids = serde_json::from_str(&ids).unwrap_or_default();
                Ok(JellyfinConfig {
                    url: row.get(0)?,
                    library_ids,
                })
            },
        )
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn jellyfin_client(state: &AppState) -> Result<Option<JellyfinClient>, StatusCode> {
    let Some(config) = load_jellyfin_config(state)? else {
        return Ok(None);
    };
    let secrets = state
        .store
        .load()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some(key) = secrets.jellyfin_api_key else {
        return Ok(None);
    };
    JellyfinClient::new(config, key)
        .map(Some)
        .map_err(|_| StatusCode::BAD_REQUEST)
}

async fn put_jellyfin_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<PutJellyfinConfig>,
) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status;
    }
    let Ok(mut secrets) = state.store.load() else {
        return StatusCode::INTERNAL_SERVER_ERROR;
    };
    let effective_key = if input.api_key.trim().is_empty() {
        secrets.jellyfin_api_key.clone()
    } else {
        Some(input.api_key.clone())
    };
    if input.library_ids.is_empty()
        || effective_key
            .as_ref()
            .is_none_or(|key| key.trim().is_empty())
        || JellyfinClient::new(
            JellyfinConfig {
                url: input.url.clone(),
                library_ids: input.library_ids.clone(),
            },
            effective_key.unwrap(),
        )
        .is_err()
    {
        return StatusCode::BAD_REQUEST;
    }
    let Ok(connection) = rusqlite::Connection::open(&state.database) else {
        return StatusCode::INTERNAL_SERVER_ERROR;
    };
    let ids = serde_json::to_string(&input.library_ids).unwrap();
    if connection.execute("INSERT INTO jellyfin_config(singleton,url,library_ids) VALUES(1,?1,?2) ON CONFLICT(singleton) DO UPDATE SET url=excluded.url,library_ids=excluded.library_ids", rusqlite::params![input.url.trim_end_matches('/'), ids]).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    if !input.api_key.trim().is_empty() {
        secrets.jellyfin_api_key = Some(input.api_key);
        if state.store.save(&secrets).is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }
    StatusCode::NO_CONTENT
}

async fn get_jellyfin_config(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    let Ok(config) = load_jellyfin_config(&state) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let configured = state
        .store
        .load()
        .ok()
        .and_then(|s| s.jellyfin_api_key)
        .is_some();
    Json(match config {
        Some(config) => serde_json::json!({"url":config.url,"library_ids":config.library_ids,"api_key_configured":configured}),
        None => serde_json::json!({"url":null,"library_ids":[],"api_key_configured":false}),
    }).into_response()
}

async fn test_jellyfin(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    let Ok(Some(client)) = jellyfin_client(&state) else {
        return (StatusCode::BAD_REQUEST, "Configure Jellyfin first").into_response();
    };
    match client.test_connection().await {
        Ok(status) => Json(status).into_response(),
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

fn save_refresh(
    state: &AppState,
    status: &str,
    attempts: u8,
    error: Option<&str>,
) -> Result<(), ()> {
    let connection = rusqlite::Connection::open(&state.database).map_err(|_| ())?;
    connection.execute("INSERT INTO jellyfin_refresh(singleton,status,attempts,error) VALUES(1,?1,?2,?3) ON CONFLICT(singleton) DO UPDATE SET status=excluded.status,attempts=excluded.attempts,error=excluded.error", rusqlite::params![status, attempts, error]).map_err(|_| ())?;
    Ok(())
}

async fn refresh_jellyfin(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    let Ok(Some(client)) = jellyfin_client(&state) else {
        return (StatusCode::BAD_REQUEST, "Configure Jellyfin first").into_response();
    };
    let _ = save_refresh(&state, "retrying", 0, None);
    let outcome = client.refresh_batch(RetryPolicy::default()).await;
    match outcome {
        RefreshOutcome::Completed { attempts } => {
            let _ = save_refresh(&state, "completed", attempts, None);
            Json(serde_json::json!({"status":"completed","attempts":attempts})).into_response()
        }
        RefreshOutcome::ManualRetryRequired { attempts } => {
            let _ = save_refresh(
                &state,
                "manual_retry_required",
                attempts,
                Some("Jellyfin remained offline after five attempts"),
            );
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"status":"manual_retry_required","attempts":attempts})),
            )
                .into_response()
        }
    }
}

async fn jellyfin_refresh_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    use rusqlite::OptionalExtension;
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    let Ok(connection) = rusqlite::Connection::open(&state.database) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let value = connection.query_row("SELECT status,attempts,error FROM jellyfin_refresh WHERE singleton=1", [], |row| Ok(serde_json::json!({"status":row.get::<_,String>(0)?,"attempts":row.get::<_,u8>(1)?,"error":row.get::<_,Option<String>>(2)?}))).optional();
    match value {
        Ok(Some(value)) => Json(value).into_response(),
        Ok(None) => Json(serde_json::json!({"status":"idle","attempts":0})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
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
    #[serde(default)]
    media_root: Option<PathBuf>,
    mode: String,
    #[serde(default)]
    operations: Vec<String>,
    #[serde(default)]
    plan_id: Option<String>,
    #[serde(default)]
    confirmed: bool,
}

#[derive(Debug, Deserialize)]
struct StoredOperationPlan {
    canonical_media_root: PathBuf,
    operations: Vec<String>,
    actions: Vec<StoredOperationAction>,
    #[serde(default)]
    warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct StoredOperationAction {
    kind: String,
    source: Option<PathBuf>,
    target: Option<PathBuf>,
    warning: Option<String>,
    source_identity: Option<StoredSourceIdentity>,
}

#[derive(Debug, Deserialize)]
struct StoredSourceIdentity {
    device: u64,
    inode: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    size: u64,
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
    if input.mode == "apply" {
        return confirm_operation_plan(state, input).await;
    }
    let Some(media_root) = input.media_root else {
        return (
            StatusCode::BAD_REQUEST,
            "media_root is required for preview",
        )
            .into_response();
    };
    let operations = match canonical_operations(&input.operations) {
        Some(operations) => operations,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                "operations must contain known operation names",
            )
                .into_response()
        }
    };
    let media_root_text = media_root.display().to_string();
    let kind = TaskKind::Preview;
    let new_task = NewTask::preview(&input.task_type, &media_root_text);
    let task = match state.tasks.create(new_task, state.now()) {
        Ok(task) => task,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let task_id = task.id.clone();
    tokio::spawn(run_operations_task(
        state, task_id, media_root, operations, kind, None,
    ));
    (StatusCode::ACCEPTED, Json(task)).into_response()
}

async fn confirm_operation_plan(
    state: AppState,
    input: CreateTaskRequest,
) -> axum::response::Response {
    if !input.confirmed {
        return (
            StatusCode::BAD_REQUEST,
            "confirmed must be true to execute an Operation Plan",
        )
            .into_response();
    }
    let Some(plan_id) = input.plan_id else {
        return (
            StatusCode::BAD_REQUEST,
            "plan_id is required to apply operations",
        )
            .into_response();
    };
    let plan_task = match state.tasks.get(&plan_id) {
        Ok(Some(task)) => task,
        Ok(None) => return (StatusCode::BAD_REQUEST, "Operation Plan not found").into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    if plan_task.kind != TaskKind::Preview
        || plan_task.status != crate::management_tasks::TaskStatus::Completed
    {
        return (StatusCode::BAD_REQUEST, "Operation Plan is not ready").into_response();
    }
    if plan_task
        .plan_expires_at
        .is_none_or(|expires| state.now() > expires)
    {
        return (StatusCode::BAD_REQUEST, "Operation Plan has expired").into_response();
    }
    if plan_task.plan_consumed_at.is_some() {
        return (
            StatusCode::CONFLICT,
            "Operation Plan has already been consumed",
        )
            .into_response();
    }
    let Some(plan) = plan_task.operation_plan.clone() else {
        return (StatusCode::BAD_REQUEST, "Operation Plan is unavailable").into_response();
    };
    let stored_plan = match serde_json::from_value::<StoredOperationPlan>(plan) {
        Ok(plan) => plan,
        Err(_) => return (StatusCode::BAD_REQUEST, "Operation Plan is invalid").into_response(),
    };
    let Some(operations) = canonical_operations(&stored_plan.operations) else {
        return (StatusCode::BAD_REQUEST, "Operation Plan is invalid").into_response();
    };
    let (consumed_plan, task) = match state
        .tasks
        .consume_plan_and_create_mutation(&plan_id, state.now())
    {
        Ok(Some(tasks)) => tasks,
        Ok(None) => {
            return (
                StatusCode::CONFLICT,
                "Operation Plan has expired or already been consumed",
            )
                .into_response()
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let stored_plan = consumed_plan
        .operation_plan
        .and_then(|plan| serde_json::from_value::<StoredOperationPlan>(plan).ok())
        .expect("validated consumed Operation Plan must remain readable");
    let confirmed_actions = stored_plan
        .actions
        .into_iter()
        .map(|action| ConfirmedAction {
            action: PlannedAction {
                kind: action.kind,
                source: action.source,
                target: action.target,
                reason: action.warning,
            },
            source_identity: action.source_identity.map(|identity| SourceIdentity {
                device: identity.device,
                inode: identity.inode,
                modified_seconds: identity.modified_seconds,
                modified_nanoseconds: identity.modified_nanoseconds,
                size: identity.size,
            }),
        })
        .collect::<Vec<_>>();
    let media_root = PathBuf::from(&consumed_plan.media_root);
    tokio::spawn(run_operations_task(
        state,
        task.id.clone(),
        media_root,
        operations,
        TaskKind::Mutation,
        Some((
            stored_plan.canonical_media_root,
            confirmed_actions,
            stored_plan.warnings,
        )),
    ));
    (StatusCode::ACCEPTED, Json(task)).into_response()
}

async fn run_operations_task(
    state: AppState,
    task_id: String,
    media_root: PathBuf,
    operations: Vec<OperationType>,
    kind: TaskKind,
    confirmed_plan: Option<(PathBuf, Vec<ConfirmedAction>, Vec<String>)>,
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
    let persisted_during_execution = confirmed_plan.is_some();
    let report = if let Some((canonical_media_root, actions, warnings)) = confirmed_plan {
        let tasks = state.tasks.clone();
        let finishing_tasks = state.tasks.clone();
        let persisted_task_id = task_id.clone();
        let finishing_task_id = task_id.clone();
        crate::operations::run_confirmed_operation_plan(
            media_root.clone(),
            canonical_media_root,
            operations.clone(),
            actions,
            warnings,
            move |confirmed| {
                let action = &confirmed.action;
                let destructive = matches!(action.kind.as_str(), "delete-file" | "delete-dir");
                let path = if destructive {
                    action.source.as_ref()
                } else {
                    action.target.as_ref().or(action.source.as_ref())
                }
                .map(|path| path.display().to_string());
                tasks
                    .start_item(
                        &persisted_task_id,
                        &action.kind,
                        path.as_deref(),
                        action.source.as_ref().and_then(|path| path.to_str()),
                    )
                    .map(|journal| (journal.id, journal.quarantine_token))
                    .map_err(|error| error.to_string())
            },
            move |item_id, action| {
                finishing_tasks
                    .complete_item(
                        &finishing_task_id,
                        item_id,
                        action.status.as_str(),
                        action.reason.as_deref(),
                    )
                    .map_err(|error| error.to_string())
            },
        )
    } else if kind == TaskKind::Preview {
        crate::operations::plan_canonical_operation_snapshot(
            media_root.clone(),
            operations.clone(),
            ActiveRuleSet::embedded(),
        )
        .await
    } else {
        let request = if kind == TaskKind::Mutation {
            OperationsRequest::apply(media_root.clone(), operations.clone())
        } else {
            OperationsRequest::preview(media_root.clone(), operations.clone())
        };
        ApplicationServices::new().operations().run(request).await
    };
    for action in report
        .actions
        .iter()
        .filter(|_| !persisted_during_execution)
    {
        let destructive = matches!(action.kind.as_str(), "delete-file" | "delete-dir");
        let path = if destructive {
            action.source.as_ref()
        } else {
            action.target.as_ref().or(action.source.as_ref())
        }
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
    if kind == TaskKind::Preview {
        let operations = operations.iter().map(operation_key).collect::<Vec<_>>();
        let canonical_media_root = match fs::canonicalize(&media_root) {
            Ok(root) => root,
            Err(error) => {
                let _ = state.tasks.mark_failed(
                    &task_id,
                    state.now(),
                    &format!("cannot canonicalize media root for Operation Plan: {error}"),
                );
                return;
            }
        };
        let mut origins = HashMap::<PathBuf, PathBuf>::new();
        let mut actions = Vec::with_capacity(report.actions.len());
        for action in &report.actions {
            let destructive = matches!(action.kind.as_str(), "delete-file" | "delete-dir");
            let path = if destructive {
                action.source.as_ref()
            } else {
                action.target.as_ref().or(action.source.as_ref())
            };
            let original_source = action.source.as_ref().map(|source| {
                origins
                    .get(source)
                    .cloned()
                    .unwrap_or_else(|| source.clone())
            });
            let source_identity = match original_source.as_ref() {
                Some(source) => match fs::symlink_metadata(source) {
                    Ok(metadata) => Some(serde_json::json!({
                        "device": metadata.dev(), "inode": metadata.ino(),
                        "modified_seconds": metadata.mtime(),
                        "modified_nanoseconds": metadata.mtime_nsec(), "size": metadata.size(),
                    })),
                    Err(error) => {
                        let _ = state.tasks.mark_failed(
                            &task_id,
                            state.now(),
                            &format!("cannot record stored source identity: {error}"),
                        );
                        return;
                    }
                },
                None => None,
            };
            if let (Some(source), Some(target), Some(origin)) = (
                action.source.as_ref(),
                action.target.as_ref(),
                original_source.clone(),
            ) {
                origins.insert(
                    target.clone(),
                    origins.get(source).cloned().unwrap_or(origin),
                );
            }
            let canonical_path = |candidate: &PathBuf| {
                candidate
                    .strip_prefix(&media_root)
                    .map(|relative| canonical_media_root.join(relative))
                    .unwrap_or_else(|_| candidate.clone())
            };
            actions.push(serde_json::json!({
                    "kind": action.kind,
                    "path": path.map(|path| canonical_path(path).display().to_string()),
                    "source": action.source.as_ref().map(|path| canonical_path(path).display().to_string()),
                    "target": action.target.as_ref().map(|path| canonical_path(path).display().to_string()),
                    "destructive": destructive,
                    "warning": action.reason,
                    "source_identity": source_identity,
                }));
        }
        let plan = serde_json::json!({
            "canonical_media_root": canonical_media_root,
            "operations": operations,
            "actions": actions,
            "warnings": report.warnings,
            "requires_confirmation": true,
        });
        if state
            .tasks
            .save_operation_plan(
                &task_id,
                state.now().saturating_add(15 * 60),
                &plan.to_string(),
            )
            .is_err()
        {
            return;
        }
    }
    if state
        .tasks
        .save_report(&task_id, &report.to_json())
        .is_err()
    {
        return;
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
        if kind == TaskKind::Mutation {
            // A Management Task is the batch boundary: regardless of how many
            // files changed, enqueue exactly one Jellyfin library refresh.
            if let Ok(Some(client)) = jellyfin_client(&state) {
                let _ = save_refresh(&state, "retrying", 0, None);
                let outcome = client.refresh_batch(RetryPolicy::default()).await;
                match outcome {
                    RefreshOutcome::Completed { attempts } => {
                        let _ = save_refresh(&state, "completed", attempts, None);
                    }
                    RefreshOutcome::ManualRetryRequired { attempts } => {
                        let _ = save_refresh(
                            &state,
                            "manual_retry_required",
                            attempts,
                            Some("Jellyfin remained offline after five attempts"),
                        );
                    }
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct TaskListQuery {
    limit: Option<usize>,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    active: bool,
}

async fn list_tasks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<TaskListQuery>,
) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    let limit = query.limit.map(|limit| limit.clamp(1, 200));
    let listed = if query.active {
        state.tasks.list_active()
    } else {
        state.tasks.list_page(limit, query.offset)
    };
    let total = if query.active {
        listed.as_ref().map(Vec::len).map_err(|_| ())
    } else {
        state.tasks.count().map_err(|_| ())
    };
    match (listed, total) {
        (Ok(tasks), Ok(total)) => {
            let mut response = Json(tasks).into_response();
            if let Ok(value) = HeaderValue::from_str(&total.to_string()) {
                response.headers_mut().insert("x-total-count", value);
            }
            response
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
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

fn operation_key(value: &OperationType) -> &'static str {
    match value {
        OperationType::DeleteAdFiles => "delete_ad_files",
        OperationType::OrganizeByCode => "organize_by_code",
        OperationType::CleanEmptyDirs => "clean_empty_dirs",
        OperationType::StandardizeNames => "standardize_names",
        OperationType::ExtractCodes => "extract_codes",
        OperationType::CategorizeFiles => "categorize_files",
        OperationType::MoveOrigin => "move_origin",
        OperationType::RemoveDuplicates => "remove_duplicates",
    }
}

fn canonical_operations(values: &[String]) -> Option<Vec<OperationType>> {
    if values.is_empty() || values.iter().any(|value| parse_operation(value).is_none()) {
        return None;
    }
    Some(
        OperationType::all()
            .into_iter()
            .filter(|candidate| {
                values
                    .iter()
                    .any(|value| parse_operation(value) == Some(*candidate))
            })
            .collect(),
    )
}

async fn openapi(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(status) = authorized(&state, &headers) {
        return status.into_response();
    }
    Json(openapi_document()).into_response()
}

fn openapi_document() -> serde_json::Value {
    let mut document = serde_json::json!({
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
            "/api/v1/deletion-candidates":{"get":{"summary":"Browse Active Rule Set Deletion Candidates","responses":{"200":{"description":"Current candidates and sizes"}}}},
            "/api/v1/deletion-plans":{"post":{"summary":"Create a selected or unified permanent-deletion Operation Plan","responses":{"201":{"description":"Time-limited plan"}}}},
            "/api/v1/deletion-plans/{plan_id}/execute":{"post":{"summary":"Consume and execute an irreversibly confirmed Operation Plan","responses":{"202":{"description":"Persistent Management Task"},"409":{"description":"Expired or unconfirmed"}}}},
            "/api/v1/deletion-audits":{"get":{"summary":"List indefinite permanent-deletion audit records","responses":{"200":{"description":"Audit records"}}}},
            "/api/v1/actors":{"get":{"summary":"Browse derived Actor Folders with inode-aware storage metrics","responses":{"200":{"description":"Actor Folders","content":{"application/json":{"schema":{"type":"array","items":{"$ref":"#/components/schemas/ActorFolder"}}}}}}}},
            "/api/v1/actors/{actor_name}":{"get":{"summary":"Recompute Actor Folder removal confirmation","responses":{"200":{"description":"Fresh confirmation metrics"}}},"delete":{"summary":"Remove derived paths as a Management Task","responses":{"202":{"description":"Accepted Management Task"},"404":{"description":"Actor Folder not found"}}}},
            "/api/v1/media-roots/health":{"get":{"summary":"Report TrueNAS Host Path access and process UID/GID","responses":{"200":{"description":"Media Root permission reports","content":{"application/json":{"schema":{"type":"array","items":{"$ref":"#/components/schemas/RootHealth"}}}}}}}},
            "/api/v1/media-roots/storage":{"get":{"summary":"Report deduplicated Media Root filesystem capacity","responses":{"200":{"description":"Per-root and aggregate capacity in the process mount namespace","content":{"application/json":{"schema":{"$ref":"#/components/schemas/MediaRootStorage"}}}}}}},
            "/api/v1/jellyfin/config":{"get":{"summary":"Get non-secret Jellyfin configuration","responses":{"200":{"description":"Configuration without API key"}}},"put":{"summary":"Store Jellyfin configuration and server-only API key","responses":{"204":{"description":"Saved"}}}},
            "/api/v1/jellyfin/test":{"post":{"summary":"Test Jellyfin connectivity and selected libraries","responses":{"200":{"description":"Connected"},"502":{"description":"Jellyfin unavailable"}}}},
            "/api/v1/jellyfin/refresh":{"get":{"summary":"Get separately tracked refresh status","responses":{"200":{"description":"Refresh status"}}},"post":{"summary":"Manually refresh once with bounded retries","responses":{"200":{"description":"Completed"},"502":{"description":"Manual retry required"}}}}
        },
        "components": {"schemas": {
            "CreateTaskRequest": {"type":"object","required":["task_type","mode"],"properties":{
                "task_type":{"type":"string","const":"operations"},"media_root":{"type":"string"},"mode":{"type":"string","enum":["preview","apply"]},"operations":{"type":"array","minItems":1,"items":{"type":"string","enum":["delete_ad_files","organize_by_code","clean_empty_dirs","standardize_names","extract_codes","categorize_files","move_origin","remove_duplicates"]}},"plan_id":{"type":"string"},"confirmed":{"type":"boolean"}
            }},
            "TaskItem": {"type":"object","required":["id","kind","status"],"properties":{"id":{"type":"integer"},"kind":{"type":"string"},"path":{"type":["string","null"]},"status":{"type":"string"},"message":{"type":["string","null"]}}},
            "ScanAssetsRequest":{"type":"object","required":["mode"],"properties":{"mode":{"type":"string","enum":["manual","incremental"]},"media_root":{"type":"string"},"paths":{"type":"array","items":{"type":"string"}}}},
            "MediaAsset":{"type":"object","required":["id","media_root","path","device","inode","observed_at","captured_date","state"],"properties":{"id":{"type":"string"},"media_root":{"type":"string"},"path":{"type":"string"},"device":{"type":"integer"},"inode":{"type":"integer"},"jav_code":{"type":["string","null"]},"title":{"type":["string","null"]},"nfo_path":{"type":["string","null"]},"artwork_url":{"type":["string","null"]},"observed_at":{"type":"integer"},"captured_date":{"type":"string","format":"date"},"state":{"type":"string","enum":["normal","synchronizing","exception"]},"exception":{"type":["string","null"]}}},
            "AssetActor":{"type":"object","required":["name"],"properties":{"name":{"type":"string"},"poster_url":{"type":["string","null"]},"actor_folder_url":{"type":["string","null"]}}},
            "ActorFolder":{"type":"object","required":["name","movie_count","derived_file_count","unique_inode_count","hard_link_count","logical_size","reclaimable_space","linked_assets"],"properties":{"name":{"type":"string"},"movie_count":{"type":"integer"},"derived_file_count":{"type":"integer"},"unique_inode_count":{"type":"integer"},"hard_link_count":{"type":"integer","description":"Compatibility alias for derived_file_count"},"logical_size":{"type":"integer"},"reclaimable_space":{"type":"integer"},"poster_url":{"type":["string","null"]},"linked_assets":{"type":"array","items":{"$ref":"#/components/schemas/MediaAsset"}}}},
            "RootCapacity":{"type":"object","required":["status","filesystem_id","total_bytes","used_bytes","available_bytes","error"],"properties":{"status":{"type":"string","enum":["healthy","degraded"]},"filesystem_id":{"type":["integer","null"]},"total_bytes":{"type":["integer","null"],"minimum":0},"used_bytes":{"type":["integer","null"],"minimum":0},"available_bytes":{"type":["integer","null"],"minimum":0},"error":{"type":["string","null"]}}},
            "RootHealth":{"type":"object","required":["path","readable","writable","uid","gid","owner_uid","owner_gid","action","capacity"],"properties":{"path":{"type":"string"},"readable":{"type":"boolean"},"writable":{"type":"boolean"},"uid":{"type":"integer"},"gid":{"type":"integer"},"owner_uid":{"type":["integer","null"]},"owner_gid":{"type":["integer","null"]},"action":{"type":["string","null"]},"capacity":{"$ref":"#/components/schemas/RootCapacity"}}},
            "AggregateCapacity":{"type":"object","required":["status","filesystem_count","total_bytes","used_bytes","available_bytes"],"properties":{"status":{"type":"string","enum":["healthy","degraded"]},"filesystem_count":{"type":"integer","minimum":0},"total_bytes":{"type":["integer","null"],"minimum":0},"used_bytes":{"type":["integer","null"],"minimum":0},"available_bytes":{"type":["integer","null"],"minimum":0}}},
            "MediaRootStorage":{"type":"object","required":["roots","aggregate"],"properties":{"roots":{"type":"array","items":{"$ref":"#/components/schemas/RootHealth"}},"aggregate":{"$ref":"#/components/schemas/AggregateCapacity"}}},
            "JellyfinAssociation":{"type":"object","required":["status"],"properties":{"status":{"type":"string","enum":["played","in_progress","unplayed","not_found","offline","not_configured"]},"confidence":{"type":["string","null"],"enum":["certain_path","uncertain_metadata",null]},"reason":{"type":["string","null"]},"play_count":{"type":["integer","null"],"minimum":0},"playback_position_ticks":{"type":["integer","null"],"minimum":0},"open_url":{"type":["string","null"],"format":"uri"},"may_authorize_deletion":{"type":["boolean","null"]}},"description":"Read-only Jellyfin Association. Metadata-only uncertainty never authorizes deletion."},
            "AssetDetail":{"type":"object","required":["id","path","captured_date","actors","tags","parse_status","state","jellyfin"],"properties":{"id":{"type":"string"},"path":{"type":"string"},"jav_code":{"type":["string","null"]},"title":{"type":["string","null"]},"artwork_url":{"type":["string","null"]},"captured_date":{"type":"string","format":"date"},"actors":{"type":"array","items":{"$ref":"#/components/schemas/AssetActor"}},"studio":{"type":["string","null"]},"release_date":{"type":["string","null"],"format":"date"},"runtime_minutes":{"type":["integer","null"]},"director":{"type":["string","null"]},"tags":{"type":"array","items":{"type":"string"}},"plot":{"type":["string","null"]},"parse_status":{"type":"string","enum":["valid","missing","invalid"]},"source_path":{"type":["string","null"]},"state":{"type":"string","enum":["normal","synchronizing","exception"]},"exception":{"type":["string","null"]},"jellyfin":{"$ref":"#/components/schemas/JellyfinAssociation"}}},
            "AssetPage":{"type":"object","required":["items","groups","page","per_page","total","total_pages"],"properties":{"items":{"type":"array","items":{"$ref":"#/components/schemas/MediaAsset"}},"groups":{"type":"array","items":{"type":"object","properties":{"date":{"type":"string","format":"date"},"count":{"type":"integer"}}}},"page":{"type":"integer"},"per_page":{"type":"integer"},"total":{"type":"integer"},"total_pages":{"type":"integer"}}},
            "ManagementTask": {"type":"object","required":["id","task_type","media_root","kind","status","created_at","items"],"properties":{
                "id":{"type":"string"},"task_type":{"type":"string"},"media_root":{"type":"string"},"kind":{"type":"string","enum":["preview","mutation"]},"status":{"type":"string","enum":["queued","running","completed","failed","interrupted"]},"created_at":{"type":"integer"},"started_at":{"type":["integer","null"]},"finished_at":{"type":["integer","null"]},"error":{"type":["string","null"]},"plan_expires_at":{"type":["integer","null"]},"operation_plan":{"type":["object","null"]},"report":{"type":["object","null"]},"items":{"type":"array","items":{"$ref":"#/components/schemas/TaskItem"}}
            }}
        }}
    });
    let properties = &mut document["components"]["schemas"]["ManagementTask"]["properties"];
    properties["source_plan_id"] = serde_json::json!({"type":["string","null"]});
    properties["plan_consumed_at"] = serde_json::json!({"type":["integer","null"]});
    properties["planned_item_count"] = serde_json::json!({"type":["integer","null"]});
    let item_properties = &mut document["components"]["schemas"]["TaskItem"]["properties"];
    item_properties["source_path"] = serde_json::json!({"type":["string","null"]});
    item_properties["quarantine_token"] = serde_json::json!({"type":["string","null"]});
    document["paths"]["/api/v1/tasks"]["get"]["parameters"] = serde_json::json!([
        {"name":"limit","in":"query","schema":{"type":"integer","minimum":1,"maximum":200}},
        {"name":"offset","in":"query","schema":{"type":"integer","minimum":0}}
        ,{"name":"active","in":"query","schema":{"type":"boolean"}}
    ]);
    document["paths"]["/api/v1/tasks"]["get"]["responses"]["200"]["headers"] =
        serde_json::json!({"X-Total-Count":{"schema":{"type":"integer"}}});
    document
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
        .header(header::CACHE_CONTROL, "no-cache")
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
    let html = String::from_utf8_lossy(INDEX_HTML)
        .replace("/assets/app.js", "/assets/app.js?v=2")
        .replace("/assets/app.css", "/assets/app.css?v=2");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(html))
        .unwrap()
}

pub async fn serve(config: ManagementConfig) -> Result<(), Error> {
    if config.container {
        config.validate_truenas_mounts()?;
    }
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

#[cfg(test)]
mod tests {
    use super::actor_poster_url;

    #[test]
    fn actor_poster_urls_encode_one_path_segment() {
        assert_eq!(
            actor_poster_url("Alice #100%"),
            "/api/v1/actors/Alice%20%23100%25/poster"
        );
    }
}
