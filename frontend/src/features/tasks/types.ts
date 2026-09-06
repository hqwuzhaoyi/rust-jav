export type OperationPlan = {
  operations: string[];
  actions: Array<{
    kind: string;
    path: string | null;
    source?: string | null;
    target?: string | null;
    destructive: boolean;
    warning: string | null;
  }>;
  warnings: string[];
  requires_confirmation: boolean;
};

export type TaskItem = {
  id: number;
  kind: string;
  path: string | null;
  status: string;
  message: string | null;
};

export type Task = {
  id: string;
  task_type: string;
  media_root: string;
  kind: "preview" | "mutation";
  status: "queued" | "running" | "completed" | "failed" | "interrupted";
  created_at: number;
  error: string | null;
  plan_expires_at: number | null;
  operation_plan: OperationPlan | null;
  report: Record<string, unknown> | null;
  source_plan_id?: string | null;
  plan_consumed_at?: number | null;
  planned_item_count?: number | null;
  items: TaskItem[];
};

export type TaskDisplayStatus = Task["status"] | "blocked-for-confirmation";

export const operations = [
  ["delete_ad_files", "删除广告文件"],
  ["organize_by_code", "按番号整理"],
  ["clean_empty_dirs", "清理空目录"],
  ["standardize_names", "规范文件名"],
  ["extract_codes", "提取番号"],
  ["categorize_files", "分类文件"],
  ["move_origin", "移动到 ORIGIN"],
  ["remove_duplicates", "移除重复文件"],
] as const;

export const taskStatusLabels: Record<TaskDisplayStatus, string> = {
  queued: "排队中",
  running: "运行中",
  "blocked-for-confirmation": "等待确认",
  completed: "已完成",
  failed: "失败",
  interrupted: "已中断",
};

export const taskKindLabels: Record<Task["kind"], string> = {
  preview: "预览",
  mutation: "执行",
};

export const taskItemStatusLabels: Record<string, string> = {
  queued: "排队中",
  running: "运行中",
  completed: "已完成",
  applied: "已应用",
  deleted: "已删除",
  changed: "已变更",
  failed: "失败",
  planned: "已计划",
  skipped: "已跳过",
  interrupted: "已中断",
  deleted_needs_audit: "已删除，待审计",
};

export function taskDisplayStatus(task: Task): TaskDisplayStatus {
  if (
    task.kind === "preview" &&
    task.status === "completed" &&
    task.operation_plan?.requires_confirmation &&
    task.plan_expires_at !== null &&
    !task.plan_consumed_at &&
    Date.now() / 1000 <= task.plan_expires_at
  ) return "blocked-for-confirmation";
  return task.status;
}

export function taskProgressPercent(task: Task) {
  const total = task.planned_item_count ?? task.items.length;
  if (total <= 0) return undefined;
  const finished = task.items.filter((item) =>
    ["completed", "applied", "deleted", "changed", "failed", "planned", "skipped"].includes(item.status),
  ).length;
  return Math.min(100, Math.round(finished / total * 100));
}

export function isCompleteTask(task: Partial<Task>): task is Task {
  return (
    typeof task.id === "string" &&
    typeof task.task_type === "string" &&
    typeof task.media_root === "string" &&
    (task.kind === "preview" || task.kind === "mutation") &&
    typeof task.status === "string" &&
    typeof task.created_at === "number" &&
    Array.isArray(task.items)
  );
}

export function taskKindLabel(kind: string) {
  const known = operations.find(([key]) => key === kind)?.[1];
  return known ?? ({
    permanent_deletion: "永久删除",
    remove_actor_folder: "移除演员目录",
    operations: "整理操作",
  } as Record<string, string>)[kind] ?? kind;
}
