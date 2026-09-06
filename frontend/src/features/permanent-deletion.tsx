import type { Dispatch, SetStateAction } from "react";
import { AlertDialog } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";

export type DeletionCandidate = {
  path: string;
  matching_rule: string;
  type: string;
  video_warning: string | null;
  logical_size: number;
  reclaimable_space: number;
};

export type DeletionPlan = {
  id: string;
  selection: "selected" | "unified";
  logical_size: number;
  reclaimable_space: number;
  created_at: number;
  expires_at: number;
  hard_link_search_roots: string[];
  paths: Array<{ path: string; type: string; video_warning: string | null }>;
  discovered_hard_links: Array<{ path: string; type: string }>;
};

export type DeletionExecutionTask = {
  id: string;
  task_type: string;
  status: string;
  error: string | null;
  items: Array<{
    path: string | null;
    status: string;
    message: string | null;
  }>;
};

type FormatBytes = (bytes: number) => string;
type FileTypeLabel = (type: string) => string;
type WarningLabel = (warning: string) => string;

export function DeletionCandidateBrowser({
  candidates,
  selected,
  setSelected,
  preview,
  planning,
  formatBytes,
  fileTypeLabel,
  warningLabel,
}: {
  candidates: DeletionCandidate[];
  selected: string[];
  setSelected: Dispatch<SetStateAction<string[]>>;
  preview: (selection: "selected" | "unified") => void;
  planning: boolean;
  formatBytes: FormatBytes;
  fileTypeLabel: FileTypeLabel;
  warningLabel: WarningLabel;
}) {
  return (
    <section className="deletion-browser">
      <div className="deletion-intro">
        <div>
          <p className="eyebrow">当前规则集</p>
          <h2>检查永久删除</h2>
          <p>大小来自当前文件系统观测。只有明确确认操作计划后才会删除文件。</p>
        </div>
        <Button
          variant="destructive"
          disabled={!selected.length || planning}
          onClick={() => preview("selected")}
        >
          {planning ? "正在检查…" : `检查 ${selected.length || "已选择项"}`}
        </Button>
      </div>
      <div className="candidate-list">
        {candidates.map((candidate) => {
          const id = `deletion-candidate-${candidate.path}`;
          const checked = selected.includes(candidate.path);
          return (
            <label className="candidate" key={candidate.path} htmlFor={id}>
              <Card className="candidate-card">
                <Checkbox
                  id={id}
                  aria-label={`选择 ${candidate.path}`}
                  checked={checked}
                  disabled={planning}
                  onChange={(event) =>
                    setSelected((current) =>
                      event.target.checked
                        ? [...current, candidate.path]
                        : current.filter((path) => path !== candidate.path),
                    )
                  }
                />
                <div>
                  <code title={candidate.path}>{candidate.path}</code>
                  <small>
                    规则：{candidate.matching_rule} · {fileTypeLabel(candidate.type)}
                  </small>
                  {candidate.video_warning && <strong>{warningLabel(candidate.video_warning)}</strong>}
                </div>
                <dl>
                  <div>
                    <dt>逻辑大小</dt>
                    <dd>{formatBytes(candidate.logical_size)}</dd>
                  </div>
                  <div>
                    <dt>可回收空间</dt>
                    <dd>{formatBytes(candidate.reclaimable_space)}</dd>
                  </div>
                </dl>
              </Card>
            </label>
          );
        })}
      </div>
      {!candidates.length && <p className="task-empty">没有路径命中当前规则集。</p>}
    </section>
  );
}

export function PermanentDeletionReview({
  plan,
  outcome,
  pending,
  invalid,
  error,
  confirmText,
  onConfirmTextChange,
  onPreview,
  onExecute,
  onClose,
  formatBytes,
  fileTypeLabel,
  warningLabel,
  taskStatusLabel,
}: {
  plan: DeletionPlan | null;
  outcome: DeletionExecutionTask | null;
  pending: "planning" | "executing" | null;
  invalid: boolean;
  error: string;
  confirmText: string;
  onConfirmTextChange: (value: string) => void;
  onPreview: (selection: "selected" | "unified") => void;
  onExecute: () => void;
  onClose: () => void;
  formatBytes: FormatBytes;
  fileTypeLabel: FileTypeLabel;
  warningLabel: WarningLabel;
  taskStatusLabel: (status: string) => string;
}) {
  const open = Boolean(plan || outcome);
  const outcomeTitle = outcome
    ? outcome.status === "failed"
      ? "永久删除已完成，但部分路径失败"
      : outcome.status === "completed"
        ? "永久删除已完成"
        : outcome.status === "interrupted"
          ? "永久删除已中断"
          : `永久删除状态：${taskStatusLabel(outcome.status)}`
    : undefined;

  return (
    <AlertDialog
      open={open}
      dismissible={!pending}
      onClose={onClose}
      initialAnimation={false}
      className="permanent-deletion-modal"
      contentClassName={outcome ? "delete-confirm deletion-outcome" : "delete-confirm"}
      title={outcomeTitle ?? (plan ? `永久删除 ${plan.paths.length} 个路径？` : undefined)}
      description={
        outcome
          ? undefined
          : "服务器会在解除链接前重新验证每个文件系统身份。只有这份最新操作计划能够授权变更。"
      }
    >
      {outcome ? (
        <DeletionOutcome outcome={outcome} onClose={onClose} />
      ) : plan ? (
        <DeletionPlanReview
          plan={plan}
          pending={pending}
          invalid={invalid}
          error={error}
          confirmText={confirmText}
          onConfirmTextChange={onConfirmTextChange}
          onPreview={onPreview}
          onExecute={onExecute}
          onClose={onClose}
          formatBytes={formatBytes}
          fileTypeLabel={fileTypeLabel}
          warningLabel={warningLabel}
        />
      ) : null}
    </AlertDialog>
  );
}

function DeletionPlanReview({
  plan,
  pending,
  invalid,
  error,
  confirmText,
  onConfirmTextChange,
  onPreview,
  onExecute,
  onClose,
  formatBytes,
  fileTypeLabel,
  warningLabel,
}: {
  plan: DeletionPlan;
  pending: "planning" | "executing" | null;
  invalid: boolean;
  error: string;
  confirmText: string;
  onConfirmTextChange: (value: string) => void;
  onPreview: (selection: "selected" | "unified") => void;
  onExecute: () => void;
  onClose: () => void;
  formatBytes: FormatBytes;
  fileTypeLabel: FileTypeLabel;
  warningLabel: WarningLabel;
}) {
  const busy = Boolean(pending);
  return (
    <>
      <p className="eyebrow">不可撤销操作</p>
      <ToggleGroup
        value={plan.selection}
        aria-label="删除范围"
        className="choice"
        onValueChange={(selection) => {
          if (selection === "selected" || selection === "unified") onPreview(selection);
        }}
      >
        <ToggleGroupItem className="choice-item ui-touch-target" disabled={busy} value="selected">
          仅选择的路径
        </ToggleGroupItem>
        <ToggleGroupItem className="choice-item ui-touch-target" disabled={busy} value="unified">
          所有已发现硬链接（{plan.discovered_hard_links.length}）
        </ToggleGroupItem>
      </ToggleGroup>
      <dl className="deletion-plan-metrics">
        <div><dt>逻辑大小</dt><dd>{formatBytes(plan.logical_size)}</dd></div>
        <div><dt>可回收空间</dt><dd>{formatBytes(plan.reclaimable_space)}</dd></div>
      </dl>
      <section className="deletion-scope" aria-labelledby="hard-link-roots-title">
        <h3 id="hard-link-roots-title">硬链接搜索根目录</h3>
        <ScrollArea aria-label="硬链接搜索根目录" className="deletion-root-list">
          {(plan.hard_link_search_roots ?? []).map((root) => <code key={root}>{root}</code>)}
        </ScrollArea>
      </section>
      {plan.paths.some((path) => path.video_warning) && (
        <p className="video-warning">⚠ 此计划会永久删除视频内容。</p>
      )}
      <DeletionPathList
        title="此计划已批准的路径"
        titleId="approved-paths-title"
        paths={plan.paths}
        fileTypeLabel={fileTypeLabel}
        warningLabel={warningLabel}
      />
      {plan.selection === "selected" && plan.discovered_hard_links.length > 0 && (
        <DeletionPathList
          title="已发现但未批准的硬链接"
          titleId="discovered-links-title"
          paths={plan.discovered_hard_links}
          fileTypeLabel={fileTypeLabel}
          discovered
        />
      )}
      {plan.selection === "unified" && plan.discovered_hard_links.length > 0 && (
        <p className="unified-scope-note">
          已发现的 {plan.discovered_hard_links.length} 个硬链接全部包含在上述批准路径中。
        </p>
      )}
      {error && <p className="deletion-inline-error" role="alert">{error}</p>}
      {invalid && (
        <Button
          type="button"
          className="fresh-plan-button"
          disabled={busy}
          onClick={() => onPreview(plan.selection)}
        >
          {pending === "planning" ? "正在创建最新操作计划…" : "创建最新操作计划"}
        </Button>
      )}
      <label htmlFor="confirm-delete">输入 <b>PERMANENTLY DELETE</b> 进行确认</label>
      <Input
        id="confirm-delete"
        value={confirmText}
        disabled={invalid || busy}
        onChange={(event) => onConfirmTextChange(event.target.value)}
        autoComplete="off"
      />
      <div className="confirm-actions">
        <Button type="button" variant="secondary" disabled={busy} onClick={onClose}>取消</Button>
        <Button
          type="button"
          variant="destructive"
          disabled={invalid || busy || confirmText !== "PERMANENTLY DELETE"}
          onClick={onExecute}
        >
          {pending === "executing" ? "正在重新验证并删除…" : "永久删除"}
        </Button>
      </div>
    </>
  );
}

function DeletionPathList({
  title,
  titleId,
  paths,
  fileTypeLabel,
  warningLabel,
  discovered = false,
}: {
  title: string;
  titleId: string;
  paths: Array<{ path: string; type: string; video_warning?: string | null }>;
  fileTypeLabel: FileTypeLabel;
  warningLabel?: WarningLabel;
  discovered?: boolean;
}) {
  return (
    <section className="deletion-scope" aria-labelledby={titleId}>
      <h3 id={titleId}>{title}</h3>
      <ScrollArea className="plan-paths" aria-label={title}>
        {paths.map((path) => (
          <div className="deletion-path" key={path.path}>
            <code style={{ overflowWrap: "anywhere" }}>{path.path}</code>
            <span>{discovered ? "已发现 · " : ""}{fileTypeLabel(path.type ?? "file")}</span>
            {path.video_warning && warningLabel && <small>{warningLabel(path.video_warning)}</small>}
          </div>
        ))}
      </ScrollArea>
    </section>
  );
}

function DeletionOutcome({ outcome, onClose }: { outcome: DeletionExecutionTask; onClose: () => void }) {
  return (
    <>
      <p className="eyebrow">文件系统结果</p>
      {outcome.error && <p className="deletion-inline-error">{outcome.error}</p>}
      <ScrollArea className="deletion-outcome-list" aria-label="永久删除逐路径结果">
        <ol>
          {outcome.items.map((item, index) => (
            <li key={`${item.path}-${index}`}>
              <div>
                <b>{item.status === "deleted" ? "已删除" : item.status === "changed" ? "计划后已被替换" : "失败"}</b>
                <code style={{ overflowWrap: "anywhere" }}>{item.path ?? "未知路径"}</code>
              </div>
              {item.message && <p>{item.message}</p>}
            </li>
          ))}
        </ol>
      </ScrollArea>
      {outcome.status !== "completed" && <p className="no-rollback">未尝试回滚。</p>}
      <div className="confirm-actions">
        <Button type="button" variant="secondary" onClick={onClose}>关闭</Button>
      </div>
    </>
  );
}
