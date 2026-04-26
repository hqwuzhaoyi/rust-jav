---
name: rust-jav-guide
description: 讲解如何使用 rust-jav 通过 `tui`、`ops`、`actor-links` 和统一迁移验收输出整理并核对 JAV 媒体目录。适用于用户询问 rust-jav 怎么运行、某个命令做什么、该用哪些参数、preview / `--apply` / `--json` 的区别、如何生成示例 fixtures、如何检查迁移前后有没有遗漏文件，或想要一套安全的 rust-jav 常见使用流程时。
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
   - 想检查迁移前后是否有遗漏 → 优先读 `--apply` 输出里的 `verification_status` / `approval_status` / `report_path`
2. 先强调安全规则：文件类操作默认都是 **preview**，只有加 `--apply` 才会真正改动文件系统。
3. 优先给出可以直接复制的命令，不要只讲抽象概念。
4. 如果示例里用到 `./examples/test`，提醒用户先重新生成 fixtures：
   - `bash examples/create_test_files.sh ./examples/test`
   因为之前执行过 `--apply` 后，示例目录可能已经不是初始状态。

## 回答要点

- 用户想看详细计划时，推荐加 `--json`。
- 讲到 `delete-ad-files` 时，必须明确提醒：`--apply` 下可能删除匹配到的**视频文件**。
- 用户说“全量整理”时，要说明不加 `--op` 的 `ops` 会跑完整流程，且 `delete-ad-files` 会最先执行。
- 用户说“查有没有遗漏”“迁移前后文件数对比”“统计迁移前后文件总数”时，优先给 `--apply` 输出里的统一迁移验收摘要，并指出详细 diff 在 `report_path`。
- 讲统一迁移验收时，必须明确：`verification_status=ok` 只表示技术结果匹配理论目标；是否能继续自动流程还要看 `approval_status`。
- 讲统一迁移验收时，优先把退出码也说清楚：`0=auto_pass`，`10=manual_confirm_required`，`20=mismatch`，`30=error`。
- 讲迁移校验时，必须明确：`delete-ad-files` / `remove-duplicates` 会让 `approval_status=manual_confirm_required`；`actor-links` 需要同时看 `source` 和 `actors_root` 两个 scope。
- 讲迁移门禁时，必须明确：只有 `status=ok` 才能继续自动迁移；只要是 `status=mismatch`，就要停下来做人工确认。
- 用户问“它能做什么”时，先简要概括能力，再给最相关的命令例子。
- 用户问精确参数、命令格式或推荐流程时，优先用 `references/usage.md` 里的 repo 实际命令。

## 推荐输出结构

默认用这个简洁结构回答，除非用户要求更详细：

- 该用哪个命令
- 最安全的 preview 命令
- 如果需要，再给 apply 命令
- 最后补一个注意事项或小建议

## 边界

- 不要编造 `tui`、`ops`、`actor-links` 之外的 rust-jav 命令。
- 不要把统一迁移验收层说成“只比总数”；它会输出清单级 diff，并把详细内容落到 `report_path`。
- 不要把 `status=mismatch` 说成“可以忽略的小告警”；它应被当作人工确认门槛。
- 不要默认 `./examples/test` 是干净的；命令验证前通常需要先重跑 `bash examples/create_test_files.sh ./examples/test`。
- 不要暗示“不加 `--apply` 也会修改文件”。
- 不要假设 `./examples/test` 一定是干净的，除非刚运行过 fixture 脚本。
