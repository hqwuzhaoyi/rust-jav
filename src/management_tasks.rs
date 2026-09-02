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
    pub source_plan_id: Option<String>,
}

impl NewTask {
    pub fn preview(task_type: impl Into<String>, media_root: impl Into<String>) -> Self {
        Self {
            task_type: task_type.into(),
            media_root: media_root.into(),
            kind: TaskKind::Preview,
            source_plan_id: None,
        }
    }

    pub fn mutation(task_type: impl Into<String>, media_root: impl Into<String>) -> Self {
        Self {
            task_type: task_type.into(),
            media_root: media_root.into(),
            kind: TaskKind::Mutation,
            source_plan_id: None,
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
    pub source_path: Option<String>,
    pub quarantine_token: Option<String>,
    pub intent: Option<String>,
    pub mutation_phase: Option<String>,
    pub identity_device: Option<u64>,
    pub identity_inode: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartedTaskItem {
    pub id: i64,
    pub quarantine_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningMutationItem {
    pub id: i64,
    pub task_type: String,
    pub media_root: String,
    pub source_path: Option<String>,
    pub quarantine_token: Option<String>,
    pub intent: Option<String>,
    pub mutation_phase: Option<String>,
    pub identity_device: Option<u64>,
    pub identity_inode: Option<u64>,
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
    pub plan_expires_at: Option<u64>,
    pub operation_plan: Option<serde_json::Value>,
    pub report: Option<serde_json::Value>,
    pub source_plan_id: Option<String>,
    pub plan_consumed_at: Option<u64>,
    pub planned_item_count: Option<u64>,
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
               started_at INTEGER, finished_at INTEGER, error TEXT,
               plan_expires_at INTEGER, operation_plan TEXT, report TEXT,
               source_plan_id TEXT, plan_consumed_at INTEGER, planned_item_count INTEGER
             );
             CREATE TABLE IF NOT EXISTS management_task_items (
               id INTEGER PRIMARY KEY AUTOINCREMENT, task_id TEXT NOT NULL,
               kind TEXT NOT NULL, path TEXT, status TEXT NOT NULL, message TEXT,
               source_path TEXT, quarantine_token TEXT, intent TEXT,
               mutation_phase TEXT, identity_device TEXT, identity_inode TEXT,
               FOREIGN KEY(task_id) REFERENCES management_tasks(id) ON DELETE CASCADE
             );
             CREATE TABLE IF NOT EXISTS deletion_audit_records (
               id INTEGER PRIMARY KEY AUTOINCREMENT, task_id TEXT NOT NULL UNIQUE,
               created_at INTEGER NOT NULL, record_json TEXT NOT NULL,
               FOREIGN KEY(task_id) REFERENCES management_tasks(id)
             );",
        )?;
        for migration in [
            "ALTER TABLE management_tasks ADD COLUMN plan_expires_at INTEGER",
            "ALTER TABLE management_tasks ADD COLUMN operation_plan TEXT",
            "ALTER TABLE management_tasks ADD COLUMN report TEXT",
            "ALTER TABLE management_tasks ADD COLUMN source_plan_id TEXT",
            "ALTER TABLE management_tasks ADD COLUMN plan_consumed_at INTEGER",
            "ALTER TABLE management_tasks ADD COLUMN planned_item_count INTEGER",
            "ALTER TABLE management_task_items ADD COLUMN source_path TEXT",
            "ALTER TABLE management_task_items ADD COLUMN quarantine_token TEXT",
            "ALTER TABLE management_task_items ADD COLUMN intent TEXT",
            "ALTER TABLE management_task_items ADD COLUMN mutation_phase TEXT",
            "ALTER TABLE management_task_items ADD COLUMN identity_device TEXT",
            "ALTER TABLE management_task_items ADD COLUMN identity_inode TEXT",
        ] {
            let _ = connection.execute(migration, []);
        }
        Ok(Self(Arc::new(Mutex::new(connection))))
    }

    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, Error> {
        self.0.lock().map_err(|_| Error::Poisoned)
    }

    pub fn create(&self, input: NewTask, now: u64) -> Result<ManagementTask, Error> {
        let id = task_id();
        self.connection()?.execute(
            "INSERT INTO management_tasks (id, task_type, media_root, kind, status, created_at, source_plan_id) VALUES (?1, ?2, ?3, ?4, 'queued', ?5, ?6)",
            params![id, input.task_type, input.media_root, input.kind.as_str(), now, input.source_plan_id],
        )?;
        Ok(self.get(&id)?.expect("inserted task must exist"))
    }

    pub fn create_deletion_mutation(
        &self,
        media_root: &str,
        now: u64,
        authority: &serde_json::Value,
    ) -> Result<ManagementTask, Error> {
        let valid_authority = authority["id"].is_string()
            && authority["created_at"].is_u64()
            && authority["expires_at"].is_u64()
            && matches!(
                authority["selection"].as_str(),
                Some("selected" | "unified")
            )
            && authority["hard_link_search_roots"].is_array()
            && authority["paths"].is_array()
            && authority["rule_set_version"].is_u64()
            && authority["rules"]
                .as_array()
                .is_some_and(|rules| rules.iter().all(serde_json::Value::is_string));
        if !valid_authority {
            return Err(rusqlite::Error::InvalidParameterName(
                "permanent deletion authority snapshot is incomplete".to_string(),
            )
            .into());
        }
        let id = task_id();
        let plan_id = authority["id"].as_str();
        let expires_at = authority["expires_at"].as_u64();
        let item_count = authority["paths"]
            .as_array()
            .map(|paths| paths.len() as u64);
        self.connection()?.execute(
            "INSERT INTO management_tasks
               (id, task_type, media_root, kind, status, created_at, source_plan_id,
                plan_expires_at, operation_plan, planned_item_count)
             VALUES (?1, 'permanent_deletion', ?2, 'mutation', 'queued', ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                media_root,
                now,
                plan_id,
                expires_at,
                authority.to_string(),
                item_count
            ],
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

    pub fn start_item(
        &self,
        task_id: &str,
        kind: &str,
        path: Option<&str>,
        source_path: Option<&str>,
    ) -> Result<StartedTaskItem, Error> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO management_task_items (task_id, kind, path, status, source_path) VALUES (?1, ?2, ?3, 'running', ?4)",
            params![task_id, kind, path, source_path],
        )?;
        let id = transaction.last_insert_rowid();
        let quarantine_token = format!(".rust-jav-quarantine-item-{id}");
        transaction.execute(
            "UPDATE management_task_items SET quarantine_token=?2 WHERE id=?1",
            params![id, quarantine_token],
        )?;
        transaction.commit()?;
        Ok(StartedTaskItem {
            id,
            quarantine_token,
        })
    }

    pub fn start_deletion_item(
        &self,
        task_id: &str,
        source_path: &str,
        identity_device: u64,
        identity_inode: u64,
    ) -> Result<StartedTaskItem, Error> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let inserted = transaction.execute(
            "INSERT INTO management_task_items
               (task_id, kind, path, status, source_path, intent, mutation_phase, identity_device, identity_inode)
             SELECT ?1, 'permanent_deletion', ?2, 'running', ?2, 'permanent_delete', 'intent', ?3, ?4
             FROM management_tasks
             WHERE id=?1 AND task_type='permanent_deletion' AND kind='mutation'
               AND status='running' AND operation_plan IS NOT NULL",
            params![
                task_id,
                source_path,
                identity_device.to_string(),
                identity_inode.to_string()
            ],
        )?;
        if inserted != 1 {
            transaction.rollback()?;
            return Err(rusqlite::Error::QueryReturnedNoRows.into());
        }
        let id = transaction.last_insert_rowid();
        let quarantine_token = format!(".rust-jav-quarantine-item-{id}");
        transaction.execute(
            "UPDATE management_task_items SET quarantine_token=?2 WHERE id=?1",
            params![id, quarantine_token],
        )?;
        transaction.commit()?;
        Ok(StartedTaskItem {
            id,
            quarantine_token,
        })
    }

    pub fn mark_deletion_item_quarantined(&self, task_id: &str, item_id: i64) -> Result<(), Error> {
        let updated = self.connection()?.execute(
            "UPDATE management_task_items SET mutation_phase='quarantined'
             WHERE id=?2 AND task_id=?1 AND status='running' AND mutation_phase='intent'",
            params![task_id, item_id],
        )?;
        if updated == 1 {
            Ok(())
        } else {
            Err(rusqlite::Error::QueryReturnedNoRows.into())
        }
    }

    pub fn advance_deletion_item_phase(
        &self,
        task_id: &str,
        item_id: i64,
        expected: &str,
        next: &str,
    ) -> Result<(), Error> {
        let updated = self.connection()?.execute(
            "UPDATE management_task_items SET mutation_phase=?4
             WHERE id=?2 AND task_id=?1 AND status='running' AND mutation_phase=?3",
            params![task_id, item_id, expected, next],
        )?;
        if updated == 1 {
            Ok(())
        } else {
            Err(rusqlite::Error::QueryReturnedNoRows.into())
        }
    }

    pub fn running_mutation_items(&self) -> Result<Vec<RunningMutationItem>, Error> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT i.id, t.task_type, t.media_root, i.source_path, i.quarantine_token,
                    i.intent, i.mutation_phase, i.identity_device, i.identity_inode
             FROM management_task_items i JOIN management_tasks t ON t.id=i.task_id
             WHERE t.kind='mutation' AND t.status IN ('queued','running') AND i.status='running'
             ORDER BY i.id",
        )?;
        let items = statement
            .query_map([], |row| {
                Ok(RunningMutationItem {
                    id: row.get(0)?,
                    task_type: row.get(1)?,
                    media_root: row.get(2)?,
                    source_path: row.get(3)?,
                    quarantine_token: row.get(4)?,
                    intent: row.get(5)?,
                    mutation_phase: row.get(6)?,
                    identity_device: parse_u64_column(row, 7)?,
                    identity_inode: parse_u64_column(row, 8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(items)
    }

    pub fn interrupt_item(&self, item_id: i64, message: &str) -> Result<(), Error> {
        self.connection()?.execute(
            "UPDATE management_task_items SET status='interrupted', message=?2 WHERE id=?1 AND status='running'",
            params![item_id, message],
        )?;
        Ok(())
    }

    pub fn recover_deletion_item(
        &self,
        item_id: i64,
        status: &str,
        message: &str,
    ) -> Result<(), Error> {
        let updated = self.connection()?.execute(
            "UPDATE management_task_items
             SET status=?2, message=?3, mutation_phase='recovered'
             WHERE id=?1 AND status='running' AND intent='permanent_delete'",
            params![item_id, status, message],
        )?;
        if updated == 1 {
            Ok(())
        } else {
            Err(rusqlite::Error::QueryReturnedNoRows.into())
        }
    }

    pub fn complete_item(
        &self,
        task_id: &str,
        item_id: i64,
        status: &str,
        message: Option<&str>,
    ) -> Result<(), Error> {
        let updated = self.connection()?.execute(
            "UPDATE management_task_items
             SET status=?3, message=?4,
                 mutation_phase=CASE WHEN mutation_phase IS NULL THEN NULL ELSE 'finished' END
             WHERE id=?2 AND task_id=?1 AND status='running'",
            params![task_id, item_id, status, message],
        )?;
        if updated == 1 {
            Ok(())
        } else {
            Err(rusqlite::Error::QueryReturnedNoRows.into())
        }
    }

    pub fn save_operation_plan(
        &self,
        task_id: &str,
        expires_at: u64,
        plan_json: &str,
    ) -> Result<(), Error> {
        let action_count = serde_json::from_str::<serde_json::Value>(plan_json)
            .ok()
            .and_then(|plan| {
                plan["actions"]
                    .as_array()
                    .map(|actions| actions.len() as u64)
            });
        self.connection()?.execute(
            "UPDATE management_tasks SET plan_expires_at=?2, operation_plan=?3, planned_item_count=?4 WHERE id=?1",
            params![task_id, expires_at, plan_json, action_count],
        )?;
        Ok(())
    }

    pub fn save_report(&self, task_id: &str, report_json: &str) -> Result<(), Error> {
        self.connection()?.execute(
            "UPDATE management_tasks SET report=?2 WHERE id=?1",
            params![task_id, report_json],
        )?;
        Ok(())
    }

    pub fn consume_plan_and_create_mutation(
        &self,
        plan_id: &str,
        now: u64,
    ) -> Result<Option<(ManagementTask, ManagementTask)>, Error> {
        let mutation_id = task_id();
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let consumed = transaction.execute(
            "UPDATE management_tasks SET plan_consumed_at=?2
             WHERE id=?1 AND kind='preview' AND status='completed'
               AND operation_plan IS NOT NULL AND plan_expires_at>=?2
               AND plan_consumed_at IS NULL",
            params![plan_id, now],
        )?;
        if consumed == 0 {
            transaction.rollback()?;
            return Ok(None);
        }
        transaction.execute(
            "INSERT INTO management_tasks
               (id, task_type, media_root, kind, status, created_at, source_plan_id, planned_item_count)
             SELECT ?1, task_type, media_root, 'mutation', 'queued', ?3, id, planned_item_count
             FROM management_tasks WHERE id=?2",
            params![mutation_id, plan_id, now],
        )?;
        transaction.commit()?;
        drop(connection);
        let plan = self.get(plan_id)?.expect("consumed plan must exist");
        let mutation = self
            .get(&mutation_id)?
            .expect("bound mutation task must exist");
        Ok(Some((plan, mutation)))
    }

    pub fn get(&self, id: &str) -> Result<Option<ManagementTask>, Error> {
        let connection = self.connection()?;
        let mut task = connection.query_row(
            "SELECT id, task_type, media_root, kind, status, created_at, started_at, finished_at, error, plan_expires_at, operation_plan, report, source_plan_id, plan_consumed_at, planned_item_count FROM management_tasks WHERE id=?1",
            [id], task_from_row,
        ).optional()?;
        if let Some(task) = task.as_mut() {
            task.items = items(&connection, id)?;
        }
        Ok(task)
    }

    pub fn list(&self) -> Result<Vec<ManagementTask>, Error> {
        self.list_page(None, 0)
    }

    pub fn count(&self) -> Result<usize, Error> {
        Ok(self
            .connection()?
            .query_row("SELECT COUNT(*) FROM management_tasks", [], |row| {
                row.get::<_, i64>(0)
            })? as usize)
    }

    pub fn list_page(
        &self,
        limit: Option<usize>,
        offset: usize,
    ) -> Result<Vec<ManagementTask>, Error> {
        let connection = self.connection()?;
        let base = "SELECT id, task_type, media_root, kind, status, created_at, started_at, finished_at, error, plan_expires_at, operation_plan, report, source_plan_id, plan_consumed_at, planned_item_count FROM management_tasks ORDER BY created_at DESC, id DESC";
        let mut tasks = if let Some(limit) = limit {
            let mut statement = connection.prepare(&format!("{base} LIMIT ?1 OFFSET ?2"))?;
            let tasks = statement
                .query_map(params![limit as i64, offset as i64], task_from_row)?
                .collect::<Result<Vec<_>, _>>()?;
            tasks
        } else {
            let mut statement = connection.prepare(base)?;
            let tasks = statement
                .query_map([], task_from_row)?
                .collect::<Result<Vec<_>, _>>()?;
            tasks
        };
        hydrate_task_items(&connection, &mut tasks)?;
        Ok(tasks)
    }

    pub fn list_active(&self) -> Result<Vec<ManagementTask>, Error> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, task_type, media_root, kind, status, created_at, started_at, finished_at, error, plan_expires_at, operation_plan, report, source_plan_id, plan_consumed_at, planned_item_count FROM management_tasks WHERE status IN ('queued','running') ORDER BY created_at DESC, id DESC",
        )?;
        let mut tasks = statement
            .query_map([], task_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        hydrate_task_items(&connection, &mut tasks)?;
        Ok(tasks)
    }

    pub fn interrupt_running_destructive(&self, now: u64) -> Result<usize, Error> {
        Ok(self.connection()?.execute(
            "UPDATE management_tasks SET status='interrupted', finished_at=?1, error='service restarted during destructive task' WHERE status IN ('queued','running') AND kind='mutation'",
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

fn hydrate_task_items(
    connection: &Connection,
    tasks: &mut [ManagementTask],
) -> rusqlite::Result<()> {
    let mut grouped = HashMap::<String, Vec<TaskItem>>::new();
    for task_chunk in tasks.chunks(500) {
        let placeholders = vec!["?"; task_chunk.len()].join(",");
        if placeholders.is_empty() {
            continue;
        }
        let mut statement = connection.prepare(&format!(
            "SELECT id, task_id, kind, path, status, message, source_path, quarantine_token,
                    intent, mutation_phase, identity_device, identity_inode
             FROM management_task_items WHERE task_id IN ({placeholders}) ORDER BY id"
        ))?;
        let ids = task_chunk
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>();
        let rows = statement.query_map(rusqlite::params_from_iter(ids), |row| {
            Ok((
                row.get::<_, String>(1)?,
                TaskItem {
                    id: row.get(0)?,
                    kind: row.get(2)?,
                    path: row.get(3)?,
                    status: row.get(4)?,
                    message: row.get(5)?,
                    source_path: row.get(6)?,
                    quarantine_token: row.get(7)?,
                    intent: row.get(8)?,
                    mutation_phase: row.get(9)?,
                    identity_device: parse_u64_column(row, 10)?,
                    identity_inode: parse_u64_column(row, 11)?,
                },
            ))
        })?;
        for row in rows {
            let (task_id, item) = row?;
            grouped.entry(task_id).or_default().push(item);
        }
    }
    for task in tasks {
        task.items = grouped.remove(&task.id).unwrap_or_default();
    }
    Ok(())
}

fn parse_u64_column(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<String>>(index)?
        .map_or(Ok(None), |value| {
            value.parse::<u64>().map(Some).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    index,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })
        })
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
        plan_expires_at: row.get(9)?,
        operation_plan: row
            .get::<_, Option<String>>(10)?
            .and_then(|value| serde_json::from_str(&value).ok()),
        report: row
            .get::<_, Option<String>>(11)?
            .and_then(|value| serde_json::from_str(&value).ok()),
        source_plan_id: row.get(12)?,
        plan_consumed_at: row.get(13)?,
        planned_item_count: row.get(14)?,
        items: Vec::new(),
    })
}

fn items(connection: &Connection, task_id: &str) -> rusqlite::Result<Vec<TaskItem>> {
    let mut statement = connection.prepare(
        "SELECT id, kind, path, status, message, source_path, quarantine_token,
                intent, mutation_phase, identity_device, identity_inode
         FROM management_task_items WHERE task_id=?1 ORDER BY id",
    )?;
    let rows = statement.query_map([task_id], |row| {
        Ok(TaskItem {
            id: row.get(0)?,
            kind: row.get(1)?,
            path: row.get(2)?,
            status: row.get(3)?,
            message: row.get(4)?,
            source_path: row.get(5)?,
            quarantine_token: row.get(6)?,
            intent: row.get(7)?,
            mutation_phase: row.get(8)?,
            identity_device: parse_u64_column(row, 9)?,
            identity_inode: parse_u64_column(row, 10)?,
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
