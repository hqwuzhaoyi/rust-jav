#![cfg(unix)]

use std::fs;

use rust_jav::{
    actor_links::execute_actor_links_command,
    actor_views::{browse_actor_folders, remove_actor_folder, validate_hard_link_compatibility},
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
    assert_eq!(folders[0].logical_size, 12);
    assert_eq!(folders[0].reclaimable_space, 0);
    assert!(folders[0]
        .poster_path
        .as_ref()
        .unwrap()
        .ends_with("ABC-123-poster.jpg"));
}

#[test]
fn actor_folder_uses_a_nested_folder_image_as_its_poster() {
    let fixture = tempdir().unwrap();
    let actors = fixture.path().join("actors");
    let movie = actors.join("Alice/ABC-123");
    fs::create_dir_all(&movie).unwrap();
    fs::write(movie.join("ABC-123.mp4"), b"movie").unwrap();
    fs::write(movie.join("folder.jpg"), b"poster").unwrap();

    let folders = browse_actor_folders(&actors).unwrap();

    assert!(folders[0]
        .poster_path
        .as_ref()
        .unwrap()
        .ends_with("ABC-123/folder.jpg"));
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
