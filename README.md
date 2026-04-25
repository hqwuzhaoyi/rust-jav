# Rust Jav

为 jav torrent 编写的一些小工具，用于整理 jav torrent 文件夹，可以搭配 [MetaTube](https://metatube-community.github.io/) 使用。

当前版本采用 **CLI-first / AI-friendly** 设计：

- 先把能力沉淀为清晰的 CLI 命令
- 所有文件类操作默认 **preview**
- 只有显式传入 `--apply` 才会真正改动文件系统
- TUI 是这套能力的交互界面，而不是另一套独立语义

## 预览

![image](./images/preview.png)

## 功能

- 使用 rust 编写，tokio 异步，速度快
- 删除 jav torrent 中的无用文件，如 楼风最全资源\*
- 重命名 jav torrent 中的文件，如 hhd800.com@SSIS-001.mp4 -> SSIS-001.mp4
- 文件夹名重命名，如 ssis-001 -> SSIS-001
- 根据后缀整理文件，如 `-C` `ch` 结尾的文件放到 `CHINESE` 文件夹中，`-UC` 结尾的文件放到 `UNCENSORED` 文件夹中
- 普通视频（不是 CHINESE 或 UNCENSORED）可以移动到 `ORIGIN` 文件夹中
- 所有文件操作默认 `preview`，显式 `--apply` 才会落盘
- 支持基于 NFO 演员信息建立目录式硬链接

## 推荐目录结构

- `jav/`
  - CHINESE 有码
  - UNCENSORED 无码
  - ORIGIN 普通视频
  - jav torrent jav下载的视频文件夹

## CLI 概览

```shell
cargo run -- --help
```

当前支持的顶层命令：

- `tui`：启动 TUI
- `ops`：预览或执行文件整理操作
- `actor-links`：根据 NFO 演员信息建立目录式硬链接

### 默认 preview 语义

这是当前 CLI 最重要的约定：

- **默认不落盘**
- 所有文件相关命令，不带 `--apply` 时都只是预览
- 推荐先跑 preview，再决定是否执行
- 若需要给 AI / 脚本消费，建议加 `--json`

## 使用

### 查看帮助

```shell
cargo run -- --help
cargo run -- ops --help
cargo run -- actor-links --help
```

### TUI

```shell
cargo run -- tui --dir ./examples/test
```

### `ops`：预览文件操作（默认，不落盘）

```shell
cargo run -- ops --dir ./examples/test --json
```

返回结果可用于：

- 人类阅读：默认 text 输出
- AI / 脚本消费：`--json`

### `ops`：真正执行文件操作

```shell
cargo run -- ops --dir ./examples/test --apply
```

### `ops`：只执行指定操作

```shell
cargo run -- ops --dir ./examples/test --op standardize-names --op move-origin --apply
```

支持的 operation：

- `delete-ad-files` — 删除文件名匹配 `patterns.txt` 广告模式的文件（包括视频，执行前务必 preview）

> ⚠️ `delete-ad-files` 在 `--apply` 时会删除匹配到的**视频文件**。请始终先运行 preview / `--json` 检查计划动作。
- `organize-by-code`
- `clean-empty-dirs`
- `standardize-names`
- `extract-codes`
- `categorize-files`
- `move-origin`
- `remove-duplicates`

全量 `ops`（不加 `--op`）时，`delete-ad-files` 始终**最先执行**，确保广告文件在其他操作处理之前被清除。

说明：

- `extract-codes` 会把文件名规范化为“提取出的 JAV 编号 + 原后缀片段”
- 例如：`sample__abp123-C.mp4` 会变成 `ABP-123-C.mp4`
- 推荐先 preview，再决定是否 `--apply`

### `actor-links`：基于 NFO 演员建立硬链接

```shell
# 预览（默认）
cargo run -- actor-links --source ./examples/test --actors-root ./actors --json

# 执行
cargo run -- actor-links --source ./examples/test --actors-root ./actors --apply
```

硬链接输出结构为：

```text
<actors-root>/<actor-name>/<movie-code>/...
```

例如：

```text
./actors/miru/REBD-615/REBD-615.mp4
./actors/miru/REBD-615/REBD-615.nfo
./actors/miru/REBD-615/REBD-615-poster.jpg
./actors/miru/REBD-615/REBD-615-backdrop.jpg
```

其中演员名来自 NFO 中的：

```xml
<actor>
  <name>miru</name>
</actor>
```

## 推荐工作流

### 1. 先预览全部操作

```shell
cargo run -- ops --dir ./examples/test --json
```

### 2. 确认后执行

```shell
cargo run -- ops --dir ./examples/test --apply --json
```

### 3. 如需演员视图，再建立演员硬链接目录

```shell
cargo run -- actor-links --source ./examples/test --actors-root ./actors --apply --json
```

## Build

编译：

```shell
cargo build --release
```

跨平台编译：

先安装 [cross](https://github.com/cross-rs/cross)：

```shell
cargo install cross --git https://github.com/cross-rs/cross
```

```shell
CROSS_CONTAINER_OPTS="--platform linux/amd64" cross build --target x86_64-unknown-linux-gnu -v
```

## 测试命令

### 生成测试文件

推荐使用场景化 fixture 生成脚本：

```shell
bash examples/create_test_files.sh ./examples/test
```

生成后的目录结构为：

- `./examples/test/delete-ad-files`
- `./examples/test/standardize-names`
- `./examples/test/extract-codes`
- `./examples/test/categorize-files`
- `./examples/test/move-origin`
- `./examples/test/organize-by-code`
- `./examples/test/clean-empty-dirs`
- `./examples/test/actor-links`

### 单场景验证示例

```shell
cargo run -- ops --dir ./examples/test/delete-ad-files --op delete-ad-files --json
cargo run -- ops --dir ./examples/test/delete-ad-files --op delete-ad-files --apply --json
cargo run -- ops --dir ./examples/test/extract-codes --op extract-codes --apply --json
cargo run -- actor-links --source ./examples/test/actor-links --actors-root ./actors --apply --json
```

### 迁移前后文件数校验

如果你想检查迁移脚本有没有遗漏文件，推荐在迁移前先做一次 snapshot：

```shell
bash scripts/verify_migration_counts.sh snapshot --dir /path/to/media --output /tmp/media-before.txt
```

迁移完成后，再对比迁移后目录：

```shell
bash scripts/verify_migration_counts.sh compare --before /tmp/media-before.txt --after-dir /path/to/media
```

脚本会比较两项：

- 总文件数
- 按扩展名统计的文件数（如 `mp4` `nfo` `jpg`）

注意：

- 这个脚本适合检查 move / rename 类迁移有没有遗漏文件。
- 如果你执行了 `delete-ad-files` 或 `remove-duplicates`，计数下降可能是预期行为。
- `actor-links` 会在另一个目录树里创建硬链接，应该单独统计，不要和源目录混在一起比。
- 把它当作迁移门禁更合适：只有 `compare` 返回 `status=ok` 时，才继续自动迁移或接受迁移结果；如果返回 `status=mismatch`，就停止并转人工确认。

### 用 fixtures 验证计数校验脚本

`examples/create_test_files.sh` 生成的场景可以直接用来验证迁移前后计数对比是否符合预期。

#### 适合通过的场景：move / rename 不改总数

例如 `standardize-names`：

```shell
bash examples/create_test_files.sh ./examples/test
bash scripts/verify_migration_counts.sh snapshot --dir ./examples/test/standardize-names --output /tmp/standardize-before.txt
cargo run -- ops --dir ./examples/test/standardize-names --op standardize-names --apply --json
bash scripts/verify_migration_counts.sh compare --before /tmp/standardize-before.txt --after-dir ./examples/test/standardize-names
```

例如 `organize-by-code`：

```shell
bash examples/create_test_files.sh ./examples/test
bash scripts/verify_migration_counts.sh snapshot --dir ./examples/test/organize-by-code --output /tmp/organize-before.txt
cargo run -- ops --dir ./examples/test/organize-by-code --op organize-by-code --apply --json
bash scripts/verify_migration_counts.sh compare --before /tmp/organize-before.txt --after-dir ./examples/test/organize-by-code
```

这两类场景预期是 `status=ok`，因为它们主要是移动或重命名，不应改变源目录文件总数。
只有这种结果，才适合继续自动迁移流程。

#### 适合报 mismatch 的场景：显式删除文件

例如 `delete-ad-files`：

```shell
bash examples/create_test_files.sh ./examples/test
bash scripts/verify_migration_counts.sh snapshot --dir ./examples/test/delete-ad-files --output /tmp/delete-ad-before.txt
cargo run -- ops --dir ./examples/test/delete-ad-files --op delete-ad-files --apply --json
bash scripts/verify_migration_counts.sh compare --before /tmp/delete-ad-before.txt --after-dir ./examples/test/delete-ad-files
```

这个场景预期是 `status=mismatch`，因为它会删除广告文件，迁移后总文件数本来就应该下降。
这类结果不应直接继续自动迁移，必须先人工确认“数量变化是否符合预期”。

#### `actor-links` 的计数口径

`actor-links` 不会减少 source 目录文件数，但会在 `actors-root` 下额外创建硬链接文件：

```shell
bash examples/create_test_files.sh ./examples/test
bash scripts/verify_migration_counts.sh snapshot --dir ./examples/test/actor-links --output /tmp/actor-links-before.txt
cargo run -- actor-links --source ./examples/test/actor-links --actors-root ./actors --apply --json
bash scripts/verify_migration_counts.sh compare --before /tmp/actor-links-before.txt --after-dir ./examples/test/actor-links
```

这里 source 目录预期仍然是 `status=ok`。如果你想检查演员视图目录，应单独对 `./actors` 再做一次 snapshot / compare。
同样，只有 source 或目标目录各自比对为 `status=ok`，才适合继续无人值守流程。

### 全部操作（显式 apply）

```shell
cargo run -- ops --dir ./examples/test --apply --json
```
