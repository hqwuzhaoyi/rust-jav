# Release acceptance record for issue 20

This record separates repeatable local evidence from checks that require real external systems. Never mark an unexecuted row as passed. Record immutable image digests, OS/browser versions, timestamps, and redacted evidence paths when running the external procedures.

## Automated local evidence

| Area | Evidence |
| --- | --- |
| CLI/TUI compatibility | parser, workflow, runtime dispatch, application service, migration verifier, and 34 TUI state tests |
| Authentication and secrets | management interface initialization/login/reset/session and `0600` secrets tests |
| Rules | versioned YAML, allowlisted HTTPS download failures, validation, empty-set confirmation, and atomic activation tests |
| Tasks/SSE/restart | SQLite persistence, serialization, interruption, final snapshot reconnect, report/audit tests |
| Assets/NFO/actors | scan/rebuild, permission degradation, search/state, artwork confinement, NFO details, actor folder removal/regeneration tests |
| Deletion | root confinement, symlink defense, expiry/revalidation, hard-link selection, partial outcomes, audit tests |
| Jellyfin | mock-server auth, selected libraries, association confidence, batch refresh, bounded retry tests |
| TrueNAS packaging | Compose, non-root image, layered health, distinct mount roles, overlap/missing mount tests |
| Responsive UI | jsdom interaction tests for navigation, asset/NFO/actor details, exceptions, Rules, dialogs, task recovery, and audits; CSS breakpoint assertions |

## Native Linux and macOS matrix

Run on a native filesystem (APFS for macOS, ext4/XFS/ZFS for Linux) with disposable fixtures:

1. Run `cargo test --all-targets`, frontend checks, and `test-tui-demos.sh`.
2. Run full and incremental scans; make one root unreadable and confirm an actionable exception without service loss.
3. Download a valid allowlisted Rule proposal, then test invalid YAML, disallowed host, non-HTTPS, oversized response, and timeout. Confirm the active file is unchanged on failure.
4. Preview then apply an ordinary rename/move and a disposable permanent deletion. Verify final reports and deletion audit history.
5. Create Actor View hard links; compare device/inode/link count. Point Actor View at a different filesystem and confirm explicit failure.

Cross-filesystem tests may skip when the runner has no second device. Record that as unavailable, not passed. CI results on one OS do not substitute for the other OS row.

| Environment | Status in this worktree | Evidence |
| --- | --- | --- |
| macOS native | locally automatable subset recorded in final handoff | command output and Rust/frontend suites |
| Linux native | not available from this macOS worktree | run the same commit in Linux CI/host |
| TrueNAS SCALE | not available from this worktree | execute `docs/truenas-scale.md` checklist |

## Phone, tablet, desktop browser matrix

Test at minimum 390×844, 768×1024, and 1440×900. At each size verify login/logout, all navigation destinations, asset search/filter/pagination, artwork fallback, NFO and actor detail, exception text, Rule edit/validation/failure, deletion and Actor Folder confirmation/cancel paths, task progress/reconnect, and audit history. Verify keyboard focus/Escape on desktop and touch targets/no horizontal overflow on phone/tablet.

Automated jsdom tests prove interactions and semantic labels but not real layout, touch behavior, or browser rendering. Those three viewport passes therefore remain external until screenshots and browser/version metadata are attached by the parent verifier.

## TrueNAS SCALE acceptance matrix

Execute the detailed steps in [TrueNAS SCALE deployment](truenas-scale.md) and record:

| Scenario | Required observation |
| --- | --- |
| Auth | one-use initialization, login, logout, reset revokes sessions, restart retains credentials |
| Host Paths | correct UID/GID works; removed read/write ACL or missing mount gives path/identity diagnostic |
| Asset Index | full rebuild and incremental scan survive restart; permission exception is visible |
| Jellyfin | dedicated key, DNS/port connection, selected library scope, exact/uncertain association, one batch refresh |
| Rule Source failure | bad YAML/host/TLS/size/timeout never replaces active Rules |
| Permanent deletion | disposable paths only; preview, typed confirmation, hard-link choice, audit and partial result |
| ZFS/Actor Folders | same dataset hard links; sibling dataset rejected; removal preserves media; regeneration succeeds |
| Restart interruption | running destructive task becomes `interrupted`, is not replayed, and remains in history |
| Upgrade/rollback | pinned digest, coordinated backup/snapshot, schema/UI/OpenAPI health, previous digest retained |

## Real Jellyfin acceptance

The authoritative procedure is [Jellyfin integration acceptance](jellyfin-acceptance.md). A real pass requires a reachable Jellyfin server with a disposable library and dedicated API key; mock-server tests are not a substitute.

Status for this worktree: **not executed — no real Jellyfin or TrueNAS endpoint was supplied**. Parent verification must record server version, rust-jav commit/image digest, selected disposable library IDs, timestamp, step results 1–8, and redacted logs/screenshots. Do not paste the API key or session cookie.

## Sign-off

- Commit/image digest:
- TrueNAS SCALE version:
- Native Linux distribution/kernel:
- macOS version:
- Browser/device matrix:
- Real Jellyfin version and pass timestamp:
- Failed/skipped scenarios and reason:
- Verifier:
