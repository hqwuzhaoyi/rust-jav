import type { ReactNode } from "react";

export function DetailSection({ title, children }: { title: string; children: ReactNode }) {
  return <section className="detail-section"><h2>{title}</h2>{children}</section>;
}

export function DataList({ items }: { items: Array<readonly [string, ReactNode | null | undefined]> }) {
  return <dl className="detail-list">{items.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value ?? "未提供"}</dd></div>)}</dl>;
}

export function InspectorStatus({
  name, label, description, tone, action,
}: { name: string; label: string; description: string; tone: "normal" | "synchronizing" | "exception"; action?: ReactNode }) {
  return <div className={`detail-status ${tone}`}><div><b>{name}</b><span>{label}</span></div><p>{description}</p>{action}</div>;
}

export function Info({ k, v }: { k: string; v: ReactNode | null | undefined }) {
  return <div><dt>{k}</dt><dd>{v ?? "未提供"}</dd></div>;
}
