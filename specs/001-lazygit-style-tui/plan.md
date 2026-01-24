# Implementation Plan: LazyGit 风格 TUI 界面

**Branch**: `001-lazygit-style-tui` | **Date**: 2025-12-07 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-lazygit-style-tui/spec.md`

## Summary

Replace the current dialoguer-based interactive CLI with a full-screen Ratatui TUI featuring a three-panel layout (directory tree, operations list, preview). The TUI will provide LazyGit-style keyboard navigation, real-time operation preview, batch operation management, manual file movement with quick targets, and execution progress tracking with persistent logging.

## Technical Context

**Language/Version**: Rust 1.75+ (edition 2021)
**Primary Dependencies**:
- Existing: `tokio`, `clap`, `regex`, `once_cell`, `indicatif`, `log`, `env_logger`
- New: `ratatui` (TUI framework), `crossterm` (terminal backend)
- Remove: `dialoguer` (replaced by TUI)

**Storage**: Local filesystem only; logs persisted to `~/.rust-jav/logs/`
**Testing**: `cargo test` (existing pattern)
**Target Platform**: Cross-platform terminals (Linux, macOS, Windows) supporting 256 colors
**Project Type**: Single Rust CLI application
**Performance Goals**:
- Startup to browsing: <3 seconds
- Directory scan (1000 files): <10 seconds
- UI refresh rate: ≥10 fps
- Manual move: ≤5 keystrokes

**Constraints**:
- Minimum terminal size: 80x24 characters
- Memory: Handle 10,000+ file nodes efficiently
- No network operations

**Scale/Scope**: Single-user CLI tool for local file management

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Evidence |
|-----------|--------|----------|
| **I. Performance First** | ✅ PASS | Async directory scanning with tokio; ratatui uses efficient diffing |
| **II. User Safety & Interaction** | ✅ PASS | TUI provides visual preview before execution; confirmation dialogs for dangerous ops |
| **III. Pattern-Driven Operations** | ✅ PASS | Existing patterns in `config.rs` reused; TUI surfaces pattern matches in preview |
| **IV. Logging & Observability** | ✅ PASS | Logs to `~/.rust-jav/logs/` with timestamps; real-time log panel in TUI |
| **V. Simplicity & Single Responsibility** | ✅ PASS | Ratatui is minimal TUI framework; replaces dialoguer entirely (no dual interfaces) |

**Gate Status**: PASSED - All principles satisfied

## Project Structure

### Documentation (this feature)

```text
specs/001-lazygit-style-tui/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (internal module contracts)
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
src/
├── main.rs              # CLI entry point, TUI initialization
├── config.rs            # Pattern definitions (existing)
├── file_utils/          # Existing file operations
│   ├── mod.rs
│   ├── move_files.rs
│   ├── rename_files.rs
│   ├── rename_files_async.rs
│   ├── delete_files.rs
│   └── create_dir.rs
└── tui/                 # NEW: TUI module
    ├── mod.rs           # TUI exports
    ├── app.rs           # Application state machine
    ├── ui.rs            # Layout and rendering
    ├── event.rs         # Keyboard/terminal event handling
    ├── components/      # Reusable UI components
    │   ├── mod.rs
    │   ├── file_tree.rs     # Directory tree panel
    │   ├── operations.rs    # Operations list panel
    │   ├── preview.rs       # Preview panel
    │   ├── progress.rs      # Progress bars
    │   ├── log_viewer.rs    # Scrolling log window
    │   └── dialogs.rs       # Modal dialogs (confirm, move, help)
    └── state/           # State management
        ├── mod.rs
        ├── file_state.rs    # FileNode tree state
        ├── op_state.rs      # Operation selection state
        └── exec_state.rs    # Execution progress state

tests/
├── tui_tests.rs         # TUI component tests
└── integration/         # End-to-end workflow tests
```

**Structure Decision**: Single project structure maintained. New `src/tui/` module added following the existing pattern of feature modules under `src/`. The `dialoguer` dependency will be removed and replaced by the TUI module.

## Complexity Tracking

> No constitution violations. TUI replacement is a direct substitution, not added complexity.

| Aspect | Decision | Rationale |
|--------|----------|-----------|
| TUI Framework | Ratatui + Crossterm | Industry standard for Rust TUI; LazyGit uses similar approach |
| State Management | In-memory structs | Simple, no external state library needed |
| Event Loop | Single-threaded with async | Tokio-compatible; avoids threading complexity |
