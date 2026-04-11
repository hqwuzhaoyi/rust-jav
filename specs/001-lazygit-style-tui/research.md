# Research: LazyGit 风格 TUI 界面

**Date**: 2025-12-07
**Branch**: `001-lazygit-style-tui`

## Executive Summary

Research findings for building a Ratatui-based TUI application with three-panel layout, async file operations, and LazyGit-style navigation.

## 1. Dependency Decisions

### Core TUI Dependencies

| Decision | Choice | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| TUI Framework | `ratatui 0.30.0` | cursive, tui-rs | Industry standard, immediate mode rendering, excellent documentation |
| Terminal Backend | `crossterm 0.29.0` | termion, termwiz | Cross-platform (Windows/Mac/Linux), async event support |
| Event Stream | `crossterm` with `event-stream` feature | manual polling | Native async support via `EventStream`, integrates with tokio |

### Updated Cargo.toml Dependencies

```toml
[dependencies]
# Core TUI (NEW)
ratatui = "0.30.0"
crossterm = { version = "0.29.0", features = ["event-stream"] }
futures = "0.3"

# Error handling (NEW)
color-eyre = "0.6"

# Existing (KEEP)
tokio = { version = "1", features = ["full"] }
clap = { version = "4.4.7", features = ["derive"] }
log = "0.4"
env_logger = "0.9"
regex = "1"
once_cell = "1.18.0"
async-recursion = "1.0.5"
indicatif = { version = "0.17.8", features = ["rayon"] }

# REMOVE
# dialoguer = "0.11.0"  # Replaced by TUI
```

## 2. Architecture Decisions

### Rendering Model

| Decision | Immediate Mode Rendering |
|----------|--------------------------|
| Rationale | Ratatui's core design; rebuild widget tree each frame, framework handles diffing |
| Trade-off | More control, simpler state vs. manual render management |

### Component Pattern

| Decision | Component trait + StatefulWidget pattern |
|----------|------------------------------------------|
| Rationale | Composable, testable, separates state from rendering |
| Pattern | Each panel is a Component with `render()` and `handle_event()` methods |

```rust
pub trait Component {
    fn render(&mut self, f: &mut Frame, area: Rect);
    fn handle_event(&mut self, event: &Event) -> bool;
}
```

### Event Loop Architecture

| Decision | Tokio async with action channel |
|----------|--------------------------------|
| Rationale | Non-blocking file operations, clean async integration |
| Pattern | `tokio::select!` on EventStream + tick interval + action channel |

```rust
pub enum Action {
    Tick,
    KeyPress(KeyEvent),
    FileOperationComplete(Result<(), Error>),
    ProgressUpdate { processed: u64, current_file: String },
}
```

## 3. Layout Design

### Three-Panel Layout

| Panel | Width | Content | Focus Key |
|-------|-------|---------|-----------|
| Left | 30% | Directory tree | Tab (cycle) |
| Center | 40% | Operations list | Tab (cycle) |
| Right | 30% | Preview/details | Tab (cycle) |

```rust
let panel_chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Percentage(30),  // File tree
        Constraint::Percentage(40),  // Operations
        Constraint::Percentage(30),  // Preview
    ])
    .spacing(1)
    .split(main_area);
```

### Focus Indication

- Focused panel: Cyan border, bold title
- Unfocused panel: Default gray border

## 4. File Tree State Management

### State Separation Pattern

| Decision | Split TreeState (UI) from FileNode (data) |
|----------|-------------------------------------------|
| Rationale | Separation of concerns; UI state (cursor, scroll) independent of file data |

```rust
pub struct TreeState {
    selected: Option<usize>,
    offset: usize,
    expanded: HashSet<PathBuf>,
}

pub struct FileNode {
    path: PathBuf,
    name: String,
    is_dir: bool,
    size: u64,
    children: Vec<FileNode>,
    depth: usize,
    pending_operation: Option<OperationType>,
}
```

### Lazy Loading Strategy

| Decision | Load children on expand |
|----------|------------------------|
| Rationale | Fast startup, low memory for large directories |
| Pattern | Only root visible initially; `tokio::spawn` loads children async |

## 5. Keyboard Navigation

### Vim-Style Bindings

| Key | Action | Context |
|-----|--------|---------|
| `j` / `↓` | Move down | All panels |
| `k` / `↑` | Move up | All panels |
| `l` / `→` / `Enter` | Expand/Enter | File tree |
| `h` / `←` / `Backspace` | Collapse/Back | File tree |
| `Tab` | Next panel | Global |
| `Shift+Tab` | Previous panel | Global |
| `Space` | Toggle selection | Operations panel |
| `a` | Toggle all | Operations panel |
| `m` | Move dialog | File tree |
| `v` | Multi-select mode | File tree |
| `q` | Quit (with confirm) | Global |
| `F1` / `h` | Help panel | Global (when not in tree) |

## 6. Progress Bar Integration

### Pattern: Gauge with Action Updates

| Decision | Use Gauge/LineGauge widgets with progress via action channel |
|----------|-------------------------------------------------------------|
| Rationale | Real-time updates without blocking render loop |

```rust
pub struct ExecutionState {
    total_files: u64,
    processed_files: u64,
    current_operation: String,
    current_file: String,
    start_time: Instant,
}
```

Update frequency: 10 updates/sec minimum for smooth visual feedback.

## 7. Error Handling

| Decision | Use `color-eyre` |
|----------|-----------------|
| Rationale | Auto-restores terminal before displaying errors; better backtraces |
| Alternative | `anyhow` (lighter but requires manual terminal restore) |

## 8. Testing Strategy

| Aspect | Approach |
|--------|----------|
| Unit tests | `TestBackend` for rendering tests |
| Snapshot tests | `insta` crate for UI regression detection |
| Integration | End-to-end with test directories |

## 9. Key Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Large directory performance | Lazy loading, async scan, virtual scrolling |
| UI blocking during file ops | Always use `tokio::spawn`, action channel updates |
| Terminal state corruption | `color-eyre` panic hooks, cleanup on all exit paths |

## Sources

- [Ratatui Installation Guide](https://ratatui.rs/installation/)
- [Ratatui Async Counter Tutorial](https://ratatui.rs/tutorials/counter-async-app/)
- [Ratatui Component Architecture](https://ratatui.rs/concepts/application-patterns/component-architecture/)
- [Crossterm 0.29.0 Documentation](https://docs.rs/crate/crossterm/latest)
- [LazyGit Panel Architecture](https://www.oliverguenther.de/2021/04/lazygit-the-files-panel/)
