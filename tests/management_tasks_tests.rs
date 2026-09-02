use std::{sync::Arc, time::Duration};

use rust_jav::management_tasks::{NewTask, TaskCoordinator, TaskKind, TaskStatus, TaskStore};

#[test]
fn operation_plan_and_final_report_survive_reopening_the_task_store() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasks.sqlite3");
    let store = TaskStore::open(&path).unwrap();
    let task = store
        .create(NewTask::preview("operations", "/media/a"), 100)
        .unwrap();

    store
        .save_operation_plan(
            &task.id,
            1_000,
            r#"{"operations":["delete_ad_files","standardize_names"],"actions":[{"path":"/media/a/ad.txt","destructive":true}]}"#,
        )
        .unwrap();
    store
        .save_report(
            &task.id,
            r#"{"summary":{"failed_actions":1},"verification":{"verification_status":"failed"}}"#,
        )
        .unwrap();
    drop(store);

    let task = TaskStore::open(&path)
        .unwrap()
        .get(&task.id)
        .unwrap()
        .unwrap();
    assert_eq!(task.plan_expires_at, Some(1_000));
    assert_eq!(
        task.operation_plan.unwrap()["actions"][0]["path"],
        "/media/a/ad.txt"
    );
    assert_eq!(task.report.unwrap()["summary"]["failed_actions"], 1);
}
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
    let queued = store
        .create(NewTask::mutation("remove_actor_folder", "/media/b"), 100)
        .unwrap();
    store.mark_running(&destructive.id, 101).unwrap();
    drop(store);

    let reopened = TaskStore::open(&path).unwrap();
    assert_eq!(reopened.interrupt_running_destructive(200).unwrap(), 2);
    let task = reopened.get(&destructive.id).unwrap().unwrap();
    assert_eq!(task.status, TaskStatus::Interrupted);
    assert_eq!(task.finished_at, Some(200));
    let queued = reopened.get(&queued.id).unwrap().unwrap();
    assert_eq!(queued.status, TaskStatus::Interrupted);
    assert_eq!(queued.finished_at, Some(200));
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

#[test]
fn permanent_deletion_intent_identity_and_token_are_durable_before_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("tasks.sqlite3");
    let store = TaskStore::open(&database).unwrap();
    assert!(store
        .create_deletion_mutation("/media", 99, &serde_json::json!({"id":"incomplete"}))
        .is_err());
    let task = store
        .create_deletion_mutation(
            "/media",
            100,
            &serde_json::json!({
                "id": "plan-1",
                "created_at": 100,
                "expires_at": 700,
                "selection": "selected",
                "hard_link_search_roots": ["/media"],
                "paths": [{"path": "/media/movie.mp4"}],
                "rule_set_version": 3,
                "rules": ["delete-*"]
            }),
        )
        .unwrap();
    store.mark_running(&task.id, 101).unwrap();

    let started = store
        .start_deletion_item(&task.id, "/media/movie.mp4", 17, 42)
        .unwrap();
    assert_eq!(
        started.quarantine_token,
        format!(".rust-jav-quarantine-item-{}", started.id)
    );
    drop(store);

    let reopened = TaskStore::open(&database).unwrap();
    let item = &reopened.get(&task.id).unwrap().unwrap().items[0];
    assert_eq!(item.status, "running");
    assert_eq!(item.mutation_phase.as_deref(), Some("intent"));
    assert_eq!(item.intent.as_deref(), Some("permanent_delete"));
    assert_eq!(item.identity_device, Some(17));
    assert_eq!(item.identity_inode, Some(42));
    assert_eq!(item.source_path.as_deref(), Some("/media/movie.mp4"));
    assert_eq!(item.quarantine_token, Some(started.quarantine_token));
}
