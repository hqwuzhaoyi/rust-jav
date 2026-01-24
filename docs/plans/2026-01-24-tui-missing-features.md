# TUI 缺失功能补全实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 补全 TUI 重构中缺失的关键功能，包括冲突检测、路径补全、执行模式布局等

**Architecture:** 采用 DDD (Domain-Driven Design) 方法，每个功能模块先定义领域模型，再实现应用层逻辑，最后集成到 UI 层

**Tech Stack:** Rust 1.75+, ratatui 0.30.0, crossterm 0.29.0, tokio

---

## 功能模块概览

| 模块 | 优先级 | 任务数 | 状态 |
|------|--------|--------|------|
| Module 1: 冲突检测与解决 | P0 | 6 | 待实现 |
| Module 2: 移动后刷新 | P0 | 3 | 待实现 |
| Module 3: Tab 路径补全 | P1 | 4 | 待实现 |
| Module 4: 执行模式布局 | P1 | 4 | 待实现 |
| Module 5: 大目录处理 | P2 | 3 | 待实现 |

---

## Module 1: 冲突检测与解决 (P0)

### DDD 领域分析

**领域概念:**
- `ConflictType`: 冲突类型 (文件已存在、权限不足、磁盘空间不足)
- `ConflictResolution`: 解决策略 (跳过、覆盖、重命名)
- `MoveConflict`: 冲突实体，包含源文件、目标路径、冲突类型
- `ConflictChecker`: 领域服务，检测移动操作的潜在冲突

**聚合根:** `MoveOperation` - 包含源文件列表、目标路径、冲突列表

**领域事件:**
- `ConflictDetected`: 检测到冲突时触发
- `ConflictResolved`: 用户选择解决方案后触发

---

### Task 1.1: 定义冲突领域模型

**Files:**
- Create: `src/tui/state/conflict.rs`
- Modify: `src/tui/state/mod.rs`
- Test: `tests/tui_conflict_tests.rs`

**Step 1: Write the failing test**

```rust
// tests/tui_conflict_tests.rs
use rust_jav::tui::state::conflict::{ConflictType, ConflictResolution, MoveConflict};
use std::path::PathBuf;

#[test]
fn test_move_conflict_new() {
    let source = PathBuf::from("/test/source.mp4");
    let target = PathBuf::from("/test/target.mp4");
    let conflict = MoveConflict::new(source.clone(), target.clone(), ConflictType::FileExists);

    assert_eq!(conflict.source, source);
    assert_eq!(conflict.target, target);
    assert!(matches!(conflict.conflict_type, ConflictType::FileExists));
    assert!(conflict.resolution.is_none());
}

#[test]
fn test_move_conflict_resolve() {
    let source = PathBuf::from("/test/source.mp4");
    let target = PathBuf::from("/test/target.mp4");
    let mut conflict = MoveConflict::new(source, target, ConflictType::FileExists);

    conflict.resolve(ConflictResolution::Skip);
    assert!(matches!(conflict.resolution, Some(ConflictResolution::Skip)));
}

#[test]
fn test_conflict_resolution_variants() {
    assert!(matches!(ConflictResolution::Skip, ConflictResolution::Skip));
    assert!(matches!(ConflictResolution::Overwrite, ConflictResolution::Overwrite));
    assert!(matches!(ConflictResolution::Rename, ConflictResolution::Rename));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test tui_conflict_tests --no-run 2>&1 | head -20`
Expected: Compilation error - module `conflict` not found

**Step 3: Write minimal implementation**

```rust
// src/tui/state/conflict.rs
use std::path::PathBuf;

/// 冲突类型
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictType {
    /// 目标文件已存在
    FileExists,
    /// 权限不足
    PermissionDenied,
    /// 磁盘空间不足
    InsufficientSpace,
}

/// 冲突解决策略
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictResolution {
    /// 跳过此文件
    Skip,
    /// 覆盖目标文件
    Overwrite,
    /// 重命名源文件
    Rename,
}

/// 移动冲突实体
#[derive(Debug, Clone)]
pub struct MoveConflict {
    /// 源文件路径
    pub source: PathBuf,
    /// 目标文件路径
    pub target: PathBuf,
    /// 冲突类型
    pub conflict_type: ConflictType,
    /// 用户选择的解决方案
    pub resolution: Option<ConflictResolution>,
}

impl MoveConflict {
    /// 创建新的冲突实例
    pub fn new(source: PathBuf, target: PathBuf, conflict_type: ConflictType) -> Self {
        Self {
            source,
            target,
            conflict_type,
            resolution: None,
        }
    }

    /// 设置解决方案
    pub fn resolve(&mut self, resolution: ConflictResolution) {
        self.resolution = Some(resolution);
    }
}
```

**Step 4: Update mod.rs**

```rust
// Add to src/tui/state/mod.rs
pub mod conflict;
pub use conflict::{ConflictType, ConflictResolution, MoveConflict};
```

**Step 5: Run test to verify it passes**

Run: `cargo test tui_conflict_tests -v`
Expected: All 3 tests PASS

**Step 6: Commit**

```bash
git add src/tui/state/conflict.rs src/tui/state/mod.rs tests/tui_conflict_tests.rs
git commit -m "feat(tui): add conflict domain model (T059)"
```

---

### Task 1.2: 实现冲突检测服务

**Files:**
- Modify: `src/tui/state/conflict.rs`
- Test: `tests/tui_conflict_tests.rs`

**Step 1: Write the failing test**

```rust
// Add to tests/tui_conflict_tests.rs
use rust_jav::tui::state::conflict::ConflictChecker;
use tempfile::TempDir;
use std::fs::File;

#[test]
fn test_conflict_checker_no_conflict() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source.mp4");
    let target = temp.path().join("target.mp4");

    File::create(&source).unwrap();
    // target does not exist

    let checker = ConflictChecker::new();
    let result = checker.check(&source, &target);

    assert!(result.is_none());
}

#[test]
fn test_conflict_checker_file_exists() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source.mp4");
    let target = temp.path().join("target.mp4");

    File::create(&source).unwrap();
    File::create(&target).unwrap(); // target exists

    let checker = ConflictChecker::new();
    let result = checker.check(&source, &target);

    assert!(result.is_some());
    let conflict = result.unwrap();
    assert!(matches!(conflict.conflict_type, ConflictType::FileExists));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_conflict_checker -v`
Expected: FAIL - ConflictChecker not found

**Step 3: Write minimal implementation**

```rust
// Add to src/tui/state/conflict.rs
use std::path::Path;

/// 冲突检测服务
pub struct ConflictChecker;

impl ConflictChecker {
    pub fn new() -> Self {
        Self
    }

    /// 检测移动操作是否存在冲突
    pub fn check(&self, source: &Path, target: &Path) -> Option<MoveConflict> {
        // 检查目标文件是否已存在
        if target.exists() {
            return Some(MoveConflict::new(
                source.to_path_buf(),
                target.to_path_buf(),
                ConflictType::FileExists,
            ));
        }

        // 检查目标目录是否可写
        if let Some(parent) = target.parent() {
            if parent.exists() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if let Ok(meta) = parent.metadata() {
                        let mode = meta.mode();
                        if mode & 0o200 == 0 {
                            return Some(MoveConflict::new(
                                source.to_path_buf(),
                                target.to_path_buf(),
                                ConflictType::PermissionDenied,
                            ));
                        }
                    }
                }
            }
        }

        None
    }

    /// 批量检测多个文件的冲突
    pub fn check_batch(&self, moves: &[(PathBuf, PathBuf)]) -> Vec<MoveConflict> {
        moves.iter()
            .filter_map(|(src, dst)| self.check(src, dst))
            .collect()
    }
}

impl Default for ConflictChecker {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test test_conflict_checker -v`
Expected: All tests PASS

**Step 5: Commit**

```bash
git add src/tui/state/conflict.rs tests/tui_conflict_tests.rs
git commit -m "feat(tui): add ConflictChecker service (T059)"
```

---

### Task 1.3: 集成冲突检测到移动对话框

**Files:**
- Modify: `src/tui/app.rs`
- Modify: `src/tui/event.rs`

**Step 1: Update Dialog::Conflict to use MoveConflict**

```rust
// In src/tui/app.rs, update Dialog enum
use crate::tui::state::conflict::MoveConflict;

pub enum Dialog {
    Confirm { title: String, message: String, on_confirm: DialogAction },
    Help { scroll_offset: usize },
    Move { source_files: Vec<PathBuf>, selected_target: Option<usize>, custom_path: String },
    Conflict { conflict: MoveConflict, selected_option: usize },
}
```

**Step 2: Modify execute_move_files to check conflicts**

```rust
// In src/tui/app.rs
use crate::tui::state::conflict::ConflictChecker;

impl App {
    pub fn execute_move_files(&mut self, source_files: Vec<PathBuf>, target_path: PathBuf) {
        let checker = ConflictChecker::new();

        let moves: Vec<_> = source_files.iter()
            .filter_map(|src| {
                src.file_name().map(|name| (src.clone(), target_path.join(name)))
            })
            .collect();

        let conflicts = checker.check_batch(&moves);

        if !conflicts.is_empty() {
            let conflict = conflicts.into_iter().next().unwrap();
            self.dialog = Some(Dialog::Conflict {
                conflict,
                selected_option: 0,
            });
            return;
        }

        self.do_move_files(source_files, target_path);
    }
}
```

**Step 3: Run existing tests**

Run: `cargo test`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add src/tui/app.rs
git commit -m "feat(tui): integrate conflict detection into move dialog (T059)"
```

---

### Task 1.4: 实现冲突解决逻辑

**Files:**
- Modify: `src/tui/state/conflict.rs`
- Test: `tests/tui_conflict_tests.rs`

**Step 1: Write the failing test**

```rust
// Add to tests/tui_conflict_tests.rs
use rust_jav::tui::state::conflict::ConflictResolver;

#[test]
fn test_conflict_resolver_skip() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source.mp4");
    let target = temp.path().join("target.mp4");

    File::create(&source).unwrap();
    File::create(&target).unwrap();

    let resolver = ConflictResolver::new();
    let result = resolver.resolve_skip(&source, &target);

    assert!(result.is_ok());
    assert!(source.exists()); // source unchanged
    assert!(target.exists()); // target unchanged
}

#[test]
fn test_conflict_resolver_overwrite() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source.mp4");
    let target = temp.path().join("target.mp4");

    std::fs::write(&source, b"source content").unwrap();
    std::fs::write(&target, b"target content").unwrap();

    let resolver = ConflictResolver::new();
    let result = resolver.resolve_overwrite(&source, &target);

    assert!(result.is_ok());
    assert!(!source.exists()); // source moved
    assert!(target.exists());
    assert_eq!(std::fs::read(&target).unwrap(), b"source content");
}

#[test]
fn test_conflict_resolver_rename() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source.mp4");
    let target = temp.path().join("target.mp4");

    std::fs::write(&source, b"source content").unwrap();
    std::fs::write(&target, b"target content").unwrap();

    let resolver = ConflictResolver::new();
    let (result, new_path) = resolver.resolve_rename(&source, &target);

    assert!(result.is_ok());
    assert!(!source.exists());
    assert!(target.exists()); // original target unchanged
    assert!(new_path.exists()); // renamed file exists
    assert!(new_path.to_string_lossy().contains("_1"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_conflict_resolver -v`
Expected: FAIL - ConflictResolver not found

**Step 3: Write minimal implementation**

```rust
// Add to src/tui/state/conflict.rs
use std::io;

/// 冲突解决执行器
pub struct ConflictResolver;

impl ConflictResolver {
    pub fn new() -> Self {
        Self
    }

    /// 跳过 - 不执行任何操作
    pub fn resolve_skip(&self, _source: &Path, _target: &Path) -> io::Result<()> {
        Ok(())
    }

    /// 覆盖 - 删除目标后移动源文件
    pub fn resolve_overwrite(&self, source: &Path, target: &Path) -> io::Result<()> {
        if target.exists() {
            std::fs::remove_file(target)?;
        }
        std::fs::rename(source, target)?;
        Ok(())
    }

    /// 重命名 - 给源文件添加后缀后移动
    pub fn resolve_rename(&self, source: &Path, target: &Path) -> (io::Result<()>, PathBuf) {
        let new_target = self.generate_unique_name(target);
        let result = std::fs::rename(source, &new_target);
        (result, new_target)
    }

    /// 生成唯一文件名 (添加 _1, _2 等后缀)
    fn generate_unique_name(&self, path: &Path) -> PathBuf {
        let stem = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let parent = path.parent().unwrap_or(Path::new("."));

        let mut counter = 1;
        loop {
            let new_name = if ext.is_empty() {
                format!("{}_{}", stem, counter)
            } else {
                format!("{}_{}.{}", stem, counter, ext)
            };
            let new_path = parent.join(&new_name);
            if !new_path.exists() {
                return new_path;
            }
            counter += 1;
        }
    }
}

impl Default for ConflictResolver {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 4: Update mod.rs export**

```rust
// In src/tui/state/mod.rs
pub use conflict::{ConflictType, ConflictResolution, MoveConflict, ConflictChecker, ConflictResolver};
```

**Step 5: Run test to verify it passes**

Run: `cargo test test_conflict_resolver -v`
Expected: All 3 tests PASS

**Step 6: Commit**

```bash
git add src/tui/state/conflict.rs src/tui/state/mod.rs tests/tui_conflict_tests.rs
git commit -m "feat(tui): add ConflictResolver for handling conflicts (T061)"
```

---

### Task 1.5: 更新冲突对话框渲染

**Files:**
- Modify: `src/tui/components/dialogs.rs`

**Step 1: Update render_conflict_dialog function**

```rust
// In src/tui/components/dialogs.rs
use crate::tui::state::conflict::{MoveConflict, ConflictType};

pub fn render_conflict_dialog(
    f: &mut Frame,
    conflict: &MoveConflict,
    selected_option: usize,
) {
    let area = centered_rect(60, 50, f.area());

    let conflict_msg = match &conflict.conflict_type {
        ConflictType::FileExists => "Target file already exists",
        ConflictType::PermissionDenied => "Permission denied",
        ConflictType::InsufficientSpace => "Insufficient disk space",
    };

    let options = vec![
        ("1", "Skip", "Do not move this file"),
        ("2", "Overwrite", "Replace existing file"),
        ("3", "Rename", "Add suffix to filename"),
    ];

    let items: Vec<ListItem> = options.iter().enumerate()
        .map(|(i, (key, label, desc))| {
            let style = if i == selected_option {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let marker = if i == selected_option { "▶ " } else { "  " };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{}[{}] ", marker, key), style),
                Span::styled(*label, style),
                Span::styled(format!(" - {}", desc), Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let block = Block::default()
        .title(" Conflict Detected ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(conflict_msg, Style::default().fg(Color::Red))),
        Line::from(""),
        Line::from(vec![
            Span::raw("Source: "),
            Span::styled(
                conflict.source.display().to_string(),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::raw("Target: "),
            Span::styled(
                conflict.target.display().to_string(),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
        Line::from("Choose an action:"),
        Line::from(""),
    ];

    let paragraph = Paragraph::new(content).block(block.clone());
    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);

    // Render options below the message
    let options_area = Rect {
        x: area.x + 2,
        y: area.y + 9,
        width: area.width - 4,
        height: 4,
    };
    let list = List::new(items);
    f.render_widget(list, options_area);
}
```

**Step 2: Update render_dialog to use new function**

```rust
// In src/tui/components/dialogs.rs, update render_dialog
pub fn render_dialog(f: &mut Frame, dialog: &Dialog) {
    match dialog {
        Dialog::Confirm { title, message, .. } => {
            render_confirm_dialog(f, title, message);
        }
        Dialog::Help { scroll_offset } => {
            render_help_dialog(f, *scroll_offset, &Panel::FileTree);
        }
        Dialog::Move { source_files, selected_target, custom_path } => {
            render_move_dialog(f, source_files, *selected_target, custom_path);
        }
        Dialog::Conflict { conflict, selected_option } => {
            render_conflict_dialog(f, conflict, *selected_option);
        }
    }
}
```

**Step 3: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 4: Commit**

```bash
git add src/tui/components/dialogs.rs
git commit -m "feat(tui): update conflict dialog rendering (T060)"
```

---

### Task 1.6: 处理冲突对话框确认

**Files:**
- Modify: `src/tui/app.rs`
- Modify: `src/tui/event.rs`

**Step 1: Update confirm_dialog for Conflict**

```rust
// In src/tui/app.rs, update confirm_dialog method
use crate::tui::state::conflict::{ConflictResolution, ConflictResolver};

impl App {
    pub fn confirm_dialog(&mut self) {
        let dialog = self.dialog.take();
        match dialog {
            Some(Dialog::Conflict { conflict, selected_option }) => {
                let resolver = ConflictResolver::new();
                let resolution = match selected_option {
                    0 => ConflictResolution::Skip,
                    1 => ConflictResolution::Overwrite,
                    _ => ConflictResolution::Rename,
                };

                match resolution {
                    ConflictResolution::Skip => {
                        self.add_log(LogEntry::warning(format!(
                            "Skipped: {}",
                            conflict.source.display()
                        )));
                    }
                    ConflictResolution::Overwrite => {
                        match resolver.resolve_overwrite(&conflict.source, &conflict.target) {
                            Ok(_) => {
                                self.add_log(LogEntry::success(format!(
                                    "Overwritten: {} -> {}",
                                    conflict.source.display(),
                                    conflict.target.display()
                                )));
                            }
                            Err(e) => {
                                self.add_log(LogEntry::error(format!(
                                    "Failed to overwrite: {}",
                                    e
                                )));
                            }
                        }
                    }
                    ConflictResolution::Rename => {
                        let (result, new_path) = resolver.resolve_rename(
                            &conflict.source,
                            &conflict.target
                        );
                        match result {
                            Ok(_) => {
                                self.add_log(LogEntry::success(format!(
                                    "Renamed: {} -> {}",
                                    conflict.source.display(),
                                    new_path.display()
                                )));
                            }
                            Err(e) => {
                                self.add_log(LogEntry::error(format!(
                                    "Failed to rename: {}",
                                    e
                                )));
                            }
                        }
                    }
                }
            }
            // ... existing dialog handling
            _ => {}
        }
    }
}
```

**Step 2: Run cargo test**

Run: `cargo test`
Expected: All tests PASS

**Step 3: Manual test**

Run: `cargo run -- /path/to/test/dir`
Test: Select file, press 'm', choose target with existing file, verify conflict dialog appears

**Step 4: Commit**

```bash
git add src/tui/app.rs src/tui/event.rs
git commit -m "feat(tui): handle conflict resolution in dialog (T061)"
```

---

## Module 2: 移动后刷新 (P0)

### DDD 领域分析

**领域概念:**
- `FileTreeRefresh`: 文件树刷新事件
- `MoveResult`: 移动操作结果，包含成功/失败状态和影响的路径

**领域服务:**
- `FileTreeService`: 管理文件树状态，支持增量更新

**领域事件:**
- `FileMoved`: 文件移动完成后触发
- `FileTreeUpdated`: 文件树更新完成后触发

---

### Task 2.1: 实现移动结果追踪

**Files:**
- Create: `src/tui/state/move_result.rs`
- Modify: `src/tui/state/mod.rs`
- Test: `tests/tui_move_result_tests.rs`

**Step 1: Write the failing test**

```rust
// tests/tui_move_result_tests.rs
use rust_jav::tui::state::move_result::{MoveResult, MoveStatus};
use std::path::PathBuf;

#[test]
fn test_move_result_success() {
    let source = PathBuf::from("/test/source.mp4");
    let target = PathBuf::from("/test/target.mp4");
    let result = MoveResult::success(source.clone(), target.clone());

    assert!(matches!(result.status, MoveStatus::Success));
    assert_eq!(result.source, source);
    assert_eq!(result.target, Some(target));
    assert!(result.error.is_none());
}

#[test]
fn test_move_result_skipped() {
    let source = PathBuf::from("/test/source.mp4");
    let result = MoveResult::skipped(source.clone(), "User skipped");

    assert!(matches!(result.status, MoveStatus::Skipped));
    assert_eq!(result.source, source);
    assert!(result.target.is_none());
}

#[test]
fn test_move_result_failed() {
    let source = PathBuf::from("/test/source.mp4");
    let result = MoveResult::failed(source.clone(), "Permission denied");

    assert!(matches!(result.status, MoveStatus::Failed));
    assert!(result.error.is_some());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test tui_move_result -v`
Expected: FAIL - module not found

**Step 3: Write minimal implementation**

```rust
// src/tui/state/move_result.rs
use std::path::PathBuf;

/// 移动操作状态
#[derive(Debug, Clone, PartialEq)]
pub enum MoveStatus {
    Success,
    Skipped,
    Failed,
}

/// 移动操作结果
#[derive(Debug, Clone)]
pub struct MoveResult {
    pub source: PathBuf,
    pub target: Option<PathBuf>,
    pub status: MoveStatus,
    pub error: Option<String>,
}

impl MoveResult {
    pub fn success(source: PathBuf, target: PathBuf) -> Self {
        Self {
            source,
            target: Some(target),
            status: MoveStatus::Success,
            error: None,
        }
    }

    pub fn skipped(source: PathBuf, reason: impl Into<String>) -> Self {
        Self {
            source,
            target: None,
            status: MoveStatus::Skipped,
            error: Some(reason.into()),
        }
    }

    pub fn failed(source: PathBuf, error: impl Into<String>) -> Self {
        Self {
            source,
            target: None,
            status: MoveStatus::Failed,
            error: Some(error.into()),
        }
    }
}
```

**Step 4: Update mod.rs**

```rust
// Add to src/tui/state/mod.rs
pub mod move_result;
pub use move_result::{MoveResult, MoveStatus};
```

**Step 5: Run test to verify it passes**

Run: `cargo test tui_move_result -v`
Expected: All tests PASS

**Step 6: Commit**

```bash
git add src/tui/state/move_result.rs src/tui/state/mod.rs tests/tui_move_result_tests.rs
git commit -m "feat(tui): add MoveResult for tracking move operations (T065)"
```

---

### Task 2.2: 实现文件树增量刷新

**Files:**
- Modify: `src/tui/components/file_tree.rs`
- Test: `tests/tui_file_tree_tests.rs`

**Step 1: Write the failing test**

```rust
// tests/tui_file_tree_tests.rs
use rust_jav::tui::components::FileTreeComponent;
use rust_jav::tui::state::move_result::MoveResult;
use std::path::PathBuf;
use tempfile::TempDir;
use std::fs::File;

#[tokio::test]
async fn test_file_tree_remove_node() {
    let temp = TempDir::new().unwrap();
    let file1 = temp.path().join("file1.mp4");
    let file2 = temp.path().join("file2.mp4");

    File::create(&file1).unwrap();
    File::create(&file2).unwrap();

    let mut tree = FileTreeComponent::new(temp.path().to_path_buf());
    tree.scan_directory().await;

    let initial_count = tree.node_count();
    assert!(initial_count >= 2);

    // Remove file1 from tree
    tree.remove_node(&file1);

    assert_eq!(tree.node_count(), initial_count - 1);
    assert!(tree.find_node(&file1).is_none());
    assert!(tree.find_node(&file2).is_some());
}

#[tokio::test]
async fn test_file_tree_refresh_after_move() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source.mp4");
    let target_dir = temp.path().join("target");
    let target = target_dir.join("source.mp4");

    File::create(&source).unwrap();
    std::fs::create_dir(&target_dir).unwrap();

    let mut tree = FileTreeComponent::new(temp.path().to_path_buf());
    tree.scan_directory().await;

    // Simulate move
    std::fs::rename(&source, &target).unwrap();

    let result = MoveResult::success(source.clone(), target.clone());
    tree.apply_move_result(&result);

    assert!(tree.find_node(&source).is_none());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_file_tree_remove -v`
Expected: FAIL - remove_node method not found

**Step 3: Write minimal implementation**

```rust
// Add to src/tui/components/file_tree.rs
use crate::tui::state::move_result::MoveResult;

impl FileTreeComponent {
    /// 获取节点数量
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 查找节点
    pub fn find_node(&self, path: &Path) -> Option<&FileNode> {
        self.nodes.iter().find(|n| n.path == path)
    }

    /// 从树中移除节点
    pub fn remove_node(&mut self, path: &Path) {
        if let Some(pos) = self.nodes.iter().position(|n| n.path == path) {
            self.nodes.remove(pos);
            // Adjust selected index if needed
            if self.state.selected >= self.nodes.len() && !self.nodes.is_empty() {
                self.state.selected = self.nodes.len() - 1;
            }
            self.list_state.select(Some(self.state.selected));
        }
    }

    /// 应用移动结果，更新文件树
    pub fn apply_move_result(&mut self, result: &MoveResult) {
        match result.status {
            MoveStatus::Success => {
                // Remove source from tree
                self.remove_node(&result.source);
                // Note: target might be outside our tree, so we don't add it
            }
            MoveStatus::Skipped | MoveStatus::Failed => {
                // No tree changes needed
            }
        }
    }

    /// 批量应用移动结果
    pub fn apply_move_results(&mut self, results: &[MoveResult]) {
        for result in results {
            self.apply_move_result(result);
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test test_file_tree -v`
Expected: All tests PASS

**Step 5: Commit**

```bash
git add src/tui/components/file_tree.rs tests/tui_file_tree_tests.rs
git commit -m "feat(tui): add incremental file tree refresh (T066)"
```

---

### Task 2.3: 集成移动后刷新到 App

**Files:**
- Modify: `src/tui/app.rs`

**Step 1: Update do_move_files to refresh tree**

```rust
// In src/tui/app.rs
use crate::tui::state::move_result::{MoveResult, MoveStatus};

impl App {
    /// 执行实际的文件移动操作
    fn do_move_files(&mut self, source_files: Vec<PathBuf>, target_path: PathBuf) {
        // Create target directory if needed
        if !target_path.exists() {
            if let Err(e) = std::fs::create_dir_all(&target_path) {
                self.add_log(LogEntry::error(format!(
                    "Failed to create directory: {}",
                    e
                )));
                return;
            }
        }

        let mut results = Vec::new();

        for source in source_files {
            if let Some(file_name) = source.file_name() {
                let dest = target_path.join(file_name);

                match std::fs::rename(&source, &dest) {
                    Ok(_) => {
                        self.add_log(LogEntry::success(format!(
                            "Moved: {} -> {}",
                            source.display(),
                            dest.display()
                        )));
                        results.push(MoveResult::success(source, dest));
                    }
                    Err(e) => {
                        self.add_log(LogEntry::error(format!(
                            "Failed to move {}: {}",
                            source.display(),
                            e
                        )));
                        results.push(MoveResult::failed(source, e.to_string()));
                    }
                }
            }
        }

        // Apply results to file tree (T066)
        self.file_tree.apply_move_results(&results);

        // Clear multi-select mode
        if self.file_tree.is_multi_select() {
            self.file_tree.toggle_multi_select();
        }
    }
}
```

**Step 2: Run cargo test**

Run: `cargo test`
Expected: All tests PASS

**Step 3: Manual test**

Run: `cargo run -- /path/to/test/dir`
Test: Move a file, verify it disappears from tree immediately

**Step 4: Commit**

```bash
git add src/tui/app.rs
git commit -m "feat(tui): integrate move result refresh into App (T066)"
```

---

## Module 3: Tab 路径补全 (P1)

### DDD 领域分析

**领域概念:**
- `PathCompletion`: 路径补全结果，包含匹配的路径列表
- `CompletionState`: 补全状态，追踪当前输入和候选列表

**领域服务:**
- `PathCompleter`: 根据部分路径返回匹配的目录/文件列表

**值对象:**
- `PartialPath`: 用户输入的部分路径

---

### Task 3.1: 实现路径补全服务

**Files:**
- Create: `src/tui/state/path_completion.rs`
- Modify: `src/tui/state/mod.rs`
- Test: `tests/tui_path_completion_tests.rs`

**Step 1: Write the failing test**

```rust
// tests/tui_path_completion_tests.rs
use rust_jav::tui::state::path_completion::PathCompleter;
use tempfile::TempDir;
use std::fs;

#[test]
fn test_path_completer_empty_input() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("dir1")).unwrap();
    fs::create_dir(temp.path().join("dir2")).unwrap();

    let completer = PathCompleter::new(temp.path().to_path_buf());
    let completions = completer.complete("");

    assert_eq!(completions.len(), 2);
}

#[test]
fn test_path_completer_partial_match() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("CHINESE")).unwrap();
    fs::create_dir(temp.path().join("UNCENSORED")).unwrap();
    fs::create_dir(temp.path().join("ORIGIN")).unwrap();

    let completer = PathCompleter::new(temp.path().to_path_buf());
    let completions = completer.complete("CH");

    assert_eq!(completions.len(), 1);
    assert!(completions[0].ends_with("CHINESE"));
}

#[test]
fn test_path_completer_case_insensitive() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("CHINESE")).unwrap();

    let completer = PathCompleter::new(temp.path().to_path_buf());
    let completions = completer.complete("chi");

    assert_eq!(completions.len(), 1);
}

#[test]
fn test_path_completer_nested_path() {
    let temp = TempDir::new().unwrap();
    let nested = temp.path().join("parent").join("child");
    fs::create_dir_all(&nested).unwrap();

    let completer = PathCompleter::new(temp.path().to_path_buf());
    let completions = completer.complete("parent/");

    assert_eq!(completions.len(), 1);
    assert!(completions[0].contains("child"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test tui_path_completion -v`
Expected: FAIL - module not found

**Step 3: Write minimal implementation**

```rust
// src/tui/state/path_completion.rs
use std::path::{Path, PathBuf};

/// 路径补全服务
pub struct PathCompleter {
    /// 基础目录
    base_dir: PathBuf,
}

impl PathCompleter {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// 根据部分输入返回匹配的路径列表
    pub fn complete(&self, partial: &str) -> Vec<PathBuf> {
        let (search_dir, prefix) = self.parse_partial(partial);

        let dir_to_search = if search_dir.is_empty() {
            self.base_dir.clone()
        } else {
            self.base_dir.join(&search_dir)
        };

        if !dir_to_search.exists() || !dir_to_search.is_dir() {
            return Vec::new();
        }

        let prefix_lower = prefix.to_lowercase();

        match std::fs::read_dir(&dir_to_search) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|n| n.to_lowercase().starts_with(&prefix_lower))
                        .unwrap_or(false)
                })
                .map(|e| {
                    if search_dir.is_empty() {
                        e.path()
                    } else {
                        PathBuf::from(&search_dir).join(e.file_name())
                    }
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// 解析部分路径，返回 (搜索目录, 前缀)
    fn parse_partial(&self, partial: &str) -> (String, String) {
        if let Some(pos) = partial.rfind('/') {
            let dir = &partial[..pos];
            let prefix = &partial[pos + 1..];
            (dir.to_string(), prefix.to_string())
        } else {
            (String::new(), partial.to_string())
        }
    }

    /// 获取下一个补全结果 (循环)
    pub fn next_completion(&self, partial: &str, current_idx: usize) -> Option<(PathBuf, usize)> {
        let completions = self.complete(partial);
        if completions.is_empty() {
            return None;
        }
        let next_idx = (current_idx + 1) % completions.len();
        Some((completions[next_idx].clone(), next_idx))
    }
}
```

**Step 4: Update mod.rs**

```rust
// Add to src/tui/state/mod.rs
pub mod path_completion;
pub use path_completion::PathCompleter;
```

**Step 5: Run test to verify it passes**

Run: `cargo test tui_path_completion -v`
Expected: All 4 tests PASS

**Step 6: Commit**

```bash
git add src/tui/state/path_completion.rs src/tui/state/mod.rs tests/tui_path_completion_tests.rs
git commit -m "feat(tui): add PathCompleter for tab completion (T057)"
```

---

### Task 3.2: 添加补全状态到移动对话框

**Files:**
- Modify: `src/tui/app.rs`

**Step 1: Update Dialog::Move to include completion state**

```rust
// In src/tui/app.rs, update Dialog enum
pub enum Dialog {
    Confirm { title: String, message: String, on_confirm: DialogAction },
    Help { scroll_offset: usize },
    Move {
        source_files: Vec<PathBuf>,
        selected_target: Option<usize>,
        custom_path: String,
        // New fields for tab completion
        completions: Vec<PathBuf>,
        completion_idx: usize,
    },
    Conflict { conflict: MoveConflict, selected_option: usize },
}
```

**Step 2: Update move dialog creation**

```rust
// In src/tui/event.rs, update move dialog creation
KeyCode::Char('m') => {
    if let Some(selected) = app.file_tree.selected_path() {
        let sources = if app.file_tree.is_multi_select() && app.file_tree.selected_count() > 0 {
            app.file_tree.selected_files()
        } else {
            vec![selected]
        };

        app.dialog = Some(Dialog::Move {
            source_files: sources,
            selected_target: None,
            custom_path: String::new(),
            completions: Vec::new(),
            completion_idx: 0,
        });
    }
}
```

**Step 3: Run cargo check**

Run: `cargo check`
Expected: No errors (may need to update other Dialog::Move usages)

**Step 4: Commit**

```bash
git add src/tui/app.rs src/tui/event.rs
git commit -m "feat(tui): add completion state to Move dialog (T057)"
```

---

### Task 3.3: 实现 Tab 键补全逻辑

**Files:**
- Modify: `src/tui/event.rs`

**Step 1: Add Tab key handling in move dialog**

```rust
// In src/tui/event.rs, update handle_dialog_event for Move dialog
Some(Dialog::Move { custom_path, completions, completion_idx, .. }) => match key {
    KeyCode::Tab => {
        // Trigger path completion
        let completer = PathCompleter::new(app.source_dir.clone());

        if completions.is_empty() {
            // First Tab press - get completions
            *completions = completer.complete(custom_path);
            *completion_idx = 0;
        } else {
            // Subsequent Tab - cycle through completions
            *completion_idx = (*completion_idx + 1) % completions.len();
        }

        // Apply current completion to custom_path
        if let Some(completion) = completions.get(*completion_idx) {
            *custom_path = completion.to_string_lossy().to_string();
        }
    }
    KeyCode::Char(c) => {
        custom_path.push(c);
        // Clear completions when user types
        completions.clear();
        *completion_idx = 0;
    }
    KeyCode::Backspace => {
        custom_path.pop();
        // Clear completions when user deletes
        completions.clear();
        *completion_idx = 0;
    }
    // ... rest of existing handling
}
```

**Step 2: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/tui/event.rs
git commit -m "feat(tui): implement Tab key path completion (T057)"
```

---

### Task 3.4: 更新移动对话框显示补全提示

**Files:**
- Modify: `src/tui/components/dialogs.rs`

**Step 1: Update render_move_dialog to show completions**

```rust
// In src/tui/components/dialogs.rs
pub fn render_move_dialog(
    f: &mut Frame,
    source_files: &[PathBuf],
    selected_target: Option<usize>,
    custom_path: &str,
    completions: &[PathBuf],
    completion_idx: usize,
) {
    let area = centered_rect(70, 60, f.area());

    // ... existing rendering code ...

    // Add completion hint at bottom
    if !completions.is_empty() {
        let hint_area = Rect {
            x: area.x + 2,
            y: area.y + area.height - 4,
            width: area.width - 4,
            height: 2,
        };

        let hint_text = format!(
            "Tab: {} ({}/{})",
            completions.get(completion_idx)
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            completion_idx + 1,
            completions.len()
        );

        let hint = Paragraph::new(Line::from(vec![
            Span::styled("Completions: ", Style::default().fg(Color::DarkGray)),
            Span::styled(hint_text, Style::default().fg(Color::Cyan)),
        ]));

        f.render_widget(hint, hint_area);
    } else if !custom_path.is_empty() {
        let hint_area = Rect {
            x: area.x + 2,
            y: area.y + area.height - 3,
            width: area.width - 4,
            height: 1,
        };

        let hint = Paragraph::new(Span::styled(
            "Press Tab for path completion",
            Style::default().fg(Color::DarkGray),
        ));

        f.render_widget(hint, hint_area);
    }
}
```

**Step 2: Update render_dialog call**

```rust
// In src/tui/components/dialogs.rs
Dialog::Move { source_files, selected_target, custom_path, completions, completion_idx } => {
    render_move_dialog(f, source_files, *selected_target, custom_path, completions, *completion_idx);
}
```

**Step 3: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 4: Manual test**

Run: `cargo run -- /path/to/test/dir`
Test: Press 'm', type partial path, press Tab, verify completion appears

**Step 5: Commit**

```bash
git add src/tui/components/dialogs.rs
git commit -m "feat(tui): show path completion hints in move dialog (T057)"
```

---

## Module 4: 执行模式布局 (P1)

### DDD 领域分析

**领域概念:**
- `ExecutionLayout`: 执行模式专用布局配置
- `StatisticsPanel`: 统计面板，显示操作计数器

**UI 组件:**
- `ProgressPanel`: 进度条面板
- `StatsPanel`: 统计数据面板
- `LogPanel`: 日志面板

**布局结构:**
```
┌─────────────────────────────────────────┐
│ Progress: [████████░░░░░░░░] 50%        │
│ Current: Organizing files...            │
├─────────────────────────────────────────┤
│ Statistics                              │
│ ✓ Success: 25  ⚠ Skipped: 3  ✗ Error: 1│
│ Moved: 15  Deleted: 5  Renamed: 5       │
├─────────────────────────────────────────┤
│ Log                                     │
│ [OK] Moved ABC-123.mp4 -> CHINESE/      │
│ [OK] Moved DEF-456.mp4 -> UNCENSORED/   │
│ [WARN] Skipped: file already exists     │
└─────────────────────────────────────────┘
```

---

### Task 4.1: 实现执行模式布局函数

**Files:**
- Modify: `src/tui/ui.rs`

**Step 1: Add execution mode layout function**

```rust
// In src/tui/ui.rs
use crate::tui::state::ExecutionProgress;

/// 绘制执行模式布局
fn draw_execution_mode(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Status bar
            Constraint::Length(5),  // Progress panel
            Constraint::Length(5),  // Statistics panel
            Constraint::Min(10),    // Log panel
            Constraint::Length(1),  // Help bar
        ])
        .split(f.area());

    // Status bar
    draw_status_bar(f, app, chunks[0]);

    // Progress panel
    draw_progress_panel(f, app, chunks[1]);

    // Statistics panel (T080)
    draw_statistics_panel(f, app, chunks[2]);

    // Log panel
    draw_log_panel(f, app, chunks[3]);

    // Help bar
    draw_execution_help_bar(f, chunks[4]);
}

/// 绘制进度面板
fn draw_progress_panel(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Progress ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(exec) = &app.execution {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        // Overall progress bar
        let progress = exec.percentage();
        let gauge = Gauge::default()
            .ratio(progress)
            .gauge_style(Style::default().fg(Color::Green))
            .label(format!("{}%", exec.percentage_int()));
        f.render_widget(gauge, chunks[0]);

        // Current operation
        let current_op = exec.current_operation.as_deref().unwrap_or("Idle");
        let op_text = Paragraph::new(format!("Operation: {}", current_op));
        f.render_widget(op_text, chunks[1]);

        // Current file
        let current_file = exec.current_file
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "None".to_string());
        let file_text = Paragraph::new(format!("File: {}", current_file))
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(file_text, chunks[2]);
    }
}
```

**Step 2: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/tui/ui.rs
git commit -m "feat(tui): add execution mode layout structure (T078)"
```

---

### Task 4.2: 实现统计面板

**Files:**
- Modify: `src/tui/ui.rs`

**Step 1: Add statistics panel function**

```rust
// In src/tui/ui.rs

/// 绘制统计面板 (T080)
fn draw_statistics_panel(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Statistics ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(exec) = &app.execution {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        // Row 1: Success, Skipped, Error counts
        let row1 = Line::from(vec![
            Span::styled("✓ Success: ", Style::default().fg(Color::Green)),
            Span::styled(
                format!("{:<6}", exec.success_count),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("⚠ Skipped: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{:<6}", exec.skip_count),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("✗ Error: ", Style::default().fg(Color::Red)),
            Span::styled(
                format!("{:<6}", exec.error_count),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]);
        f.render_widget(Paragraph::new(row1), chunks[0]);

        // Row 2: Progress fraction and elapsed time
        let elapsed = exec.elapsed();
        let elapsed_str = format!("{}:{:02}", elapsed.as_secs() / 60, elapsed.as_secs() % 60);

        let row2 = Line::from(vec![
            Span::raw("Processed: "),
            Span::styled(
                format!("{}/{}", exec.processed_files, exec.total_files),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  |  Elapsed: "),
            Span::styled(elapsed_str, Style::default().fg(Color::Cyan)),
        ]);
        f.render_widget(Paragraph::new(row2), chunks[1]);

        // Row 3: ETA (if we have enough data)
        if exec.processed_files > 0 && exec.total_files > exec.processed_files {
            let rate = exec.processed_files as f64 / elapsed.as_secs_f64().max(0.1);
            let remaining = exec.total_files - exec.processed_files;
            let eta_secs = (remaining as f64 / rate) as u64;
            let eta_str = format!("{}:{:02}", eta_secs / 60, eta_secs % 60);

            let row3 = Line::from(vec![
                Span::raw("ETA: "),
                Span::styled(eta_str, Style::default().fg(Color::Magenta)),
                Span::raw(format!("  |  Rate: {:.1} files/sec", rate)),
            ]);
            f.render_widget(Paragraph::new(row3), chunks[2]);
        }
    }
}
```

**Step 2: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/tui/ui.rs
git commit -m "feat(tui): add statistics panel rendering (T080)"
```

---

### Task 4.3: 实现执行模式帮助栏

**Files:**
- Modify: `src/tui/ui.rs`

**Step 1: Add execution help bar function**

```rust
// In src/tui/ui.rs

/// 绘制执行模式帮助栏
fn draw_execution_help_bar(f: &mut Frame, area: Rect) {
    let help_items = vec![
        ("Ctrl+C/Esc", "Cancel"),
        ("j/k", "Scroll log"),
        ("?", "Help"),
    ];

    let spans: Vec<Span> = help_items
        .iter()
        .flat_map(|(key, desc)| {
            vec![
                Span::styled(
                    format!(" {} ", key),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {} ", desc), Style::default().fg(Color::White)),
                Span::raw(" "),
            ]
        })
        .collect();

    let help_line = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(Color::DarkGray));

    f.render_widget(help_line, area);
}
```

**Step 2: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/tui/ui.rs
git commit -m "feat(tui): add execution mode help bar"
```

---

### Task 4.4: 集成执行模式布局到主绘制函数

**Files:**
- Modify: `src/tui/ui.rs`

**Step 1: Update draw function to use execution layout**

```rust
// In src/tui/ui.rs, update main draw function
pub fn draw(f: &mut Frame, app: &App) {
    match app.mode {
        AppMode::Executing => {
            draw_execution_mode(f, app);
        }
        _ => {
            draw_normal_mode(f, app);
        }
    }

    // Render dialog on top if present
    if let Some(ref dialog) = app.dialog {
        render_dialog(f, dialog);
    }
}

/// 绘制正常模式布局 (原有的三面板布局)
fn draw_normal_mode(f: &mut Frame, app: &App) {
    // ... existing layout code ...
}
```

**Step 2: Run cargo test**

Run: `cargo test`
Expected: All tests PASS

**Step 3: Manual test**

Run: `cargo run -- /path/to/test/dir`
Test: Enable operations, press Enter, verify execution mode layout appears

**Step 4: Commit**

```bash
git add src/tui/ui.rs
git commit -m "feat(tui): integrate execution mode layout (T078)"
```

---

## Module 5: 大目录处理 (P2)

### DDD 领域分析

**领域概念:**
- `ScanProgress`: 扫描进度，追踪已扫描文件数
- `ScanConfig`: 扫描配置，包含批次大小、最大文件数限制

**领域服务:**
- `DirectoryScanner`: 支持分批扫描和进度回调

**性能约束:**
- 单次扫描不超过 10,000 文件
- 扫描进度每 100 个文件更新一次
- 支持取消扫描

---

### Task 5.1: 实现扫描进度追踪

**Files:**
- Create: `src/tui/state/scan_progress.rs`
- Modify: `src/tui/state/mod.rs`
- Test: `tests/tui_scan_progress_tests.rs`

**Step 1: Write the failing test**

```rust
// tests/tui_scan_progress_tests.rs
use rust_jav::tui::state::scan_progress::{ScanProgress, ScanConfig};

#[test]
fn test_scan_progress_new() {
    let progress = ScanProgress::new();

    assert_eq!(progress.scanned_count, 0);
    assert_eq!(progress.total_estimate, None);
    assert!(!progress.is_complete);
    assert!(!progress.cancelled);
}

#[test]
fn test_scan_progress_update() {
    let mut progress = ScanProgress::new();

    progress.update(100, Some(1000));

    assert_eq!(progress.scanned_count, 100);
    assert_eq!(progress.total_estimate, Some(1000));
}

#[test]
fn test_scan_progress_percentage() {
    let mut progress = ScanProgress::new();
    progress.update(500, Some(1000));

    assert_eq!(progress.percentage(), Some(50));
}

#[test]
fn test_scan_config_default() {
    let config = ScanConfig::default();

    assert_eq!(config.max_files, 10_000);
    assert_eq!(config.batch_size, 100);
    assert!(config.show_progress);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test tui_scan_progress -v`
Expected: FAIL - module not found

**Step 3: Write minimal implementation**

```rust
// src/tui/state/scan_progress.rs

/// 扫描进度
#[derive(Debug, Clone, Default)]
pub struct ScanProgress {
    /// 已扫描文件数
    pub scanned_count: usize,
    /// 预估总数 (如果已知)
    pub total_estimate: Option<usize>,
    /// 是否完成
    pub is_complete: bool,
    /// 是否已取消
    pub cancelled: bool,
    /// 当前扫描的目录
    pub current_dir: Option<String>,
}

impl ScanProgress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, count: usize, total: Option<usize>) {
        self.scanned_count = count;
        if total.is_some() {
            self.total_estimate = total;
        }
    }

    pub fn complete(&mut self) {
        self.is_complete = true;
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub fn percentage(&self) -> Option<u8> {
        self.total_estimate.map(|total| {
            if total == 0 {
                100
            } else {
                ((self.scanned_count as f64 / total as f64) * 100.0) as u8
            }
        })
    }
}

/// 扫描配置
#[derive(Debug, Clone)]
pub struct ScanConfig {
    /// 最大文件数限制
    pub max_files: usize,
    /// 批次大小 (每批更新进度)
    pub batch_size: usize,
    /// 是否显示进度
    pub show_progress: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            max_files: 10_000,
            batch_size: 100,
            show_progress: true,
        }
    }
}
```

**Step 4: Update mod.rs**

```rust
// Add to src/tui/state/mod.rs
pub mod scan_progress;
pub use scan_progress::{ScanProgress, ScanConfig};
```

**Step 5: Run test to verify it passes**

Run: `cargo test tui_scan_progress -v`
Expected: All 4 tests PASS

**Step 6: Commit**

```bash
git add src/tui/state/scan_progress.rs src/tui/state/mod.rs tests/tui_scan_progress_tests.rs
git commit -m "feat(tui): add ScanProgress for large directory handling (T099)"
```

---

### Task 5.2: 实现分批扫描

**Files:**
- Modify: `src/tui/components/file_tree.rs`

**Step 1: Add batched scan method**

```rust
// In src/tui/components/file_tree.rs
use crate::tui::state::scan_progress::{ScanProgress, ScanConfig};
use tokio::sync::mpsc;

impl FileTreeComponent {
    /// 分批扫描目录，支持进度回调
    pub async fn scan_directory_with_progress(
        &mut self,
        config: ScanConfig,
        progress_tx: mpsc::Sender<ScanProgress>,
    ) {
        let mut progress = ScanProgress::new();
        let mut count = 0;

        // Clear existing nodes
        self.nodes.clear();
        self.state = TreeState::new();

        // Recursive scan with batching
        let mut stack = vec![(self.root.clone(), 0usize)];

        while let Some((dir, depth)) = stack.pop() {
            if progress.cancelled || count >= config.max_files {
                break;
            }

            progress.current_dir = Some(dir.display().to_string());

            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    if count >= config.max_files {
                        break;
                    }

                    let path = entry.path();
                    let name = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let is_dir = path.is_dir();

                    let node = FileNode::new(name, path.clone(), is_dir, depth);
                    self.nodes.push(node);
                    count += 1;

                    // Add directories to stack for recursive scan
                    if is_dir && depth < 3 {
                        stack.push((path, depth + 1));
                    }

                    // Update progress every batch_size files
                    if count % config.batch_size == 0 {
                        progress.update(count, None);
                        let _ = progress_tx.send(progress.clone()).await;
                    }
                }
            }
        }

        // Sort nodes
        self.nodes.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });

        // Final progress update
        progress.update(count, Some(count));
        progress.complete();
        let _ = progress_tx.send(progress).await;

        // Initialize list state
        if !self.nodes.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// 检查是否为大目录
    pub fn is_large_directory(&self) -> bool {
        self.nodes.len() > 1000
    }
}
```

**Step 2: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/tui/components/file_tree.rs
git commit -m "feat(tui): add batched directory scanning (T099)"
```

---

### Task 5.3: 显示扫描进度

**Files:**
- Modify: `src/tui/event.rs`
- Modify: `src/tui/ui.rs`

**Step 1: Add scan progress state to App**

```rust
// In src/tui/app.rs
use crate::tui::state::scan_progress::ScanProgress;

pub struct App {
    // ... existing fields ...
    /// 扫描进度 (用于大目录)
    pub scan_progress: Option<ScanProgress>,
}
```

**Step 2: Update run_app to show scan progress**

```rust
// In src/tui/event.rs
pub async fn run_app(terminal: &mut Tui, mut app: App) -> Result<()> {
    // Check terminal size
    let size = terminal.size()?;
    if size.width < MIN_TERMINAL_WIDTH || size.height < MIN_TERMINAL_HEIGHT {
        app.add_log(LogEntry::warning(format!(
            "Terminal size {}x{} is smaller than recommended {}x{}",
            size.width, size.height, MIN_TERMINAL_WIDTH, MIN_TERMINAL_HEIGHT
        )));
    }

    app.add_log(LogEntry::info("rust-jav TUI started"));
    app.add_log(LogEntry::info(format!(
        "Scanning directory: {}",
        app.source_dir.display()
    )));

    // Check if directory is large (quick estimate)
    let entry_count = std::fs::read_dir(&app.source_dir)
        .map(|e| e.count())
        .unwrap_or(0);

    if entry_count > 500 {
        // Large directory - use progress scanning
        app.add_log(LogEntry::info(format!(
            "Large directory detected (~{} entries), scanning with progress...",
            entry_count
        )));

        let (tx, mut rx) = tokio::sync::mpsc::channel(10);
        let config = ScanConfig::default();

        // Spawn scan task
        let source_dir = app.source_dir.clone();
        let scan_handle = tokio::spawn(async move {
            let mut tree = FileTreeComponent::new(source_dir);
            tree.scan_directory_with_progress(config, tx).await;
            tree
        });

        // Show progress while scanning
        loop {
            terminal.draw(|f| {
                draw_scan_progress(f, &app);
            })?;

            tokio::select! {
                Some(progress) = rx.recv() => {
                    app.scan_progress = Some(progress.clone());
                    if progress.is_complete {
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(50)) => {}
            }
        }

        // Get scanned tree
        app.file_tree = scan_handle.await.unwrap_or_else(|_| {
            FileTreeComponent::new(app.source_dir.clone())
        });
        app.scan_progress = None;
    } else {
        // Normal scan
        app.file_tree.scan_directory().await;
    }

    app.add_log(LogEntry::success("Directory scan complete"));

    // ... rest of event loop ...
}
```

**Step 3: Add scan progress UI**

```rust
// In src/tui/ui.rs
fn draw_scan_progress(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 20, f.area());

    let block = Block::default()
        .title(" Scanning Directory ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    if let Some(progress) = &app.scan_progress {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(1),
                Constraint::Length(2),
            ])
            .margin(1)
            .split(inner);

        // Progress text
        let text = format!("Scanned {} files...", progress.scanned_count);
        let para = Paragraph::new(text)
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(para, chunks[0]);

        // Progress bar (if we have estimate)
        if let Some(pct) = progress.percentage() {
            let gauge = Gauge::default()
                .ratio(pct as f64 / 100.0)
                .gauge_style(Style::default().fg(Color::Green));
            f.render_widget(gauge, chunks[1]);
        } else {
            // Indeterminate progress (spinner effect)
            let spinner = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";
            let idx = (progress.scanned_count / 10) % spinner.chars().count();
            let char = spinner.chars().nth(idx).unwrap_or('⠋');
            let spin_text = Paragraph::new(format!("{} Scanning...", char))
                .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(spin_text, chunks[1]);
        }

        // Current directory
        if let Some(dir) = &progress.current_dir {
            let dir_short = if dir.len() > 40 {
                format!("...{}", &dir[dir.len()-37..])
            } else {
                dir.clone()
            };
            let dir_text = Paragraph::new(dir_short)
                .style(Style::default().fg(Color::DarkGray))
                .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(dir_text, chunks[2]);
        }
    }
}
```

**Step 4: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 5: Commit**

```bash
git add src/tui/app.rs src/tui/event.rs src/tui/ui.rs
git commit -m "feat(tui): show scan progress for large directories (T099)"
```

---

## 总结

### 实现顺序

1. **Module 1** (Task 1.1-1.6): 冲突检测与解决 - 最关键的安全功能
2. **Module 2** (Task 2.1-2.3): 移动后刷新 - 提升用户体验
3. **Module 3** (Task 3.1-3.4): Tab 路径补全 - 提升操作效率
4. **Module 4** (Task 4.1-4.4): 执行模式布局 - 专业的执行界面
5. **Module 5** (Task 5.1-5.3): 大目录处理 - 性能优化

### 测试命令汇总

```bash
# 运行所有新测试
cargo test tui_conflict
cargo test tui_move_result
cargo test tui_path_completion
cargo test tui_scan_progress

# 运行完整测试套件
cargo test

# 手动测试
cargo run -- /path/to/test/directory
```

### 预计新增文件

- `src/tui/state/conflict.rs`
- `src/tui/state/move_result.rs`
- `src/tui/state/path_completion.rs`
- `src/tui/state/scan_progress.rs`
- `tests/tui_conflict_tests.rs`
- `tests/tui_move_result_tests.rs`
- `tests/tui_path_completion_tests.rs`
- `tests/tui_scan_progress_tests.rs`
