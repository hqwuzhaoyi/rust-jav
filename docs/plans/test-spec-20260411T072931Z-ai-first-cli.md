# Test Spec — AI-first CLI refactor for rust-jav

## Test goals
Prove that the new CLI is:
1. commandable without TUI
2. preview-by-default for all file operations
3. correct in filesystem mutation behavior when apply is explicit
4. stable in JSON/text output shape
5. correct for NFO-driven actor hard-link generation

## Coverage matrix

| Area | What to verify | Test type |
|---|---|---|
| CLI parsing | top-level subcommands, operation selection, preview/apply routing | unit |
| Preview default | running file operations without apply does not mutate filesystem | integration |
| Operation planning | each of the 7 operations produces planned actions/summary | integration |
| Operation apply | explicit apply mutates filesystem as expected | integration |
| JSON output | parseable stable structure with summary + actions | integration |
| Exit codes | success vs failure codes are stable | integration |
| NFO parsing | actor names extracted from `<actor><name>` | unit |
| Actor-link preview | planned actor folders / hard-links are reported correctly | integration |
| Actor-link apply | expected directory tree and hard links are created | integration |
| TUI safety | TUI still builds / shared layer integration doesn’t break core flow | smoke |

## Proposed fixtures

### Existing/available
- `REBD-615.nfo` as actor-parse fixture
- `examples/` and `test/` trees as operation behavior references where useful

### New temporary fixtures to create in tests
1. **basic_ops_tree**
   - mixed files that trigger rename/prefix/category/origin behavior
2. **empty_and_nfo_dirs_tree**
   - directories with only nfo/trailers/empty contents
3. **duplicates_tree**
   - files that represent duplicate detection scenarios
4. **actor_link_tree**
   - media file + matching NFO + poster/backdrop assets

## Detailed test cases

### 1. CLI parser tests
- parse `rust-jav tui --dir ...`
- parse `rust-jav ops preview --dir ... --op organize-by-code`
- parse `rust-jav ops apply --dir ... --op move-origin`
- parse `rust-jav actor-links preview --source ... --actors-root ...`
- parse `rust-jav actor-links apply --source ... --actors-root ... --json`

### 2. Preview-by-default tests
For each mutating command family:
- create temp fixture
- run preview/default command
- assert no files moved/renamed/deleted/linked
- assert output contains planned actions

This specifically applies to:
- ops commands
- actor-links commands

### 3. Per-operation preview/apply tests
For each TUI-derived operation:
- build fixture that should trigger it
- run preview
- assert planned action list and counts
- run apply
- assert expected filesystem state

At minimum:
- remove prefixes / standardize naming
- move CHINESE / UNCENSORED / ORIGIN
- clean empty dirs / dirs with only nfo or trailers
- organize by code
- extract codes exposure
- remove duplicates behavior

### 4. JSON contract tests
- parse stdout as JSON when `--json` is supplied
- verify presence of:
  - mode
  - selected_ops / command kind
  - summary
  - actions[]
  - warnings/errors when relevant
- verify preview vs apply changes `mode` / action statuses predictably

### 5. Exit code tests
- success preview returns 0
- success apply returns 0
- invalid args return non-zero
- missing directory / malformed NFO / hard-link failure return stable non-zero

### 6. NFO actor parse tests
Using `REBD-615.nfo`-style XML:
- extract `miru` from `<actor><name>miru</name>`
- support one-to-many actors in synthetic fixtures
- malformed XML yields structured error

### 7. Actor-link preview tests
Fixture:
- source directory with video + `REBD-615.nfo` + poster + backdrop
- actors root temp dir

Assertions:
- preview reports target directory like `<actors_root>/miru/REBD-615/`
- preview reports intended hard-link actions
- source files unchanged
- actors root untouched on disk

### 8. Actor-link apply tests
Assertions after apply:
- directory `<actors_root>/miru/REBD-615/` exists
- expected hard-linked files exist there
- hard links point to original inode where supported
- rerun behavior is deterministic/documented (idempotent or clear conflict report)

### 9. Regression / smoke tests
- `cargo test`
- `cargo check`
- optionally a smoke test ensuring `rust-jav tui --dir ...` still parses and the binary still compiles with TUI enabled

## Execution ordering
1. parser tests first
2. preview-default regression tests
3. per-operation preview/apply tests
4. actor-link parse/preview/apply tests
5. full suite + cargo check

## Evidence required before implementation is considered done
- PRD-backed command tree exists in code
- tests added for preview default semantics
- tests added for actor-link workflow
- full test suite passes locally
- any unsupported operation gaps are either fixed or explicitly removed from scope with spec change

## Open implementation-sensitive checks
These do not block planning, but must be resolved during implementation:
- exact output schema field names
- whether `ExtractCodes` and `RemoveDuplicates` already have real apply behavior or need new implementation
- exact file-set inclusion rules for actor-link apply
