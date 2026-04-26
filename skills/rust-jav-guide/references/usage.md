# rust-jav 使用参考

## 查看帮助

```bash
rust-jav --help
rust-jav ops --help
rust-jav actor-links --help
rust-jav nfo-check --help
rust-jav tui --help
```

## 核心命令

### 1. `tui`

适合交互式查看目录、勾选操作、手动浏览状态。

```bash
rust-jav tui --dir ./examples/test
```

### 2. `ops`

适合做目录整理、清理、重命名、分类等文件操作。

#### 预览全部操作

```bash
rust-jav ops --dir ./examples/test --json
```

#### 真正执行全部操作（自动触发迁移验收）

```bash
rust-jav ops --dir ./examples/test --apply --json
```

#### 只执行指定操作

```bash
rust-jav ops --dir ./examples/test --op standardize-names --op move-origin --apply
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
rust-jav actor-links --source /path/to/media --actors-root /path/to/actors --json
```

#### 执行（自动触发迁移验收）

```bash
rust-jav actor-links --source /path/to/media --actors-root /path/to/actors --apply --json
```

### 4. `nfo-check`

检查哪些影片目录缺少 NFO 元数据文件。

```bash
rust-jav nfo-check --dir /path/to/media
rust-jav nfo-check --dir /path/to/media --codes-only --skip actors --skip organized
rust-jav nfo-check --dir /path/to/media --json --max-depth 3
```

参数：
- `--max-depth <N>`: 检查深度（默认 2）
- `--skip <DIR>`: 跳过指定目录名（可重复）
- `--json`: JSON 输出
- `--codes-only`: 只输出番号，适合管道给其他工具

## 内建迁移验收层

`ops --apply` 和 `actor-links --apply` 会自动触发迁移验收，无需单独命令。

### 验收流程

1. **before manifest**: 执行前扫描文件清单
2. **expected manifest**: 根据计划动作推导理论目标
3. **执行迁移动作**
4. **after manifest**: 执行后扫描文件清单
5. **清单级比对**: 路径、扩展名、大小、来源追溯
6. **输出报告**: JSON 写入 `.omx/reports/migrations/`

### 验收状态

- `verification_status`: `ok` | `mismatch` | `error`
- `approval_status`: `auto_pass` | `manual_confirm_required` | `blocked`
- 只有 `ok` + `auto_pass` 才能继续自动流程

### actor-links 双 scope 验收

- **source scope**: before/expected/after 必须一致（source 不应被修改）
- **actors_root scope**: after 必须等于 expected（所有链接应存在）
- 两个 scope 分开统计，不能混在一起比

### destructive 操作特殊处理

`delete-ad-files` 和 `remove-duplicates` 即使结果符合计划，`approval_status` 也是 `manual_confirm_required`，必须人工确认。

### 报告字段

- `scope_counts`: 各 scope 的 before/expected/after 计数
- `diffs`: 各 scope 的 missing_files / unexpected_files / mismatched_files
- `expected_stats`: expected_new_links / expected_existing_links / plan_conflicts
- `report_path`: 详细 JSON 报告路径

## 推荐工作流

### 第一次使用：先预览，再执行

```bash
rust-jav ops --dir /path/to/media --json
```

先检查计划动作，确认没问题后再执行（自动触发验收）：

```bash
rust-jav ops --dir /path/to/media --apply --json
```

### 只先试广告文件清理

```bash
rust-jav ops --dir /path/to/media --op delete-ad-files --json
rust-jav ops --dir /path/to/media --op delete-ad-files --apply --json
```

注意：`delete-ad-files` 在 `--apply` 下可能删除匹配到的**视频文件**。且 `approval_status` 会是 `manual_confirm_required`。

### 整理完成后再生成演员视图

```bash
rust-jav actor-links --source /path/to/media --actors-root /path/to/actors --apply --json
```

验收层会同时验证 source 不变 + actors_root 完整。

### 检查哪些目录缺 NFO

```bash
rust-jav nfo-check --dir /path/to/media --codes-only --skip actors --skip organized
```

## 示例 fixtures

如果你想用仓库自带示例快速体验，先重新生成 fixtures：

```bash
bash examples/create_test_files.sh ./examples/test
```

然后可以这样分别体验：

```bash
rust-jav ops --dir ./examples/test/delete-ad-files --op delete-ad-files --json
rust-jav ops --dir ./examples/test/extract-codes --op extract-codes --apply --json
rust-jav actor-links --source ./examples/test/actor-links --actors-root ./actors --apply --json
```

## 关键行为说明

- `ops` 和 `actor-links` 默认都是 **preview** 模式。
- 只有加 `--apply` 才会真正修改文件系统并触发验收。
- `--json` 适合脚本、AI 或需要仔细检查输出时使用。
- `ops` 不加 `--op` 时，会运行完整流程，`delete-ad-files` 会最先执行。
- 验收报告自动写入 `.omx/reports/migrations/` 目录。
