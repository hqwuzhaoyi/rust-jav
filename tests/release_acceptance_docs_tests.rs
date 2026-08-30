use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("unable to read {path}: {error}"))
}

#[test]
fn final_acceptance_record_covers_issue_twenty_matrix_and_external_truthfulness() {
    let acceptance = read("docs/release-acceptance.md");
    for required in [
        "CLI/TUI compatibility",
        "Linux native",
        "macOS native",
        "Phone, tablet, desktop browser matrix",
        "TrueNAS SCALE acceptance matrix",
        "Auth",
        "Host Paths",
        "Asset Index",
        "Rule Source failure",
        "Permanent deletion",
        "ZFS/Actor Folders",
        "Restart interruption",
        "Upgrade/rollback",
        "Real Jellyfin acceptance",
        "not executed",
    ] {
        assert!(
            acceptance.contains(required),
            "missing acceptance topic: {required}"
        );
    }
}

#[test]
fn migration_samples_and_client_workflow_are_complete_and_secret_free() {
    let migration = read("docs/migration-and-compatibility.md");
    let client = read("docs/openapi-and-generated-client.md");
    let native = read("deploy/native/management.yaml");
    let truenas = read("deploy/truenas/management.yaml");
    let native_secrets = read("deploy/native/management.secrets.example.yaml");
    let truenas_secrets = read("deploy/truenas/management.secrets.example.yaml");

    for required in [
        "preview",
        "--apply",
        "--json",
        "TUI",
        "Rollback",
        "RUST_JAV_UID",
    ] {
        assert!(
            migration.contains(required),
            "migration guide missing {required}"
        );
    }
    for required in [
        "/api/v1/openapi.json",
        "OpenAPI 3.1",
        "typescript-fetch",
        "EventSource",
        "regenerate",
    ] {
        assert!(
            client.contains(required),
            "client workflow missing {required}"
        );
    }
    for config in [&native, &truenas] {
        for key in [
            "active_rule_set_file:",
            "database_file:",
            "artwork_cache_root:",
            "media_roots:",
            "actor_view_root:",
        ] {
            assert!(config.contains(key), "sample config missing {key}");
        }
    }
    for example in [native_secrets, truenas_secrets] {
        assert!(example.contains("jellyfin_api_key: null"));
        assert!(!example.contains("X-Emby-Token"));
    }
}

#[test]
fn prototype_record_is_outside_runtime_and_maps_decisions_to_evidence() {
    let prototype = read("docs/prototypes/management-interface-decisions.md");
    for required in [
        "not production code",
        "small and large screens",
        "Permanent deletion",
        "Actor Folder removal",
        "SSE",
        "NFO",
        "Rejected prototype ideas",
    ] {
        assert!(
            prototype.contains(required),
            "prototype record missing {required}"
        );
    }
}
