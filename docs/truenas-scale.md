# TrueNAS SCALE 24.10+ deployment

TrueNAS SCALE 24.10 replaced the Kubernetes Apps backend with Docker and supports Custom Apps from either an image wizard or Docker Compose YAML. This deployment deliberately keeps persistent data in explicit Host Paths instead of hidden app volumes. See the official [24.10 release notes](https://www.truenas.com/docs/scale/24.10/gettingstarted/scalereleasenotes/), [Apps UI reference](https://www.truenas.com/docs/scale/25.10/scaleuireference/apps/), [ACL guidance](https://www.truenas.com/docs/scale/24.10/scaletutorials/datasets/permissions/configuringacls/), and [snapshot guidance](https://www.truenas.com/docs/scale/24.10/scaletutorials/datasets/managesnapshotsscale/).

## Datasets and UID/GID and ACL

Create five Host Paths before installing. Do not point two roles at the same path or nest one below another.

| Role | Container path | Required access | Backup class |
| --- | --- | --- | --- |
| Configuration and secrets | `/config` | read/write | critical |
| SQLite state | `/state` | read/write | critical |
| Artwork/cache | `/cache` | read/write | disposable/rebuildable |
| Media Roots | `/media` | read/write for management operations | source media |
| Actor View | `/actors` | read/write | derived/rebuildable |

The image defaults to UID/GID `568:568`; change both `RUST_JAV_UID` and `RUST_JAV_GID` together if your datasets use another identity. Grant that numeric user/group traverse, read, and write permissions on every mounted dataset using the TrueNAS ACL editor. The process refuses UID or GID 0 and startup reports the effective identity and the failing path when access is wrong.

Actor View uses hard links. `/media` and `/actors` must report the same filesystem device from inside the container. On ZFS, create them as directories in one dataset, or otherwise verify they resolve to the same filesystem; sibling ZFS datasets cannot hard-link across the dataset boundary. Startup fails before serving when the requirement is not met.

## Install via YAML

1. Copy `deploy/truenas/management.yaml` into the configuration Host Path and adjust paths only if you also adjust the Compose mounts.
2. In **Apps**, select **Discover Apps > Custom App > Install via YAML** and paste `deploy/truenas/compose.yaml`.
3. `RUST_JAV_IMAGE` is required and has no default. Set it to the image produced for the reviewed commit, using a commit tag or immutable digest; floating release aliases and `latest` are not accepted for this deployment. Replace all `/mnt/tank/...` examples with your existing dataset paths.
4. Keep the `9317:9317` port mapping. Host networking is neither required nor recommended.
5. Start the app and inspect `/health/live`, `/health/ready`, `/health/mounts`, and `/health/jellyfin`. Local readiness checks process and SQLite independently of Jellyfin availability.
6. Initialize the administrator with `docker exec rust-jav rust-jav administrator init --config /config/management.yaml`, then open the one-use URL. The packaged `rust-jav` command re-enters the container identity wrapper, so the secrets document is written as the configured `RUST_JAV_UID:RUST_JAV_GID`; do not bypass it by invoking `rust-jav-bin` directly.

## Jellyfin network

The Compose example joins an external `jellyfin-net`. Create that Docker network once and attach the Jellyfin App/container to it as well, or replace the network name with an existing shared app network. Configure the Jellyfin URL as `http://<jellyfin-container-name>:8096`; never use `localhost`, because that refers to rust-jav itself. The published management port remains available on the NAS without `network_mode: host`.

Jellyfin being offline makes `/health/jellyfin` report `available: false`, but `affects_local_readiness` remains false and `/health/ready` stays healthy when local state is sound.

## Backup, Restore, upgrade, and Rollback

Before changing the image, stop or quiesce the app and create coordinated dataset snapshots of configuration and SQLite state. Actor View and cache can be regenerated, but snapshot them if faster recovery matters. Media Roots follow the storage policy for source media.

For an application-level Backup, stop the container and run an SQLite online backup from a temporary container or a host with SQLite installed:

```sh
sqlite3 /state/management.sqlite3 .backup /state/management.sqlite3.backup
cp -a /config /your/backup/location/config
```

Also replicate or otherwise protect the configuration and state datasets. Never copy a live SQLite database with plain `cp` while writes can occur.

For Restore, stop the app, preserve the failed state, restore `/config` and the SQLite backup to `/state/management.sqlite3`, fix ownership to the configured numeric UID/GID, then start and check all four health endpoints. Restore matching dataset snapshots together when using ZFS.

For an upgrade, take backups/snapshots, pin the new immutable image tag or digest in Compose, redeploy, and execute the update acceptance below. Do not use `latest` for a controlled production upgrade.

For Rollback, stop the app and restore the previous image digest plus the matching config/SQLite backup or coordinated snapshots. TrueNAS app rollback snapshots cover app-dataset/iXvolume data only: **Host Path data is not rolled back**. Because this Compose deployment uses Host Paths, restoring the previous image alone does not restore application state. TrueNAS documents ZFS rollback as disruptive; cloning a snapshot and copying verified data back is safer when practical.

## TrueNAS acceptance run

Record the SCALE version, image digest, UID/GID, dataset names, and results.

Use disposable media and a disposable Jellyfin library. The complete sign-off matrix and evidence fields are in [the release acceptance record](release-acceptance.md); the real-server Jellyfin steps are in [the Jellyfin acceptance procedure](jellyfin-acceptance.md).

### Authentication acceptance

Initialize through the local container command and prove that the bootstrap URL works once. Log in and out, then reset the password locally and confirm every previous session becomes unauthorized. Restart the App and confirm the new credential remains usable. Never capture the bootstrap token, cookie, password, or Jellyfin API key in evidence.

### Installation and permissions acceptance

Install from Compose, confirm the container is not root with `docker exec rust-jav id`, and verify `/health/live`, `/health/ready`, and `/health/mounts` return success. Confirm an asset scan can read and write the Media Root, SQLite persists under `/state`, secrets persist under `/config`, and cache output can be created under `/cache`. Temporarily remove write ACL access from a disposable test path and confirm restart fails with the path and UID/GID; restore the ACL before continuing.

Run a full Asset Index rebuild, add/change/remove a disposable asset, and run incremental reconciliation. Confirm search, NFO details, artwork, actor details, and exceptions reflect the changes. Restart and confirm the rebuilt index persists. Remove read access temporarily and confirm the root degrades with an actionable permission report rather than crashing the service.

### Rules failure acceptance

Activate a known valid disposable Rule Set. Attempt downloads with invalid YAML, a non-HTTPS URL, a host outside `rule_source_hosts`, an oversized response, and an unreachable/timeout source. Confirm every failure is visible and the prior active Rules remain byte-for-byte unchanged. Confirm an empty set requires its separate acknowledgement.

### Restart acceptance

Initialize and configure Jellyfin, create a management task, restart the app from TrueNAS, and confirm login, task history, asset index, Actor Folders, and Jellyfin settings survive. Confirm all local health layers recover without requiring Jellyfin to be online.

For interruption behavior, start a disposable destructive task and restart while it is running. The durable record must become `interrupted`, must not be automatically replayed, and must expose its final snapshot/audit after reconnect. Re-preview current filesystem state before any retry.

### Update acceptance

Take backups and dataset snapshots, note the old digest, deploy a newer pinned digest, and confirm schema startup, UI, OpenAPI, Actor Folder operations, Jellyfin connection, and all health endpoints. Keep the old digest until the observation period ends.

### Host Path deletion acceptance

Use disposable datasets only. Stop the app, rename or unmount one test Host Path (do not destroy source media), then start it and confirm startup refuses service with an actionable mount/ACL diagnostic. Restore the exact path and permissions and confirm recovery. This validates failure behavior without trusting hidden app storage.

Then use disposable files to exercise permanent deletion: review candidates, create a fresh plan, compare selected-only versus discovered-hard-links scope, type the confirmation phrase, and execute. Confirm expired or replaced files are rejected, other paths continue after a partial failure, and the audit retains per-path outcomes. Never use production media for this acceptance step.

### ZFS hard-link acceptance

Create a disposable media file and Actor View link, then compare device and inode values with `stat` inside the container. They must share both device and inode and link count must increase. Attempt a disposable configuration where Actor View is a different ZFS dataset and confirm startup rejects it. Remove the derived Actor Folder and verify the source Media Asset remains; regenerate the Actor View from NFO metadata.

### Jellyfin and port acceptance

Confirm port 9317 is reachable through the mapped NAS address without host networking. Confirm Jellyfin is reachable by container DNS on the shared network. Stop Jellyfin: `/health/jellyfin` must degrade while `/health/ready` remains successful. Start Jellyfin and confirm availability recovers.
