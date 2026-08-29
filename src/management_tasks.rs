use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{rngs::OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("task database failed: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("task database lock was poisoned")]
    Poisoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Preview,
    Mutation,
}

impl TaskKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Mutation => "mutation",
        }
    }

    fn parse(value: &str) -> Self {
        if value == "mutation" {
            Self::Mutation
        } else {
            Self::Preview
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Interrupted,
}

impl TaskStatus {
    fn parse(value: &str) -> Self {
        match value {
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "interrupted" => Self::Interrupted,
            _ => Self::Queued,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Interrupted)
    }
}

#[derive(Debug, Clone)]
pub struct NewTask {
    pub task_type: String,
    pub media_root: String,
    pub kind: TaskKind,
}

impl NewTask {
    pub fn preview(task_type: impl Into<String>, media_root: impl Into<String>) -> Self {
        Self {
            task_type: task_type.into(),
            media_root: media_root.into(),
            kind: TaskKind::Preview,
        }
    }

    pub fn mutation(task_type: impl Into<String>, media_root: impl Into<String>) -> Self {
        Self {
            task_type: task_type.into(),
            media_root: media_root.into(),
            kind: TaskKind::Mutation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: i64,
    pub kind: String,
    pub path: Option<String>,
    pub status: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagementTask {
    pub id: String,
    pub task_type: String,
    pub media_root: String,
    pub kind: TaskKind,
    pub status: TaskStatus,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
    pub error: Option<String>,
    pub items: Vec<TaskItem>,
}

#[derive(Clone)]
pub struct TaskStore(Arc<Mutex<Connection>>);

impl TaskStore {
    pub fn open(path: &Path) -> Result<Self, Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| rusqlite::Error::InvalidPath(path.to_owned()))?;
        }
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS management_tasks (
               id TEXT PRIMARY KEY, task_type TEXT NOT NULL, media_root TEXT NOT NULL,
               kind TEXT NOT NULL, status TEXT NOT NULL, created_at INTEGER NOT NULL,
               started_at INTEGER, finished_at INTEGER, error TEXT
             );
             CREATE TABLE IF NOT EXISTS management_task_items (
               id INTEGER PRIMARY KEY AUTOINCREMENT, task_id TEXT NOT NULL,
               kind TEXT NOT NULL, path TEXT, status TEXT NOT NULL, message TEXT,
               FOREIGN KEY(task_id) REFERENCES management_tasks(id) ON DELETE CASCADE
             );
             CREATE TABLE IF NOT EXISTS deletion_audit_records (
               id INTEGER PRIMARY KEY AUTOINCREMENT, task_id TEXT NOT NULL UNIQUE,
               created_at INTEGER NOT NULL, record_json TEXT NOT NULL,
               FOREIGN KEY(task_id) REFERENCES management_tasks(id)
             );",
        )?;
        Ok(Self(Arc::new(Mutex::new(connection))))
    }

    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, Error> {
        self.0.lock().map_err(|_| Error::Poisoned)
    }

    pub fn create(&self, input: NewTask, now: u64) -> Result<ManagementTask, Error> {
        let id = task_id();
        self.connection()?.execute(
            "INSERT INTO management_tasks (id, task_type, media_root, kind, status, created_at) VALUES (?1, ?2, ?3, ?4, 'queued', ?5)",
            params![id, input.task_type, input.media_root, input.kind.as_str(), now],
        )?;
        Ok(self.get(&id)?.expect("inserted task must exist"))
    }

    pub fn mark_running(&self, id: &str, now: u64) -> Result<(), Error> {
        self.connection()?.execute(
            "UPDATE management_tasks SET status='running', started_at=?2 WHERE id=?1",
            params![id, now],
        )?;
        Ok(())
    }

    pub fn mark_completed(&self, id: &str, now: u64) -> Result<(), Error> {
        self.connection()?.execute(
            "UPDATE management_tasks SET status='completed', finished_at=?2 WHERE id=?1",
            params![id, now],
        )?;
        Ok(())
    }

    pub fn mark_failed(&self, id: &str, now: u64, error: &str) -> Result<(), Error> {
        self.connection()?.execute(
            "UPDATE management_tasks SET status='failed', finished_at=?2, error=?3 WHERE id=?1",
            params![id, now, error],
        )?;
        Ok(())
    }

    pub fn finish_item(
        &self,
        task_id: &str,
        kind: &str,
        path: Option<&str>,
        status: &str,
        message: Option<&str>,
    ) -> Result<(), Error> {
        self.connection()?.execute(
            "INSERT INTO management_task_items (task_id, kind, path, status, message) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![task_id, kind, path, status, message],
        )?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<ManagementTask>, Error> {
        let connection = self.connection()?;
        let mut task = connection.query_row(
            "SELECT id, task_type, media_root, kind, status, created_at, started_at, finished_at, error FROM management_tasks WHERE id=?1",
            [id], task_from_row,
        ).optional()?;
        if let Some(task) = task.as_mut() {
            task.items = items(&connection, id)?;
        }
        Ok(task)
    }

    pub fn list(&self) -> Result<Vec<ManagementTask>, Error> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT id, task_type, media_root, kind, status, created_at, started_at, finished_at, error FROM management_tasks ORDER BY created_at DESC, id DESC")?;
        let mut tasks = statement
            .query_map([], task_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        for task in &mut tasks {
            task.items = items(&connection, &task.id)?;
        }
        Ok(tasks)
    }

    pub fn interrupt_running_destructive(&self, now: u64) -> Result<usize, Error> {
        Ok(self.connection()?.execute(
            "UPDATE management_tasks SET status='interrupted', finished_at=?1, error='service restarted during destructive task' WHERE status='running' AND kind='mutation'",
            [now],
        )?)
    }

    pub fn runnable_tasks(&self) -> Result<Vec<ManagementTask>, Error> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|task| task.status == TaskStatus::Queued && task.kind != TaskKind::Mutation)
            .collect())
    }

    /// Audit records intentionally have no retention/deletion API. They remain
    /// durable for the lifetime of the management database by default.
    pub fn record_deletion_audit(
        &self,
        task_id: &str,
        created_at: u64,
        record: &serde_json::Value,
    ) -> Result<(), Error> {
        self.connection()?.execute(
            "INSERT INTO deletion_audit_records(task_id,created_at,record_json) VALUES (?1,?2,?3)",
            params![task_id, created_at, record.to_string()],
        )?;
        Ok(())
    }

    pub fn deletion_audits(&self) -> Result<Vec<serde_json::Value>, Error> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT record_json FROM deletion_audit_records ORDER BY created_at DESC,id DESC",
        )?;
        let records = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|value| value.ok().and_then(|json| serde_json::from_str(&json).ok()))
            .collect();
        Ok(records)
    }
}

fn task_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ManagementTask> {
    let kind: String = row.get(3)?;
    let status: String = row.get(4)?;
    Ok(ManagementTask {
        id: row.get(0)?,
        task_type: row.get(1)?,
        media_root: row.get(2)?,
        kind: TaskKind::parse(&kind),
        status: TaskStatus::parse(&status),
        created_at: row.get(5)?,
        started_at: row.get(6)?,
        finished_at: row.get(7)?,
        error: row.get(8)?,
        items: Vec::new(),
    })
}

fn items(connection: &Connection, task_id: &str) -> rusqlite::Result<Vec<TaskItem>> {
    let mut statement = connection.prepare("SELECT id, kind, path, status, message FROM management_task_items WHERE task_id=?1 ORDER BY id")?;
    let rows = statement.query_map([task_id], |row| {
        Ok(TaskItem {
            id: row.get(0)?,
            kind: row.get(1)?,
            path: row.get(2)?,
            status: row.get(3)?,
            message: row.get(4)?,
        })
    })?;
    rows.collect()
}

fn task_id() -> String {
    let mut bytes = [0u8; 18];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[derive(Clone, Default)]
pub struct TaskCoordinator {
    roots: Arc<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>>,
}

impl TaskCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn mutation(&self, media_root: &str) -> OwnedMutexGuard<()> {
        let lock = self
            .roots
            .lock()
            .unwrap()
            .entry(media_root.to_owned())
            .or_default()
            .clone();
        lock.lock_owned().await
    }

    pub async fn preview(&self, _media_root: &str) {}
}
