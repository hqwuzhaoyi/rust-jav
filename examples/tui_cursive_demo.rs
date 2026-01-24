use cursive::traits::*;
use cursive::views::{
    Button, Checkbox, Dialog, DummyView, EditView, LinearLayout, ProgressBar,
    SelectView, TextView, Panel, ScrollView,
};
use cursive::{Cursive, CursiveExt};
use cursive::theme::{Color, PaletteColor, Theme};
use cursive::align::HAlign;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 应用配置
#[derive(Clone, Debug)]
struct AppConfig {
    delete_empty_dirs: bool,
    move_chinese: bool,
    move_uncensored: bool,
    rename_uppercase: bool,
    remove_prefixes: bool,
    delete_ads: bool,
    output_dir: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            delete_empty_dirs: true,
            move_chinese: true,
            move_uncensored: true,
            rename_uppercase: true,
            remove_prefixes: true,
            delete_ads: true,
            output_dir: String::new(),
        }
    }
}

fn main() {
    let mut siv = cursive::default();

    // 设置主题
    let mut theme = Theme::default();
    theme.palette[PaletteColor::Background] = Color::TerminalDefault;
    theme.palette[PaletteColor::View] = Color::TerminalDefault;
    theme.palette[PaletteColor::Primary] = Color::Rgb(100, 181, 246);
    theme.palette[PaletteColor::TitlePrimary] = Color::Rgb(129, 212, 250);
    siv.set_theme(theme);

    // 显示主界面
    show_main_screen(&mut siv);

    siv.run();
}

fn show_main_screen(siv: &mut Cursive) {
    let config = Arc::new(Mutex::new(AppConfig::default()));

    // 创建复选框选项
    let mut checkboxes = LinearLayout::vertical();

    let options = vec![
        ("delete_empty_dirs", "删除没有视频文件的文件夹", "清理空文件夹和无用目录"),
        ("move_chinese", "移动中文字幕视频", "将-C、ch结尾的文件移至CHINESE目录"),
        ("move_uncensored", "移动无码视频", "将-UC结尾的文件移至UNCENSORED目录"),
        ("rename_uppercase", "重命名文件夹名为大写", "统一文件夹命名为大写格式"),
        ("remove_prefixes", "删除文件名前缀", "移除hhd800.com@等前缀"),
        ("delete_ads", "删除广告文件", "删除楼风、广告等无用文件"),
    ];

    for (id, name, desc) in options {
        let config_clone = Arc::clone(&config);
        let checkbox = Checkbox::new()
            .checked()
            .with_name(id)
            .on_change(move |_s, checked| {
                let mut cfg = config_clone.lock().unwrap();
                match id {
                    "delete_empty_dirs" => cfg.delete_empty_dirs = checked,
                    "move_chinese" => cfg.move_chinese = checked,
                    "move_uncensored" => cfg.move_uncensored = checked,
                    "rename_uppercase" => cfg.rename_uppercase = checked,
                    "remove_prefixes" => cfg.remove_prefixes = checked,
                    "delete_ads" => cfg.delete_ads = checked,
                    _ => {}
                }
            });

        let item = LinearLayout::horizontal()
            .child(checkbox)
            .child(DummyView.fixed_width(2))
            .child(
                LinearLayout::vertical()
                    .child(TextView::new(name).style(Color::Light(cursive::theme::BaseColor::White)))
                    .child(TextView::new(format!("  {}", desc))
                        .style(Color::Dark(cursive::theme::BaseColor::White)))
            );

        checkboxes.add_child(item);
        checkboxes.add_child(DummyView.fixed_height(1));
    }

    // 输出目录输入框
    let config_clone = Arc::clone(&config);
    let output_input = EditView::new()
        .on_edit(move |_s, text, _cursor| {
            config_clone.lock().unwrap().output_dir = text.to_string();
        })
        .with_name("output_dir")
        .fixed_width(50);

    // 创建主布局
    let layout = LinearLayout::vertical()
        .child(
            Panel::new(
                LinearLayout::vertical()
                    .child(TextView::new("请选择要执行的操作：").h_align(HAlign::Left))
                    .child(DummyView.fixed_height(1))
                    .child(ScrollView::new(checkboxes))
            )
            .title("操作选项")
        )
        .child(DummyView.fixed_height(1))
        .child(
            Panel::new(
                LinearLayout::vertical()
                    .child(TextView::new("输出目录（整理后文件存放位置）："))
                    .child(DummyView.fixed_height(1))
                    .child(output_input)
            )
            .title("配置")
        )
        .child(DummyView.fixed_height(1))
        .child(
            LinearLayout::horizontal()
                .child(DummyView.fixed_width(2))
                .child(Button::new("开始处理", move |s| {
                    let cfg = config.lock().unwrap().clone();
                    show_progress_dialog(s, cfg);
                }))
                .child(DummyView.fixed_width(2))
                .child(Button::new("全选", |s| {
                    s.call_on_name("delete_empty_dirs", |v: &mut Checkbox| v.check());
                    s.call_on_name("move_chinese", |v: &mut Checkbox| v.check());
                    s.call_on_name("move_uncensored", |v: &mut Checkbox| v.check());
                    s.call_on_name("rename_uppercase", |v: &mut Checkbox| v.check());
                    s.call_on_name("remove_prefixes", |v: &mut Checkbox| v.check());
                    s.call_on_name("delete_ads", |v: &mut Checkbox| v.check());
                }))
                .child(DummyView.fixed_width(2))
                .child(Button::new("全不选", |s| {
                    s.call_on_name("delete_empty_dirs", |v: &mut Checkbox| v.uncheck());
                    s.call_on_name("move_chinese", |v: &mut Checkbox| v.uncheck());
                    s.call_on_name("move_uncensored", |v: &mut Checkbox| v.uncheck());
                    s.call_on_name("rename_uppercase", |v: &mut Checkbox| v.uncheck());
                    s.call_on_name("remove_prefixes", |v: &mut Checkbox| v.uncheck());
                    s.call_on_name("delete_ads", |v: &mut Checkbox| v.uncheck());
                }))
                .child(DummyView.fixed_width(2))
                .child(Button::new("退出", |s| s.quit()))
        );

    siv.add_layer(
        Dialog::around(layout)
            .title("🎬 Rust JAV 文件整理工具")
            .padding_lrtb(2, 2, 1, 1)
    );
}

fn show_progress_dialog(siv: &mut Cursive, config: AppConfig) {
    // 验证配置
    if config.output_dir.is_empty() {
        siv.add_layer(
            Dialog::text("请先设置输出目录！")
                .title("错误")
                .button("确定", |s| {
                    s.pop_layer();
                })
        );
        return;
    }

    let max = 100;
    let counter = Arc::new(Mutex::new(0));
    let cb_sink = siv.cb_sink().clone();

    // 创建进度条
    let progress = ProgressBar::new()
        .with_value(counter.clone())
        .max(max)
        .with_name("progress");

    let status_text = TextView::new("正在初始化...")
        .h_align(HAlign::Center)
        .with_name("status");

    let layout = LinearLayout::vertical()
        .child(status_text)
        .child(DummyView.fixed_height(1))
        .child(progress)
        .child(DummyView.fixed_height(1))
        .child(
            TextView::new("处理中的文件会显示在这里...")
                .with_name("current_file")
                .h_align(HAlign::Center)
        );

    siv.add_layer(
        Dialog::around(layout)
            .title("处理进度")
            .padding_lrtb(2, 2, 1, 1)
    );

    // 模拟异步处理
    let counter_clone = Arc::clone(&counter);
    std::thread::spawn(move || {
        let operations = vec![
            "扫描目录结构...",
            "删除空文件夹...",
            "移动中文字幕视频...",
            "移动无码视频...",
            "重命名文件夹...",
            "删除文件名前缀...",
            "删除广告文件...",
            "完成！",
        ];

        for (i, op) in operations.iter().enumerate() {
            std::thread::sleep(Duration::from_millis(500));

            let progress_value = ((i + 1) * 100 / operations.len()) as usize;
            *counter_clone.lock().unwrap() = progress_value;

            let status_msg = op.to_string();
            let file_msg = format!("处理: example-{}.mp4", i);

            cb_sink.send(Box::new(move |s| {
                s.call_on_name("status", |v: &mut TextView| {
                    v.set_content(&status_msg);
                });
                s.call_on_name("current_file", |v: &mut TextView| {
                    v.set_content(&file_msg);
                });
            })).unwrap();
        }

        // 完成后显示摘要
        cb_sink.send(Box::new(|s| {
            s.pop_layer();
            s.add_layer(
                Dialog::text(
                    "处理完成！\n\n\
                    处理文件数: 156\n\
                    移动文件数: 89\n\
                    删除文件数: 23\n\
                    重命名文件数: 44"
                )
                .title("✓ 完成")
                .button("确定", |s| {
                    s.pop_layer();
                })
            );
        })).unwrap();
    });
}
