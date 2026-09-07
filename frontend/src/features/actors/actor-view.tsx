import {
  type ReactNode,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  AlertTriangle,
  ArrowDown,
  ArrowUp,
  Ellipsis,
  RefreshCw,
  Trash2,
  UserRound,
  X,
} from "lucide-react";
import { AlertDialog, Sheet } from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";

export type ActorAsset = {
  id: string;
  path: string;
  jav_code: string | null;
  title: string | null;
  artwork_url: string | null;
  captured_date: string;
  state: "normal" | "synchronizing" | "exception";
  exception: string | null;
};

export type ActorFolder = {
  name: string;
  movie_count: number;
  hard_link_count: number;
  logical_size: number;
  reclaimable_space: number;
  poster_url: string | null;
  derived_file_count?: number;
  unique_inode_count?: number;
  linked_assets?: ActorAsset[];
};

export type ActorLoadState = "idle" | "loading" | "ready" | "error";
type ActorSortKey = "name" | "count" | "size";
type SortDirection = "asc" | "desc";

function formatBytes(value: number) {
  if (!value) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
  const amount = value / 1024 ** index;
  return `${amount >= 10 || index === 0 ? amount.toFixed(0) : amount.toFixed(1)} ${units[index]}`;
}

const ACTOR_POSTER_FALLBACK =
  "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 400 600'%3E%3Crect width='400' height='600' fill='%232c3f5d'/%3E%3Ccircle cx='200' cy='220' r='72' fill='%2398a9bf'/%3E%3Cpath d='M76 510c14-114 72-171 124-171s110 57 124 171' fill='%2398a9bf'/%3E%3C/svg%3E";

function ActorPortrait({
  actor,
  loading,
}: {
  actor: ActorFolder;
  loading?: "lazy";
}) {
  const [failed, setFailed] = useState(false);
  const unavailable = !actor.poster_url || failed;
  return (
    <img
      src={unavailable ? ACTOR_POSTER_FALLBACK : actor.poster_url ?? ACTOR_POSTER_FALLBACK}
      alt={unavailable ? `${actor.name} 暂无头像` : `${actor.name} 头像`}
      loading={loading}
      onError={() => setFailed(true)}
    />
  );
}

function ActorAssetArtwork({ asset }: { asset: ActorAsset }) {
  return asset.artwork_url ? (
    <img src={asset.artwork_url} alt="" />
  ) : (
    <span className="linked-asset-fallback" aria-hidden="true">影片</span>
  );
}

function ActorMetric({ k, v }: { k: string; v: ReactNode }) {
  return <div><dt>{k}</dt><dd>{v}</dd></div>;
}

export function ActorFolders({
  actors,
  state,
  inspect,
  retry,
}: {
  actors: ActorFolder[];
  state: ActorLoadState;
  inspect: (actor: ActorFolder) => void;
  retry: () => void;
}) {
  const [sortKey, setSortKey] = useState<ActorSortKey>("name");
  const [sortDirection, setSortDirection] = useState<SortDirection>("asc");
  const [query, setQuery] = useState("");
  const sortedActors = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    const nameOrder = (left: ActorFolder, right: ActorFolder) =>
      left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
    return actors
      .filter((actor) => !normalized || actor.name.toLocaleLowerCase().includes(normalized))
      .sort((left, right) => {
        const order =
          sortKey === "count"
            ? left.movie_count - right.movie_count
            : sortKey === "size"
              ? left.logical_size - right.logical_size
              : nameOrder(left, right);
        if (order === 0) return nameOrder(left, right);
        return sortDirection === "asc" ? order : -order;
      });
  }, [actors, query, sortDirection, sortKey]);
  if (state === "loading")
    return (
      <div className="actor-feedback" role="status" aria-label="正在加载演员目录">
        <RefreshCw aria-hidden="true" />
        <p>正在加载演员目录…</p>
      </div>
    );
  if (state === "error")
    return (
      <div className="actor-feedback actor-error" role="alert">
        <AlertTriangle aria-hidden="true" />
        <h2>无法加载演员目录</h2>
        <p>派生演员视图暂时不可用。</p>
        <Button onClick={retry}>重试</Button>
      </div>
    );
  if (state === "ready" && !actors.length)
    return (
      <div className="empty">
        <span><UserRound aria-hidden="true" /></span>
        <h2>暂无演员目录</h2>
        <p>请根据 NFO 元数据生成派生演员视图。</p>
      </div>
    );
  return (
    <>
      <div className="actor-sort-toolbar" aria-label="演员排序和筛选">
        <Input
          aria-label="筛选演员"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="筛选演员名称"
        />
        <label>
          <span>排序</span>
          <Select
            aria-label="演员排序字段"
            value={sortKey}
            onChange={(event) => {
              const nextKey = event.target.value as ActorSortKey;
              setSortKey(nextKey);
              setSortDirection(nextKey === "name" ? "asc" : "desc");
            }}
          >
            <option value="name">演员名</option>
            <option value="count">媒体资产数量</option>
            <option value="size">逻辑大小</option>
          </Select>
        </label>
        <Button
          variant="outline"
          className="actor-sort-direction"
          aria-label={sortDirection === "asc" ? "切换为降序" : "切换为升序"}
          onClick={() => setSortDirection((current) => current === "asc" ? "desc" : "asc")}
        >
          {sortDirection === "asc" ? <ArrowUp aria-hidden="true" /> : <ArrowDown aria-hidden="true" />}
          {sortDirection === "asc" ? "升序" : "降序"}
        </Button>
      </div>
      {!sortedActors.length && query ? (
        <div className="actor-feedback" role="status">
          <p>没有匹配“{query}”的演员目录。</p>
        </div>
      ) : (
        <div className="actor-folder-grid">
          {sortedActors.map((actor) => (
            <Card className="actor-folder-card" key={actor.name}>
              <Button
                variant="ghost"
                className="actor-folder-open"
                aria-label={`打开演员 ${actor.name}`}
                onClick={() => inspect(actor)}
              >
                <div className="actor-folder-poster" style={{ aspectRatio: "2 / 3" }}>
                  <ActorPortrait actor={actor} loading="lazy" />
                  <div>
                    <b>{actor.name}</b>
                    <p>{actor.movie_count} 个媒体资产 · {formatBytes(actor.logical_size)}</p>
                  </div>
                </div>
              </Button>
            </Card>
          ))}
        </div>
      )}
    </>
  );
}

export function ActorInspectorSheet({
  actor,
  loading,
  error,
  close,
  openAsset,
  remove,
  retry,
  linkedFocusRef,
  selectedAssetIds,
  onSelectedAssetIdsChange,
  onPermanentDelete,
}: {
  actor: ActorFolder | null;
  loading: boolean;
  error: string | null;
  close: () => void;
  openAsset: (asset: ActorAsset) => void;
  remove: (actor: ActorFolder) => void;
  retry: () => void;
  linkedFocusRef: { current: string | null };
  selectedAssetIds: ReadonlySet<string>;
  onSelectedAssetIdsChange: (ids: Set<string>) => void;
  onPermanentDelete: (assetIds: string[]) => void;
}) {
  const contentRef = useRef<HTMLElement>(null);
  useEffect(() => {
    if (!actor || !linkedFocusRef.current || !contentRef.current) return;
    const target = Array.from(contentRef.current.querySelectorAll<HTMLElement>("[data-asset-id]"))
      .find((element) => element.dataset.assetId === linkedFocusRef.current);
    if (target) {
      target.focus();
      linkedFocusRef.current = null;
    }
  }, [actor, linkedFocusRef]);
  const title = actor ? actor.name : "演员目录详情";
  return (
    <Sheet
      open
      onClose={close}
      title={title}
      className="actor-inspector-sheet"
      contentClassName="actor-inspector"
    >
      <section ref={contentRef} className="actor-inspector-content">
        <div className="sheet-handle" aria-hidden="true" />
        <Button
          className="inspector-close ui-icon-button"
          variant="secondary"
          onClick={close}
          aria-label="关闭演员详情"
        >
          <X aria-hidden="true" />
        </Button>
        {actor ? (
          <DropdownMenu>
            <DropdownMenuTrigger className="actor-action-menu-trigger ui-icon-button" aria-label="更多操作">
              <Ellipsis aria-hidden="true" />
            </DropdownMenuTrigger>
            <DropdownMenuContent className="actor-action-menu-popover" aria-label="演员操作">
              <DropdownMenuItem
                density="touch"
                className="actor-remove-menu-item"
                onClick={() => remove(actor)}
              >
                <Trash2 aria-hidden="true" /> 删除演员目录…
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        ) : null}
        {loading && !actor ? <p role="status">正在加载演员目录…</p> : actor ? (
          <>
            <div className="actor-detail-hero">
              <ActorPortrait actor={actor} />
              <div><p className="eyebrow">演员视图</p><p className="actor-detail-name">{actor.name}</p></div>
            </div>
            <dl className="actor-metrics">
              <ActorMetric k="派生路径" v={String(actor.derived_file_count ?? actor.hard_link_count)} />
              <ActorMetric k="去重文件" v={String(actor.unique_inode_count ?? actor.movie_count)} />
              <ActorMetric k="逻辑大小" v={formatBytes(actor.logical_size)} />
              <ActorMetric k="可回收空间" v={formatBytes(actor.reclaimable_space)} />
            </dl>
            <span className="sr-only">引用的逻辑大小</span>
            <span className="sr-only">移除后可回收空间</span>
            <section className="linked-assets">
              <div className="section-title"><h3>关联媒体资产</h3><span>{actor.linked_assets?.length ?? 0}</span></div>
              {(actor.linked_assets ?? []).length ? (
                <div className="linked-asset-grid">
                  {(actor.linked_assets ?? []).map((asset) => (
                    <div className="actor-linked-asset" key={asset.id}>
                      <Checkbox
                        checked={selectedAssetIds.has(asset.id)}
                        aria-label={`选择资产 ${asset.jav_code ?? asset.title ?? "媒体资产"}`}
                        onChange={() => {
                          const next = new Set(selectedAssetIds);
                          if (next.has(asset.id)) next.delete(asset.id);
                          else next.add(asset.id);
                          onSelectedAssetIdsChange(next);
                        }}
                      />
                      <Button
                        variant="ghost"
                        data-asset-id={asset.id}
                        aria-label={`打开资产 ${asset.jav_code ?? asset.title ?? "媒体资产"}`}
                        onClick={() => openAsset(asset)}
                      >
                        <ActorAssetArtwork asset={asset} />
                        <span><b>{asset.jav_code ?? "媒体资产"}</b><small>{asset.title ?? asset.path}</small></span>
                      </Button>
                    </div>
                  ))}
                </div>
              ) : <p className="muted">暂无关联媒体资产。</p>}
            </section>
          </>
        ) : null}
        {actor && selectedAssetIds.size > 0 ? (
          <div className="actor-deletion-selection-bar" role="status">
            <span>已选择 {selectedAssetIds.size} 个媒体资产</span>
            <Button variant="destructive" onClick={() => onPermanentDelete([...selectedAssetIds])}>
              永久删除源媒体…
            </Button>
          </div>
        ) : null}
        {!loading && error ? (
          <div className="actor-feedback actor-detail-error" role="alert">
            <AlertTriangle aria-hidden="true" />
            <h3>{error}</h3>
            <p>演员目录仍然存在，请重新读取当前文件系统状态。</p>
            <Button onClick={retry}>重试演员目录</Button>
          </div>
        ) : null}
      </section>
    </Sheet>
  );
}

export function ActorFolderRemovalAlert({
  actor,
  busy,
  cancel,
  remove,
}: {
  actor: ActorFolder | null;
  busy: boolean;
  cancel: () => void;
  remove: () => void;
}) {
  return (
    <AlertDialog
      open={Boolean(actor)}
      onClose={cancel}
      title={actor ? `移除 ${actor.name}？` : "移除演员目录"}
      description="这是仅移除派生路径的操作。"
      className="actor-removal-modal"
      contentClassName="confirm-dialog"
      dismissible
    >
      {actor ? (
        <>
          <p>
            只会解除此演员目录下的派生演员视图路径。源媒体资产、NFO 元数据和 Jellyfin 项目都不会被删除。
          </p>
          <Card className="actor-removal-metrics">
            <dl>
              <ActorMetric k="演员目录" v={actor.name} />
              <ActorMetric k="影片" v={actor.movie_count} />
              <ActorMetric k="派生路径" v={actor.derived_file_count ?? actor.hard_link_count} />
              <ActorMetric k="去重文件" v={actor.unique_inode_count ?? 0} />
              <ActorMetric k="引用的逻辑大小" v={formatBytes(actor.logical_size)} />
              <ActorMetric k="移除后可回收空间" v={formatBytes(actor.reclaimable_space)} />
            </dl>
          </Card>
          <p className="regenerate-note">
            之后可根据源 NFO 元数据重新生成演员链接。硬链接要求演员视图和媒体根目录位于同一文件系统。
          </p>
          <div className="dialog-actions">
            <Button variant="outline" disabled={busy} onClick={cancel}>取消</Button>
            <Button variant="destructive" disabled={busy} onClick={remove}>通过管理任务移除</Button>
          </div>
        </>
      ) : null}
    </AlertDialog>
  );
}
