# Plan — Add ad-file cleanup into default ops flow

## Context
Current AI-first CLI `ops` does not expose ad-file cleanup, even though the legacy repository still contains:
- ad filename patterns in `patterns.txt`
- deletion logic in `src/file_utils/delete_files.rs`

Fresh verification evidence gathered during clarification:
- `cargo run -- ops --help` does **not** list any ad-cleanup operation
- `cargo run -- ops --op delete-ad-files --json` fails with invalid value / exit code 2
- legacy patterns currently match examples such as:
  - `新片首发每天更新.txt`
  - `大平台真人荷官.html`
  - `乐鱼体育投注.url`
  - `美女荷官在线发牌.jpg`
  - `新片首发每天更新.mp4`

## Clarified decisions
1. Add a new operation: `delete-ad-files`
2. Include it in default full `ops` execution
3. Also expose it via `--op delete-ad-files`
4. Preview remains default; `--apply` is required to delete
5. Match rule: if filename matches `patterns.txt`, delete it
6. This includes matching video files; no extension-based safety exemption
7. Execution order: run `delete-ad-files` first in the full ops pipeline
8. Preview/reporting: one `delete-file` action per matched file in `actions[]`

## Implementation plan

### 1. CLI surface
- Add `DeleteAdFiles` to `CliOperation`
- Add `DeleteAdFiles` to `OperationType`
- Ensure `OperationType::all()` puts `DeleteAdFiles` first
- Update names/descriptions/help output

### 2. Shared planning / execution layer
- Add planner for ad-file deletion in `src/tui/executor.rs`
  - enumerate files recursively
  - match by legacy `patterns.txt` semantics
  - emit per-file `PlannedAction { kind: "delete-file" }`
- Add executor for ad-file deletion
  - delete matched files on apply
  - return partial success when some deletions fail
- Keep current `CommandReport` shape: summary + per-action details

### 3. Reuse strategy
- Prefer reusing legacy matcher semantics from `patterns.txt`
- Either:
  - extract shared matcher helper from legacy path, or
  - mirror the same wildcard-matching behavior in the new executor
- Avoid wiring new CLI behavior through global legacy config flags

### 4. Fixtures / docs
- Add `delete-ad-files` scenario to `examples/create_test_files.sh`
- Update README supported operations list and smoke examples
- Update `docs/testing-report.md`

## Test plan

### Automated
1. Parser test: `--op delete-ad-files`
2. Preview test:
   - matched files appear as planned `delete-file` actions
   - no files are deleted
3. Apply test:
   - matched files are deleted
   - unmatched files remain
4. Video deletion test:
   - matching `.mp4` is deleted because the clarified spec allows it
5. Conflict/partial failure test:
   - if a deletion fails, action becomes `failed`, report exit code becomes non-zero
6. Full ops ordering test:
   - ad files disappear before later ops would otherwise rename/move them

### Manual smoke
- `cargo run -- ops --dir ./examples/test/delete-ad-files --op delete-ad-files --json`
- `cargo run -- ops --dir ./examples/test/delete-ad-files --op delete-ad-files --apply --json`
- `cargo run -- ops --dir ./examples/test --apply --json`

## Risks
1. Matching video files is intentionally destructive; preview visibility is critical
2. Legacy pattern semantics are broad and may catch more than expected
3. Default full-ops inclusion changes behavior materially; tests and docs must be explicit

## Acceptance criteria
- `delete-ad-files` appears in `ops --help`
- `ops --json` preview shows per-file `delete-file` actions for matched ad files
- `ops --apply` deletes matched files, including matched videos
- full `ops` executes ad cleanup first
- tests and fixture generator cover the new behavior
