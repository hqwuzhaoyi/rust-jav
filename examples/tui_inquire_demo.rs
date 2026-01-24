use inquire::{
    Confirm, MultiSelect, Select, Text,
    ui::{RenderConfig, StyleSheet, Attributes, Color},
};
use std::fmt;
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use std::time::Duration;

#[derive(Debug, Clone)]
struct Operation {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    enabled: bool,
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} - {}", self.name, self.description)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 自定义主题
    let render_config = get_custom_theme();

    println!("\n🎬 Rust JAV 文件整理工具\n");
    println!("═══════════════════════════════════════════════════════\n");

    // 询问是否使用所有选项
    let use_all = Confirm::new("是否启用所有操作？")
        .with_default(true)
        .with_help_message("选择'是'将启用所有文件整理功能")
        .with_render_config(render_config)
        .prompt()?;

    let selected_operations = if use_all {
        get_all_operations()
    } else {
        // 多选操作
        let operations = get_all_operations();
        let ans = MultiSelect::new("请选择要执行的操作：", operations)
            .with_default(&[0, 1, 2, 3, 4, 5])
            .with_help_message("使用 ↑↓ 导航，空格选择，Enter确认")
            .with_render_config(render_config)
            .prompt()?;
        ans
    };

    if selected_operations.is_empty() {
        println!("\n❌ 未选择任何操作，退出。");
        return Ok(());
    }

    // 输入源目录
    let source_dir = Text::new("请输入源文件夹路径：")
        .with_default("./examples/test")
        .with_help_message("包含待整理JAV文件的目录")
        .with_render_config(render_config)
        .prompt()?;

    // 是否需要输出目录
    let need_output = selected_operations.iter().any(|op| {
        op.id == "move_chinese" || op.id == "move_uncensored"
    });

    let output_dir = if need_output {
        Some(
            Text::new("请输入输出目录：")
                .with_default(".")
                .with_help_message("整理后的文件将存放在此目录的CHINESE/UNCENSORED子目录中")
                .with_render_config(render_config)
                .prompt()?,
        )
    } else {
        None
    };

    // 选择日志级别
    let log_levels = vec!["trace", "debug", "info", "warn", "error"];
    let log_level = Select::new("选择日志级别：", log_levels)
        .with_starting_cursor(2) // 默认选择 "info"
        .with_help_message("控制输出详细程度")
        .with_render_config(render_config)
        .prompt()?;

    // 确认配置
    println!("\n📋 配置摘要");
    println!("───────────────────────────────────────────────────────");
    println!("源目录: {}", source_dir);
    if let Some(ref output) = output_dir {
        println!("输出目录: {}", output);
    }
    println!("日志级别: {}", log_level);
    println!("\n已选择的操作:");
    for (i, op) in selected_operations.iter().enumerate() {
        println!("  {}. {}", i + 1, op.name);
    }
    println!("───────────────────────────────────────────────────────\n");

    let proceed = Confirm::new("确认开始处理？")
        .with_default(true)
        .with_render_config(render_config)
        .prompt()?;

    if !proceed {
        println!("\n❌ 操作已取消。");
        return Ok(());
    }

    // 执行操作并显示进度
    println!("\n🚀 开始处理...\n");
    execute_operations(&selected_operations, &source_dir, output_dir.as_deref())?;

    println!("\n✅ 所有操作已完成！\n");

    // 显示统计信息
    show_summary();

    Ok(())
}

fn get_custom_theme() -> RenderConfig {
    let mut render_config = RenderConfig::default();

    render_config.prompt = StyleSheet::new()
        .with_fg(Color::LightCyan)
        .with_attr(Attributes::BOLD);

    render_config.answered_prompt = StyleSheet::new()
        .with_fg(Color::LightGreen)
        .with_attr(Attributes::BOLD);

    render_config.highlighted_option = StyleSheet::new()
        .with_fg(Color::LightYellow)
        .with_attr(Attributes::BOLD);

    render_config.selected_option = Some(StyleSheet::new()
        .with_fg(Color::LightGreen));

    render_config.help_message = StyleSheet::new()
        .with_fg(Color::DarkGrey);

    render_config
}

fn get_all_operations() -> Vec<Operation> {
    vec![
        Operation {
            id: "delete_empty",
            name: "删除没有视频文件的文件夹",
            description: "清理空文件夹和无用目录",
            enabled: true,
        },
        Operation {
            id: "move_chinese",
            name: "移动中文字幕视频",
            description: "将-C、ch结尾的文件移至CHINESE目录",
            enabled: true,
        },
        Operation {
            id: "move_uncensored",
            name: "移动无码视频",
            description: "将-UC结尾的文件移至UNCENSORED目录",
            enabled: true,
        },
        Operation {
            id: "rename_uppercase",
            name: "重命名文件夹名为大写",
            description: "统一文件夹命名为大写格式",
            enabled: true,
        },
        Operation {
            id: "remove_prefixes",
            name: "删除文件名前缀",
            description: "移除hhd800.com@等前缀",
            enabled: true,
        },
        Operation {
            id: "delete_ads",
            name: "删除广告文件",
            description: "删除楼风、广告等无用文件",
            enabled: true,
        },
    ]
}

fn execute_operations(
    operations: &[Operation],
    source_dir: &str,
    output_dir: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let multi_progress = MultiProgress::new();

    // 主进度条
    let main_pb = multi_progress.add(ProgressBar::new(operations.len() as u64));
    main_pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} {msg}")?
            .progress_chars("#>-"),
    );

    // 文件进度条
    let file_pb = multi_progress.add(ProgressBar::new(100));
    file_pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.yellow} {msg} [{wide_bar:.green/white}] {pos}%")?
            .progress_chars("█▓▒░ "),
    );

    // 启动进度条渲染线程
    let multi_progress_clone = multi_progress.clone();
    std::thread::spawn(move || {
        multi_progress_clone.join().unwrap();
    });

    for (i, operation) in operations.iter().enumerate() {
        main_pb.set_message(format!("执行: {}", operation.name));

        // 模拟文件处理
        file_pb.set_message(operation.name.to_string());
        for j in 0..=100 {
            file_pb.set_position(j);
            std::thread::sleep(Duration::from_millis(20));
        }

        main_pb.inc(1);
        std::thread::sleep(Duration::from_millis(100));
    }

    main_pb.finish_with_message("✓ 所有操作完成");
    file_pb.finish_and_clear();

    Ok(())
}

fn show_summary() {
    println!("📊 处理统计");
    println!("═══════════════════════════════════════════════════════");
    println!("  扫描文件数:     1,234");
    println!("  处理文件数:       856");
    println!("  移动文件数:       345");
    println!("  删除文件数:        89");
    println!("  重命名文件数:     422");
    println!("  跳过文件数:       378");
    println!("═══════════════════════════════════════════════════════");
    println!("  用时: 2分35秒");
    println!("  平均速度: 5.5 文件/秒");
    println!("═══════════════════════════════════════════════════════\n");
}
