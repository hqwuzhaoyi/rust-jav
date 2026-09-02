use std::{
    collections::{HashMap, HashSet},
    ffi::{CStr, CString, OsString},
    fs,
    io::Read,
    mem::MaybeUninit,
    os::fd::{AsRawFd, FromRawFd},
    os::unix::ffi::{OsStrExt, OsStringExt},
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use rand::{rngs::OsRng, RngCore};
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("asset index failed: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("asset index lock was poisoned")]
    Poisoned,
    #[error("unable to scan {path}: {source}")]
    Scan {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetState {
    Normal,
    Synchronizing,
    Exception,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtworkStatus {
    #[default]
    Missing,
    Valid,
    Empty,
    Unrecognized,
    Animated,
    TruncatedOrCorrupt,
    TooLarge,
    Unreadable,
}

impl ArtworkStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Valid => "valid",
            Self::Empty => "empty",
            Self::Unrecognized => "unrecognized",
            Self::Animated => "animated",
            Self::TruncatedOrCorrupt => "truncated_or_corrupt",
            Self::TooLarge => "too_large",
            Self::Unreadable => "unreadable",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "valid" => Self::Valid,
            "empty" => Self::Empty,
            "unrecognized" => Self::Unrecognized,
            "animated" => Self::Animated,
            "truncated_or_corrupt" => Self::TruncatedOrCorrupt,
            "too_large" => Self::TooLarge,
            "unreadable" => Self::Unreadable,
            _ => Self::Missing,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtworkProvenance {
    pub status: ArtworkStatus,
    pub source_path: Option<String>,
    pub content_type: Option<String>,
    pub error: Option<String>,
}

impl AssetState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Synchronizing => "synchronizing",
            Self::Exception => "exception",
        }
    }
    fn parse(value: &str) -> Self {
        match value {
            "normal" => Self::Normal,
            "synchronizing" => Self::Synchronizing,
            _ => Self::Exception,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanMode {
    Startup,
    Manual,
    Incremental,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAsset {
    pub id: String,
    pub media_root: String,
    pub path: String,
    pub device: u64,
    pub inode: u64,
    pub jav_code: Option<String>,
    pub title: Option<String>,
    pub nfo_path: Option<String>,
    pub artwork_url: Option<String>,
    #[serde(skip)]
    pub artwork_path: Option<String>,
    #[serde(skip)]
    pub artwork_status: ArtworkStatus,
    #[serde(skip)]
    pub artwork_content_type: Option<String>,
    #[serde(skip)]
    pub artwork_error: Option<String>,
    pub observed_at: u64,
    pub captured_date: String,
    pub state: AssetState,
    pub exception: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetActor {
    pub name: String,
    pub poster_url: Option<String>,
    pub actor_folder_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetDetail {
    pub id: String,
    pub path: String,
    pub jav_code: Option<String>,
    pub title: Option<String>,
    pub artwork_url: Option<String>,
    pub artwork: ArtworkProvenance,
    pub captured_date: String,
    pub actors: Vec<AssetActor>,
    pub studio: Option<String>,
    pub release_date: Option<String>,
    pub runtime_minutes: Option<u32>,
    pub director: Option<String>,
    pub tags: Vec<String>,
    pub plot: Option<String>,
    pub parse_status: String,
    pub source_path: Option<String>,
    pub state: AssetState,
    pub exception: Option<String>,
}

#[derive(Debug)]
pub struct IndexedArtwork {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}

#[derive(Debug)]
struct ArtworkInspection {
    status: ArtworkStatus,
    content_type: Option<&'static str>,
    error: Option<String>,
    identity: Option<ArtworkIdentity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ArtworkIdentity {
    root_device: u64,
    root_inode: u64,
    device: u64,
    inode: u64,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

#[derive(Debug)]
struct IndexedArtworkRecord {
    path: PathBuf,
    root: PathBuf,
    content_type: String,
    identity: ArtworkIdentity,
}

impl ArtworkInspection {
    fn invalid(status: ArtworkStatus, content_type: Option<&'static str>, error: String) -> Self {
        Self {
            status,
            content_type,
            error: Some(error),
            identity: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ParsedNfo {
    title: Option<String>,
    actors: Vec<String>,
    studio: Option<String>,
    release_date: Option<String>,
    runtime_minutes: Option<u32>,
    director: Option<String>,
    tags: Vec<String>,
    plot: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AssetQuery {
    pub query: Option<String>,
    pub state: Option<AssetState>,
    pub page: usize,
    pub per_page: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateGroup {
    pub date: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetPage {
    pub items: Vec<MediaAsset>,
    pub groups: Vec<DateGroup>,
    pub page: usize,
    pub per_page: usize,
    pub total: usize,
    pub total_pages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootHealth {
    pub path: String,
    pub readable: bool,
    pub writable: bool,
    pub uid: u32,
    pub gid: u32,
    pub owner_uid: Option<u32>,
    pub owner_gid: Option<u32>,
    pub action: Option<String>,
    pub capacity: RootCapacity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RootCapacity {
    pub status: String,
    pub filesystem_id: Option<u64>,
    pub total_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateCapacity {
    pub status: String,
    pub filesystem_count: usize,
    pub total_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaRootHealth {
    pub roots: Vec<RootHealth>,
    pub aggregate: AggregateCapacity,
}

impl MediaRootHealth {
    pub fn from_roots(mut roots: Vec<RootHealth>) -> Self {
        let mut observed: HashMap<u64, RootCapacity> = HashMap::new();
        for root in &mut roots {
            let Some(filesystem_id) = root.capacity.filesystem_id else {
                continue;
            };
            if let Some(capacity) = observed.get(&filesystem_id) {
                root.capacity = capacity.clone();
            } else {
                observed.insert(filesystem_id, root.capacity.clone());
            }
        }
        let mut filesystems = HashSet::new();
        let mut total = 0_u64;
        let mut used = 0_u64;
        let mut available = 0_u64;
        let mut degraded = false;

        for capacity in roots.iter().map(|root| &root.capacity) {
            let Some(filesystem_id) = capacity.filesystem_id else {
                degraded = true;
                continue;
            };
            if capacity.status != "healthy" || !filesystems.insert(filesystem_id) {
                degraded |= capacity.status != "healthy";
                continue;
            }
            let Some(next_total) = capacity
                .total_bytes
                .and_then(|value| total.checked_add(value))
            else {
                degraded = true;
                continue;
            };
            let Some(next_used) = capacity
                .used_bytes
                .and_then(|value| used.checked_add(value))
            else {
                degraded = true;
                continue;
            };
            let Some(next_available) = capacity
                .available_bytes
                .and_then(|value| available.checked_add(value))
            else {
                degraded = true;
                continue;
            };
            total = next_total;
            used = next_used;
            available = next_available;
        }

        let aggregate = AggregateCapacity {
            status: if degraded { "degraded" } else { "healthy" }.to_owned(),
            filesystem_count: filesystems.len(),
            total_bytes: (!degraded).then_some(total),
            used_bytes: (!degraded).then_some(used),
            available_bytes: (!degraded).then_some(available),
        };
        Self { roots, aggregate }
    }
}

fn root_capacity(path: &Path) -> RootCapacity {
    let result = (|| {
        let directory = fs::File::open(path).map_err(|error| error.to_string())?;
        let metadata = directory.metadata().map_err(|error| error.to_string())?;
        let mut statistics = MaybeUninit::<libc::statvfs>::uninit();
        if unsafe { libc::fstatvfs(directory.as_raw_fd(), statistics.as_mut_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let statistics = unsafe { statistics.assume_init() };
        let fragment_size = statistics.f_frsize as u64;
        let blocks = statistics.f_blocks as u64;
        if fragment_size == 0 || blocks == 0 {
            return Err("Media Root capacity is unsupported".to_owned());
        }
        let free_blocks = statistics.f_bfree as u64;
        let available_blocks = statistics.f_bavail as u64;
        if free_blocks > blocks || available_blocks > blocks {
            return Err("Media Root capacity counters are invalid".to_owned());
        }
        let total_bytes = blocks
            .checked_mul(fragment_size)
            .ok_or_else(|| "Media Root total capacity overflowed".to_owned())?;
        let used_bytes = blocks
            .checked_sub(free_blocks)
            .and_then(|value| value.checked_mul(fragment_size))
            .ok_or_else(|| "Media Root used capacity overflowed".to_owned())?;
        let available_bytes = available_blocks
            .checked_mul(fragment_size)
            .ok_or_else(|| "Media Root available capacity overflowed".to_owned())?;
        Ok((metadata.dev(), total_bytes, used_bytes, available_bytes))
    })();

    match result {
        Ok((filesystem_id, total_bytes, used_bytes, available_bytes)) => RootCapacity {
            status: "healthy".to_owned(),
            filesystem_id: Some(filesystem_id),
            total_bytes: Some(total_bytes),
            used_bytes: Some(used_bytes),
            available_bytes: Some(available_bytes),
            error: None,
        },
        Err(error) => RootCapacity {
            status: "degraded".to_owned(),
            filesystem_id: None,
            total_bytes: None,
            used_bytes: None,
            available_bytes: None,
            error: Some(error),
        },
    }
}

#[derive(Clone)]
pub struct AssetIndex(Arc<Mutex<Connection>>);

impl AssetIndex {
    pub fn open(path: &Path) -> Result<Self, Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::Scan {
                path: parent.to_owned(),
                source,
            })?;
        }
        let connection = Connection::open(path)?;
        connection.execute_batch("PRAGMA foreign_keys=ON;
          CREATE TABLE IF NOT EXISTS media_assets (
            id TEXT PRIMARY KEY, media_root TEXT NOT NULL, path TEXT NOT NULL UNIQUE,
            device INTEGER NOT NULL, inode INTEGER NOT NULL, jav_code TEXT, title TEXT,
            nfo_path TEXT, artwork_path TEXT, observed_at INTEGER NOT NULL,
            captured_date TEXT NOT NULL, state TEXT NOT NULL, exception TEXT, generation INTEGER NOT NULL
          );
          CREATE INDEX IF NOT EXISTS idx_assets_identity ON media_assets(media_root,device,inode);
          CREATE INDEX IF NOT EXISTS idx_assets_state_date ON media_assets(state,captured_date DESC);
          CREATE INDEX IF NOT EXISTS idx_assets_code ON media_assets(jav_code);
          CREATE TABLE IF NOT EXISTS asset_index_health (
            singleton INTEGER PRIMARY KEY CHECK(singleton=1), state TEXT NOT NULL,
            mode TEXT, started_at INTEGER, completed_at INTEGER, error TEXT
          );
          INSERT OR IGNORE INTO asset_index_health(singleton,state) VALUES(1,'idle');")?;
        ensure_column(
            &connection,
            "media_assets",
            "artwork_status",
            "TEXT NOT NULL DEFAULT 'missing'",
        )?;
        ensure_column(&connection, "media_assets", "artwork_content_type", "TEXT")?;
        ensure_column(&connection, "media_assets", "artwork_error", "TEXT")?;
        for column in [
            "artwork_root_device",
            "artwork_root_inode",
            "artwork_device",
            "artwork_inode",
            "artwork_size",
            "artwork_modified_seconds",
            "artwork_modified_nanoseconds",
            "artwork_changed_seconds",
            "artwork_changed_nanoseconds",
        ] {
            ensure_column(&connection, "media_assets", column, "INTEGER")?;
        }
        Ok(Self(Arc::new(Mutex::new(connection))))
    }

    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, Error> {
        self.0.lock().map_err(|_| Error::Poisoned)
    }

    pub fn reconcile(&self, roots: &[PathBuf], mode: ScanMode, now: u64) -> Result<(), Error> {
        self.set_health("synchronizing", Some(mode), Some(now), None, None)?;
        let result: Result<(), Error> = (|| {
            validate_roots(roots)?;
            let generation = now as i64;
            let configured = roots
                .iter()
                .map(|root| root.display().to_string())
                .collect::<Vec<_>>();
            let stale_roots = {
                let connection = self.connection()?;
                let mut statement =
                    connection.prepare("SELECT DISTINCT media_root FROM media_assets")?;
                let values = statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                values
            };
            for stale in stale_roots
                .into_iter()
                .filter(|root| !configured.contains(root))
            {
                self.connection()?
                    .execute("DELETE FROM media_assets WHERE media_root=?1", [stale])?;
            }
            for root in roots {
                let (paths, root_identity) = media_files(root)?;
                for path in paths {
                    self.observe(root, root_identity, &path, now, generation)?;
                }
                self.connection()?.execute(
                    "DELETE FROM media_assets WHERE media_root=?1 AND generation<>?2",
                    params![root.display().to_string(), generation],
                )?;
            }
            Ok(())
        })();
        match &result {
            Ok(()) => self.set_health("healthy", Some(mode), None, Some(now), None)?,
            Err(error) => self.set_health(
                "degraded",
                Some(mode),
                None,
                Some(now),
                Some(&error.to_string()),
            )?,
        }
        result
    }

    pub fn reconcile_paths(&self, root: &Path, paths: &[PathBuf], now: u64) -> Result<(), Error> {
        self.set_health(
            "synchronizing",
            Some(ScanMode::Incremental),
            Some(now),
            None,
            None,
        )?;
        let root_file = open_root(root).map_err(|source| Error::Scan {
            path: root.to_owned(),
            source,
        })?;
        let root_metadata = root_file.metadata().map_err(|source| Error::Scan {
            path: root.to_owned(),
            source,
        })?;
        let root_identity = (root_metadata.dev(), root_metadata.ino());
        for path in paths {
            if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
                && is_video(path)
            {
                self.observe(root, root_identity, path, now, now as i64)?;
            } else {
                self.connection()?.execute(
                    "DELETE FROM media_assets WHERE media_root=?1 AND path=?2",
                    params![root.display().to_string(), path.display().to_string()],
                )?;
            }
        }
        self.set_health(
            "healthy",
            Some(ScanMode::Incremental),
            None,
            Some(now),
            None,
        )
    }

    fn observe(
        &self,
        root: &Path,
        root_identity: (u64, u64),
        path: &Path,
        now: u64,
        generation: i64,
    ) -> Result<(), Error> {
        let root_file = open_root(root).map_err(|source| Error::Scan {
            path: root.to_owned(),
            source,
        })?;
        let root_metadata = root_file.metadata().map_err(|source| Error::Scan {
            path: root.to_owned(),
            source,
        })?;
        if (root_metadata.dev(), root_metadata.ino()) != root_identity {
            return Err(Error::Scan {
                path: root.to_owned(),
                source: std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "Media Root identity changed while scanning",
                ),
            });
        }
        let media_file =
            open_beneath_from(&root_file, root, path).map_err(|source| Error::Scan {
                path: path.to_owned(),
                source,
            })?;
        let metadata = media_file.metadata().map_err(|source| Error::Scan {
            path: path.to_owned(),
            source,
        })?;
        let stem = path
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or_default();
        let jav_code = Regex::new(r"(?i)([a-z]{2,10})[-_ ]?(\d{2,5})")
            .unwrap()
            .captures(stem)
            .map(|c| format!("{}-{}", c[1].to_uppercase(), &c[2]));
        let nfo = metadata_companion(path, &["movie.nfo"], &["nfo"]);
        let (artwork, artwork_inspection) = artwork_companion(root, path);
        let artwork_status = artwork_inspection
            .as_ref()
            .map_or(ArtworkStatus::Missing, |inspection| inspection.status);
        let artwork_content_type = artwork_inspection
            .as_ref()
            .and_then(|inspection| inspection.content_type);
        let artwork_error = artwork_inspection
            .as_ref()
            .and_then(|inspection| inspection.error.as_deref());
        let artwork_identity = artwork_inspection
            .as_ref()
            .and_then(|inspection| inspection.identity);
        let parsed = nfo.as_deref().map(|path| parse_nfo(root, path));
        let title = parsed
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .and_then(|nfo| nfo.title.clone());
        let (state, exception) = match &parsed {
            None => (AssetState::Exception, Some("NFO metadata is missing. Add a sibling .nfo file and reconcile the Asset Index.".to_owned())),
            Some(Err(reason)) if is_empty_nfo_error(reason) => (AssetState::Exception, Some(format!("NFO metadata file is empty. Regenerate it and reconcile the Asset Index: {reason}"))),
            Some(Err(reason)) => (AssetState::Exception, Some(format!("Fix invalid NFO metadata and reconcile the Asset Index: {reason}"))),
            Some(Ok(_)) => (AssetState::Normal, None),
        };
        let captured_date =
            DateTime::<Utc>::from(metadata.modified().unwrap_or(std::time::UNIX_EPOCH))
                .format("%Y-%m-%d")
                .to_string();
        let connection = self.connection()?;
        let id: Option<String> = connection.query_row(
            "SELECT id FROM media_assets WHERE media_root=?1 AND ((device=?2 AND inode=?3) OR path=?4) ORDER BY CASE WHEN device=?2 AND inode=?3 THEN 0 ELSE 1 END LIMIT 1",
            params![root.display().to_string(), metadata.dev() as i64, metadata.ino() as i64, path.display().to_string()], |row| row.get(0)).optional()?;
        let id = id.unwrap_or_else(asset_id);
        connection.execute(
            "DELETE FROM media_assets WHERE path=?1 AND id<>?2",
            params![path.display().to_string(), id],
        )?;
        connection.execute("INSERT INTO media_assets(id,media_root,path,device,inode,jav_code,title,nfo_path,artwork_path,artwork_status,artwork_content_type,artwork_error,artwork_root_device,artwork_root_inode,artwork_device,artwork_inode,artwork_size,artwork_modified_seconds,artwork_modified_nanoseconds,artwork_changed_seconds,artwork_changed_nanoseconds,observed_at,captured_date,state,exception,generation)
          VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26)
          ON CONFLICT(id) DO UPDATE SET media_root=excluded.media_root,path=excluded.path,device=excluded.device,inode=excluded.inode,jav_code=excluded.jav_code,title=excluded.title,nfo_path=excluded.nfo_path,artwork_path=excluded.artwork_path,artwork_status=excluded.artwork_status,artwork_content_type=excluded.artwork_content_type,artwork_error=excluded.artwork_error,artwork_root_device=excluded.artwork_root_device,artwork_root_inode=excluded.artwork_root_inode,artwork_device=excluded.artwork_device,artwork_inode=excluded.artwork_inode,artwork_size=excluded.artwork_size,artwork_modified_seconds=excluded.artwork_modified_seconds,artwork_modified_nanoseconds=excluded.artwork_modified_nanoseconds,artwork_changed_seconds=excluded.artwork_changed_seconds,artwork_changed_nanoseconds=excluded.artwork_changed_nanoseconds,observed_at=excluded.observed_at,captured_date=excluded.captured_date,state=excluded.state,exception=excluded.exception,generation=excluded.generation",
          params![id, root.display().to_string(), path.display().to_string(), metadata.dev() as i64, metadata.ino() as i64, jav_code, title, nfo.map(|p|p.display().to_string()), artwork.map(|p|p.display().to_string()), artwork_status.as_str(), artwork_content_type, artwork_error, artwork_identity.map(|identity| identity.root_device as i64), artwork_identity.map(|identity| identity.root_inode as i64), artwork_identity.map(|identity| identity.device as i64), artwork_identity.map(|identity| identity.inode as i64), artwork_identity.map(|identity| identity.size as i64), artwork_identity.map(|identity| identity.modified_seconds), artwork_identity.map(|identity| identity.modified_nanoseconds), artwork_identity.map(|identity| identity.changed_seconds), artwork_identity.map(|identity| identity.changed_nanoseconds), now as i64, captured_date, state.as_str(), exception, generation])?;
        Ok(())
    }

    pub fn search(&self, query: AssetQuery) -> Result<AssetPage, Error> {
        let requested_page = query.page.max(1);
        let per_page = if query.per_page == 0 {
            48
        } else {
            query.per_page.min(200)
        };
        let escaped_query = query
            .query
            .unwrap_or_default()
            .to_lowercase()
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let needle = format!("%{escaped_query}%");
        let state = query.state.map(|s| s.as_str().to_owned());
        let connection = self.connection()?;
        let filter = "(?1='%%' OR lower(coalesce(jav_code,'')||' '||coalesce(title,'')||' '||path) LIKE ?1 ESCAPE '\\') AND (?2 IS NULL OR state=?2)";
        let total: i64 = connection.query_row(
            &format!("SELECT count(*) FROM media_assets WHERE {filter}"),
            params![needle, state],
            |r| r.get(0),
        )?;
        let total = total as usize;
        let total_pages = total.div_ceil(per_page);
        let page = if total_pages == 0 {
            1
        } else {
            requested_page.min(total_pages)
        };
        let mut statement = connection.prepare(&format!("SELECT id,media_root,path,device,inode,jav_code,title,nfo_path,artwork_path,artwork_status,artwork_content_type,artwork_error,observed_at,captured_date,state,exception FROM media_assets WHERE {filter} ORDER BY captured_date DESC, path LIMIT ?3 OFFSET ?4"))?;
        let items = statement
            .query_map(
                params![
                    needle,
                    state,
                    per_page as i64,
                    ((page - 1) * per_page) as i64
                ],
                asset_from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        let mut groups_stmt = connection.prepare(&format!("SELECT captured_date,count(*) FROM media_assets WHERE {filter} GROUP BY captured_date ORDER BY captured_date DESC"))?;
        let groups = groups_stmt
            .query_map(params![needle, state], |r| {
                Ok(DateGroup {
                    date: r.get(0)?,
                    count: r.get::<_, i64>(1)? as usize,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AssetPage {
            items,
            groups,
            page,
            per_page,
            total,
            total_pages,
        })
    }

    pub fn indexed_artwork(&self, id: &str) -> Result<Option<PathBuf>, Error> {
        let Some(record) = self.indexed_artwork_record(id)? else {
            return Ok(None);
        };
        let path = record.path.clone();
        Ok(read_artwork_fast(&record).map(|_| path))
    }

    pub fn read_indexed_artwork(&self, id: &str) -> Result<Option<IndexedArtwork>, Error> {
        let Some(record) = self.indexed_artwork_record(id)? else {
            return Ok(None);
        };
        Ok(read_artwork_fast(&record))
    }

    fn indexed_artwork_record(&self, id: &str) -> Result<Option<IndexedArtworkRecord>, Error> {
        let value = self.connection()?.query_row(
            "SELECT artwork_path,media_root,artwork_status,artwork_content_type,artwork_root_device,artwork_root_inode,artwork_device,artwork_inode,artwork_size,artwork_modified_seconds,artwork_modified_nanoseconds,artwork_changed_seconds,artwork_changed_nanoseconds FROM media_assets WHERE id=?1",
            [id],
            |row| Ok((
                row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?, row.get::<_, Option<i64>>(4)?, row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<i64>>(6)?, row.get::<_, Option<i64>>(7)?, row.get::<_, Option<i64>>(8)?,
                row.get::<_, Option<i64>>(9)?, row.get::<_, Option<i64>>(10)?, row.get::<_, Option<i64>>(11)?,
                row.get::<_, Option<i64>>(12)?,
            )),
        ).optional()?;
        let Some((
            Some(path),
            root,
            status,
            Some(content_type),
            Some(root_device),
            Some(root_inode),
            Some(device),
            Some(inode),
            Some(size),
            Some(modified_seconds),
            Some(modified_nanoseconds),
            Some(changed_seconds),
            Some(changed_nanoseconds),
        )) = value
        else {
            return Ok(None);
        };
        if ArtworkStatus::parse(&status) != ArtworkStatus::Valid {
            return Ok(None);
        }
        Ok(Some(IndexedArtworkRecord {
            path: PathBuf::from(path),
            root: PathBuf::from(root),
            content_type,
            identity: ArtworkIdentity {
                root_device: root_device as u64,
                root_inode: root_inode as u64,
                device: device as u64,
                inode: inode as u64,
                size: size as u64,
                modified_seconds,
                modified_nanoseconds,
                changed_seconds,
                changed_nanoseconds,
            },
        }))
    }

    pub fn assets_by_identities(
        &self,
        identities: &[(u64, u64)],
    ) -> Result<Vec<MediaAsset>, Error> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT id,media_root,path,device,inode,jav_code,title,nfo_path,artwork_path,artwork_status,artwork_content_type,artwork_error,observed_at,captured_date,state,exception FROM media_assets WHERE device=?1 AND inode=?2 ORDER BY path")?;
        let mut seen = HashSet::new();
        let mut assets = Vec::new();
        for &(device, inode) in identities {
            let matches =
                statement.query_map(params![device as i64, inode as i64], asset_from_row)?;
            for asset in matches {
                let asset = asset?;
                if seen.insert(asset.id.clone()) {
                    assets.push(asset);
                }
            }
        }
        Ok(assets)
    }

    pub fn detail(&self, id: &str) -> Result<Option<AssetDetail>, Error> {
        let asset = self.connection()?.query_row(
            "SELECT id,media_root,path,device,inode,jav_code,title,nfo_path,artwork_path,artwork_status,artwork_content_type,artwork_error,observed_at,captured_date,state,exception FROM media_assets WHERE id=?1",
            [id], asset_from_row,
        ).optional()?;
        let Some(asset) = asset else { return Ok(None) };
        let parsed = asset
            .nfo_path
            .as_deref()
            .map(Path::new)
            .map(|path| parse_nfo(Path::new(&asset.media_root), path));
        let (metadata, parse_status, live_exception) = match parsed {
            Some(Ok(metadata)) => (metadata, "valid", None),
            Some(Err(reason)) if is_empty_nfo_error(&reason) => (
                ParsedNfo::default(),
                "empty",
                Some(format!(
                    "NFO metadata file is empty. Regenerate it: {reason}"
                )),
            ),
            Some(Err(reason)) => (
                ParsedNfo::default(),
                "invalid",
                Some(format!("NFO metadata is no longer safe or valid: {reason}")),
            ),
            None => (ParsedNfo::default(), "missing", None),
        };
        let actors = metadata
            .actors
            .into_iter()
            .map(|name| AssetActor {
                name,
                poster_url: None,
                actor_folder_url: None,
            })
            .collect();
        let artwork = ArtworkProvenance {
            status: asset.artwork_status,
            source_path: asset.artwork_path.clone(),
            content_type: asset.artwork_content_type.clone(),
            error: asset.artwork_error.clone(),
        };
        Ok(Some(AssetDetail {
            id: asset.id,
            path: asset.path,
            jav_code: asset.jav_code,
            title: metadata.title.or(asset.title),
            artwork_url: asset.artwork_url,
            artwork,
            captured_date: asset.captured_date,
            actors,
            studio: metadata.studio,
            release_date: metadata.release_date,
            runtime_minutes: metadata.runtime_minutes,
            director: metadata.director,
            tags: metadata.tags,
            plot: metadata.plot,
            parse_status: parse_status.to_owned(),
            source_path: asset.nfo_path,
            state: asset.state,
            exception: live_exception.or(asset.exception),
        }))
    }

    pub fn root_health(&self, path: &Path) -> RootHealth {
        let metadata = fs::metadata(path).ok();
        let readable = fs::read_dir(path).is_ok();
        let writable = metadata
            .as_ref()
            .is_some_and(|m| !m.permissions().readonly() && access(path, libc::W_OK));
        let action = (!readable || !writable).then(|| format!("TrueNAS Host Path '{}' must be mounted with {} access for container UID {} / GID {}; update the dataset ACL or container security context.", path.display(), if readable { "read/write" } else { "read" }, unsafe { libc::geteuid() }, unsafe { libc::getegid() }));
        RootHealth {
            path: path.display().to_string(),
            readable,
            writable,
            uid: unsafe { libc::geteuid() },
            gid: unsafe { libc::getegid() },
            owner_uid: metadata.as_ref().map(MetadataExt::uid),
            owner_gid: metadata.as_ref().map(MetadataExt::gid),
            action,
            capacity: root_capacity(path),
        }
    }

    pub fn health_json(&self) -> Result<serde_json::Value, Error> {
        let connection = self.connection()?;
        connection.query_row("SELECT state,mode,started_at,completed_at,error FROM asset_index_health WHERE singleton=1", [], |r| Ok(serde_json::json!({"state":r.get::<_,String>(0)?,"mode":r.get::<_,Option<String>>(1)?,"started_at":r.get::<_,Option<i64>>(2)?,"completed_at":r.get::<_,Option<i64>>(3)?,"error":r.get::<_,Option<String>>(4)?}))).map_err(Error::from)
    }

    fn set_health(
        &self,
        state: &str,
        mode: Option<ScanMode>,
        started: Option<u64>,
        completed: Option<u64>,
        error: Option<&str>,
    ) -> Result<(), Error> {
        self.connection()?.execute("UPDATE asset_index_health SET state=?1,mode=?2,started_at=coalesce(?3,started_at),completed_at=?4,error=?5 WHERE singleton=1",params![state,mode.map(|m|format!("{:?}",m).to_lowercase()),started.map(|v|v as i64),completed.map(|v|v as i64),error])?;
        Ok(())
    }
}

fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), rusqlite::Error> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    if !columns.iter().any(|existing| existing == column) {
        connection.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }
    Ok(())
}

fn validate_roots(roots: &[PathBuf]) -> Result<(), Error> {
    for (i, a) in roots.iter().enumerate() {
        for b in &roots[i + 1..] {
            if a.starts_with(b) || b.starts_with(a) {
                return Err(Error::Scan {
                    path: a.clone(),
                    source: std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Media Roots must not overlap",
                    ),
                });
            }
        }
    }
    Ok(())
}
fn media_files(root: &Path) -> Result<(Vec<PathBuf>, (u64, u64)), Error> {
    let directory = open_root(root).map_err(|source| Error::Scan {
        path: root.to_owned(),
        source,
    })?;
    let metadata = directory.metadata().map_err(|source| Error::Scan {
        path: root.to_owned(),
        source,
    })?;
    let identity = (metadata.dev(), metadata.ino());
    Ok((media_files_from(&directory, root)?, identity))
}

fn media_files_from(directory: &fs::File, display_path: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut out = Vec::new();
    let entries = directory_entries(directory).map_err(|source| Error::Scan {
        path: display_path.to_owned(),
        source,
    })?;
    for name in entries {
        let path = display_path.join(&name);
        let name = CString::new(name.as_bytes()).map_err(|_| Error::Scan {
            path: path.clone(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "directory entry contains NUL",
            ),
        })?;
        let mut stat = MaybeUninit::<libc::stat>::zeroed();
        if unsafe {
            libc::fstatat(
                directory.as_raw_fd(),
                name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            return Err(Error::Scan {
                path,
                source: std::io::Error::last_os_error(),
            });
        }
        let stat = unsafe { stat.assume_init() };
        let kind = stat.st_mode & libc::S_IFMT;
        if kind == libc::S_IFDIR {
            let fd = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY
                        | libc::O_DIRECTORY
                        | libc::O_CLOEXEC
                        | libc::O_NOFOLLOW
                        | libc::O_NONBLOCK,
                )
            };
            if fd < 0 {
                return Err(Error::Scan {
                    path,
                    source: std::io::Error::last_os_error(),
                });
            }
            let child = unsafe { fs::File::from_raw_fd(fd) };
            if !child.metadata().is_ok_and(|metadata| {
                metadata.file_type().is_dir()
                    && metadata.dev() == stat.st_dev as u64
                    && metadata.ino() == stat.st_ino as u64
            }) {
                return Err(Error::Scan {
                    path,
                    source: std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "directory entry changed while scanning",
                    ),
                });
            }
            out.extend(media_files_from(&child, &path)?)
        } else if kind == libc::S_IFREG && is_video(&path) && !is_secondary_multipart(&path) {
            out.push(path)
        }
    }
    Ok(out)
}

fn directory_entries(directory: &fs::File) -> std::io::Result<Vec<OsString>> {
    let duplicate = unsafe { libc::dup(directory.as_raw_fd()) };
    if duplicate < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let stream = unsafe { libc::fdopendir(duplicate) };
    if stream.is_null() {
        unsafe { libc::close(duplicate) };
        return Err(std::io::Error::last_os_error());
    }
    let mut entries = Vec::new();
    let mut read_error = None;
    loop {
        set_errno(0);
        let entry = unsafe { libc::readdir(stream) };
        if entry.is_null() {
            let errno = current_errno();
            if errno != 0 {
                read_error = Some(std::io::Error::from_raw_os_error(errno));
            }
            break;
        }
        let bytes = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        entries.push(OsString::from_vec(bytes.to_vec()));
    }
    if unsafe { libc::closedir(stream) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    if let Some(error) = read_error {
        return Err(error);
    }
    Ok(entries)
}

#[cfg(target_os = "linux")]
fn errno_location() -> *mut libc::c_int {
    unsafe { libc::__errno_location() }
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn errno_location() -> *mut libc::c_int {
    unsafe { libc::__error() }
}

fn set_errno(value: libc::c_int) {
    unsafe { *errno_location() = value }
}

fn current_errno() -> libc::c_int {
    unsafe { *errno_location() }
}

fn is_secondary_multipart(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    if !parent.join("movie.nfo").is_file() {
        return false;
    }
    let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
        return false;
    };
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    let Some((base, suffix)) = stem.rsplit_once('-') else {
        return false;
    };
    let primary_suffix = if suffix.len() == 1
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() && !byte.eq_ignore_ascii_case(&b'A'))
    {
        if suffix.as_bytes()[0].is_ascii_uppercase() {
            "A"
        } else {
            "a"
        }
    } else if suffix.parse::<u32>().is_ok_and(|part| part > 1) {
        "1"
    } else {
        return false;
    };
    let primary = parent.join(format!("{base}-{primary_suffix}.{extension}"));
    let unsuffixed = parent.join(format!("{base}.{extension}"));
    primary.is_file() || unsuffixed.is_file()
}
fn is_video(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|v| v.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("mp4" | "mkv" | "avi" | "mov" | "m4v" | "wmv" | "ts" | "m2ts")
    )
}
fn sibling(path: &Path, exts: &[&str]) -> Option<PathBuf> {
    exts.iter()
        .map(|e| path.with_extension(e))
        .find(|p| p.is_file())
}
fn metadata_companion(path: &Path, conventional_names: &[&str], exts: &[&str]) -> Option<PathBuf> {
    sibling(path, exts)
        .filter(|candidate| {
            fs::symlink_metadata(candidate).is_ok_and(|metadata| metadata.file_type().is_file())
        })
        .or_else(|| {
            let parent = path.parent()?;
            conventional_names
                .iter()
                .map(|name| parent.join(name))
                .find(|candidate| {
                    fs::symlink_metadata(candidate)
                        .is_ok_and(|metadata| metadata.file_type().is_file())
                })
        })
}

fn artwork_companion(
    root: &Path,
    media_path: &Path,
) -> (Option<PathBuf>, Option<ArtworkInspection>) {
    let mut candidates = ["jpg", "jpeg", "png", "webp"]
        .into_iter()
        .map(|extension| media_path.with_extension(extension))
        .collect::<Vec<_>>();
    if let Some(parent) = media_path.parent() {
        candidates.extend(
            [
                "folder.jpg",
                "poster.jpg",
                "cover.jpg",
                "folder.png",
                "poster.png",
                "cover.png",
                "folder.webp",
                "poster.webp",
                "cover.webp",
            ]
            .into_iter()
            .map(|name| parent.join(name)),
        );
    }

    let mut first_invalid = None;
    for candidate in candidates {
        if fs::symlink_metadata(&candidate).is_err() {
            continue;
        }
        let inspection = inspect_artwork(root, &candidate);
        if inspection.status == ArtworkStatus::Valid {
            return (Some(candidate), Some(inspection));
        }
        if first_invalid.is_none() {
            first_invalid = Some((candidate, inspection));
        }
    }
    first_invalid.map_or((None, None), |(path, inspection)| {
        (Some(path), Some(inspection))
    })
}
fn asset_id() -> String {
    let mut bytes = [0u8; 18];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
fn access(path: &Path, mode: i32) -> bool {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    CString::new(path.as_os_str().as_bytes())
        .ok()
        .is_some_and(|p| unsafe { libc::access(p.as_ptr(), mode) == 0 })
}
fn asset_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MediaAsset> {
    let id: String = row.get(0)?;
    let artwork_path: Option<String> = row.get(8)?;
    let artwork_status = ArtworkStatus::parse(&row.get::<_, String>(9)?);
    Ok(MediaAsset {
        id: id.clone(),
        media_root: row.get(1)?,
        path: row.get(2)?,
        device: row.get::<_, i64>(3)? as u64,
        inode: row.get::<_, i64>(4)? as u64,
        jav_code: row.get(5)?,
        title: row.get(6)?,
        nfo_path: row.get(7)?,
        artwork_url: (artwork_status == ArtworkStatus::Valid)
            .then(|| format!("/api/v1/assets/{id}/artwork")),
        artwork_path,
        artwork_status,
        artwork_content_type: row.get(10)?,
        artwork_error: row.get(11)?,
        observed_at: row.get::<_, i64>(12)? as u64,
        captured_date: row.get(13)?,
        state: AssetState::parse(&row.get::<_, String>(14)?),
        exception: row.get(15)?,
    })
}

fn metadata_matches(metadata: &fs::Metadata, identity: ArtworkIdentity) -> bool {
    metadata.file_type().is_file()
        && metadata.dev() == identity.device
        && metadata.ino() == identity.inode
        && metadata.len() == identity.size
        && metadata.mtime() == identity.modified_seconds
        && metadata.mtime_nsec() == identity.modified_nanoseconds
        && metadata.ctime() == identity.changed_seconds
        && metadata.ctime_nsec() == identity.changed_nanoseconds
}

fn read_artwork_fast(record: &IndexedArtworkRecord) -> Option<IndexedArtwork> {
    let root_file = open_root(&record.root).ok()?;
    let root_metadata = root_file.metadata().ok()?;
    if root_metadata.dev() != record.identity.root_device
        || root_metadata.ino() != record.identity.root_inode
    {
        return None;
    }
    let mut file = open_beneath_from(&root_file, &record.root, &record.path).ok()?;
    let before = file.metadata().ok()?;
    if !metadata_matches(&before, record.identity)
        || before.len() > crate::artwork_image::MAX_ARTWORK_BYTES
    {
        return None;
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    file.by_ref()
        .take(crate::artwork_image::MAX_ARTWORK_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 != before.len()
        || !metadata_matches(&file.metadata().ok()?, record.identity)
    {
        return None;
    }
    let content_type = crate::artwork_image::sniff_content_type(&bytes)?;
    if content_type != record.content_type {
        return None;
    }
    Some(IndexedArtwork {
        bytes,
        content_type,
    })
}

fn inspect_artwork(root: &Path, path: &Path) -> ArtworkInspection {
    let root_file = match open_root(root) {
        Ok(root_file) => root_file,
        Err(error) => {
            return ArtworkInspection::invalid(
                ArtworkStatus::Unreadable,
                None,
                format!("Media Root cannot be opened safely: {error}"),
            )
        }
    };
    let root_metadata = match root_file.metadata() {
        Ok(metadata) => metadata,
        Err(error) => {
            return ArtworkInspection::invalid(
                ArtworkStatus::Unreadable,
                None,
                format!("Media Root metadata is unavailable: {error}"),
            )
        }
    };
    let mut file = match open_beneath_from(&root_file, root, path) {
        Ok(file) => file,
        Err(error) => {
            return ArtworkInspection::invalid(
                ArtworkStatus::Unreadable,
                None,
                format!(
                    "Local artwork cannot be opened safely: {error}; replace or remove {}, then reconcile the Asset Index.",
                    path.display()
                ),
            )
        }
    };
    let metadata = match file.metadata() {
        Ok(metadata) if metadata.file_type().is_file() => metadata,
        Ok(_) => {
            return ArtworkInspection::invalid(
                ArtworkStatus::Unreadable,
                None,
                format!("Local artwork is not a regular file: {}", path.display()),
            )
        }
        Err(error) => {
            return ArtworkInspection::invalid(
                ArtworkStatus::Unreadable,
                None,
                format!("Local artwork metadata is unavailable: {error}"),
            )
        }
    };
    let identity = ArtworkIdentity {
        root_device: root_metadata.dev(),
        root_inode: root_metadata.ino(),
        device: metadata.dev(),
        inode: metadata.ino(),
        size: metadata.len(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        changed_seconds: metadata.ctime(),
        changed_nanoseconds: metadata.ctime_nsec(),
    };
    if metadata.len() == 0 {
        return ArtworkInspection::invalid(
            ArtworkStatus::Empty,
            None,
            format!(
                "Local artwork is empty; replace or remove {}, then reconcile the Asset Index.",
                path.display()
            ),
        );
    }
    if metadata.len() > crate::artwork_image::MAX_ARTWORK_BYTES {
        return ArtworkInspection::invalid(
            ArtworkStatus::TooLarge,
            None,
            format!(
                "Local artwork exceeds the {} byte safety limit: {}",
                crate::artwork_image::MAX_ARTWORK_BYTES,
                path.display()
            ),
        );
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    if let Err(error) = file
        .by_ref()
        .take(crate::artwork_image::MAX_ARTWORK_BYTES + 1)
        .read_to_end(&mut bytes)
    {
        return ArtworkInspection::invalid(
            ArtworkStatus::Unreadable,
            None,
            format!("Local artwork cannot be read: {error}"),
        );
    }
    if bytes.len() as u64 > crate::artwork_image::MAX_ARTWORK_BYTES {
        return ArtworkInspection::invalid(
            ArtworkStatus::TooLarge,
            None,
            format!(
                "Local artwork exceeds the {} byte safety limit",
                crate::artwork_image::MAX_ARTWORK_BYTES
            ),
        );
    }

    let sniffed_content_type = crate::artwork_image::sniff_content_type(&bytes);
    let validated = match crate::artwork_image::validate(bytes) {
        Ok(validated) => validated,
        Err(error) => {
            let status = match error.kind {
                crate::artwork_image::ValidationErrorKind::Unrecognized => {
                    ArtworkStatus::Unrecognized
                }
                crate::artwork_image::ValidationErrorKind::Animated => ArtworkStatus::Animated,
                crate::artwork_image::ValidationErrorKind::TruncatedOrCorrupt => {
                    ArtworkStatus::TruncatedOrCorrupt
                }
                crate::artwork_image::ValidationErrorKind::TooLarge => ArtworkStatus::TooLarge,
                crate::artwork_image::ValidationErrorKind::Unreadable => ArtworkStatus::Unreadable,
            };
            return ArtworkInspection::invalid(
                status,
                sniffed_content_type,
                format!(
                    "Local artwork is unusable: {error}; replace or remove {}, then reconcile the Asset Index.",
                    path.display()
                ),
            );
        }
    };

    ArtworkInspection {
        status: ArtworkStatus::Valid,
        content_type: Some(validated.content_type),
        error: None,
        identity: Some(identity),
    }
}

fn open_beneath(root: &Path, path: &Path) -> std::io::Result<fs::File> {
    let root_file = open_root(root)?;
    open_beneath_from(&root_file, root, path)
}

fn open_beneath_from(root_file: &fs::File, root: &Path, path: &Path) -> std::io::Result<fs::File> {
    let relative = path.strip_prefix(root).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "path is outside Media Root",
        )
    })?;
    let parts = relative
        .components()
        .map(|component| match component {
            Component::Normal(value) => Ok(value.to_owned()),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "path contains a non-normal component",
            )),
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    if parts.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path does not name a file",
        ));
    }
    let root_fd = unsafe { libc::dup(root_file.as_raw_fd()) };
    if root_fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut current = unsafe { fs::File::from_raw_fd(root_fd) };
    for (index, part) in parts.iter().enumerate() {
        let name = CString::new(part.as_bytes()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains NUL")
        })?;
        let directory_flag = if index + 1 == parts.len() {
            0
        } else {
            libc::O_DIRECTORY
        };
        let fd = unsafe {
            libc::openat(
                current.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY
                    | libc::O_CLOEXEC
                    | libc::O_NOFOLLOW
                    | libc::O_NONBLOCK
                    | directory_flag,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        current = unsafe { fs::File::from_raw_fd(fd) };
    }
    if !current.metadata()?.file_type().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path is not an ordinary regular file",
        ));
    }
    Ok(current)
}

fn open_root(root: &Path) -> std::io::Result<fs::File> {
    let name = CString::new(root.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "root contains NUL"))?;
    let fd = unsafe {
        libc::open(
            name.as_ptr(),
            libc::O_RDONLY
                | libc::O_DIRECTORY
                | libc::O_CLOEXEC
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let root = unsafe { fs::File::from_raw_fd(fd) };
    if !root.metadata()?.file_type().is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Media Root is not a real directory",
        ));
    }
    Ok(root)
}

fn parse_nfo(root: &Path, path: &Path) -> Result<ParsedNfo, String> {
    let mut file = open_beneath(root, path)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let mut xml = String::new();
    file.read_to_string(&mut xml)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if xml.trim().is_empty() {
        return Err(format!("{} is empty", path.display()));
    }
    let document = roxmltree::Document::parse(&xml)
        .map_err(|error| format!("{} ({error})", path.display()))?;
    let movie = document
        .descendants()
        .find(|node| node.has_tag_name("movie"))
        .ok_or_else(|| format!("{} does not contain a <movie> element", path.display()))?;
    let text = |tag: &str| {
        movie
            .children()
            .find(|node| node.has_tag_name(tag))
            .and_then(|node| node.text())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let mut tags = Vec::new();
    for tag in ["genre", "tag"] {
        tags.extend(
            movie
                .children()
                .filter(|node| node.has_tag_name(tag))
                .filter_map(|node| node.text())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
        );
    }
    let actors = movie
        .children()
        .filter(|node| node.has_tag_name("actor"))
        .filter_map(|actor| actor.children().find(|node| node.has_tag_name("name")))
        .filter_map(|node| node.text())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect();
    let runtime_minutes =
        text("runtime").and_then(|value| value.split_whitespace().next()?.parse().ok());
    Ok(ParsedNfo {
        title: text("title"),
        actors,
        studio: text("studio"),
        release_date: text("premiered")
            .or_else(|| text("releasedate"))
            .or_else(|| text("date")),
        runtime_minutes,
        director: text("director"),
        tags,
        plot: text("plot"),
    })
}

fn is_empty_nfo_error(reason: &str) -> bool {
    reason.ends_with(" is empty")
}
