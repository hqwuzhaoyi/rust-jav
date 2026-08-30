# rust-jav 使用参考

## 查看帮助

```bash
cargo run -- --help
cargo run -- ops --help
cargo run -- actor-links --help
```

## 核心命令

### 1. `tui`

适合交互式查看目录、勾选操作、手动浏览状态。

```bash
cargo run -- tui --dir ./examples/test
```

### 2. `ops`

适合做目录整理、清理、重命名、分类等文件操作。

#### 预览全部操作

```bash
cargo run -- ops --dir ./examples/test --json
```

#### 真正执行全部操作

```bash
cargo run -- ops --dir ./examples/test --apply --json
```

执行后，输出里会附带统一迁移验收摘要：

- `verification.verification_status`
- `verification.approval_status`
- `verification.exit_code`
- `verification.report_path`

退出码语义：

- `0`：技术结果正确，且可继续自动流程
- `10`：技术结果正确，但 destructive 操作需要人工确认
- `20`：实际结果与理论目标不一致
- `30`：验收层自身出错，例如预扫描或报告写入失败

#### 只执行指定操作

```bash
cargo run -- ops --dir ./examples/test --op standardize-names --op move-origin --apply
```

支持的 `--op`：

- `delete-ad-files`
- `organize-by-code`
- `clean-empty-dirs`
- `standardize-names`
- `extract-codes`
- `categorize-files`
- `move-origin`
- `remove-duplicates`

### 3. `actor-links`

适合根据 NFO 里的演员信息，创建演员目录式硬链接视图。

#### 预览

```bash
cargo run -- actor-links --source ./examples/test/actor-links --actors-root ./actors --json
```

#### 执行

```bash
cargo run -- actor-links --source ./examples/test/actor-links --actors-root ./actors --apply --json
```

`actor-links --apply` 的统一迁移验收会同时验证：

- `source` scope 是否保持不变
- `actors_root` scope 是否精确等于理论目标

## 推荐工作流

### 第一次使用：先预览，再执行

```bash
cargo run -- ops --dir /path/to/media --json
```

先检查计划动作，确认没问题后再执行：

```bash
cargo run -- ops --dir /path/to/media --apply --json
```

### 只先试广告文件清理

```bash
cargo run -- ops --dir /path/to/media --op delete-ad-files --json
cargo run -- ops --dir /path/to/media --op delete-ad-files --apply --json
```

注意：`delete-ad-files` 在 `--apply` 下可能删除匹配到的**视频文件**。

### 整理完成后再生成演员视图

```bash
cargo run -- actor-links --source /path/to/media --actors-root /path/to/actors --apply --json
```

### 检查迁移前后有没有遗漏文件

优先看 `--apply --json` 输出里的统一迁移验收结果，而不是先跑外部脚本：

```bash
cargo run -- ops --dir /path/to/media --apply --json
cargo run -- actor-links --source /path/to/media --actors-root /path/to/actors --apply --json
```

重点字段：

- `verification.verification_status`
- `verification.approval_status`
- `verification.report_path`

其中：

- `verification_status=ok`：实际结果与理论目标一致
- `approval_status=auto_pass`：可继续自动流程
- `approval_status=manual_confirm_required`：结果技术上正确，但 destructive 操作需要人工确认

详细文件清单 diff 会在 `report_path` 对应的 JSON 报告里。

## 示例 fixtures

如果你想用仓库自带示例快速体验，先重新生成 fixtures：

```bash
bash examples/create_test_files.sh ./examples/test
```

注意：如果你之前已经对 `./examples/test` 跑过 `--apply`，要再次重跑上面的脚本把示例目录重置回初始状态。

会生成这些场景目录：

- `./examples/test/delete-ad-files`
- `./examples/test/standardize-names`
- `./examples/test/extract-codes`
- `./examples/test/categorize-files`
- `./examples/test/move-origin`
- `./examples/test/organize-by-code`
- `./examples/test/clean-empty-dirs`
- `./examples/test/actor-links`

然后可以这样分别体验：

```bash
cargo run -- ops --dir ./examples/test/delete-ad-files --op delete-ad-files --json
cargo run -- ops --dir ./examples/test/extract-codes --op extract-codes --apply --json
cargo run -- actor-links --source ./examples/test/actor-links --actors-root ./actors --apply --json
```

## 关键行为说明

- `ops` 和 `actor-links` 默认都是 **preview** 模式。
- 只有加 `--apply` 才会真正修改文件系统。
- `--json` 适合脚本、AI 或需要仔细检查输出时使用。
- `ops` 不加 `--op` 时，会运行完整流程。
- 完整流程里，`delete-ad-files` 会最先执行。
