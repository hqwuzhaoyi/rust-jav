import type { FormEvent } from "react";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import {
  operations,
  taskDisplayStatus,
  taskItemStatusLabels,
  taskKindLabel,
  taskKindLabels,
  taskProgressPercent,
  taskStatusLabels,
  type Task,
} from "./types";

export function TaskPanel({
  tasks,
  taskTotal,
  hasMoreTasks,
  historyPageLoading,
  mediaRoot,
  setMediaRoot,
  selectedOps,
  setSelectedOps,
  createTask,
  requestPlanConfirmation,
  refresh,
  loadMore,
}: {
  tasks: Task[];
  taskTotal: number;
  hasMoreTasks: boolean;
  historyPageLoading: boolean;
  mediaRoot: string;
  setMediaRoot: (v: string) => void;
  selectedOps: string[];
  setSelectedOps: (v: string[]) => void;
  createTask: (e: FormEvent) => void;
  requestPlanConfirmation: (task: Task) => void;
  refresh: () => Promise<void>;
  loadMore: () => Promise<void>;
}) {
  const toggle = (key: string) =>
    setSelectedOps(
      selectedOps.includes(key)
        ? selectedOps.filter((value) => value !== key)
        : [...selectedOps, key],
    );
  return (
    <div className="task-dashboard">
      <Card className="task-create p-4">
        <h2>新建操作计划</h2>
        <form className="task-form" onSubmit={createTask}>
          <label htmlFor="media-root">媒体根目录</label>
          <Input
            id="media-root"
            value={mediaRoot}
            onChange={(e) => setMediaRoot(e.target.value)}
            placeholder="/media/library"
            required
          />
          <div className="operation-heading">
            <label>操作</label>
            <Button
              type="button"
              variant="secondary"
              onClick={() => setSelectedOps(operations.map(([key]) => key))}
            >
              完整流程
            </Button>
          </div>
          <div className="operation-list">
            {operations.map(([key, label]) => (
              <label key={key}>
                <Checkbox
                  checked={selectedOps.includes(key)}
                  onChange={() => toggle(key)}
                />
                <span>{label}</span>
              </label>
            ))}
          </div>
          <Button type="submit" variant="default" disabled={!selectedOps.length}>
            预览 15 分钟有效的计划
          </Button>
        </form>
      </Card>
      <Card className="task-history p-4">
        <div className="task-title">
          <div>
            <h2>任务生命周期</h2>
            <p>持久化历史、实时进度、报告与验证</p>
            <p className="task-count">{taskTotal} 个任务</p>
          </div>
          <Button variant="outline" className="refresh" onClick={() => void refresh()}>
            刷新
          </Button>
        </div>
        {tasks.length === 0 ? (
          <p className="task-empty">暂无管理任务。</p>
        ) : (
          <ol className="tasks">
            {tasks.map((task) => {
              const progress = taskProgressPercent(task);
              return (
                <li key={task.id}>
                  <div className="task-summary">
                    <Badge
                      className={`status status-${taskDisplayStatus(task)}`}
                      variant={taskDisplayStatus(task) === "failed" ? "destructive" : "secondary"}
                    >
                      {taskStatusLabels[taskDisplayStatus(task)]}
                    </Badge>
                    <strong>{taskKindLabels[task.kind]}</strong>
                    <span className="task-root">{task.media_root}</span>
                  </div>
                  <small>{task.items.length} 个项目结果 · {task.id}</small>
                  {(task.status === "queued" || task.status === "running") && (
                    <Progress aria-label="任务进度" value={progress ?? 0} className="task-progress" />
                  )}
                  {task.error && (task.status === "failed" ? (
                    <p className="task-error" role="alert">{task.error}</p>
                  ) : (
                    <p className="task-error">{task.error}</p>
                  ))}
                  {task.operation_plan && (
                    <Accordion type="multiple" defaultValue="plan" className="plan">
                      <AccordionItem value="plan">
                        <AccordionTrigger>
                          <span>检查最终路径 · 到期时间 {new Date(task.plan_expires_at! * 1000).toLocaleTimeString()}</span>
                        </AccordionTrigger>
                        <AccordionContent>
                          {task.operation_plan.warnings.map((warning) => (
                            <p className="task-error" key={warning}>{warning}</p>
                          ))}
                          <ul>
                            {task.operation_plan.actions.map((action, index) => (
                              <li className={action.destructive ? "destructive" : ""} key={`${action.kind}-${action.path}-${index}`}>
                                <span>{action.destructive ? "破坏性操作" : taskKindLabel(action.kind)}</span>
                                <code>{action.path ?? "—"}</code>
                              </li>
                            ))}
                          </ul>
                          {task.status === "completed" && !task.plan_consumed_at && Date.now() / 1000 <= task.plan_expires_at! && (
                            <Button variant="destructive" onClick={() => requestPlanConfirmation(task)}>确认并执行</Button>
                          )}
                        </AccordionContent>
                      </AccordionItem>
                    </Accordion>
                  )}
                  {task.items.length > 0 && (
                    <Accordion type="multiple" defaultValue="items" className="task-items">
                      <AccordionItem value="items">
                        <AccordionTrigger>逐项结果（{task.items.length}）</AccordionTrigger>
                        <AccordionContent>
                          <ul>
                            {task.items.map((item) => (
                              <li key={item.id}>
                                <span>{taskItemStatusLabels[item.status] ?? item.status}</span>
                                <b>{taskKindLabel(item.kind)}</b>
                                <span className="task-item-path">
                                  <code>{item.path ?? "—"}</code>
                                  {item.path && (
                                    <Button
                                      type="button"
                                      variant="ghost"
                                      className="copy-path"
                                      aria-label={`复制完整路径 ${item.path}`}
                                      onClick={() => void navigator.clipboard?.writeText(item.path!)}
                                    >
                                      复制
                                    </Button>
                                  )}
                                </span>
                                {item.message && <small>{item.message}</small>}
                              </li>
                            ))}
                          </ul>
                        </AccordionContent>
                      </AccordionItem>
                    </Accordion>
                  )}
                  {task.report && (
                    <Accordion type="single" defaultValue="report">
                      <AccordionItem value="report">
                        <AccordionTrigger>最终报告和迁移验证</AccordionTrigger>
                        <AccordionContent><pre>{JSON.stringify(task.report, null, 2)}</pre></AccordionContent>
                      </AccordionItem>
                    </Accordion>
                  )}
                </li>
              );
            })}
          </ol>
        )}
        {hasMoreTasks && (
          <Button type="button" variant="secondary" className="show-more-tasks" disabled={historyPageLoading} onClick={() => void loadMore()}>
            {historyPageLoading ? "正在加载任务…" : "再加载 20 个任务"}
          </Button>
        )}
      </Card>
    </div>
  );
}
