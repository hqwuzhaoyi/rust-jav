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

先对迁移前目录做 snapshot：

```bash
bash scripts/verify_migration_counts.sh snapshot --dir /path/to/media --output /tmp/media-before.txt
```

迁移完成后，再比较迁移后目录：

```bash
bash scripts/verify_migration_counts.sh compare --before /tmp/media-before.txt --after-dir /path/to/media
```

这个脚本比较的是：

- 总文件数
- 按扩展名统计的文件数

适合：

- `standardize-names`
- `extract-codes`
- `categorize-files`
- `move-origin`
- `organize-by-code`

这些主要是 move / rename，预期通常应为 `status=ok`。
只有 `status=ok`，才适合继续自动迁移或接受迁移结果。

不应直接按“数量必须相等”理解的场景：

- `delete-ad-files`
- `remove-duplicates`

这些会删文件，出现 `status=mismatch` 可能是预期结果。
但即使是预期结果，也不应直接继续无人值守迁移；应先人工确认数量变化是否符合预期。

`actor-links` 要单独理解：

- source 目录通常仍应是 `status=ok`
- `actors-root` 会新增硬链接文件，应单独统计，不要和 source 混在一起比
- 只有 source / actors-root 各自的目标比对都符合预期，才适合继续后续自动流程

## 示例 fixtures

如果你想用仓库自带示例快速体验，先重新生成 fixtures：

```bash
bash examples/create_test_files.sh ./examples/test
```

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

也可以直接用这些 fixtures 验证迁移计数脚本：

```bash
bash examples/create_test_files.sh ./examples/test
bash scripts/verify_migration_counts.sh snapshot --dir ./examples/test/standardize-names --output /tmp/standardize-before.txt
cargo run -- ops --dir ./examples/test/standardize-names --op standardize-names --apply --json
bash scripts/verify_migration_counts.sh compare --before /tmp/standardize-before.txt --after-dir ./examples/test/standardize-names
```

上面这个 `standardize-names` 场景预期应为 `status=ok`。
这类结果适合作为自动迁移门禁通过条件。

```bash
bash examples/create_test_files.sh ./examples/test
bash scripts/verify_migration_counts.sh snapshot --dir ./examples/test/delete-ad-files --output /tmp/delete-ad-before.txt
cargo run -- ops --dir ./examples/test/delete-ad-files --op delete-ad-files --apply --json
bash scripts/verify_migration_counts.sh compare --before /tmp/delete-ad-before.txt --after-dir ./examples/test/delete-ad-files
```

上面这个 `delete-ad-files` 场景预期应为 `status=mismatch`，因为文件数量会下降。
这类结果必须转人工确认，不能直接当作自动迁移通过。

## 关键行为说明

- `ops` 和 `actor-links` 默认都是 **preview** 模式。
- 只有加 `--apply` 才会真正修改文件系统。
- `--json` 适合脚本、AI 或需要仔细检查输出时使用。
- `ops` 不加 `--op` 时，会运行完整流程。
- 完整流程里，`delete-ad-files` 会最先执行。
