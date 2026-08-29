use std::{sync::Arc, time::Duration};

use rust_jav::management_tasks::{NewTask, TaskCoordinator, TaskKind, TaskStatus, TaskStore};
use tokio::sync::{mpsc, Barrier};

#[test]
fn sqlite_persists_task_identity_scope_timestamps_and_item_outcomes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("management.sqlite3");
    let store = TaskStore::open(&path).unwrap();
    let task = store
        .create(NewTask::preview("operations", "/media/a"), 100)
        .unwrap();
    store.mark_running(&task.id, 101).unwrap();
    store
        .finish_item(&task.id, "rename", Some("/media/a/A.mp4"), "planned", None)
        .unwrap();
    store.mark_completed(&task.id, 102).unwrap();
    drop(store);

    let reopened = TaskStore::open(&path).unwrap();
    let task = reopened.get(&task.id).unwrap().unwrap();
    assert_eq!(task.task_type, "operations");
    assert_eq!(task.media_root, "/media/a");
    assert_eq!(task.kind, TaskKind::Preview);
    assert_eq!(task.status, TaskStatus::Completed);
    assert_eq!(
        (task.created_at, task.started_at, task.finished_at),
        (100, Some(101), Some(102))
    );
    assert_eq!(task.items.len(), 1);
    assert_eq!(task.items[0].status, "planned");
}

#[test]
fn restart_marks_running_destructive_tasks_interrupted_without_requeueing_them() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("management.sqlite3");
    let store = TaskStore::open(&path).unwrap();
    let destructive = store
        .create(NewTask::mutation("operations", "/media/a"), 100)
        .unwrap();
    store.mark_running(&destructive.id, 101).unwrap();
    drop(store);

    let reopened = TaskStore::open(&path).unwrap();
    assert_eq!(reopened.interrupt_running_destructive(200).unwrap(), 1);
    let task = reopened.get(&destructive.id).unwrap().unwrap();
    assert_eq!(task.status, TaskStatus::Interrupted);
    assert_eq!(task.finished_at, Some(200));
    assert!(reopened.runnable_tasks().unwrap().is_empty());
}

#[tokio::test]
async fn mutations_serialize_per_media_root_but_independent_roots_overlap() {
    let coordinator = TaskCoordinator::new();
    let first_entered = Arc::new(Barrier::new(2));
    let release_first = Arc::new(Barrier::new(2));
    let (entered_tx, mut entered_rx) = mpsc::unbounded_channel();

    let first = {
        let coordinator = coordinator.clone();
        let entered = first_entered.clone();
        let release = release_first.clone();
        tokio::spawn(async move {
            let _lease = coordinator.mutation("/media/a").await;
            entered.wait().await;
            release.wait().await;
        })
    };
    first_entered.wait().await;

    let same_root = {
        let coordinator = coordinator.clone();
        let tx = entered_tx.clone();
        tokio::spawn(async move {
            let _lease = coordinator.mutation("/media/a").await;
            tx.send("same").unwrap();
        })
    };
    let other_root = {
        let coordinator = coordinator.clone();
        tokio::spawn(async move {
            let _lease = coordinator.mutation("/media/b").await;
            entered_tx.send("other").unwrap();
        })
    };

    assert_eq!(entered_rx.recv().await.unwrap(), "other");
    assert!(
        tokio::time::timeout(Duration::from_millis(50), entered_rx.recv())
            .await
            .is_err()
    );
    release_first.wait().await;
    assert_eq!(entered_rx.recv().await.unwrap(), "same");
    first.await.unwrap();
    same_root.await.unwrap();
    other_root.await.unwrap();
}

#[tokio::test]
async fn previews_do_not_wait_for_a_media_root_mutation() {
    let coordinator = TaskCoordinator::new();
    let _lease = coordinator.mutation("/media/a").await;
    tokio::time::timeout(Duration::from_millis(50), coordinator.preview("/media/a"))
        .await
        .expect("preview should remain available while a mutation runs");
}
