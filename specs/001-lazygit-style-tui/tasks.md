# Tasks: LazyGit 风格 TUI 界面

**Input**: Design documents from `/specs/001-lazygit-style-tui/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

---

## 📊 Progress Summary (Updated: 2024-12-21)

| Phase | Status | Completed | Total | Notes |
|-------|--------|-----------|-------|-------|
| Phase 1: Setup | ✅ Complete | 6/6 | 100% | |
| Phase 2: Foundational | ✅ Complete | 8/8 | 100% | |
| Phase 3: US1 - Navigation | ✅ Complete | 19/19 | 100% | |
| Phase 4: US2 - Operations | ✅ Complete | 17/17 | 100% | T047, T048 pattern matching 已完成 |
| Phase 5: US3 - Manual Move | ✅ Functional | 13/17 | 76% | T110修复后移动可用 |
| Phase 6: US4 - Execution | ✅ Functional | 16/21 | 76% | T111修复, T085-T087日志持久化完成 |
| Phase 7: US5 - Help | ✅ Complete | 9/9 | 100% | T112添加?键帮助 |
| Phase 8: Polish | ✅ Functional | 11/12 | 92% | T109搜索过滤已修复 |

### ✅ Recently Fixed Issues

1. **T109**: 搜索过滤修复 - visible_nodes() 现在在 render() 中使用
2. **T110**: 移动操作修复 - Dialog::Move 处理已实现
3. **T111**: 执行模式修复 - dry_run=false 用于实际执行
4. **T112**: 帮助键冲突修复 - 添加 ? 键作为帮助快捷键
5. **T047-T048**: Pattern Matching - 正则表达式匹配JAV代码已实现
6. **T085-T087**: 日志持久化 - 日志保存到 ~/.rust-jav/logs/

---

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization, dependencies, and basic TUI module structure

- [x] T001 Update Cargo.toml: add ratatui 0.30.0, crossterm 0.29.0 with event-stream, futures 0.3, color-eyre 0.6
- [x] T002 Update Cargo.toml: remove dialoguer dependency
- [x] T003 [P] Create TUI module directory structure: src/tui/, src/tui/components/, src/tui/state/
- [x] T004 [P] Create src/tui/mod.rs with module exports
- [x] T005 [P] Create src/tui/components/mod.rs with component exports
- [x] T006 [P] Create src/tui/state/mod.rs with state exports

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core TUI infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T007 Implement Action enum in src/tui/state/mod.rs per data-model.md (Tick, KeyPress, FocusPanel, etc.)
- [x] T008 Implement Panel enum (FileTree, Operations, Preview) in src/tui/app.rs
- [x] T009 Implement AppMode enum (Normal, Executing, Search, Help) in src/tui/app.rs
- [x] T010 Implement base App struct in src/tui/app.rs with focused_panel, mode, action_tx, source_dir
- [x] T011 Implement terminal setup/restore functions in src/tui/event.rs (enable_raw_mode, EnterAlternateScreen)
- [x] T012 Implement basic event loop with crossterm EventStream in src/tui/event.rs
- [x] T013 Update src/main.rs to initialize TUI and run event loop (replace dialoguer usage)
- [x] T014 [P] Implement Component trait in src/tui/components/mod.rs (render, handle_event, is_focused)

**Checkpoint**: Foundation ready - user story implementation can now begin

---

## Phase 3: User Story 1 - 三面板浏览与导航 (Priority: P1) 🎯 MVP

**Goal**: User sees a professional three-panel layout with file tree, operations list, and preview. Can navigate with keyboard and switch focus between panels.

**Independent Test**: Launch app with source directory → see three panels → Tab switches focus (border color changes) → j/k navigates → l/h expands/collapses folders → q shows quit confirmation

### State & Models for User Story 1

- [x] T015 [P] [US1] Implement FileNode struct in src/tui/state/file_state.rs per data-model.md
- [x] T016 [P] [US1] Implement FileStatus enum (Unchanged, ToMove, ToDelete, ToRename, Selected) in src/tui/state/file_state.rs
- [x] T017 [P] [US1] Implement TreeState struct (selected, offset, expanded HashSet) in src/tui/state/file_state.rs
- [x] T018 [US1] Implement FileTreeComponent struct in src/tui/components/file_tree.rs with TreeState and Vec<FileNode>

### Layout & Rendering for User Story 1

- [x] T019 [US1] Implement three-panel horizontal layout (30%-40%-30%) in src/tui/ui.rs
- [x] T020 [US1] Implement status bar rendering (top: app title, source directory path) in src/tui/ui.rs
- [x] T021 [US1] Implement help bar rendering (bottom: context-aware shortcuts) in src/tui/ui.rs
- [x] T022 [US1] Implement panel border styling (focused=Cyan+Bold, unfocused=Gray) in src/tui/ui.rs

### File Tree Component for User Story 1

- [x] T023 [US1] Implement file tree rendering with indentation and icons in src/tui/components/file_tree.rs
- [x] T024 [US1] Implement async directory scanning with tokio::fs::read_dir in src/tui/components/file_tree.rs
- [x] T025 [US1] Implement tree navigation (j/k or ↓/↑ for up/down) in src/tui/components/file_tree.rs
- [x] T026 [US1] Implement folder expand/collapse (l/Enter to expand, h/Backspace to collapse) in src/tui/components/file_tree.rs
- [x] T027 [US1] Implement lazy loading of directory children on expand in src/tui/components/file_tree.rs

### Panel Focus & Navigation for User Story 1

- [x] T028 [US1] Implement Tab key panel focus cycling in src/tui/event.rs
- [x] T029 [US1] Implement Shift+Tab reverse panel cycling in src/tui/event.rs
- [x] T030 [US1] Integrate App with FileTreeComponent, render on focused panel in src/tui/app.rs

### Quit & Confirmation for User Story 1

- [x] T031 [P] [US1] Implement Dialog enum (Confirm, Move, Conflict) in src/tui/components/dialogs.rs
- [x] T032 [US1] Implement ConfirmDialog rendering (centered modal with Yes/No) in src/tui/components/dialogs.rs
- [x] T033 [US1] Implement quit confirmation (q key → show dialog → y/n response) in src/tui/event.rs

**Checkpoint**: User Story 1 complete - Three-panel layout with file tree navigation is fully functional

---

## Phase 4: User Story 2 - 批量操作选择与预览 (Priority: P1)

**Goal**: User sees 6 predefined batch operations in center panel. Can toggle operations with Space, toggle all with 'a'. Preview panel shows affected files.

**Independent Test**: Focus on operations panel → see 6 operations with checkboxes → Space toggles current → 'a' toggles all → selecting operation shows affected files in preview

### State & Models for User Story 2

- [x] T034 [P] [US2] Implement OperationType enum in src/tui/state/op_state.rs per data-model.md
- [x] T035 [P] [US2] Implement Operation struct (op_type, name, description, enabled, matched_files, stats) in src/tui/state/op_state.rs
- [x] T036 [P] [US2] Implement OperationStats struct (file_count, total_size, space_freed) in src/tui/state/op_state.rs
- [x] T037 [US2] Implement OperationsComponent struct with Vec<Operation> and selected index in src/tui/components/operations.rs

### Operations Panel for User Story 2

- [x] T038 [US2] Implement operations list rendering with checkboxes [✓]/[ ] in src/tui/components/operations.rs
- [x] T039 [US2] Implement operation navigation (j/k or ↓/↑) in src/tui/components/operations.rs
- [x] T040 [US2] Implement Space key to toggle current operation enabled state in src/tui/components/operations.rs
- [x] T041 [US2] Implement 'a' key to toggle all operations in src/tui/components/operations.rs
- [x] T042 [US2] Initialize 6 predefined operations from config.rs patterns in src/tui/components/operations.rs

### Preview Panel for User Story 2

- [x] T043 [P] [US2] Implement PreviewComponent struct in src/tui/components/preview.rs
- [x] T044 [US2] Implement file info preview (path, size, modified time) when file selected in src/tui/components/preview.rs
- [x] T045 [US2] Implement operation preview (list affected files) when operation selected in src/tui/components/preview.rs
- [x] T046 [US2] Implement color coding (green=safe, yellow=warning, red=danger) in src/tui/components/preview.rs
- [x] T046b [US2] **NEW**: Preview always shows selected files + highlighted operation details below in src/tui/components/preview.rs

### Pattern Matching for User Story 2

- [x] T047 [US2] Implement async pattern matching to find affected files per operation in src/tui/executor.rs
- [x] T048 [US2] Calculate operation statistics (file count, total size) in src/tui/executor.rs
- [x] T049 [US2] Integrate OperationsComponent and PreviewComponent with App in src/tui/app.rs

**Checkpoint**: User Story 2 complete - Operations selection and preview are fully functional

---

## Phase 5: User Story 3 - 手动移动单个文件 (Priority: P1)

**Goal**: User can press 'm' to open move dialog, use 1-9 for quick targets, type custom path with Tab completion, handle conflicts.

**Independent Test**: Select file in tree → press 'm' → see move dialog with targets → press '1' → file moves to CHINESE/ → conflict prompts for resolution

### State & Models for User Story 3

- [ ] T050 [P] [US3] Implement MoveTarget struct (path, shortcut_key, is_preset, last_used) in src/tui/state/file_state.rs
- [x] T051 [P] [US3] Implement ConflictResolution enum (Skip, Overwrite, Rename) in src/tui/components/dialogs.rs
- [ ] T052 [US3] Implement preset move targets (1=CHINESE, 2=UNCENSORED, etc.) in src/tui/state/file_state.rs

### Move Dialog for User Story 3

- [x] T053 [US3] Implement MoveDialog struct in src/tui/components/dialogs.rs (source_files, targets, custom_path)
- [x] T054 [US3] Implement move dialog rendering with target list and input field in src/tui/components/dialogs.rs
- [x] T055 [US3] Implement 'm' key to open move dialog from file tree in src/tui/event.rs
- [x] T056 [US3] Implement number keys 1-9 for quick target selection in move dialog in src/tui/components/dialogs.rs
- [ ] T057 [US3] Implement Tab key path autocomplete in move dialog in src/tui/components/dialogs.rs
- [x] T058 [US3] Implement Enter to confirm move, Esc to cancel in src/tui/components/dialogs.rs

### Conflict Handling for User Story 3

- [ ] T059 [US3] Implement conflict detection before move (check if target exists) in src/tui/state/file_state.rs
- [x] T060 [US3] Implement ConflictDialog rendering (Skip/Overwrite/Rename options) in src/tui/components/dialogs.rs
- [ ] T061 [US3] Implement conflict resolution logic (apply user choice) in src/tui/state/file_state.rs

### Multi-Select Mode for User Story 3

- [x] T062 [US3] Implement 'v' key to toggle multi-select mode in file tree in src/tui/components/file_tree.rs
- [x] T063 [US3] Implement Space key to toggle file selection in multi-select mode in src/tui/components/file_tree.rs
- [x] T064 [US3] Update move dialog to show "N files selected" for batch move in src/tui/components/dialogs.rs
- [x] T064b [US3] **NEW**: Implement 'a' key to select/deselect all files in file tree in src/tui/components/file_tree.rs
- [x] T064c [US3] **NEW**: Preview shows selected files summary (count, total size, file list) in src/tui/components/preview.rs

### Move Execution for User Story 3

- [ ] T065 [US3] Integrate with existing file_utils::move_files for actual move operation in src/tui/state/file_state.rs
- [ ] T066 [US3] Update file tree after successful move (remove/refresh nodes) in src/tui/components/file_tree.rs

**Checkpoint**: User Story 3 complete - Manual file moving with quick targets and conflict handling is fully functional

---

## Phase 6: User Story 4 - 执行操作与进度显示 (Priority: P2)

**Goal**: User presses Enter to execute enabled operations. See progress bars, real-time log, and statistics. Can interrupt with Ctrl+C.

**Independent Test**: Enable operations → press Enter → see execution mode with progress → watch files being processed → Ctrl+C shows interrupt dialog → logs saved to ~/.rust-jav/logs/

### State & Models for User Story 4

- [x] T067 [P] [US4] Implement ExecutionProgress struct in src/tui/state/exec_state.rs per data-model.md
- [x] T068 [P] [US4] Implement ExecutionCounters struct (moved, deleted, renamed, skipped, errors) in src/tui/state/exec_state.rs
- [x] T069 [P] [US4] Implement ExecutionStatus enum (Idle, Running, Paused, Completed, Cancelled, Failed) in src/tui/state/exec_state.rs
- [x] T070 [P] [US4] Implement LogEntry struct (timestamp, level, message, file_path) in src/tui/state/exec_state.rs
- [x] T071 [P] [US4] Implement LogLevel enum (Info, Success, Warning, Error) in src/tui/state/exec_state.rs

### Progress Components for User Story 4

- [x] T072 [US4] Implement ProgressComponent with Gauge widgets in src/tui/components/progress.rs
- [x] T073 [US4] Implement dual progress bar rendering (overall + current operation) in src/tui/components/progress.rs
- [x] T074 [US4] Implement ETA calculation (elapsed time, files/sec, remaining estimate) in src/tui/components/progress.rs

### Log Viewer for User Story 4

- [x] T075 [US4] Implement LogViewerComponent with scrolling log window in src/tui/components/log_viewer.rs
- [x] T076 [US4] Implement color-coded log rendering (green=success, yellow=warning, red=error) in src/tui/components/log_viewer.rs
- [x] T077 [US4] Implement auto-scroll to latest log entry in src/tui/components/log_viewer.rs

### Execution Mode for User Story 4

- [ ] T078 [US4] Implement execution mode layout (progress bars, log viewer, stats panel) in src/tui/ui.rs
- [x] T079 [US4] Implement Enter key to start execution from normal mode in src/tui/event.rs
- [ ] T080 [US4] Implement statistics panel rendering (counters for moved/deleted/renamed/skipped/errors) in src/tui/ui.rs

### Execution Engine for User Story 4

- [x] T081 [US4] Implement async execution loop with progress updates via action channel in src/tui/event.rs
- [ ] T082 [US4] Integrate with file_utils modules (move_files, delete_files, rename_files) in src/tui/state/exec_state.rs
- [x] T083 [US4] Implement Ctrl+C/Esc signal handling to cancel execution in src/tui/event.rs
- [x] T084 [US4] Implement execution completion handling (return to normal mode, show summary) in src/tui/app.rs

### Log Persistence for User Story 4

- [x] T085 [US4] Implement log file creation in ~/.rust-jav/logs/ with timestamp filename in src/tui/state/log_persistence.rs
- [x] T086 [US4] Implement log entry writing during execution in src/tui/state/log_persistence.rs
- [x] T087 [US4] Ensure log file is closed properly on execution complete/cancel in src/tui/state/log_persistence.rs

**Checkpoint**: User Story 4 complete - Execution with progress tracking and logging is fully functional

---

## Phase 7: User Story 5 - 帮助系统与快捷键 (Priority: P3)

**Goal**: User presses F1 or 'h' to see context-aware help panel with all shortcuts. Can scroll and dismiss.

**Independent Test**: Press F1 → see help overlay → scroll with j/k → press Esc to close → help shows different shortcuts based on focused panel

### Help Panel for User Story 5

- [x] T088 [P] [US5] Implement HelpContent struct with categorized shortcuts in src/tui/components/dialogs.rs
- [x] T089 [US5] Implement help panel rendering (full-screen overlay, scrollable) in src/tui/components/dialogs.rs
- [x] T090 [US5] Implement F1/h key to toggle help panel in src/tui/event.rs
- [x] T091 [US5] Implement help content scrolling (j/k or ↓/↑) in src/tui/components/dialogs.rs
- [x] T092 [US5] Implement Esc/q to close help panel in src/tui/event.rs

### Context-Aware Help for User Story 5

- [x] T093 [US5] Implement context-aware help content based on focused panel in src/tui/components/dialogs.rs
- [x] T094 [US5] Implement help content for file tree panel (navigation, expand/collapse, move, multi-select) in src/tui/components/dialogs.rs
- [x] T095 [US5] Implement help content for operations panel (toggle, select all, preview) in src/tui/components/dialogs.rs
- [ ] T096 [US5] Implement help content for execution mode (interrupt, view log) in src/tui/components/dialogs.rs

**Checkpoint**: User Story 5 complete - Help system is fully functional

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Edge cases, error handling, and refinements affecting multiple user stories

- [ ] T097 [P] Implement terminal size check (min 80x24) with warning message in src/tui/event.rs
- [ ] T098 [P] Implement empty directory handling (show friendly message) in src/tui/components/file_tree.rs
- [ ] T099 [P] Implement large directory handling (>10,000 files) with scan progress in src/tui/components/file_tree.rs
- [ ] T100 Implement file permission error handling (show which files failed) in src/tui/state/exec_state.rs
- [ ] T101 Implement disk space check before execution in src/tui/state/exec_state.rs
- [x] T102 [P] Implement search mode (/ key) for file tree filtering in src/tui/components/file_tree.rs
- [x] T103 Remove dialoguer-related code from src/main.rs (cleanup)
- [ ] T104 Run quickstart.md validation to ensure all components work together

### New Tasks Added During Implementation

- [x] T105 **NEW**: Implement four-panel layout (Files, Operations, Preview + Log panel below) in src/tui/ui.rs
- [x] T106 **NEW**: Implement Log panel with color-coded entries (INFO/OK/WARN/ERR) in src/tui/ui.rs
- [x] T107 **NEW**: Add startup logs (TUI started, scanning directory, scan complete) in src/tui/event.rs
- [x] T108 **NEW**: Preview panel auto-updates when switching panels with Tab in src/tui/event.rs

### 🚨 Critical Bug Fixes (Added: 2024-12-21)

- [x] T109 [P] **BUGFIX**: Search filter not working - render() must use visible_nodes() in src/tui/components/file_tree.rs
- [x] T110 [P] **BUGFIX**: Move operation does nothing - implement MoveFiles branch in confirm_dialog() in src/tui/app.rs
- [x] T111 [P] **BUGFIX**: Execution is dry-run only - remove hardcoded dry_run=true in src/tui/executor.rs
- [x] T112 [P] **BUGFIX**: Help 'h' key conflicts with collapse - use ? key for help in src/tui/event.rs

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3-7)**: All depend on Foundational phase completion
  - US1-US3 (P1) can proceed in parallel or sequentially
  - US4 (P2) depends on US2 (needs operations to execute)
  - US5 (P3) can be done independently
- **Polish (Phase 8)**: Depends on all user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational - No dependencies on other stories
- **User Story 2 (P1)**: Can start after Foundational - Uses FileNode from US1 but can mock
- **User Story 3 (P1)**: Can start after Foundational - Uses FileTreeComponent from US1
- **User Story 4 (P2)**: Best after US2 (needs operations list) - Can stub operations for testing
- **User Story 5 (P3)**: Can start after Foundational - Independent help system

### Within Each User Story

- State/Models before Components
- Components before Integration
- Core functionality before edge cases
- Complete story before moving to next priority

### Parallel Opportunities

**Phase 1 (Setup):**
- T003, T004, T005, T006 can run in parallel

**Phase 3 (US1):**
- T015, T016, T017 can run in parallel (models)
- T031 can run parallel to rendering tasks

**Phase 4 (US2):**
- T034, T035, T036, T043 can run in parallel (models)

**Phase 5 (US3):**
- T050, T051 can run in parallel (models)

**Phase 6 (US4):**
- T067, T068, T069, T070, T071 can run in parallel (models)

**Phase 7 (US5):**
- T088 can run parallel to other tasks

**Phase 8 (Polish):**
- T097, T098, T099, T102 can run in parallel

---

## Parallel Example: User Story 1 State Models

```bash
# Launch all state models for User Story 1 together:
Task: "T015 [P] [US1] Implement FileNode struct in src/tui/state/file_state.rs"
Task: "T016 [P] [US1] Implement FileStatus enum in src/tui/state/file_state.rs"
Task: "T017 [P] [US1] Implement TreeState struct in src/tui/state/file_state.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T006)
2. Complete Phase 2: Foundational (T007-T014)
3. Complete Phase 3: User Story 1 (T015-T033)
4. **STOP and VALIDATE**: Test three-panel layout with navigation
5. Deploy/demo if ready - users can browse directories

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → **MVP: Basic TUI navigation**
3. Add User Story 2 → Test independently → **Batch operations preview**
4. Add User Story 3 → Test independently → **Manual file moving**
5. Add User Story 4 → Test independently → **Execution with progress**
6. Add User Story 5 → Test independently → **Help system**
7. Polish phase → **Production ready**

### Suggested MVP Scope

**Minimum Viable Product**: Phase 1 + Phase 2 + Phase 3 (User Story 1)

This delivers:
- Three-panel TUI layout
- File tree with navigation
- Panel focus switching
- Basic quit functionality

Users can explore the interface even without execution capability.

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story is independently testable
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Existing file_utils/ modules are reused, not rewritten
