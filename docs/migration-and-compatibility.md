# Migration and compatibility

This release adds the Management Interface without replacing the CLI or TUI. Existing automation can continue to call `ops`, `actor-links`, and `tui`; the web service is opt-in through `serve`.

## CLI and TUI compatibility

- File operations still default to preview. Only `--apply` mutates the filesystem.
- `--json` remains the stable machine-readable report format. Apply reports add migration-verification fields; existing action fields are retained.
- The TUI invokes the same application services and operation ordering as the CLI. It does not require the Management Interface or SQLite database.
- `delete-ad-files` remains first in the full pipeline. A YAML rule source changes selection rules, not filesystem scope or deletion authorization.
- `actor-links` retains preview/apply behavior and requires source and Actor View paths on the same filesystem.

Before upgrading, run representative previews with the old and new binaries and retain their JSON reports. After upgrading, repeat preview before any apply. Destructive results continue to require manual confirmation even when migration verification matches.

## Native migration

1. Back up media metadata and copy any custom `patterns.txt` entries into versioned `rules.yaml` entries. `patterns.txt` is a migration reference only.
2. Copy `deploy/native/management.yaml` beside the binary and edit Media Roots and Actor View. Relative state paths resolve beside this YAML.
3. Create the administrator locally with `rust-jav administrator init --config management.yaml`. Do not copy a bootstrap URL into logs.
4. Start with `rust-jav serve --config management.yaml`; native mode listens on `127.0.0.1` by default.
5. Rebuild the Asset Index, inspect exceptions, configure Jellyfin if wanted, and perform preview tasks before apply.

No media move is required to adopt the web UI. The Asset Index and Actor View are derived and rebuildable. The SQLite task/audit database and secrets file are persistent state and must be backed up together with the ordinary YAML configuration.

## TrueNAS migration

Follow [TrueNAS SCALE deployment](truenas-scale.md). Use distinct Host Paths for configuration, SQLite, cache, media, and Actor View. The media and Actor View container paths must resolve to the same ZFS dataset/device for hard links, even though they are distinct mounted directories.

Migrate native state only while both instances are stopped. Copy `management.yaml`, `management.secrets.yaml`, `active-rules.yaml`, and `management.sqlite3` to their corresponding Host Paths, then set ownership to the configured numeric UID/GID. Rewrite native paths to the container paths `/config`, `/state`, `/cache`, `/media`, and `/actors`. Do not run native and container instances against the same SQLite file.

## Secrets and environment

Ordinary YAML may be source controlled after replacing real paths. Never commit `management.secrets.yaml`, administrator passwords, bootstrap URLs, session cookies, or Jellyfin API keys.

| Name | Purpose | Required |
| --- | --- | --- |
| `RUST_JAV_ADMIN_PASSWORD` | One-time unattended initialization or local password reset; minimum 12 characters | optional |
| `RUST_JAV_CONFIG` | Container entrypoint configuration path | container image |
| `RUST_JAV_UID`, `RUST_JAV_GID` | Non-root numeric identity used for Host Paths | container image; defaults to `568:568` |

The owner-readable secrets file is created with mode `0600`. Jellyfin keys are written only there and are never returned through the API. Prefer a dedicated Jellyfin API key and rotate it after any suspected exposure.

## Rollback

Stop the service before restoring the prior binary/image and its matching YAML, secrets, and SQLite backup. Restore coordinated ZFS snapshots when application state and media were changed together. A TrueNAS App rollback does not roll back external Host Paths.
