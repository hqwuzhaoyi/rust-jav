use std::fs;

use rust_jav::management::{ManagementConfig, MountRole};

#[test]
fn truenas_config_requires_distinct_persistent_mount_roles() {
    let fixture = tempfile::tempdir().unwrap();
    let media = fixture.path().join("media");
    let actors = fixture.path().join("actors");
    let config = fixture.path().join("config");
    let state = fixture.path().join("state");
    let cache = fixture.path().join("cache");
    for path in [&media, &actors, &config, &state, &cache] {
        fs::create_dir(path).unwrap();
    }
    let yaml = fixture.path().join("management.yaml");
    fs::write(
        &yaml,
        format!(
            "port: 9317\ncontainer: true\nsecrets_file: {}/management.secrets.yaml\ndatabase_file: {}/management.sqlite3\nartwork_cache_root: {}\nmedia_roots:\n  - {}\nactor_view_root: {}\n",
            config.display(), state.display(), cache.display(), media.display(), actors.display()
        ),
    )
    .unwrap();

    let config = ManagementConfig::load(&yaml).unwrap();
    let report = config.validate_truenas_mounts().unwrap();

    assert_eq!(report.uid, unsafe { libc::geteuid() });
    assert_eq!(report.gid, unsafe { libc::getegid() });
    assert!(report.database_ready);
    assert_eq!(report.mounts.len(), 5);
    assert!(report
        .mounts
        .iter()
        .any(|mount| mount.role == MountRole::MediaRoot && mount.readable && mount.writable));
    assert!(report
        .mounts
        .iter()
        .any(|mount| mount.role == MountRole::ActorView
            && mount.same_filesystem_as_media == Some(true)));
}

#[test]
fn truenas_validation_rejects_overlapping_or_missing_mounts() {
    let fixture = tempfile::tempdir().unwrap();
    let shared = fixture.path().join("shared");
    fs::create_dir(&shared).unwrap();
    let yaml = fixture.path().join("management.yaml");
    fs::write(
        &yaml,
        format!(
            "container: true\nsecrets_file: {0}/secrets.yaml\ndatabase_file: {0}/management.sqlite3\nartwork_cache_root: {0}\nmedia_roots:\n  - {0}\nactor_view_root: {0}\n",
            shared.display()
        ),
    )
    .unwrap();

    let error = ManagementConfig::load(&yaml)
        .unwrap()
        .validate_truenas_mounts()
        .unwrap_err()
        .to_string();

    assert!(error.contains("distinct Host Paths"));
}

#[test]
fn compose_and_image_contract_are_multiarch_non_root_and_health_layered() {
    let compose = include_str!("../deploy/truenas/compose.yaml");
    let dockerfile = include_str!("../Dockerfile");
    let workflow = include_str!("../.github/workflows/container.yml");

    for mount in ["/config", "/state", "/cache", "/media", "/actors"] {
        assert!(compose.contains(mount), "missing distinct mount {mount}");
    }
    assert!(compose.contains("9317:9317"));
    assert!(!compose.contains("network_mode: host"));
    assert!(compose.contains("jellyfin-net"));
    assert!(dockerfile.contains("HEALTHCHECK"));
    assert!(dockerfile.contains("RUST_JAV_UID"));
    assert!(dockerfile.contains("RUST_JAV_GID"));
    assert!(dockerfile.contains("COPY build.rs ./build.rs"));
    assert!(dockerfile.contains("COPY build_support ./build_support"));
    assert!(dockerfile.contains("COPY frontend/src ./frontend/src"));
    for input in [
        "frontend/index.html",
        "frontend/package*.json",
        "frontend/source-digest.ts",
        "frontend/tsconfig.json",
        "frontend/vite.config.ts",
    ] {
        assert!(
            dockerfile.contains(input),
            "missing frontend build input {input}"
        );
    }
    assert!(dockerfile.contains("COPY --from=frontend /src/frontend/dist ./frontend/dist"));
    assert!(!compose.contains(":latest"));
    assert!(!compose.contains("RUST_JAV_IMAGE:-"));
    assert!(!compose.contains("ghcr.io/hqwuzhaoyi/rust-jav:0.4.0"));
    assert!(compose
        .contains("${RUST_JAV_IMAGE:?Set RUST_JAV_IMAGE to an immutable digest or commit tag}"));
    assert!(workflow.contains("linux/amd64,linux/arm64"));
}

#[test]
fn packaged_cli_reenters_the_identity_dropping_entrypoint() {
    let dockerfile = include_str!("../Dockerfile");
    let entrypoint = include_str!("../docker/entrypoint.sh");
    let wrapper = include_str!("../docker/rust-jav-wrapper.sh");
    let guide = include_str!("../docs/truenas-scale.md");

    assert!(dockerfile.contains("/usr/local/bin/rust-jav-bin"));
    assert!(dockerfile.contains("docker/rust-jav-wrapper.sh"));
    assert!(entrypoint.contains("exec /usr/local/bin/rust-jav-bin"));
    assert!(wrapper.contains("exec /usr/local/bin/rust-jav-entrypoint"));
    assert!(guide.contains("docker exec rust-jav rust-jav administrator init"));
}

#[test]
fn operations_guide_covers_the_complete_truenas_acceptance_lifecycle() {
    let guide = include_str!("../docs/truenas-scale.md");
    for requirement in [
        "Install via YAML",
        "UID/GID and ACL",
        "Jellyfin network",
        "Restart acceptance",
        "Update acceptance",
        "Host Path deletion acceptance",
        "ZFS hard-link acceptance",
        "Backup",
        "Restore",
        "Rollback",
        "dataset snapshots",
    ] {
        assert!(
            guide.contains(requirement),
            "missing operations guidance: {requirement}"
        );
    }
    assert!(guide.contains("Host Path data is not rolled back"));
    assert!(guide.contains("sqlite3 /state/management.sqlite3 .backup"));
    assert!(guide.contains("RUST_JAV_IMAGE"));
    assert!(guide.contains("required"));
    assert!(guide.contains("commit tag or immutable digest"));
    assert!(guide.contains("has no default"));
}
