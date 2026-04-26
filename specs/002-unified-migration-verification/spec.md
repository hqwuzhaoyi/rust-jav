# Feature Specification: 统一迁移验收层

**Feature Branch**: `002-unified-migration-verification`  
**Created**: 2026-04-26  
**Status**: Draft  
**Input**: 为 `ops` 与 `actor-links` 建立统一的迁移验收层，在每次迁移前生成 before/expected，迁移后生成 after，并用清单级比对决定是否允许继续自动流程。

## User Scenarios & Testing *(mandatory)*

### User Story 1 - `ops` 在迁移后输出统一验收报告 (Priority: P1)

作为执行 `ops --apply` 的用户，我希望命令在执行前后自动生成迁移验收报告，明确告诉我源目录在迁移前有多少文件、理论目标应该是多少文件、迁移后实际有多少文件，以及哪些文件缺失、多余或不匹配，这样我才能安全决定是否接受迁移结果。

**Why this priority**: `ops` 是当前最主要的文件迁移入口，统一验收层的价值首先体现在它是否能阻断错误迁移和帮助 AI/用户分析异常。

**Independent Test**: 准备一个只包含 move/rename 类操作的目录，执行 `ops --apply` 后应产生报告文件，报告中 `before/expected/after` 一致，`verification_status=ok`，并给出可解析的 `report_path`。

**Acceptance Scenarios**:

1. **Given** 一个仅包含 move / rename 操作的源目录, **When** 用户执行 `cargo run -- ops --dir <dir> --op standardize-names --apply --json`, **Then** 命令输出中包含 `before_count`、`expected_count`、`after_count` 和 `report_path`
2. **Given** 迁移结果与理论目标完全一致, **When** 验收层完成比对, **Then** `verification_status=ok` 且 `approval_status=auto_pass`
3. **Given** 迁移后缺少某个理论上应该存在的文件, **When** 验收层完成比对, **Then** 报告中列出 `missing_files`，并返回 `verification_status=mismatch`
4. **Given** 迁移后出现理论目标之外的文件, **When** 验收层完成比对, **Then** 报告中列出 `unexpected_files`，并阻断自动继续
5. **Given** 同一路径文件存在但属性不符, **When** 验收层比较 `expected` 与 `after`, **Then** 报告中列出 `mismatched_files`

---

### User Story 2 - destructive `ops` 即使技术正确也必须人工确认 (Priority: P1)

作为执行 `delete-ad-files` 或 `remove-duplicates` 的用户，我希望即使命令按计划完成，也不会被系统当成“自动通过”，而是明确标记为需要人工确认，这样我不会在删除类操作后误把结果直接放行。

**Why this priority**: 删除类操作即使“符合计划”，也具备不可逆风险；迁移验收层必须把“技术正确”和“业务放行”拆开。

**Independent Test**: 使用 `delete-ad-files` fixture 执行 `ops --apply`，如果 after 正好等于 expected，应返回 `verification_status=ok` 但 `approval_status=manual_confirm_required`。

**Acceptance Scenarios**:

1. **Given** 用户执行 `delete-ad-files`, **When** 实际结果与理论目标完全一致, **Then** 报告中的 `verification_status=ok`
2. **Given** 用户执行 `delete-ad-files`, **When** 验收层计算放行状态, **Then** 报告中的 `approval_status=manual_confirm_required`
3. **Given** 用户执行 `remove-duplicates`, **When** 结果与理论目标一致, **Then** 系统仍不能自动继续后续无人值守流程
4. **Given** destructive `ops` 的实际结果与理论目标不一致, **When** 验收层完成比对, **Then** `verification_status=mismatch` 且 `approval_status=blocked`

---

### User Story 3 - `actor-links` 在普通迁移场景下验证 source 不变、actors_root 完整 (Priority: P1)

作为执行 `actor-links --apply` 的用户，我希望系统同时验证 source 目录没有被误改，以及 `actors_root` 精确等于理论应该生成的演员链接集合，这样我才能确认演员视图既没有漏建，也没有错误地影响原始媒体目录。

**Why this priority**: `actor-links` 的目标不是移动源文件，而是在新的 scope 中生成派生结果；如果只看一个总数，无法同时保护 source 和 actors_root。

**Independent Test**: 从空的 `actors_root` 开始执行 `actor-links --apply`，source before/expected/after 必须一致，actors_root after 必须精确等于 expected，报告中无 `missing_files`/`unexpected_files`。

**Acceptance Scenarios**:

1. **Given** source 中包含带演员信息的 NFO 和相关文件, **When** 用户执行 `actor-links --apply`, **Then** 报告同时包含 `source` 与 `actors_root` 两个 scope 的 before/expected/after 统计
2. **Given** `actor-links` 正确生成所有目标硬链接, **When** 验收层完成比对, **Then** `source` scope 的 `before=expected=after` 且 `actors_root` scope 的 `after=expected`
3. **Given** `actor-links` 漏生成一条硬链接, **When** 验收层完成比对, **Then** 报告在 `actors_root` scope 中列出对应 `missing_files`
4. **Given** `actor-links` 意外修改了 source 目录, **When** 验收层完成比对, **Then** `source` scope 进入 `mismatch` 并阻断自动继续

---

### User Story 4 - `actor-links` 在“迁移到一半”的 actors 目录中支持增量验证 (Priority: P2)

作为在半完成状态的 `actors_root` 上重复运行 `actor-links` 的用户，我希望系统能识别已有正确内容并只要求补齐缺失部分，但最终仍然要求 after 精确等于完整 expected manifest，这样我可以安全地在增量场景中继续迁移而不会忽略残缺结果。

**Why this priority**: `actor-links` 很容易出现多次增量运行的场景，如果验收层只支持“从空目录开始”，就不能作为稳定门禁。

**Independent Test**: 准备一个已经存在一半演员链接的 `actors_root`，再次执行 `actor-links --apply`；如果补齐后完整匹配 expected，应返回 `ok`，否则返回 `mismatch` 并列出缺失项。

**Acceptance Scenarios**:

1. **Given** `actors_root` 中已存在一部分正确文件, **When** 用户再次执行 `actor-links --apply`, **Then** 报告区分 `expected_existing_links` 与 `expected_new_links`
2. **Given** 已有文件正确且缺失部分被补齐, **When** 验收层完成比对, **Then** `actors_root after == actors_root expected`
3. **Given** 已有文件虽然存在但内容/属性不对, **When** 验收层完成比对, **Then** 报告中列出 `mismatched_files`
4. **Given** 增量运行后仍有理论目标未补齐, **When** 验收层完成比对, **Then** 返回 `verification_status=mismatch`

---

### Edge Cases

- source 目录与目标目录之一不存在或无法扫描时，验收层返回 `verification_status=error`
- 计划中多个动作写入同一目标路径时，expected manifest 构建失败并阻断执行结果放行
- `ops` 中的 `clean-empty-dirs` 不改变文件集合，不应触发文件清单级 diff 误报
- `actor-links` 重复运行时，已有目标文件若符合预期，可记为 `expected_existing_links` 而非异常
- `actor-links` 生成的目标与 source 必须分 scope 验证，不能混在一个计数里比较
- 报告文件写入失败时，即使命令动作本身完成，也必须标记为 `verification_status=error`

## Requirements *(mandatory)*

### Functional Requirements

**统一迁移验收流程**

- **FR-001**: 系统 MUST 为 `ops --apply` 和 `actor-links --apply` 执行统一迁移验收流程：生成 before manifest、expected manifest、after manifest，并进行比对
- **FR-002**: 系统 MUST 在验收结果摘要中输出 `before_count`、`expected_count`、`after_count`
- **FR-003**: 系统 MUST 将详细验收报告落盘为 JSON 文件，并输出 `report_path`
- **FR-004**: 系统 MUST 在报告中分别给出 `verification_status` 与 `approval_status`
- **FR-005**: 系统 MUST 仅在 `verification_status=ok` 且 `approval_status=auto_pass` 时允许自动继续后续流程

**manifest 与比对能力**

- **FR-006**: 系统 MUST 以文件清单为核心进行比对，而不是仅比较单一总数
- **FR-007**: manifest 中的每个文件项 MUST 至少记录 `scope`、`relative_path`、`file_name`、`extension`、`size`
- **FR-008**: manifest 中的每个文件项 MUST 支持记录 `origin.before_entry_id` 以追溯迁移前来源
- **FR-009**: manifest 中的每个文件项 MUST 支持记录 `planning.action_ids` 以关联迁移动作
- **FR-010**: `expected` 与 `after` 的比对 MUST 产出 `missing_files`、`unexpected_files`、`mismatched_files`
- **FR-011**: 系统 MUST 在报告中输出按扩展名汇总的 before/expected/after 统计

**`ops` 验收要求**

- **FR-012**: 系统 MUST 能根据 `ops` 的迁移动作，从 before manifest 推导 expected manifest
- **FR-013**: `ops` 的 move/rename 类动作 MUST 在 expected manifest 中更新目标路径并保留来源追溯信息
- **FR-014**: `ops` 的 delete-file 类动作 MUST 从 expected manifest 中移除对应文件项
- **FR-015**: `ops` 的 `clean-empty-dirs` MUST NOT 影响文件清单级 expected/after 判定
- **FR-016**: 若 `ops` 包含 destructive 操作（至少 `delete-ad-files` 与 `remove-duplicates`），系统 MUST 返回 `approval_status=manual_confirm_required`

**`actor-links` 验收要求**

- **FR-017**: 系统 MUST 为 `actor-links` 同时验证 `source` 与 `actors_root` 两个 scope
- **FR-018**: `actor-links` 的 `source` expected manifest MUST 默认为 before manifest 不变
- **FR-019**: `actor-links` 的 `actors_root` expected manifest MUST 等于“已有正确内容 + 本次计划应新增的链接项”
- **FR-020**: `actor-links` 的 manifest MUST 记录 `link_info.link_type`，并在硬链接项上标记其 source 来源
- **FR-021**: `actor-links` MUST 支持增量验证：区分 `expected_new_links` 与 `expected_existing_links`

**状态与退出码**

- **FR-022**: 系统 MUST 支持 `verification_status` 的三种状态：`ok`、`mismatch`、`error`
- **FR-023**: 系统 MUST 支持 `approval_status` 的三种状态：`auto_pass`、`manual_confirm_required`、`blocked`
- **FR-024**: 当发生文件缺失、多余、属性不符、计划冲突或执行失败时，系统 MUST 返回 `verification_status=mismatch` 或 `error`
- **FR-025**: 系统 MUST 提供稳定的退出码语义，以便 shell、AI 与自动流程消费

### Key Entities

- **ManifestEntry**: 表示某个 scope 下的单个文件项，包含路径、文件名、扩展名、大小、来源追溯信息、关联动作信息，以及可选的链接元数据
- **MigrationAction**: 表示统一验收层可理解的一条迁移动作，包含动作类型、源路径、目标路径、作用 scope、原因与稳定 action id
- **VerificationPlan**: 表示一次迁移验收输入，包含命令名、作用 scope、动作集合、人工确认策略
- **VerificationReport**: 表示一次迁移验收的完整 JSON 报告，包含 before/expected/after、diff、状态、报告路径与统计摘要
- **ScopeSummary**: 表示某个 scope 的 before/expected/after 统计、按扩展名统计及差异摘要

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 所有 `ops --apply` 与 `actor-links --apply` 在成功执行后都能生成可读取的 JSON 验收报告
- **SC-002**: 对于纯 move / rename 场景，验收层 100% 产出 `verification_status=ok` 且 `after_count == expected_count`
- **SC-003**: 对于删除类 `ops` 场景，验收层 100% 产出 `approval_status=manual_confirm_required`
- **SC-004**: 对于故意构造的缺失文件或多余文件场景，验收层 100% 能在报告中列出对应 `missing_files` 或 `unexpected_files`
- **SC-005**: 对于 `actor-links` 的增量迁移场景，验收层能正确区分 `expected_existing_links` 与 `expected_new_links`
- **SC-006**: CLI 摘要输出始终包含 `verification_status`、`approval_status` 与 `report_path`
- **SC-007**: 报告中的清单与差异信息足以让 AI 在不重新扫描文件系统的情况下分析大多数迁移异常

## Clarifications

### Session 2026-04-26

- Q: 迁移门禁应该以什么标准放行？ → A: 只有 `verification_status=ok` 且 `approval_status=auto_pass` 才允许继续
- Q: destructive 操作如果结果符合计划，能不能自动放行？ → A: 不能；需要 `manual_confirm_required`
- Q: 这套能力是分别放进 `ops` / `actor-links`，还是抽统一层？ → A: 抽成统一迁移验收层，由两个命令复用
- Q: compare 只做 count 还是做清单级？ → A: 做清单级，并输出具体内容或报告文件路径
- Q: `actor-links` 面对一半迁移完的 `actors_root` 怎么办？ → A: 支持增量验证，最终 after 仍必须精确等于完整 expected
- Q: 对于这次设计，最重要的输出是什么？ → A: 我们只要知道目标就行，即 before / expected / after 及清单级差异必须明确

## Technical Constraints

- **TC-001**: 首版验收层必须复用现有 `ops` 与 `actor-links` 命令入口，不新增必须使用的新顶层命令
- **TC-002**: 首版必须以文件清单为核心，不要求目录级完整性成为阻断条件
- **TC-003**: 首版 MUST 支持 `source` 与 `actors_root` 的 scope 分离，避免混合统计导致误判
- **TC-004**: 首版报告格式必须是稳定的 JSON，供 AI 与脚本消费
- **TC-005**: 首版在 compare 判定中至少使用路径、扩展名、大小与来源追溯信息；全量内容 hash 可延后

## Assumptions

- `ops` 与 `actor-links` 在 apply 模式下具备稳定、可枚举的计划动作，可映射到统一 `MigrationAction`
- 源目录与目标目录都位于本地文件系统，可在执行前后进行稳定扫描
- AI 或自动流程会优先消费报告文件，而不是只依赖 stdout 文本
- 文件级门禁是首要目标，目录级门禁可以在后续版本补充
