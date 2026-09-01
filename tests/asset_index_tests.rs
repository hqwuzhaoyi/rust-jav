use std::{
    fs,
    os::unix::fs::{symlink, MetadataExt},
    path::Path,
};

use rust_jav::asset_index::{ArtworkStatus, AssetIndex, AssetQuery, AssetState, ScanMode};

#[path = "support/artwork_fixtures.rs"]
mod artwork_fixtures;

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
fn identity_lookup_matches_only_device_inode_and_deduplicates_assets() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    let first_path = root.join("ABC-123.mp4");
    let second_path = root.join("XYZ-999.mp4");
    media(&first_path);
    media(&second_path);
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();
    let assets = index.search(AssetQuery::default()).unwrap().items;
    let first = assets
        .iter()
        .find(|asset| asset.path == first_path.display().to_string())
        .unwrap();
    let second = assets
        .iter()
        .find(|asset| asset.path == second_path.display().to_string())
        .unwrap();

    let linked = index
        .assets_by_identities(&[
            (first.device, first.inode),
            (first.device, first.inode),
            (first.device + 1, first.inode),
        ])
        .unwrap();

    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].id, first.id);
    assert_ne!(linked[0].id, second.id);
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
fn duplicate_jav_codes_remain_distinct_media_assets() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("DUP-100-A.mp4"));
    media(&root.join("DUP-100-B.mp4"));
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();

    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();
    let page = index.search(AssetQuery::default()).unwrap();

    assert_eq!(page.total, 2);
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].jav_code.as_deref(), Some("DUP-100"));
    assert_eq!(page.items[1].jav_code.as_deref(), Some("DUP-100"));
    assert_ne!(page.items[0].id, page.items[1].id);
}

#[test]
fn search_treats_sql_like_wildcards_as_literal_text() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    let percent_dir = root.join("PCT-100");
    let plain_dir = root.join("PLAIN-200");
    fs::create_dir(&percent_dir).unwrap();
    fs::create_dir(&plain_dir).unwrap();
    media(&percent_dir.join("PCT-100.mp4"));
    fs::write(
        percent_dir.join("PCT-100.nfo"),
        "<movie><title>100%_literal</title></movie>",
    )
    .unwrap();
    media(&plain_dir.join("PLAIN-200.mp4"));
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();

    let percent = index
        .search(AssetQuery {
            query: Some("%".into()),
            ..Default::default()
        })
        .unwrap();
    let wildcard_pair = index
        .search(AssetQuery {
            query: Some("%_literal".into()),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(percent.total, 1);
    assert_eq!(wildcard_pair.total, 1);
    assert_eq!(percent.items[0].id, wildcard_pair.items[0].id);
}

#[test]
fn page_numbers_are_clamped_without_offset_overflow() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("ONE-001.mp4"));
    media(&root.join("TWO-002.mp4"));
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();

    let page = index
        .search(AssetQuery {
            page: usize::MAX,
            per_page: 1,
            ..Default::default()
        })
        .unwrap();

    assert_eq!(page.page, 2);
    assert_eq!(page.total_pages, 2);
    assert_eq!(page.items.len(), 1);
}

#[test]
fn indexed_artwork_rejects_a_symlink_replaced_after_reconciliation() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("ART-101.mp4"));
    let artwork = root.join("ART-101.jpg");
    fs::write(&artwork, artwork_fixtures::valid_jpeg()).unwrap();
    let outside = fixture.path().join("outside.jpg");
    fs::write(&outside, b"outside secret").unwrap();
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();
    let id = index.search(AssetQuery::default()).unwrap().items[0]
        .id
        .clone();

    fs::remove_file(&artwork).unwrap();
    symlink(&outside, &artwork).unwrap();

    assert_eq!(index.indexed_artwork(&id).unwrap(), None);
}

#[test]
fn asset_detail_rejects_nfo_replaced_by_an_external_symlink() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("NFO-101.mp4"));
    let nfo = root.join("NFO-101.nfo");
    fs::write(&nfo, "<movie><title>Safe title</title></movie>").unwrap();
    let outside = fixture.path().join("outside.nfo");
    fs::write(&outside, "<movie><title>Outside secret</title></movie>").unwrap();
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();
    let id = index.search(AssetQuery::default()).unwrap().items[0]
        .id
        .clone();

    fs::remove_file(&nfo).unwrap();
    symlink(&outside, &nfo).unwrap();
    let detail = index.detail(&id).unwrap().unwrap();

    assert_eq!(detail.parse_status, "invalid");
    assert_ne!(detail.title.as_deref(), Some("Outside secret"));
    assert!(detail
        .exception
        .as_deref()
        .is_some_and(|message| !message.is_empty()));
}

#[test]
fn artwork_resolution_accepts_only_indexed_artwork_and_never_video() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("ABC-123.mp4"));
    fs::write(root.join("ABC-123.jpg"), artwork_fixtures::valid_jpeg()).unwrap();
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

#[test]
fn asset_index_exposes_only_valid_jpeg_png_and_webp_artwork() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    let artwork = artwork_fixtures::write_artwork_fixtures(&root);
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();

    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();
    let assets = index.search(AssetQuery::default()).unwrap().items;
    let observed = artwork
        .iter()
        .map(|expected| {
            let asset = assets
                .iter()
                .find(|asset| asset.jav_code.as_deref() == Some(expected.jav_code))
                .unwrap();
            (
                expected.jav_code,
                asset.artwork_url.is_some(),
                index
                    .detail(&asset.id)
                    .unwrap()
                    .unwrap()
                    .artwork_url
                    .is_some(),
                index.indexed_artwork(&asset.id).unwrap().is_some(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        observed,
        vec![
            ("JPG-101", true, true, true),
            ("PNG-102", true, true, true),
            ("WEBP-103", true, true, true),
            ("ZERO-104", false, false, false),
            ("TRUNC-105", false, false, false),
            ("DASS-591", false, false, false),
        ]
    );
}

#[test]
fn artwork_candidates_skip_invalid_higher_priority_files_and_include_conventional_webp_names() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    let sibling = root.join("sibling");
    let conventional = root.join("conventional");
    let folder_webp = root.join("folder-webp");
    let cover_webp = root.join("cover-webp");
    fs::create_dir_all(&sibling).unwrap();
    fs::create_dir_all(&conventional).unwrap();
    fs::create_dir_all(&folder_webp).unwrap();
    fs::create_dir_all(&cover_webp).unwrap();

    media(&sibling.join("FALL-201.mp4"));
    fs::write(sibling.join("FALL-201.jpg"), b"not a jpeg").unwrap();
    fs::write(sibling.join("FALL-201.png"), artwork_fixtures::valid_png()).unwrap();

    media(&conventional.join("FALL-202.mp4"));
    fs::write(conventional.join("folder.jpg"), b"not a jpeg").unwrap();
    fs::write(
        conventional.join("poster.webp"),
        artwork_fixtures::valid_webp(),
    )
    .unwrap();

    media(&folder_webp.join("FALL-203.mp4"));
    fs::write(
        folder_webp.join("folder.webp"),
        artwork_fixtures::valid_webp(),
    )
    .unwrap();

    media(&cover_webp.join("FALL-204.mp4"));
    fs::write(
        cover_webp.join("cover.webp"),
        artwork_fixtures::valid_webp(),
    )
    .unwrap();

    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();
    let assets = index.search(AssetQuery::default()).unwrap().items;

    for (code, suffix, content_type) in [
        ("FALL-201", "FALL-201.png", "image/png"),
        ("FALL-202", "poster.webp", "image/webp"),
        ("FALL-203", "folder.webp", "image/webp"),
        ("FALL-204", "cover.webp", "image/webp"),
    ] {
        let asset = assets
            .iter()
            .find(|asset| asset.jav_code.as_deref() == Some(code))
            .unwrap();
        let detail = index.detail(&asset.id).unwrap().unwrap();
        assert!(asset.artwork_url.is_some());
        assert!(detail
            .artwork
            .source_path
            .as_deref()
            .unwrap()
            .ends_with(suffix));
        assert_eq!(detail.artwork.content_type.as_deref(), Some(content_type));
    }
}

#[cfg(unix)]
#[test]
fn artwork_fifo_is_rejected_as_unreadable_without_blocking_reconciliation() {
    use std::{ffi::CString, os::unix::ffi::OsStrExt, time::Duration};

    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("FIFO-203.mp4"));
    let fifo = root.join("FIFO-203.jpg");
    let fifo_name = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo_name.as_ptr(), 0o600) }, 0);
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();

    let started = std::time::Instant::now();
    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();
    assert!(started.elapsed() < Duration::from_secs(1));

    let asset = index.search(AssetQuery::default()).unwrap().items.remove(0);
    let detail = index.detail(&asset.id).unwrap().unwrap();
    assert!(asset.artwork_url.is_none());
    assert_eq!(detail.artwork.status, ArtworkStatus::Unreadable);
    assert!(detail
        .artwork
        .error
        .unwrap()
        .contains("ordinary regular file"));
}

#[cfg(unix)]
#[test]
fn reconciliation_rejects_a_symlink_media_root_without_indexing_external_assets() {
    let fixture = tempfile::tempdir().unwrap();
    let outside = fixture.path().join("outside");
    let root = fixture.path().join("media");
    fs::create_dir(&outside).unwrap();
    media(&outside.join("ESCAPE-204.mp4"));
    symlink(&outside, &root).unwrap();
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();

    assert!(index.reconcile(&[root], ScanMode::Startup, 100).is_err());
    assert!(index
        .search(AssetQuery::default())
        .unwrap()
        .items
        .is_empty());
}

#[cfg(unix)]
#[test]
fn recursive_reconciliation_never_follows_a_nested_directory_symlink() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    let outside = fixture.path().join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    media(&root.join("SAFE-205.mp4"));
    media(&outside.join("ESCAPE-206.mp4"));
    symlink(&outside, root.join("escape")).unwrap();
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();

    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();
    let assets = index.search(AssetQuery::default()).unwrap().items;

    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].jav_code.as_deref(), Some("SAFE-205"));
}

#[test]
fn artwork_larger_than_the_encoded_size_limit_is_rejected_before_decoding() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("HUGE-207.mp4"));
    let artwork = fs::File::create(root.join("HUGE-207.jpg")).unwrap();
    artwork.set_len(32 * 1024 * 1024 + 1).unwrap();
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();

    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();
    let asset = index.search(AssetQuery::default()).unwrap().items.remove(0);
    let detail = index.detail(&asset.id).unwrap().unwrap();

    assert!(asset.artwork_url.is_none());
    assert_eq!(detail.artwork.status, ArtworkStatus::TooLarge);
}

#[test]
fn webp_preflight_rejects_excessive_pixels_output_and_multiple_frames() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    for (code, bytes) in [
        (
            "PIXEL-208",
            artwork_fixtures::oversized_lossy_webp(4_001, 4_000),
        ),
        (
            "OUTPUT-209",
            artwork_fixtures::oversized_alpha_webp(4_000, 4_000),
        ),
        ("ANIM-210", artwork_fixtures::animated_webp()),
    ] {
        media(&root.join(format!("{code}.mp4")));
        fs::write(root.join(format!("{code}.webp")), bytes).unwrap();
    }
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();

    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();
    let assets = index.search(AssetQuery::default()).unwrap().items;
    let observed = ["PIXEL-208", "OUTPUT-209", "ANIM-210"].map(|code| {
        let asset = assets
            .iter()
            .find(|asset| asset.jav_code.as_deref() == Some(code))
            .unwrap();
        let artwork = index.detail(&asset.id).unwrap().unwrap().artwork;
        (
            code,
            asset.artwork_url.is_some(),
            artwork.status,
            artwork.error.unwrap(),
        )
    });

    assert_eq!(
        observed.map(|(code, has_url, status, error)| (
            code,
            has_url,
            status,
            error.contains("pixel")
                || error.contains("decoded output")
                || error.contains("animated")
        )),
        [
            ("PIXEL-208", false, ArtworkStatus::TooLarge, true),
            ("OUTPUT-209", false, ArtworkStatus::TooLarge, true),
            ("ANIM-210", false, ArtworkStatus::Animated, true),
        ]
    );
}

#[test]
fn concurrent_static_webp_reconciliations_complete_through_the_decode_gate() {
    use std::sync::{Arc, Barrier};

    let workers = 8;
    let barrier = Arc::new(Barrier::new(workers));
    let handles = (0..workers)
        .map(|worker| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let fixture = tempfile::tempdir().unwrap();
                let root = fixture.path().join("media");
                fs::create_dir(&root).unwrap();
                let code = format!("GATE-{}", 300 + worker);
                media(&root.join(format!("{code}.mp4")));
                fs::write(
                    root.join(format!("{code}.webp")),
                    artwork_fixtures::valid_webp(),
                )
                .unwrap();
                let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
                barrier.wait();
                index.reconcile(&[root], ScanMode::Startup, 100).unwrap();
                assert!(index.search(AssetQuery::default()).unwrap().items[0]
                    .artwork_url
                    .is_some());
            })
        })
        .collect::<Vec<_>>();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn asset_detail_parses_complete_nfo_metadata_and_actors() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("ABC-123.mp4"));
    fs::write(root.join("ABC-123.jpg"), artwork_fixtures::valid_jpeg()).unwrap();
    fs::write(
        root.join("ABC-123.nfo"),
        r#"<?xml version="1.0"?><movie>
          <title>Blue &amp; Quiet</title><studio>Example Studio</studio>
          <premiered>2026-08-24</premiered><runtime>142</runtime>
          <director>K. Mori</director><genre>Drama</genre><tag>4K</tag>
          <plot>A local story.</plot>
          <actor><name>miru</name></actor><actor><name>Mao Hamasaki</name></actor>
        </movie>"#,
    )
    .unwrap();
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();
    let asset = index.search(AssetQuery::default()).unwrap().items.remove(0);

    let detail = index.detail(&asset.id).unwrap().unwrap();
    assert_eq!(detail.title.as_deref(), Some("Blue & Quiet"));
    assert_eq!(detail.studio.as_deref(), Some("Example Studio"));
    assert_eq!(detail.release_date.as_deref(), Some("2026-08-24"));
    assert_eq!(detail.runtime_minutes, Some(142));
    assert_eq!(detail.director.as_deref(), Some("K. Mori"));
    assert_eq!(detail.tags, vec!["Drama", "4K"]);
    assert_eq!(detail.plot.as_deref(), Some("A local story."));
    assert_eq!(detail.parse_status, "valid");
    assert!(detail
        .source_path
        .as_deref()
        .unwrap()
        .ends_with("ABC-123.nfo"));
    assert_eq!(
        detail
            .actors
            .iter()
            .map(|actor| actor.name.as_str())
            .collect::<Vec<_>>(),
        vec!["miru", "Mao Hamasaki"]
    );
    assert!(detail.actors.iter().all(|actor| actor.poster_url.is_none()));
    assert!(detail
        .actors
        .iter()
        .all(|actor| actor.actor_folder_url.is_none()));
}

#[test]
fn asset_index_recognizes_movie_nfo_and_folder_artwork_conventions() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    let movie = root.join("ABC-123");
    fs::create_dir_all(&movie).unwrap();
    media(&movie.join("ABC-123.mp4"));
    fs::write(
        movie.join("movie.nfo"),
        "<movie><title>Conventional Layout</title><actor><name>Alice</name></actor></movie>",
    )
    .unwrap();
    fs::write(movie.join("folder.jpg"), artwork_fixtures::valid_jpeg()).unwrap();
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();

    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();
    let asset = index.search(AssetQuery::default()).unwrap().items.remove(0);

    assert_eq!(asset.state, AssetState::Normal);
    assert!(asset.nfo_path.as_deref().unwrap().ends_with("movie.nfo"));
    assert!(asset.artwork_url.is_some());
    assert_eq!(
        index.detail(&asset.id).unwrap().unwrap().title.as_deref(),
        Some("Conventional Layout")
    );
}

#[test]
fn missing_and_invalid_nfo_are_actionable_asset_exceptions() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("MISS-100.mp4"));
    media(&root.join("BAD-200.mp4"));
    fs::write(root.join("BAD-200.nfo"), "<movie><title>Broken").unwrap();
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();

    let assets = index.search(AssetQuery::default()).unwrap().items;
    let missing = assets
        .iter()
        .find(|asset| asset.jav_code.as_deref() == Some("MISS-100"))
        .unwrap();
    assert_eq!(missing.state, AssetState::Exception);
    assert!(missing
        .exception
        .as_deref()
        .unwrap()
        .contains("Add a sibling .nfo"));
    assert_eq!(
        index.detail(&missing.id).unwrap().unwrap().parse_status,
        "missing"
    );
    let invalid = assets
        .iter()
        .find(|asset| asset.jav_code.as_deref() == Some("BAD-200"))
        .unwrap();
    assert_eq!(invalid.state, AssetState::Exception);
    assert!(invalid
        .exception
        .as_deref()
        .unwrap()
        .contains("Fix invalid NFO"));
    assert_eq!(
        index.detail(&invalid.id).unwrap().unwrap().parse_status,
        "invalid"
    );
}

#[test]
fn empty_nfo_is_reported_as_empty_with_a_regeneration_action() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("MIDV-821-C.mp4"));
    fs::write(root.join("movie.nfo"), b"").unwrap();
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();
    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();

    let asset = index.search(AssetQuery::default()).unwrap().items.remove(0);
    let detail = index.detail(&asset.id).unwrap().unwrap();

    assert_eq!(detail.parse_status, "empty");
    assert!(detail.exception.unwrap().contains("empty"));
    assert!(asset.exception.unwrap().contains("Regenerate"));
}

#[test]
fn m2ts_files_are_indexed_as_media_assets() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    fs::create_dir(&root).unwrap();
    media(&root.join("DISC-A.m2ts"));
    fs::write(
        root.join("DISC-A.nfo"),
        "<movie><title>Disc A</title></movie>",
    )
    .unwrap();
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();

    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();

    let page = index.search(AssetQuery::default()).unwrap();
    assert_eq!(page.total, 1);
    assert!(page.items[0].path.ends_with("DISC-A.m2ts"));
}

#[test]
fn shared_movie_nfo_multipart_sets_are_one_media_asset() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("media");
    for (folder, files) in [
        ("OFJE-550", vec!["OFJE-550-1.mp4", "OFJE-550-2.mp4"]),
        ("OFJE-334", vec!["OFJE-334-A.mp4", "OFJE-334-B.mp4"]),
        ("MIDV-821-C", vec!["MIDV-821-C.mp4"]),
    ] {
        let directory = root.join(folder);
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("movie.nfo"),
            format!("<movie><title>{folder}</title></movie>"),
        )
        .unwrap();
        for file in files {
            media(&directory.join(file));
        }
    }
    let index = AssetIndex::open(&fixture.path().join("index.sqlite3")).unwrap();

    index.reconcile(&[root], ScanMode::Startup, 100).unwrap();

    let page = index.search(AssetQuery::default()).unwrap();
    assert_eq!(page.total, 3);
    assert!(page
        .items
        .iter()
        .any(|asset| asset.path.ends_with("OFJE-550-1.mp4")));
    assert!(page
        .items
        .iter()
        .any(|asset| asset.path.ends_with("OFJE-334-A.mp4")));
    assert!(page
        .items
        .iter()
        .any(|asset| asset.path.ends_with("MIDV-821-C.mp4")));
    assert!(!page
        .items
        .iter()
        .any(|asset| asset.path.ends_with("OFJE-550-2.mp4")));
    assert!(!page
        .items
        .iter()
        .any(|asset| asset.path.ends_with("OFJE-334-B.mp4")));
}
