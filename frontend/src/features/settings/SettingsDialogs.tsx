import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";

export type RuleActivation = { yaml: string; empty: boolean } | null;

export function RuleActivationDialog({
  activation,
  pending,
  close,
  activate,
}: {
  activation: RuleActivation;
  pending: boolean;
  close: () => void;
  activate: () => void;
}) {
  if (!activation) return null;
  return (
    <Dialog
      open
      title={activation.empty ? "启用空规则集" : "启用规则集"}
      description={activation.empty
        ? "此规则集没有启用的规则。在启用其他规则集前，删除候选将保持为空。"
        : "已验证的草案将替换当前规则集。"}
      className="settings-confirmation-modal"
      contentClassName="confirm-dialog settings-confirmation"
      onClose={close}
    >
      <p className="eyebrow">{activation.empty ? "高风险变更" : "规则草案"}</p>
      <pre className="rule-activation-preview">{activation.yaml}</pre>
      <div className="dialog-actions">
        <Button type="button" variant="secondary" disabled={pending} onClick={close}>取消</Button>
        <Button type="button" variant={activation.empty ? "destructive" : "default"} disabled={pending} onClick={activate}>
          {pending ? "正在启用…" : activation.empty ? "启用空规则集" : "启用规则集"}
        </Button>
      </div>
    </Dialog>
  );
}

export function DiscardSettingsDialog({
  open,
  keepEditing,
  discard,
}: {
  open: boolean;
  keepEditing: () => void;
  discard: () => void;
}) {
  return (
    <Dialog
      open={open}
      title="放弃未保存的更改？"
      description="规则和 Jellyfin 的修改尚未保存。"
      className="settings-confirmation-modal"
      contentClassName="confirm-dialog settings-confirmation"
      onClose={keepEditing}
    >
      <p className="eyebrow">未保存的设置</p>
      <div className="dialog-actions">
        <Button type="button" variant="secondary" onClick={keepEditing}>继续编辑</Button>
        <Button type="button" variant="destructive" onClick={discard}>放弃更改</Button>
      </div>
    </Dialog>
  );
}
