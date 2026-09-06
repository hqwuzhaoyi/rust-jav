import type { ReactNode } from "react";
import { AlertTriangle, Film, RefreshCw, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { TiltCard } from "@/components/motion/tilt-card";
import type { AssetState, MediaAsset, MediaAssetPage } from "./model";

const labels: Record<AssetState, string> = {
  normal: "正常",
  synchronizing: "同步中",
  exception: "异常",
};

export function MediaAssetGallery({
  assets, query, filter, loading, error, inspectedAssetId, onQueryChange,
  onFilterChange, onPageChange, onInspect, onRetry, renderArtwork, formatDate,
}: {
  assets: MediaAssetPage;
  query: string;
  filter: AssetState | "";
  loading: boolean;
  error: boolean;
  inspectedAssetId: string | null;
  onQueryChange: (query: string) => void;
  onFilterChange: (filter: AssetState | "") => void;
  onPageChange: (page: number) => void;
  onInspect: (asset: MediaAsset) => void;
  onRetry: () => void;
  renderArtwork: (asset: MediaAsset) => ReactNode;
  formatDate: (date: string) => string;
}) {
  const groups = assets.groups
    .map((group) => ({ group, items: assets.items.filter((asset) => asset.captured_date === group.date) }))
    .filter((entry) => entry.items.length);
  return <>
    <div className="toolbar">
      <label className="search">
        <Search aria-hidden="true" />
        <Input aria-label="搜索资产" placeholder="搜索番号、标题或路径" value={query} onChange={(event) => onQueryChange(event.target.value)} />
      </label>
      <Tabs defaultValue="all" value={filter || "all"} onValueChange={(value) => onFilterChange(value === "all" ? "" : value as AssetState)} variant="segment" className="gallery-filter-tabs">
        <TabsList label="资产状态筛选">
          <TabsTrigger value="all">全部</TabsTrigger>
          <TabsTrigger value="normal">正常</TabsTrigger>
          <TabsTrigger value="synchronizing">刷新中</TabsTrigger>
          <TabsTrigger value="exception">异常</TabsTrigger>
        </TabsList>
      </Tabs>
    </div>
    <div className="library" aria-busy={loading}>
      {loading ? <Card className="gallery-feedback gallery-loading" role="status" aria-label="正在加载媒体资产"><RefreshCw aria-hidden="true" /><span>正在加载媒体资产…</span></Card>
        : error ? <Card className="gallery-feedback gallery-error" role="alert"><AlertTriangle aria-hidden="true" /><h2>无法加载媒体资产</h2><p>请检查连接后重试。</p><Button variant="outline" onClick={onRetry}>重试</Button></Card>
        : groups.length === 0 ? <Card className="empty"><Film aria-hidden="true" /><h2>暂无媒体资产</h2><p>请重新扫描 Media Root，Asset Index 会以文件系统为准更新。</p></Card>
        : groups.map(({ group, items }) => <section className="date-group" key={group.date}>
          <div className="date-heading"><h2>{formatDate(group.date)}</h2><span>{group.count} 项</span></div>
          <div className="asset-grid">{items.map((asset) => <TiltCard className={`asset-card photos-tile ${inspectedAssetId === asset.id ? "selected" : ""}`} key={asset.id}>
            <Card className="overflow-hidden border-0 bg-transparent shadow-none">
              <Button variant="ghost" className="asset-select" onClick={() => onInspect(asset)} aria-label={`查看资产 ${asset.jav_code ?? asset.title ?? "未识别资产"}`}>
                <div className="poster" style={{ aspectRatio: "4 / 3" }}>
                  {renderArtwork(asset)}
                  <div className="asset-overlay"><Film aria-hidden="true" /><span><b>{asset.jav_code ?? asset.title ?? "未识别"}</b><small>{asset.title ?? asset.path.split("/").pop()}</small></span>
                    <Badge variant={asset.state === "normal" ? "success" : asset.state === "synchronizing" ? "warning" : "destructive"} className={`state-label ${asset.state}`}>{labels[asset.state]}</Badge>
                  </div>
                </div>
              </Button>
            </Card>
          </TiltCard>)}</div>
        </section>)}
    </div>
    {assets.total_pages > 1 && <nav className="pagination" aria-label="资产分页">
      <Button variant="outline" disabled={assets.page === 1} onClick={() => onPageChange(assets.page - 1)}>上一页</Button>
      <span aria-live="polite">{assets.page} / {assets.total_pages}</span>
      <Button variant="outline" disabled={assets.page === assets.total_pages} onClick={() => onPageChange(assets.page + 1)}>下一页</Button>
    </nav>}
  </>;
}
