import type { FormEvent, RefObject } from "react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";

type Validation = { valid: true; empty: boolean; yaml: string } | null;
type PendingRuleAction = "download" | "validate" | "activate" | null;
type JellyfinLoadState = "idle" | "loading" | "ready" | "error";
type JellyfinAction = "test" | "refresh" | null;

export function SettingsPage({
  sourceUrl,
  setSourceUrl,
  yaml,
  editing,
  validation,
  rulesMessage,
  rulesError,
  rulesPending,
  downloadProposal,
  updateYaml,
  beginEditing,
  validateRules,
  reviewRuleActivation,
  jfUrl,
  setJfUrl,
  jfLibraries,
  setJfLibraries,
  jfKey,
  setJfKey,
  jfKeyConfigured,
  jfDirty,
  jfLoadState,
  jfSaving,
  jfError,
  jellyfinAction,
  saveJellyfin,
  loadJellyfinConfig,
  testJellyfin,
  refreshJellyfin,
  ruleHeadingRef,
}: {
  sourceUrl: string;
  setSourceUrl: (value: string) => void;
  yaml: string;
  editing: boolean;
  validation: Validation;
  rulesMessage: string;
  rulesError: string;
  rulesPending: PendingRuleAction;
  downloadProposal: () => void;
  updateYaml: (value: string) => void;
  beginEditing: () => void;
  validateRules: () => void;
  reviewRuleActivation: () => void;
  jfUrl: string;
  setJfUrl: (value: string) => void;
  jfLibraries: string;
  setJfLibraries: (value: string) => void;
  jfKey: string;
  setJfKey: (value: string) => void;
  jfKeyConfigured: boolean;
  jfDirty: boolean;
  jfLoadState: JellyfinLoadState;
  jfSaving: boolean;
  jfError: string;
  jellyfinAction: JellyfinAction;
  saveJellyfin: (event: FormEvent) => void;
  loadJellyfinConfig: () => void;
  testJellyfin: () => void;
  refreshJellyfin: () => void;
  ruleHeadingRef: RefObject<HTMLHeadingElement | null>;
}) {
  const jellyfinBusy = jfSaving || jellyfinAction !== null;
  return (
    <div className="settings-stack">
      <section className="rules-settings">
      <Card className="p-4">
        <p className="eyebrow">删除规则</p>
        <h2 ref={ruleHeadingRef} tabIndex={-1}>当前规则集</h2>
        <p>远程 YAML 只是规则草案。服务器验证后原子启用；规则不能选择根目录或授权删除。</p>
        <label htmlFor="rule-source">规则来源 URL</label>
        <div className="rule-actions">
          <Input
            id="rule-source"
            type="url"
            placeholder="https://raw.githubusercontent.com/…"
            value={sourceUrl}
            disabled={rulesPending !== null}
            onChange={(event) => setSourceUrl(event.target.value)}
          />
          <Button type="button" variant="secondary" disabled={!sourceUrl || rulesPending !== null} onClick={downloadProposal}>
            {rulesPending === "download" ? "正在下载草案…" : "下载草案"}
          </Button>
        </div>
        <label htmlFor="rules-yaml">当前规则集 YAML</label>
        <Textarea
          id="rules-yaml"
          rows={18}
          readOnly={!editing}
          disabled={rulesPending !== null}
          value={yaml}
          onChange={(event) => updateYaml(event.target.value)}
        />
        <div className="rule-actions">
          {!editing && <Button type="button" variant="secondary" onClick={beginEditing}>编辑</Button>}
          {editing && <Button type="button" variant="secondary" disabled={rulesPending !== null} onClick={validateRules}>{rulesPending === "validate" ? "正在验证…" : "验证"}</Button>}
          {editing && !validation?.empty && <Button type="button" variant="default" disabled={!validation || validation.yaml !== yaml} onClick={reviewRuleActivation}>保存当前规则集</Button>}
          {editing && validation?.empty && <Button type="button" variant="destructive" disabled={validation.yaml !== yaml} onClick={reviewRuleActivation}>确认空规则并保存</Button>}
        </div>
        {rulesMessage && <p role="status" className="notice">{rulesMessage}</p>}
        {rulesError && <p role="alert" className="notice settings-error">{rulesError}</p>}
      </Card>
      </section>
      <section className="jellyfin-settings" aria-busy={jfLoadState === "loading" ? "true" : undefined}>
      <Card className="p-4">
        <p className="eyebrow">媒体服务器</p>
        <h2>Jellyfin</h2>
        <p>连接一个服务器并选择多个媒体库 ID。API 密钥只保存在本服务器。</p>
        <form className="task-form" onSubmit={saveJellyfin}>
          {jfDirty && <p className="settings-dirty">有未保存的更改</p>}
          <label htmlFor="jellyfin-url">服务器 URL</label>
          <Input id="jellyfin-url" type="url" value={jfUrl} disabled={jellyfinBusy} onChange={(event) => setJfUrl(event.target.value)} placeholder="http://jellyfin:8096" required />
          <label htmlFor="jellyfin-libraries">媒体库 ID</label>
          <Input id="jellyfin-libraries" value={jfLibraries} disabled={jellyfinBusy} onChange={(event) => setJfLibraries(event.target.value)} placeholder="movies, jav" required />
          <label htmlFor="jellyfin-key">服务器 API 密钥</label>
          <Input id="jellyfin-key" type="password" autoComplete="off" value={jfKey} disabled={jellyfinBusy} onChange={(event) => setJfKey(event.target.value)} required={!jfKeyConfigured} />
          <Button type="submit" variant="default" disabled={!jfDirty || jellyfinBusy}>{jfSaving ? "正在保存 Jellyfin…" : "保存 Jellyfin"}</Button>
          {jfError && <p role="alert" className="notice settings-error">{jfError}</p>}
          {jfLoadState === "error" && <Button type="button" variant="secondary" className="settings-retry" onClick={loadJellyfinConfig}>重新加载 Jellyfin 设置</Button>}
        </form>
        <div className="jellyfin-actions">
          <Button type="button" variant="secondary" disabled={jfDirty || jellyfinBusy} onClick={testJellyfin}>{jellyfinAction === "test" ? "正在测试连接…" : "测试连接"}</Button>
          <Button type="button" variant="secondary" disabled={jfDirty || jellyfinBusy} onClick={refreshJellyfin}>{jellyfinAction === "refresh" ? "正在刷新 Jellyfin…" : "刷新 Jellyfin"}</Button>
        </div>
      </Card>
      </section>
    </div>
  );
}
