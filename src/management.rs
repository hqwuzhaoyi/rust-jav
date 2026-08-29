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
    extract::State,
    http::{header, HeaderMap, HeaderValue, Response, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ManagementYaml {
    port: u16,
    container: bool,
    session_ttl_seconds: u64,
    secrets_file: PathBuf,
}

impl Default for ManagementYaml {
    fn default() -> Self {
        Self {
            port: 9317,
            container: false,
            session_ttl_seconds: 43_200,
            secrets_file: PathBuf::from("management.secrets.yaml"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ManagementConfig {
    pub port: u16,
    pub container: bool,
    pub session_ttl: Duration,
    pub secrets_file: PathBuf,
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
        Ok(Self {
            port: raw.port,
            container: raw.container,
            session_ttl: Duration::from_secs(raw.session_ttl_seconds),
            secrets_file,
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
}

#[derive(Clone)]
struct Session {
    expires_at: u64,
    credential_version: String,
}

impl AppState {
    pub fn new(config: ManagementConfig, clock: impl Clock) -> Result<Self, Error> {
        Ok(Self {
            store: SecretsStore::new(config.secrets_file.clone()),
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            clock: Arc::new(RwLock::new(Box::new(clock))),
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
