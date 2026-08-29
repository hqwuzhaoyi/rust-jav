use std::{fs, os::unix::fs::MetadataExt, path::Path};

use rust_jav::asset_index::{AssetIndex, AssetQuery, AssetState, ScanMode};

fn media(path: &Path) {
    fs::write(path, b"video").unwrap();
}

#[test]
fn full_reconciliation_tracks_filesystem_and_preserves_identity_across_rename() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("ABC-123.mp4"));
    fs::write(
        root.join("ABC-123.nfo"),
        "<movie><title>First title</title></movie>",
    )
    .unwrap();
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();

    index
        .reconcile(&[root.clone()], ScanMode::Startup, 100)
        .unwrap();
    let original = index.search(AssetQuery::default()).unwrap().items.remove(0);
    fs::rename(root.join("ABC-123.mp4"), root.join("RENAMED.mp4")).unwrap();
    fs::rename(root.join("ABC-123.nfo"), root.join("RENAMED.nfo")).unwrap();
    index
        .reconcile(&[root.clone()], ScanMode::Manual, 101)
        .unwrap();
    let renamed = index.search(AssetQuery::default()).unwrap().items.remove(0);

    assert_eq!(renamed.id, original.id);
    assert!(renamed.path.ends_with("RENAMED.mp4"));
    fs::remove_file(root.join("RENAMED.mp4")).unwrap();
    index.reconcile(&[root], ScanMode::Manual, 102).unwrap();
    assert!(index
        .search(AssetQuery::default())
        .unwrap()
        .items
        .is_empty());
}

#[test]
fn stable_identity_survives_replaced_inode_at_the_observed_path() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    let path = root.join("KEEP-123.mp4");
    media(&path);
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
    index
        .reconcile(&[root.clone()], ScanMode::Startup, 100)
        .unwrap();
    let before = index.search(AssetQuery::default()).unwrap().items.remove(0);
    fs::remove_file(&path).unwrap();
    media(&path);
    index.reconcile(&[root], ScanMode::Manual, 101).unwrap();
    let after = index.search(AssetQuery::default()).unwrap().items.remove(0);
    assert_eq!(after.id, before.id);
    assert_ne!(after.inode, before.inode);
}

#[test]
fn full_rebuild_drops_assets_from_media_roots_no_longer_configured() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("OLD-100.mp4"));
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();
    index.reconcile(&[], ScanMode::Manual, 101).unwrap();
    assert_eq!(index.search(AssetQuery::default()).unwrap().total, 0);
}

#[test]
fn incremental_reconciliation_updates_only_observed_paths() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("ONE-001.mkv"));
    media(&root.join("TWO-002.mp4"));
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
    index
        .reconcile(&[root.clone()], ScanMode::Startup, 100)
        .unwrap();
    fs::remove_file(root.join("ONE-001.mkv")).unwrap();

    index
        .reconcile_paths(&root, &[root.join("ONE-001.mkv")], 101)
        .unwrap();
    let result = index.search(AssetQuery::default()).unwrap();
    assert_eq!(result.total, 1);
    assert!(result.items[0].path.ends_with("TWO-002.mp4"));
}

#[test]
fn search_filters_state_and_code_and_paginates_with_date_groups() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("ABC-123.mp4"));
    fs::write(
        root.join("ABC-123.nfo"),
        "<movie><title>Blue Room</title></movie>",
    )
    .unwrap();
    media(&root.join("XYZ-999.mp4"));
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
    index
        .reconcile(&[root], ScanMode::Startup, 1_700_000_000)
        .unwrap();

    let result = index
        .search(AssetQuery {
            query: Some("blue".into()),
            state: Some(AssetState::Normal),
            page: 1,
            per_page: 1,
        })
        .unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.items[0].jav_code.as_deref(), Some("ABC-123"));
    assert!(!result.groups.is_empty());
    assert_eq!(
        index
            .search(AssetQuery {
                state: Some(AssetState::Exception),
                ..Default::default()
            })
            .unwrap()
            .total,
        1
    );
}

#[test]
fn permission_report_contains_host_identity_and_actionable_failure() {
    let fixture = tempfile::tempdir().unwrap();
    let missing = fixture.path().join("missing-host-path");
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
    let report = index.root_health(&missing);
    assert!(!report.readable);
    assert!(!report.writable);
    assert_eq!(report.uid, unsafe { libc::geteuid() });
    assert_eq!(report.gid, unsafe { libc::getegid() });
    assert!(report.action.as_deref().unwrap().contains("TrueNAS"));

    let present = index.root_health(fixture.path());
    assert_eq!(
        present.owner_uid,
        Some(fs::metadata(fixture.path()).unwrap().uid())
    );
}

#[test]
fn artwork_resolution_accepts_only_indexed_artwork_and_never_video() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("ABC-123.mp4"));
    fs::write(root.join("ABC-123.jpg"), b"jpeg").unwrap();
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();
    let asset = index.search(AssetQuery::default()).unwrap().items.remove(0);

    assert_eq!(
        index.indexed_artwork(&asset.id).unwrap(),
        Some(fixture.path().join("media/ABC-123.jpg"))
    );
    assert_eq!(index.indexed_artwork("../../etc/passwd").unwrap(), None);
    assert_ne!(
        index.indexed_artwork(&asset.id).unwrap(),
        Some(fixture.path().join("media/ABC-123.mp4"))
    );
}
