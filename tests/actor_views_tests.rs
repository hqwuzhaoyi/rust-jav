#![cfg(unix)]

use std::fs;

use rust_jav::{
    actor_links::execute_actor_links_command,
    actor_views::{
        actor_folder_detail, browse_actor_folders, remove_actor_folder,
        validate_hard_link_compatibility,
    },
};
use tempfile::tempdir;

#[test]
fn browses_actor_folders_with_counts_and_inode_aware_sizes() {
    let fixture = tempdir().unwrap();
    let media = fixture.path().join("media");
    let actors = fixture.path().join("actors");
    let movie = media.join("ABC-123");
    fs::create_dir_all(&movie).unwrap();
    fs::create_dir_all(actors.join("Alice/ABC-123")).unwrap();
    fs::write(movie.join("ABC-123.mp4"), b"123456").unwrap();
    fs::write(movie.join("ABC-123-poster.jpg"), b"poster").unwrap();
    fs::hard_link(
        movie.join("ABC-123.mp4"),
        actors.join("Alice/ABC-123/ABC-123.mp4"),
    )
    .unwrap();
    fs::hard_link(
        movie.join("ABC-123-poster.jpg"),
        actors.join("Alice/ABC-123/ABC-123-poster.jpg"),
    )
    .unwrap();

    let folders = browse_actor_folders(&actors).unwrap();
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].name, "Alice");
    assert_eq!(folders[0].movie_count, 1);
    assert_eq!(folders[0].hard_link_count, 2);
    assert_eq!(folders[0].unique_inode_count, 2);
    assert_eq!(folders[0].logical_size, 12);
    assert_eq!(folders[0].reclaimable_space, 0);
    assert!(folders[0].poster_path.is_none());
}

#[test]
fn actor_folder_counts_paths_but_deduplicates_inode_sizes() {
    let fixture = tempdir().unwrap();
    let actors = fixture.path().join("actors");
    let movie = actors.join("AIKA/MIAB-492-C");
    fs::create_dir_all(&movie).unwrap();
    fs::write(movie.join("MIAB-492-C.mp4"), b"123456").unwrap();
    fs::hard_link(
        movie.join("MIAB-492-C.mp4"),
        movie.join("MIAB-492-C-copy.mp4"),
    )
    .unwrap();

    let folder = browse_actor_folders(&actors).unwrap().remove(0);

    assert_eq!(folder.hard_link_count, 2, "derived regular-file paths");
    assert_eq!(folder.derived_file_count, 2);
    assert_eq!(folder.unique_inode_count, 1);
    assert_eq!(
        folder.logical_size, 6,
        "logical size counts each inode once"
    );
    assert_eq!(
        folder.reclaimable_space, 6,
        "reclaimable counts each inode once"
    );
}

#[test]
fn actor_folder_detail_exposes_distinct_regular_file_identities() {
    let fixture = tempdir().unwrap();
    let actors = fixture.path().join("actors");
    let movie = actors.join("Alice/ABC-123");
    fs::create_dir_all(&movie).unwrap();
    fs::write(movie.join("ABC-123.mp4"), b"movie").unwrap();
    fs::hard_link(movie.join("ABC-123.mp4"), movie.join("ABC-123-copy.mp4")).unwrap();
    fs::write(movie.join("ABC-123.jpg"), b"poster").unwrap();

    let detail = actor_folder_detail(&actors, "Alice").unwrap().unwrap();

    assert_eq!(detail.folder.name, "Alice");
    assert_eq!(detail.file_identities.len(), 2);
    assert!(detail
        .file_identities
        .iter()
        .all(|identity| identity.device > 0 && identity.inode > 0));
}

#[test]
fn actor_folder_detail_reuses_safe_path_validation_and_reports_missing() {
    let fixture = tempdir().unwrap();
    let actors = fixture.path().join("actors");
    fs::create_dir_all(&actors).unwrap();

    assert!(actor_folder_detail(&actors, "Missing").unwrap().is_none());
    assert_eq!(
        actor_folder_detail(&actors, "../escape")
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
}

#[test]
fn actor_folder_never_uses_a_nested_movie_cover_as_its_poster() {
    let fixture = tempdir().unwrap();
    let actors = fixture.path().join("actors");
    let movie = actors.join("Alice/ABC-123");
    fs::create_dir_all(&movie).unwrap();
    fs::write(movie.join("ABC-123.mp4"), b"movie").unwrap();
    fs::write(movie.join("folder.jpg"), b"poster").unwrap();

    let folders = browse_actor_folders(&actors).unwrap();

    assert!(folders[0].poster_path.is_none());
}

#[test]
fn removal_only_unlinks_actor_view_and_it_can_be_regenerated_from_nfo() {
    let fixture = tempdir().unwrap();
    let media = fixture.path().join("media");
    let actors = fixture.path().join("actors");
    let movie = media.join("ABC-123");
    fs::create_dir_all(&movie).unwrap();
    fs::create_dir_all(&actors).unwrap();
    fs::write(
        movie.join("ABC-123.nfo"),
        "<movie><actor><name>Alice</name></actor></movie>",
    )
    .unwrap();
    fs::write(movie.join("ABC-123.mp4"), b"movie").unwrap();
    fs::write(movie.join("ABC-123-poster.jpg"), b"poster").unwrap();
    assert_eq!(
        execute_actor_links_command(media.clone(), actors.clone(), true)
            .unwrap()
            .summary
            .failed_actions,
        0
    );

    let outcome = remove_actor_folder(&actors, "Alice").unwrap();
    assert_eq!(
        outcome
            .iter()
            .filter(|item| item.kind == "unlink-derived-hard-link")
            .count(),
        3
    );
    assert!(!actors.join("Alice").exists());
    assert!(movie.join("ABC-123.mp4").exists());
    assert!(movie.join("ABC-123.nfo").exists());
    assert!(movie.join("ABC-123-poster.jpg").exists());

    assert_eq!(
        execute_actor_links_command(media.clone(), actors.clone(), true)
            .unwrap()
            .summary
            .failed_actions,
        0
    );
    assert!(actors.join("Alice/ABC-123/ABC-123.mp4").exists());
}

#[test]
fn compatibility_validation_accepts_same_filesystem() {
    let fixture = tempdir().unwrap();
    let media = fixture.path().join("media");
    let actors = fixture.path().join("actors");
    fs::create_dir_all(&media).unwrap();
    fs::create_dir_all(&actors).unwrap();
    validate_hard_link_compatibility(&media, &actors).unwrap();
}
