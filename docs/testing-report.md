# rust-jav 测试报告

- 报告日期：2026-04-11 18:31:00 CST
- 范围：AI-first CLI、默认 preview 语义、`delete-ad-files` 广告清理、`actor-links` 能力、README 命令示例对齐

## 1. 验证目标

本轮主要验证以下内容：

- CLI 命令面是否稳定可用
- 所有文件操作是否默认 `preview`
- 只有显式 `--apply` 才会真正修改文件系统
- `ops --op ...` 是否支持按能力选择执行
- `delete-ad-files` 是否按 `patterns.txt` 规则预览/删除广告文件
- `delete-ad-files` 是否默认先于其他 `ops` 操作执行
- `actor-links` 是否基于 NFO `<actor><name>` 建立目录式硬链接
- README 中示例命令是否与真实实现一致

## 2. 静态检查与自动化测试

以下命令均已执行并通过：

```bash
cargo check --tests --bins --lib
cargo clippy --tests --bins --lib -- -D warnings
cargo test --tests --lib --bins --no-run
cargo test --tests --lib --bins
```

### 已通过的测试类别

#### 广告模式匹配单元测试

- `embedded_patterns_include_known_entries_and_match_dynamic_filenames`
- `legacy_delete_helper_uses_same_matching_rules_as_cli_path`

#### actor_links 单元测试

- `parses_actor_names_from_nfo`
- `parses_multiple_actors`

#### CLI 解析测试

- `parses_tui_command`
- `parses_actor_links_apply_command`
- `parses_ops_command_with_default_preview`
- `parses_delete_ad_files_operation`
- `delete_ad_files_is_first_in_all_operations`

#### CLI 工作流测试

- `ops_apply_mutates_files_when_explicit`
- `ops_preview_is_default_and_does_not_mutate_files`
- `actor_links_preview_warns_when_nfo_has_no_actor`
- `delete_ad_files_preview_plans_matched_files_without_deleting`
- `delete_ad_files_apply_deletes_matched_and_spares_unmatched`
- `delete_ad_files_apply_deletes_matching_video_file`
- `delete_ad_files_preview_on_empty_dir_produces_no_actions`
- `delete_ad_files_runs_before_other_ops_in_full_pipeline`
- `delete_ad_files_does_not_follow_symlinked_directories_outside_root`
- `delete_ad_files_apply_reports_failures_when_directory_is_not_writable`
- `delete_ad_files_full_pipeline_deletes_before_later_ops_can_move_or_rename`
- `categorize_files_preview_and_apply_work`
- `clean_empty_dirs_preview_and_apply_work`
- `move_origin_preview_and_apply_work`
- `actor_links_preview_does_not_create_targets`
- `remove_duplicates_preview_and_apply_work`
- `actor_links_apply_creates_directory_style_links`
- `actor_links_apply_is_idempotent_for_existing_targets`
- `extract_codes_preview_and_apply_work`
- `extract_codes_apply_fails_safely_when_target_exists`
- `organize_by_code_preview_and_apply_work`

#### runtime 分发测试

- `runtime_routes_tui_command_without_report`
- `runtime_routes_actor_links_preview_with_zero_exit_code`
- `runtime_routes_ops_preview_with_zero_exit_code`

#### 现有 TUI / 状态测试

现有 34 个 TUI/状态相关测试通过，覆盖：

- app 初始化、执行、取消、完成
- execution progress 百分比、耗时、成功、错误、跳过
- log entry 与 log persistence 的基本行为

## 3. CLI help 验证

以下命令已执行并通过：

```bash
cargo run -- --help
cargo run -- ops --help
cargo run -- actor-links --help
```

### 顶层命令

- `tui`
- `ops`
- `actor-links`

### `ops` 选项

- `--dir`
- `--apply`
- `--json`
- `--op`

### `actor-links` 选项

- `--source`
- `--actors-root`
- `--apply`
- `--json`

## 4. 端到端行为验证

### 4.1 `ops` 命令

#### 场景 A：默认 preview

执行：

```bash
cargo run -- ops --dir <temp>/examples/test --json
```

结果：

- `mode = "preview"`
- `planned_actions = 4`
- 原始文件仍存在
- 未发生实际文件变更

#### 场景 B：显式 apply

执行：

```bash
cargo run -- ops --dir <temp>/examples/test --op standardize-names --op move-origin --apply
```

结果：

- 成功执行真实文件变更
- 目标文件存在：`ORIGIN/ABP-123.mp4`
- 文本输出摘要显示：
  - `command: ops`
  - `mode: apply`
  - `selected: Standardize Names, Move to ORIGIN`
  - `summary: planned=0, applied=2, skipped=0, failed=0, warnings=0, errors=0`

#### 场景 C：`extract-codes` 预览与执行

执行：

```bash
cargo run -- ops --dir <temp> --op extract-codes --json
cargo run -- ops --dir <temp> --op extract-codes --apply --json
```

结果：

- preview:
  - `returncode = 0`
  - `mode = "preview"`
  - `planned_actions = 1`
  - 目标文件名为 `ABP-123-C.mp4`
- apply:
  - `returncode = 0`
  - `mode = "apply"`
  - `applied_actions = 1`
  - `failed_actions = 0`
  - 文件已从 `sample__abp123-C.mp4` 重命名为 `ABP-123-C.mp4`

#### 场景 D：`extract-codes` 目标已存在时的安全失败

执行前：

- 源文件：`sample__abp123-C.mp4`
- 已存在目标：`ABP-123-C.mp4`

执行：

```bash
cargo run -- ops --dir <temp> --op extract-codes --apply --json
```

结果：

- `returncode = 1`
- `mode = "apply"`
- `applied_actions = 0`
- `failed_actions = 1`
- action 状态为 `failed`
- reason 包含 `target already exists`
- 原文件仍存在
- 已存在目标文件保持不变

#### 场景 E：`delete-ad-files` 预览与执行

执行：

```bash
cargo run -- ops --dir <temp>/delete-ad-files --op delete-ad-files --json
cargo run -- ops --dir <temp>/delete-ad-files --op delete-ad-files --apply --json
```

结果：

- preview:
  - `returncode = 0`
  - `mode = "preview"`
  - 每个匹配文件都产生一个 `delete-file` planned action
  - 原始广告文件与普通视频都仍然存在
- apply:
  - `returncode = 0`
  - 匹配 `.txt` / `.html` / `.url` / `.jpg` / `.mp4` 文件会被删除
  - 未匹配的普通视频仍保留
  - 若匹配到视频文件，`warnings[]` 会提示需要谨慎预览

#### 场景 F：`delete-ad-files` 边界与顺序保证

- 不会跟随超出 `--dir` 根目录的符号链接目录
- 全量 `ops` 时，广告清理先执行，后续 rename/move 操作不会再引用已经删除的广告文件
- 对不可写目录中的匹配文件，action 会标记为 `failed`，整体返回码为非零

### 4.2 `actor-links` 命令

使用 `REBD-615.nfo` 作为参考输入。

#### 场景 A：默认 preview

执行：

```bash
cargo run -- actor-links --source <temp>/examples/test --actors-root <temp>/actors --json
```

结果：

- `mode = "preview"`
- `planned_actions = 4`
- `created_files = 0`
- 未创建任何实际目标文件

#### 场景 B：显式 apply

执行：

```bash
cargo run -- actor-links --source <temp>/examples/test --actors-root <temp>/actors --apply --json
```

结果：

- `mode = "apply"`
- `applied_actions = 4`
- 实际创建目标文件：
  - `miru/REBD-615/REBD-615.mp4`
  - `miru/REBD-615/REBD-615.nfo`
  - `miru/REBD-615/REBD-615-poster.jpg`
  - `miru/REBD-615/REBD-615-backdrop.jpg`

#### 场景 C：重复 apply 幂等性

再次执行相同命令。

结果：

- `skipped_actions = 4`
- `failed_actions = 0`

说明：已存在目标会被跳过，不会造成失败。

## 5. README 示例命令回归验证

README 中写到的命令模式已进行真实执行验证：

- `cargo run -- --help`
- `cargo run -- ops --help`
- `cargo run -- actor-links --help`
- `cargo run -- ops --dir ./examples/test --json`
- `cargo run -- ops --dir ./examples/test/delete-ad-files --op delete-ad-files --json`
- `cargo run -- ops --dir ./examples/test/delete-ad-files --op delete-ad-files --apply --json`
- `cargo run -- ops --dir ./examples/test --op standardize-names --op move-origin --apply`
- `cargo run -- actor-links --source ./examples/test --actors-root ./actors --json`
- `cargo run -- actor-links --source ./examples/test --actors-root ./actors --apply --json`

结论：README 文档与当前 CLI 实现一致，其中已明确警告 `delete-ad-files` 在 `--apply` 下可能删除匹配到的视频文件。

## 6. 新鲜验证快照

为满足 Ralph 收尾阶段的再次取证，还额外执行了新的 JSON/退出码验证：

- `cargo run -- ops --dir <temp> --op standardize-names --json`
  - `returncode = 0`
  - JSON 可解析
  - `mode = "preview"`
- `cargo run -- ops --dir <temp> --op extract-codes --json`
  - `returncode = 0`
  - JSON 可解析
  - `mode = "preview"`
  - `planned_actions = 1`
- `cargo run -- ops --dir <temp> --op extract-codes --apply --json`（目标已存在冲突场景）
  - `returncode = 1`
  - JSON 可解析
  - `failed_actions = 1`
  - 原文件与冲突目标都被安全保留
- `cargo run -- actor-links --source <temp> --actors-root <temp> --json`
  - `returncode = 0`
  - JSON 可解析
  - `mode = "preview"`
  - `planned_actions = 4`

## 7. 补充说明

### `extract-codes`

当前行为：

- preview 会列出提取后的目标文件名
- `--apply` 会真正重命名文件
- 重命名语义为：保留提取出的 JAV 编号，并保留原文件名中编号之后的后缀片段

示例：

- `sample__abp123-C.mp4` -> `ABP-123-C.mp4`

## 8. 结论

本轮结论：**通过**

- 编译通过
- Clippy 通过
- 自动化测试通过
- CLI 帮助命令通过
- README 示例命令通过
- `ops` / `actor-links` 端到端行为通过
- 默认 preview 与显式 apply 语义符合预期
