# PRD — AI-first CLI refactor for rust-jav

## Metadata
- Source of truth: `.omx/specs/deep-interview-tui-cli-parity.md`
- Planning mode: ralplan / consensus-style
- Scope type: brownfield refactor + capability expansion
- Risk level: moderate

## Problem
The repository currently launches into TUI from `main.rs`, while the underlying CLI surface is an older flag-based interface that does not cleanly represent the actual business operations. This creates product confusion: TUI appears to be the primary surface, but automation and AI cannot reliably invoke the same capabilities.

## Desired outcome
Make CLI the primary, AI-safe interface. TUI becomes a consumer of shared execution/planning logic instead of the only rich entrypoint.

## Product principles
1. **CLI-first, TUI-second**: business operations live below the UI boundary.
2. **Safe by default**: every file-mutating workflow defaults to preview; mutation requires explicit apply/execute.
3. **Machine-readable results**: analysis and execution responses must be stable and parseable.
4. **One operation model**: TUI, tests, and CLI should share one operation-planning/execution layer.
5. **Prefer clarity over backward compatibility**: old flags are not a constraint.

## Decision drivers
1. AI/automation must invoke operations without interactive UI.
2. The current flag-only CLI does not map clearly to the TUI operation model.
3. New features (preview-by-default, actor hard-linking from NFO) need explicit command boundaries.

## Viable options

### Option A — Thin wrapper around existing flags and TUI executor
- Pros: smallest immediate diff to get commands exposed.
- Cons: preserves confusing model boundaries, weak JSON semantics, hard to support preview-by-default globally, hard to add actor-linking cleanly.
- Verdict: rejected.

### Option B — Introduce a shared operation planning/execution layer and rebuild CLI around it
- Pros: aligns with AI-first requirement, clean preview/apply split, reusable from TUI, testable without terminal harness.
- Cons: larger refactor, requires touching main/config/executor/tests.
- Verdict: chosen.

### Option C — Keep TUI primary and shell out from AI through UI-inspired commands
- Pros: low architecture disruption.
- Cons: directly violates intent; operations remain tied to UI semantics.
- Verdict: invalidated by clarified requirements.

## Chosen design

### 1) Command model
Move from flag-only CLI to subcommand-oriented CLI with explicit preview/apply modes.

Proposed top-level shape:

```text
rust-jav tui --dir <DIR>
rust-jav ops preview --dir <DIR> [--json] [--select ...]
rust-jav ops apply --dir <DIR> [--json] [--select ...]
rust-jav actor-links preview --source <DIR> --actors-root <DIR> [--json]
rust-jav actor-links apply --source <DIR> --actors-root <DIR> [--json]
```

Optional refinement if needed during implementation:
- `ops preview --all`
- `ops preview --op organize-by-code --op clean-empty-dirs`
- `ops apply ...`

### 2) Shared domain layer
Add a shared, non-TUI domain layer that separates:
- operation discovery / planning
- execution intent model
- file mutation execution
- result serialization
- NFO actor parsing + hard-link planning

Suggested module direction:
- `src/cli/` — clap command definitions, JSON/text output adapters
- `src/domain/operations.rs` — operation enum + planner interface
- `src/domain/plan.rs` — preview/apply plan structures
- `src/domain/execute.rs` — mutation engine
- `src/domain/nfo.rs` — actor parsing
- `src/domain/actor_links.rs` — actor directory/hard-link planning and apply

Actual names can vary; the key is extracting shared logic out of TUI.

### 3) Preview-by-default rule
All file operations default to preview mode.
- `preview` / `analyze` prints intended actions and summary only.
- `apply` / `execute` is explicit and required for filesystem mutation.
- TUI should eventually use the same planner first, then explicit execution.

### 4) Operation coverage mapping
Map current TUI operations into CLI-selectable operations:
- `OrganizeByCode`
- `CleanEmptyDirs`
- `StandardizeNames`
- `ExtractCodes`
- `CategorizeFiles`
- `MoveOrigin`
- `RemoveDuplicates`

Implementation note:
- Some current TUI operations appear analysis-only or placeholder-like (`ExtractCodes`, `RemoveDuplicates`).
- During implementation, each operation must be classified as:
  - fully executable now
  - preview-only pending true executor
- But the CLI contract still needs explicit representation for all seven.

### 5) Actor-link capability
Add a dedicated command family for actor-based hard-link generation from NFO metadata.

Rules from clarified spec:
- Actor source: NFO `<actor><name>`
- Default mode: preview
- Apply mode: creates hard links
- Output structure:
  - `<actors_root>/<actor_name>/<title_or_code>/...`
- Start from the confirmed directory-style structure using code/title identity from the source set

Open implementation choice delegated to execution planning:
- exact file-set inclusion rules inside the per-title directory
- normalization of actor names and title/code folder names
- fallback behavior when NFO is missing or malformed

### 6) Output contract
Support machine-readable output for AI callers.

Minimum JSON response categories:
- command metadata (`mode`, `source_dir`, `selected_ops`)
- summary counts (`planned_actions`, `applied_actions`, `warnings`, `errors`)
- per-action items (`kind`, `source`, `target`, `status`, `reason`)
- actor-link parse output where relevant

Exit code policy:
- `0`: successful preview/apply with no blocking errors
- non-zero: parse/config/runtime failure or partial failure policy violation

The exact non-zero map can be finalized during implementation, but it must be stable and documented.

## Brownfield architecture implications

### Current repo facts
- `src/main.rs` always starts TUI.
- `src/config.rs` owns existing clap shape.
- `src/tui/executor.rs` already knows about the 7 operation types, but it mixes analysis and execution concerns.
- `src/file_utils/*` contains reusable file-manipulation primitives, though several are tightly coupled to global config.

### Required refactor direction
1. Decouple mutation logic from global `CONFIG` where practical.
2. Stop making TUI the only orchestrator.
3. Introduce plan/result data structures that both CLI and TUI can consume.
4. Keep filesystem semantics centralized to avoid duplicate behavior.

## Delivery phases

### Phase 1 — Command surface + shared planning model
- Define new clap command tree.
- Introduce operation selection model and plan/result structs.
- Route `main.rs` between `tui` and CLI commands.
- Preserve buildability early.

### Phase 2 — Port existing 7 operations to shared planner/executor
- Reuse/reshape `src/file_utils/*` and `src/tui/executor.rs` logic.
- Implement preview output for every operation.
- Implement apply path for operations with real mutation support.
- Mark any unsupported apply path explicitly if discovered, then complete/fix before claiming done.

### Phase 3 — Add actor-link planning/execution
- Implement NFO actor parser.
- Implement actor-link preview results.
- Implement hard-link apply logic and directory creation.
- Add fixture-driven tests using `REBD-615.nfo` shape.

### Phase 4 — TUI integration cleanup
- Point TUI operation flow at shared planner/executor where practical.
- Keep UI-specific preview/help/panel behavior separate.

### Phase 5 — Verification and docs
- CLI tests
- regression tests for preview default semantics
- help output / README refresh if needed

## Risks and mitigations

### Risk 1 — Existing operations do not all have true executors
Mitigation:
- Audit each operation early.
- Add failing tests for any missing apply semantics before implementation.
- If needed, implement missing executor behavior directly in shared layer.

### Risk 2 — Global config coupling makes CLI refactor messy
Mitigation:
- Prefer explicit parameter objects over hidden global reads in new code.
- Wrap old config reads behind adapter layers where immediate removal is too risky.

### Risk 3 — Hard-link behavior varies by filesystem / platform
Mitigation:
- Keep tests local-tempdir based.
- Detect/report hard-link errors clearly.
- Avoid cross-device assumptions in behavior promises.

## Verification path
- `cargo test`
- targeted CLI parser tests
- targeted preview/apply filesystem tests
- NFO actor parse tests
- actor hard-link integration tests
- optional `cargo fmt --check`
- optional `cargo check`

## ADR
### Decision
Refactor rust-jav around a new AI-first CLI and shared operation layer, with preview-by-default semantics and a dedicated actor-link workflow.

### Drivers
- AI-callable non-interactive interface is the clarified primary requirement.
- TUI currently outpaces CLI capability and confuses the product surface.
- New actor-link feature requires explicit non-UI modeling.

### Alternatives considered
- Thin flag compatibility expansion
- TUI-primary orchestration with UI-driven semantics

### Why chosen
This is the only approach that satisfies CLI-first, preview-by-default, structured-output, and future TUI reuse simultaneously.

### Consequences
- Larger initial refactor
- Better long-term architecture
- Possible churn in CLI usage patterns
- Stronger testability and automation support

### Follow-ups
- Implement plan artifacts in code
- Update docs/examples
- Re-evaluate whether TUI should call the shared domain layer directly or through internal command adapters

## Suggested staffing guidance
- `executor`: command model + shared domain implementation
- `architect`: refactor boundaries / module extraction review
- `test-engineer`: fixture and regression suite design
- `verifier`: final contract validation and output-shape checks
