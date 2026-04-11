use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Wrap,
    },
    Frame, Terminal,
};
use std::{error::Error, io};

/// 应用状态
struct App {
    /// 选项列表状态
    list_state: ListState,
    /// 可用选项
    options: Vec<OperationOption>,
    /// 输出目录
    output_dir: String,
    /// 是否在编辑输出目录
    editing_output: bool,
    /// 当前操作状态
    status: AppStatus,
    /// 进度（0-100）
    progress: u16,
    /// 状态消息
    message: String,
}

#[derive(Clone)]
struct OperationOption {
    name: String,
    description: String,
    enabled: bool,
}

enum AppStatus {
    Idle,
    Running,
    Completed,
}

impl Default for App {
    fn default() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        let options = vec![
            OperationOption {
                name: "删除没有视频文件的文件夹".to_string(),
                description: "清理空文件夹和无用目录".to_string(),
                enabled: true,
            },
            OperationOption {
                name: "移动中文字幕视频".to_string(),
                description: "将-C、ch结尾的文件移至CHINESE目录".to_string(),
                enabled: true,
            },
            OperationOption {
                name: "移动无码视频".to_string(),
                description: "将-UC结尾的文件移至UNCENSORED目录".to_string(),
                enabled: true,
            },
            OperationOption {
                name: "重命名文件夹名为大写".to_string(),
                description: "统一文件夹命名为大写格式".to_string(),
                enabled: true,
            },
            OperationOption {
                name: "删除文件名前缀".to_string(),
                description: "移除hhd800.com@等前缀".to_string(),
                enabled: true,
            },
            OperationOption {
                name: "删除广告文件".to_string(),
                description: "删除楼风、广告等无用文件".to_string(),
                enabled: true,
            },
        ];

        App {
            list_state,
            options,
            output_dir: String::new(),
            editing_output: false,
            status: AppStatus::Idle,
            progress: 0,
            message: "准备就绪".to_string(),
        }
    }
}

impl App {
    fn next(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.options.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.options.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn toggle_selected(&mut self) {
        if let Some(i) = self.list_state.selected() {
            self.options[i].enabled = !self.options[i].enabled;
        }
    }

    fn run_operations(&mut self) {
        self.status = AppStatus::Running;
        self.progress = 0;
        self.message = "正在处理文件...".to_string();
        // 这里会触发实际的文件操作
    }

    fn simulate_progress(&mut self) {
        if self.progress < 100 {
            self.progress += 1;
        } else {
            self.status = AppStatus::Completed;
            self.message = "处理完成！".to_string();
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // 设置终端
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 创建应用并运行
    let app = App::default();
    let res = run_app(&mut terminal, app);

    // 恢复终端
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Char(' ') => app.toggle_selected(),
                    KeyCode::Enter => {
                        app.run_operations();
                    }
                    KeyCode::Char('e') => {
                        app.editing_output = !app.editing_output;
                    }
                    KeyCode::Char(c) if app.editing_output => {
                        app.output_dir.push(c);
                    }
                    KeyCode::Backspace if app.editing_output => {
                        app.output_dir.pop();
                    }
                    _ => {}
                }
            }
        }

        // 模拟进度更新
        if matches!(app.status, AppStatus::Running) {
            app.simulate_progress();
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    // 主布局
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // 标题
            Constraint::Length(3),  // 输出目录
            Constraint::Min(10),    // 选项列表
            Constraint::Length(3),  // 进度条
            Constraint::Length(3),  // 帮助信息
        ])
        .split(f.area());

    // 标题
    let title = Paragraph::new("🎬 Rust JAV 文件整理工具")
        .style(Style::default().fg(Color::Cyan).bold())
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // 输出目录输入框
    let output_style = if app.editing_output {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let output_text = if app.output_dir.is_empty() {
        "未设置（按 'e' 编辑）"
    } else {
        &app.output_dir
    };
    let output = Paragraph::new(output_text)
        .style(output_style)
        .block(
            Block::default()
                .title("输出目录")
                .borders(Borders::ALL),
        );
    f.render_widget(output, chunks[1]);

    // 操作选项列表
    let items: Vec<ListItem> = app
        .options
        .iter()
        .map(|option| {
            let checkbox = if option.enabled { "[✓]" } else { "[ ]" };
            let line = Line::from(vec![
                Span::styled(
                    checkbox,
                    Style::default().fg(if option.enabled {
                        Color::Green
                    } else {
                        Color::Gray
                    }),
                ),
                Span::raw(" "),
                Span::styled(&option.name, Style::default().fg(Color::White)),
                Span::raw(" - "),
                Span::styled(
                    &option.description,
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let items = List::new(items)
        .block(
            Block::default()
                .title("操作选项（空格键切换，↑↓选择）")
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(items, chunks[2], &mut app.list_state);

    // 进度条
    let progress_label = format!("{}: {}%", app.message, app.progress);
    let gauge = Gauge::default()
        .block(Block::default().title("进度").borders(Borders::ALL))
        .gauge_style(
            Style::default()
                .fg(Color::Green)
                .bg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .percent(app.progress)
        .label(progress_label);
    f.render_widget(gauge, chunks[3]);

    // 帮助信息
    let help = Paragraph::new(
        "按键: [↑/↓] 选择 | [Space] 切换 | [e] 编辑目录 | [Enter] 开始 | [q] 退出",
    )
    .style(Style::default().fg(Color::Gray))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[4]);
}
