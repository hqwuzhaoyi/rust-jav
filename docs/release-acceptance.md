# Production design-system and release acceptance record

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
| Embedded interface | Vite writes source and final JS/CSS content provenance into `dist/index.html` and a tracked manifest; every Rust build independently checks HTML references and bundle hashes and rejects stale or replaced assets |
| Responsive UI | jsdom interaction tests for navigation, asset/NFO/actor details, exceptions, Rules, dialogs, task recovery, and audits; CSS breakpoint, safe-area, reduced-motion, long-content, empty-state, keyboard/focus, and explicit touch-target assertions |

## Native Linux and macOS matrix

Run on a native filesystem (APFS for macOS, ext4/XFS/ZFS for Linux) with disposable fixtures:

1. In `frontend`, run `npm test`, `npm run check`, and `npm run build`; then run `cargo test --all-targets` and `test-tui-demos.sh`. Rust must accept the digest in the newly generated embedded shell.
2. Run full and incremental scans; make one root unreadable and confirm an actionable exception without service loss.
3. Download a valid allowlisted Rule proposal, then test invalid YAML, disallowed host, non-HTTPS, oversized response, and timeout. Confirm the active file is unchanged on failure.
4. Preview then apply an ordinary rename/move and a disposable permanent deletion. Verify final reports and deletion audit history.
5. Create Actor View hard links; compare device/inode/link count. Point Actor View at a different filesystem and confirm explicit failure.

Cross-filesystem tests may skip when the runner has no second device. Record that as unavailable, not passed. CI results on one OS do not substitute for the other OS row.

| Environment | Status in this worktree | Evidence |
| --- | --- | --- |
| macOS native | passed on macOS 26.5.2 | 275 Rust tests, 119 frontend tests, TypeScript, production build, rustfmt, and strict Clippy |
| Linux native | production image build passed on TrueNAS Linux 6.6.44 x86_64 | locked release build and embedded-asset provenance gate in Docker |
| TrueNAS SCALE | smoke passed on 24.10.2.2 at commit `dd59410` | image `sha256:d2dd628211eb41b23f38716aab27745f55b9cd81b784829ffd4a8a2012609a4a`; app RUNNING and process/SQLite ready |

## Phone, tablet, desktop browser matrix

Test at minimum 390×844, 768×1024, and 1440×900. At each size verify login/logout; Media Root capacity; asset search/filter/pagination and detail; local artwork validation plus Jellyfin artwork fallback; Actor View; Management Tasks; Settings; permanent-deletion review; exception text; Rule edit/validation/failure; task progress/reconnect; and audit history. Exercise long paths/titles and empty results. Verify keyboard order, visible focus, focus trap/restore and Escape on desktop; reduced-motion behavior; close-control geometry; safe-area padding; touch targets; and no horizontal overflow on phone/tablet.

Automated jsdom tests prove interactions and semantic labels but not real layout, touch behavior, or browser rendering. A production browser pass on 2026-09-05 used Chromium 150 at 390×844, 768×1024, and 1440×900. All three viewports had no horizontal overflow. Actor Folder cards rendered per-folder Media Asset counts and Logical Size; name/count/size ordering and direction changes produced the expected first rows. The 390px sort controls remained 44px tall, the Actor Inspector close control rendered 44×44px with a 50% radius, focus entered/restored correctly, and Escape closed both loaded and still-loading detail. The permanent-deletion review showed fresh-plan authority, selected/unified hard-link scope, complete paths, metrics, required phrase, and a disabled destructive action without executing deletion. DASS-591 loaded its authenticated Jellyfin fallback at 358×538, and Overview/NFO tabs, Tasks, Settings, and Deletion Candidates completed smoke checks without alerts.

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

Status for this worktree: **live integration smoke passed; full disposable-library procedure not executed**. Jellyfin 10.11.8 was reachable from TrueNAS with four configured library IDs and a server-side API key. The production browser verified a certain Association-backed DASS-591 Primary image fallback without exposing the key or a direct Jellyfin image URL. Destructive/disposable-library steps were skipped because the real Media Root was not disposable and was at 100% capacity. Do not paste the API key or session cookie.

## Sign-off

- Commit/image digest: `dd59410`; `sha256:d2dd628211eb41b23f38716aab27745f55b9cd81b784829ffd4a8a2012609a4a`
- TrueNAS SCALE version: 24.10.2.2
- Native Linux distribution/kernel: TrueNAS Linux 6.6.44-production+truenas x86_64
- macOS version: 26.5.2 (25F84)
- Browser/device matrix: Chromium 150; 390×844, 768×1024, 1440×900; passed 2026-09-05
- Real Jellyfin version and pass timestamp: 10.11.8; live fallback/configuration smoke passed 2026-09-05
- Failed/skipped scenarios and reason: physical deletion, mutation, and disposable-library Jellyfin steps skipped on the real Media Root; it is non-disposable and currently 100% full. Mount health truthfully reports degraded while the management service remains available.
- Verifier: Codex production browser and NAS smoke
