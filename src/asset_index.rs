use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
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
    pub observed_at: u64,
    pub captured_date: String,
    pub state: AssetState,
    pub exception: Option<String>,
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
                for path in media_files(root)? {
                    self.observe(root, &path, now, generation)?;
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
        for path in paths {
            if path.exists() && is_video(path) {
                self.observe(root, path, now, now as i64)?;
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

    fn observe(&self, root: &Path, path: &Path, now: u64, generation: i64) -> Result<(), Error> {
        let metadata = fs::metadata(path).map_err(|source| Error::Scan {
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
        let nfo = sibling(path, &["nfo"]);
        let artwork = sibling(path, &["jpg", "jpeg", "png", "webp"]);
        let title = nfo
            .as_ref()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| {
                Regex::new(r"(?is)<title>\s*(.*?)\s*</title>")
                    .unwrap()
                    .captures(&s)
                    .map(|c| c[1].trim().to_owned())
            });
        let (state, exception) = if nfo.is_some() {
            (AssetState::Normal, None)
        } else {
            (AssetState::Exception, Some("NFO metadata is missing"))
        };
        let captured_date =
            DateTime::<Utc>::from(metadata.modified().unwrap_or(std::time::UNIX_EPOCH))
                .format("%Y-%m-%d")
                .to_string();
        let connection = self.connection()?;
        let id: Option<String> = connection.query_row(
            "SELECT id FROM media_assets WHERE media_root=?1 AND ((device=?2 AND inode=?3) OR path=?4 OR (?5 IS NOT NULL AND jav_code=?5)) ORDER BY CASE WHEN device=?2 AND inode=?3 THEN 0 WHEN path=?4 THEN 1 ELSE 2 END LIMIT 1",
            params![root.display().to_string(), metadata.dev() as i64, metadata.ino() as i64, path.display().to_string(), jav_code], |row| row.get(0)).optional()?;
        let id = id.unwrap_or_else(asset_id);
        connection.execute(
            "DELETE FROM media_assets WHERE path=?1 AND id<>?2",
            params![path.display().to_string(), id],
        )?;
        connection.execute("INSERT INTO media_assets(id,media_root,path,device,inode,jav_code,title,nfo_path,artwork_path,observed_at,captured_date,state,exception,generation)
          VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
          ON CONFLICT(id) DO UPDATE SET media_root=excluded.media_root,path=excluded.path,device=excluded.device,inode=excluded.inode,jav_code=excluded.jav_code,title=excluded.title,nfo_path=excluded.nfo_path,artwork_path=excluded.artwork_path,observed_at=excluded.observed_at,captured_date=excluded.captured_date,state=excluded.state,exception=excluded.exception,generation=excluded.generation",
          params![id, root.display().to_string(), path.display().to_string(), metadata.dev() as i64, metadata.ino() as i64, jav_code, title, nfo.map(|p|p.display().to_string()), artwork.map(|p|p.display().to_string()), now as i64, captured_date, state.as_str(), exception, generation])?;
        Ok(())
    }

    pub fn search(&self, query: AssetQuery) -> Result<AssetPage, Error> {
        let page = query.page.max(1);
        let per_page = if query.per_page == 0 {
            48
        } else {
            query.per_page.min(200)
        };
        let needle = format!("%{}%", query.query.unwrap_or_default().to_lowercase());
        let state = query.state.map(|s| s.as_str().to_owned());
        let connection = self.connection()?;
        let filter = "(?1='%%' OR lower(coalesce(jav_code,'')||' '||coalesce(title,'')||' '||path) LIKE ?1) AND (?2 IS NULL OR state=?2)";
        let total: i64 = connection.query_row(
            &format!("SELECT count(*) FROM media_assets WHERE {filter}"),
            params![needle, state],
            |r| r.get(0),
        )?;
        let mut statement = connection.prepare(&format!("SELECT id,media_root,path,device,inode,jav_code,title,nfo_path,artwork_path,observed_at,captured_date,state,exception FROM media_assets WHERE {filter} ORDER BY captured_date DESC, path LIMIT ?3 OFFSET ?4"))?;
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
            total: total as usize,
            total_pages: (total as usize).div_ceil(per_page),
        })
    }

    pub fn indexed_artwork(&self, id: &str) -> Result<Option<PathBuf>, Error> {
        let value: Option<String> = self
            .connection()?
            .query_row(
                "SELECT artwork_path FROM media_assets WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(value
            .map(PathBuf::from)
            .filter(|p| p.is_file() && is_artwork(p)))
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
fn media_files(root: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut out = Vec::new();
    let entries = fs::read_dir(root).map_err(|source| Error::Scan {
        path: root.to_owned(),
        source,
    })?;
    for entry in entries {
        let path = entry
            .map_err(|source| Error::Scan {
                path: root.to_owned(),
                source,
            })?
            .path();
        if path.is_dir() {
            out.extend(media_files(&path)?)
        } else if is_video(&path) {
            out.push(path)
        }
    }
    Ok(out)
}
fn is_video(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|v| v.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("mp4" | "mkv" | "avi" | "mov" | "m4v" | "wmv" | "ts")
    )
}
fn is_artwork(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|v| v.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp")
    )
}
fn sibling(path: &Path, exts: &[&str]) -> Option<PathBuf> {
    exts.iter()
        .map(|e| path.with_extension(e))
        .find(|p| p.is_file())
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
    let artwork: Option<String> = row.get(8)?;
    Ok(MediaAsset {
        id: id.clone(),
        media_root: row.get(1)?,
        path: row.get(2)?,
        device: row.get::<_, i64>(3)? as u64,
        inode: row.get::<_, i64>(4)? as u64,
        jav_code: row.get(5)?,
        title: row.get(6)?,
        nfo_path: row.get(7)?,
        artwork_url: artwork.map(|_| format!("/api/v1/assets/{id}/artwork")),
        observed_at: row.get::<_, i64>(9)? as u64,
        captured_date: row.get(10)?,
        state: AssetState::parse(&row.get::<_, String>(11)?),
        exception: row.get(12)?,
    })
}
