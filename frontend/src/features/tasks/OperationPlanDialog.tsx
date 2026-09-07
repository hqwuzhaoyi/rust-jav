import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { CopyPathButton } from "./CopyPathButton";
import { operations, taskKindLabel, type Task } from "./types";

export function OperationPlanDialog({
  task,
  close,
  confirm,
}: {
  task: Task | null;
  close: () => void;
  confirm: (planId: string) => void;
}) {
  const plan = task?.operation_plan;
  return (
    <Dialog
      open={Boolean(task && plan)}
      title="确认操作计划"
      description={task ? <>计划 <code>{task.id}</code> 将执行已检查的快照。</> : undefined}
      className="operation-plan-modal"
      contentClassName="confirm-dialog operation-plan-confirmation"
      onClose={close}
    >
      {plan && task && (
        <>
          <p className="eyebrow">操作计划</p>
          {plan.warnings.map((warning) => <p className="task-error" key={warning}>{warning}</p>)}
          <ol className="confirmation-operations">
            {plan.operations.map((operation) => (
              <li key={operation}>{operations.find(([key]) => key === operation)?.[1] ?? operation}</li>
            ))}
          </ol>
          <Accordion type="single" defaultValue="actions" className="confirmation-action-review" aria-label={`检查 ${plan.actions.length} 个已保存操作`}>
            <AccordionItem value="actions">
              <AccordionTrigger>{plan.actions.length} 个已保存操作</AccordionTrigger>
              <AccordionContent>
                <ol>
                  {plan.actions.map((action, index) => (
                    <li className={action.destructive ? "destructive" : ""} key={`${action.kind}-${action.path}-${index}`}>
                      <b>{taskKindLabel(action.kind)}</b>
                      {action.source !== undefined || action.target !== undefined ? (
                        <>
                          <span className="task-item-path"><code>来源 {action.source ?? "—"}</code>{action.source && <CopyPathButton path={action.source} />}</span>
                          <span className="task-item-path"><code>目标 {action.target ?? "—"}</code>{action.target && <CopyPathButton path={action.target} />}</span>
                        </>
                      ) : <span className="task-item-path"><code>{action.path ?? "—"}</code>{action.path && <CopyPathButton path={action.path} />}</span>}
                      {action.warning && <small>{action.warning}</small>}
                    </li>
                  ))}
                </ol>
              </AccordionContent>
            </AccordionItem>
          </Accordion>
          <div className="confirm-actions">
            <Button variant="secondary" onClick={close}>取消</Button>
            <Button variant="destructive" onClick={() => confirm(task.id)}>执行已确认计划</Button>
          </div>
        </>
      )}
    </Dialog>
  );
}
