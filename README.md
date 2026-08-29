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
- `serve`：独立启动内嵌的 Management Interface
- `administrator`：在本机初始化或重置唯一管理员

### Management Interface

普通服务配置从 YAML 读取，密码和初始化令牌只保存在权限为 `0600` 的独立 secrets 文件中。复制示例后启动：

```shell
cp management.example.yaml management.yaml
cargo run -- administrator init --config management.yaml
cargo run -- serve --config management.yaml
```

TrueNAS SCALE 24.10+ users can deploy the published multi-architecture container with the checked-in Compose example; see [the TrueNAS deployment and acceptance guide](docs/truenas-scale.md).

打开 `administrator init` 输出的一次性链接并设置至少 12 个字符的密码。也可以通过环境变量完成无人值守初始化；该变量只在尚未配置管理员时生效：

```shell
RUST_JAV_ADMIN_PASSWORD='replace-with-a-strong-password' cargo run -- serve --config management.yaml
```

本机运行默认只监听 `127.0.0.1`。TrueNAS 等容器环境需要将 YAML 中的 `container` 显式设为 `true`，服务才监听 `0.0.0.0`。本地重置不会经由 HTTP：

```shell
RUST_JAV_ADMIN_PASSWORD='replace-with-a-new-strong-password' \
  cargo run -- administrator reset-password --config management.yaml
```

重置会立即使现有会话失效。CLI/TUI 不依赖也不会自动启动此服务。

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

`--apply` 现在会在命令输出里额外给出统一迁移验收摘要：

- `verification_status`
- `approval_status`
- `exit_code`
- `report_path`
- 每个 scope 的 `before / expected / after`

### `ops`：只执行指定操作

```shell
cargo run -- ops --dir ./examples/test --op standardize-names --op move-origin --apply
```

支持的 operation：

- `delete-ad-files` — 删除文件名匹配当前 YAML Active Rule Set 的文件（包括视频，执行前务必 preview）

> ⚠️ `delete-ad-files` 在 `--apply` 时会删除匹配到的**视频文件**。请始终先运行 preview / `--json` 检查计划动作。
- `organize-by-code`
- `clean-empty-dirs`
- `standardize-names`
- `extract-codes`
- `categorize-files`
- `move-origin`
- `remove-duplicates`

全量 `ops`（不加 `--op`）时，`delete-ad-files` 始终**最先执行**，确保广告文件在其他操作处理之前被清除。

### YAML Active Rule Set

默认规则位于 `rules.yaml`。也可以为 preview 和 apply 显式加载另一个版本化规则文件：

```shell
cargo run -- ops --dir ./examples/test --op delete-ad-files --rules ./my-rules.yaml --json
cargo run -- ops --dir ./examples/test --op delete-ad-files --rules ./my-rules.yaml --apply
```

规则格式：

```yaml
version: 1
rules:
  - pattern: "广告*.html" # 必填；只有 * 是通配符
    enabled: true         # 可选，默认 true
    note: "来源说明"       # 可选
```

匹配继续采用原有语义：对完整 basename 大小写不敏感匹配，只有 `*` 表示任意字符，其余字符均按字面量处理。YAML、版本或规则无效时，候选内容不会成为 Active Rule Set，操作也不会开始。空规则集必须额外传入 `--confirm-empty-rules` 明确确认。

从旧版迁移时，将 `patterns.txt` 的每个非空行按原样写成一个 `rules[].pattern`。仓库内原有模式已逐项迁移至 `rules.yaml`；`patterns.txt` 仅保留为迁移对照，不再作为运行时规则来源。

说明：

- `extract-codes` 会把文件名规范化为“提取出的 JAV 编号 + 原后缀片段”
- 例如：`sample__abp123-C.mp4` 会变成 `ABP-123-C.mp4`
- 推荐先 preview，再决定是否 `--apply`
- 对于 `delete-ad-files` / `remove-duplicates` 这类 destructive 操作，即使迁移结果符合理论目标，`approval_status` 也会是 `manual_confirm_required`

### `actor-links`：基于 NFO 演员建立硬链接

```shell
# 预览（默认）
cargo run -- actor-links --source ./examples/test --actors-root ./actors --json

# 执行
cargo run -- actor-links --source ./examples/test --actors-root ./actors --apply
```

`actor-links --apply` 也会输出统一迁移验收摘要，但会同时验证两个 scope：

- `source`：理论上应保持不变
- `actors_root`：理论上应精确等于计划生成的演员链接结果

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

### 4. 查看统一迁移验收结果

无论是 `ops --apply` 还是 `actor-links --apply`，机器可读输出里都可以直接读取：

- `verification.verification_status`
- `verification.approval_status`
- `verification.exit_code`
- `verification.report_path`

详细清单级 diff 会落盘到 `report_path` 指向的 JSON 报告文件中。

退出码约定：

- `0`：`verification_status=ok` 且 `approval_status=auto_pass`
- `10`：`verification_status=ok` 但 `approval_status=manual_confirm_required`
- `20`：`verification_status=mismatch`
- `30`：`verification_status=error`

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

> 注意：`./examples/test` 在执行过 `--apply` 后会变脏。做命令验证前，先重新运行一次 fixture 生成脚本，确保示例目录回到初始状态。

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

### 全部操作（显式 apply）

```shell
cargo run -- ops --dir ./examples/test --apply --json
```
