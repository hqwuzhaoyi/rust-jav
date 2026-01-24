# Quickstart: LazyGit 风格 TUI 实现

## 快速开始

### 1. 添加依赖

```toml
# Cargo.toml
[dependencies]
# TUI 框架
ratatui = "0.30.0"
crossterm = { version = "0.29.0", features = ["event-stream"] }
futures = "0.3"
color-eyre = "0.6"

# 保留现有依赖
tokio = { version = "1", features = ["full"] }
# ... 其他现有依赖

# 移除
# dialoguer = "0.11.0"  # 已被 TUI 替代
```

### 2. 创建 TUI 模块结构

```bash
mkdir -p src/tui/components src/tui/state
touch src/tui/mod.rs
touch src/tui/app.rs
touch src/tui/ui.rs
touch src/tui/event.rs
touch src/tui/components/mod.rs
touch src/tui/components/file_tree.rs
touch src/tui/components/operations.rs
touch src/tui/components/preview.rs
touch src/tui/state/mod.rs
```

### 3. 最小可运行示例

```rust
// src/tui/mod.rs
mod app;
mod ui;
mod event;
mod components;
mod state;

pub use app::App;
pub use event::run_app;
```

```rust
// src/tui/app.rs
use std::path::PathBuf;
use tokio::sync::mpsc;

pub struct App {
    pub focused_panel: Panel,
    pub source_dir: PathBuf,
    pub should_quit: bool,
    pub action_tx: mpsc::UnboundedSender<Action>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Panel {
    FileTree,
    Operations,
    Preview,
}

impl App {
    pub fn new(source_dir: PathBuf, action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            focused_panel: Panel::FileTree,
            source_dir,
            should_quit: false,
            action_tx,
        }
    }

    pub fn next_panel(&mut self) {
        self.focused_panel = match self.focused_panel {
            Panel::FileTree => Panel::Operations,
            Panel::Operations => Panel::Preview,
            Panel::Preview => Panel::FileTree,
        };
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }
}
```

```rust
// src/tui/event.rs
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::sync::mpsc;

use super::app::App;
use super::ui::draw;

pub async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| draw(f, &app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => app.quit(),
                        KeyCode::Tab => app.next_panel(),
                        KeyCode::Char('j') | KeyCode::Down => {
                            // Handle navigation
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            // Handle navigation
                        }
                        _ => {}
                    }
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
```

```rust
// src/tui/ui.rs
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::app::{App, Panel};

pub fn draw(f: &mut Frame, app: &App) {
    // Main layout: status bar, panels, help bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Status bar
            Constraint::Min(10),    // Main panels
            Constraint::Length(3),  // Help bar
        ])
        .split(f.area());

    // Status bar
    let status = Paragraph::new(format!(" rust-jav | {}", app.source_dir.display()))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[0]);

    // Three panels
    let panel_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(chunks[1]);

    // File tree panel
    let tree_style = panel_style(app.focused_panel == Panel::FileTree);
    let tree_block = Block::default()
        .title(" Files ")
        .borders(Borders::ALL)
        .border_style(tree_style);
    f.render_widget(tree_block, panel_chunks[0]);

    // Operations panel
    let ops_style = panel_style(app.focused_panel == Panel::Operations);
    let ops_block = Block::default()
        .title(" Operations ")
        .borders(Borders::ALL)
        .border_style(ops_style);
    f.render_widget(ops_block, panel_chunks[1]);

    // Preview panel
    let preview_style = panel_style(app.focused_panel == Panel::Preview);
    let preview_block = Block::default()
        .title(" Preview ")
        .borders(Borders::ALL)
        .border_style(preview_style);
    f.render_widget(preview_block, panel_chunks[2]);

    // Help bar
    let help = Paragraph::new(" Tab: Switch panel | j/k: Navigate | Space: Toggle | Enter: Execute | q: Quit ")
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[2]);
}

fn panel_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    }
}
```

```rust
// src/main.rs (更新入口点)
mod config;
mod file_utils;
mod tui;

use clap::Parser;
use color_eyre::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::sync::mpsc;

#[derive(Parser)]
#[command(name = "rust-jav")]
#[command(about = "JAV file organization tool")]
struct Cli {
    /// Source directory to process
    #[arg(default_value = ".")]
    source: std::path::PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create action channel
    let (action_tx, _action_rx) = mpsc::unbounded_channel();

    // Create app
    let app = tui::App::new(cli.source, action_tx);

    // Run app
    let result = tui::event::run_app(&mut terminal, app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result.map_err(Into::into)
}
```

### 4. 运行

```bash
cargo run -- /path/to/jav/directory
```

## 键盘快捷键

| 按键 | 功能 |
|------|------|
| `Tab` | 切换面板 |
| `j` / `↓` | 向下移动 |
| `k` / `↑` | 向上移动 |
| `l` / `→` / `Enter` | 展开/进入 |
| `h` / `←` / `Backspace` | 折叠/返回 |
| `Space` | 切换选中状态 |
| `a` | 全选/取消全选 |
| `m` | 移动文件 |
| `v` | 多选模式 |
| `1-9` | 快速移动到预设目录 |
| `/` | 搜索 |
| `F1` | 帮助 |
| `q` | 退出 |

## 下一步

1. 实现 `FileTreeComponent` - 文件树面板
2. 实现 `OperationsComponent` - 操作列表面板
3. 实现 `PreviewComponent` - 预览面板
4. 添加异步目录扫描
5. 集成现有 `file_utils` 模块
6. 添加执行进度显示
7. 实现移动对话框
8. 添加日志持久化
