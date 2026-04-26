---
name: rust-jav-guide
description: 讲解如何使用 rust-jav 通过 `tui`、`ops`、`actor-links`、`nfo-check` 和内建迁移验收层整理并核对 JAV 媒体目录。适用于用户询问 rust-jav 怎么运行、某个命令做什么、该用哪些参数、preview / `--apply` / `--json` 的区别、如何生成示例 fixtures、如何检查迁移前后有没有遗漏文件，或想要一套安全的 rust-jav 常见使用流程时。
---

# Rust Jav 使用指南

## 概览

用这个 skill 来回答“rust-jav 怎么用”的问题：给出准确命令、安全默认值，以及基于真实 CLI 的实用工作流说明。

需要具体命令时，优先查看 `references/usage.md`。

## 回答流程

1. 先确认用户目标属于哪一类：
   - 想交互式查看 / 选择操作 → `tui`
   - 想整理、清理、重命名文件 → `ops`
   - 想根据 NFO 演员信息生成演员视图 → `actor-links`
   - 想检查哪些目录缺 NFO 元数据 → `nfo-check`
   - 想检查迁移前后是否有遗漏 → 内建迁移验收层（`ops --apply` / `actor-links --apply` 自动触发）
2. 先强调安全规则：文件类操作默认都是 **preview**，只有加 `--apply` 才会真正改动文件系统。
3. 优先给出可以直接复制的命令，不要只讲抽象概念。
4. 如果示例里用到 `./examples/test`，提醒用户先重新生成 fixtures：
   - `bash examples/create_test_files.sh ./examples/test`
   因为之前执行过 `--apply` 后，示例目录可能已经不是初始状态。

## 回答要点

- 用户想看详细计划时，推荐加 `--json`。
- 讲到 `delete-ad-files` 时，必须明确提醒：`--apply` 下可能删除匹配到的**视频文件**。
- 用户说“全量整理”时，要说明不加 `--op` 的 `ops` 会跑完整流程，且 `delete-ad-files` 会最先执行。
- 用户说“查有没有遗漏”“迁移前后文件数对比”“统计迁移前后文件总数”时，说明验收层已内建在 `--apply` 流程中，不需要单独脚本。
- 讲迁移验收时，必须明确：验收层自动生成 before/expected/after manifest 并做清单级比对，输出 `verification_status`（ok/mismatch/error）和 `approval_status`（auto_pass/manual_confirm_required/blocked）。
- 讲迁移门禁时，必须明确：只有 `verification_status=ok` 且 `approval_status=auto_pass` 才能继续自动迁移；`mismatch` 或 `blocked` 都要停下来做人工确认。
- `actor-links` 的验收会同时验证 source（不应变）和 actors_root（应等于 expected）两个 scope，不能混在一起比。
- destructive 操作（`delete-ad-files`、`remove-duplicates`）即使结果符合计划，`approval_status` 也是 `manual_confirm_required`，不能自动放行。
- 用户问“它能做什么”时，先简要概括能力，再给最相关的命令例子。
- 用户问精确参数、命令格式或推荐流程时，优先用 `references/usage.md` 里的 repo 实际命令。

## 推荐输出结构

默认用这个简洁结构回答，除非用户要求更详细：

- 该用哪个命令
- 最安全的 preview 命令
- 如果需要，再给 apply 命令
- 最后补一个注意事项或小建议

## 边界

- 不要编造 `tui`、`ops`、`actor-links`、`nfo-check` 之外的 rust-jav 命令。
- 不要再说 `scripts/verify_migration_counts.sh`，该脚本已删除，验收层已内建到 `--apply` 流程。
- 不要把 `verification_status=mismatch` 说成“可以忽略的小告警”；它应被当作人工确认门槛。
- 不要暗示“不加 `--apply` 也会修改文件”或“不加 `--apply` 也能触发验收”。
- 不要假设 `./examples/test` 一定是干净的，除非刚运行过 fixture 脚本。

## 已知陷阱

- **NFS 跨文件系统硬链接失败**：如果 source 目录和 actors_root 在不同文件系统（如 NFS mount），硬链接会报 `Operation not permitted (os error 1)`。确保两个目录在同一文件系统上。
- **NFS root_squash 导致硬链接失败**：TrueNAS 默认启用 `root_squash`，root 所有的文件无法被普通用户硬链接。症状：`Operation not permitted (os error 1)`，但文件确实在同一 NFS 挂载上。修复：在 NFS 服务器上 `chown` 这些文件为客户端用户（如 `prajna:prajna`），或在 TrueNAS NFS 共享设置中关闭 `root_squash`。
- **验证报告路径**：`--apply` 后的详细 JSON 报告写入 `.omx/reports/migrations/` 目录，stdout 只输出摘要。
- **plan_conflicts**：`actor-links` 中多个源文件指向同一目标路径时会产生 `duplicate actor-link target` 警告，这些链接会被跳过。
