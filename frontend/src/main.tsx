import React, {
  FormEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { createRoot } from "react-dom/client";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import {
  AlertTriangle,
  ArrowDown,
  ArrowLeft,
  ArrowUp,
  Clock3,
  Ellipsis,
  Film,
  Grid2X2,
  HardDrive,
  Image as ImageIcon,
  ListTodo,
  LogOut,
  RefreshCw,
  Settings,
  Trash2,
  UserRound,
  Users,
  X,
} from "lucide-react";
import { MorphingModal } from "./components/motion/morphing-modal";
import { Button } from "./components/ui/button";
import { Card } from "./components/ui/card";
import { Input } from "./components/ui/input";
import { Progress } from "./components/ui/progress";
import { Toast } from "./components/ui/toast";
import {
  DeletionCandidateBrowser,
  PermanentDeletionReview,
  type DeletionCandidate as Candidate,
  type DeletionExecutionTask,
  type DeletionPlan,
} from "./features/permanent-deletion";
import { SettingsPage } from "./features/settings/SettingsPage";
import { DiscardSettingsDialog, RuleActivationDialog } from "./features/settings/SettingsDialogs";
import { OperationPlanDialog } from "./features/tasks/OperationPlanDialog";
import { TaskPanel } from "./features/tasks/TaskPanel";
import { Sheet } from "./components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./components/ui/tabs";
import { MediaAssetGallery } from "./features/assets/media-asset-gallery";
import { galleryStateFromUrl, galleryUrl } from "./features/assets/url-state";
import {
  DataList,
  DetailSection,
  Info,
  InspectorStatus,
} from "./features/assets/inspector-details";
import { EASE_OUT } from "./lib/ease";
import "./design-system.css";
import "./style.css";
type View = "loading" | "initialize" | "login" | "ready";
type Navigation =
  | "assets"
  | "recent"
  | "exceptions"
  | "actors"
  | "deletion"
  | "tasks"
  | "settings";
const DEFAULT_RULE_SOURCE =
  "https://raw.githubusercontent.com/hqwuzhaoyi/rust-jav/feature/web-jellyfin-truenas/rules.yaml";
type Validation = { valid: true; empty: boolean; yaml: string } | null;
type RuleActivation = { yaml: string; empty: boolean } | null;
type JellyfinBaseline = { url: string; libraries: string };
type JellyfinLoadState = "idle" | "loading" | "ready" | "error";
const EMPTY_JELLYFIN_BASELINE: JellyfinBaseline = { url: "", libraries: "" };
type AssetState = "normal" | "synchronizing" | "exception";
type ArtworkProvenance = {
  status:
    | "missing"
    | "valid"
    | "empty"
    | "unrecognized"
    | "animated"
    | "truncated_or_corrupt"
    | "too_large"
    | "unreadable";
  source_path: string | null;
  content_type: "image/jpeg" | "image/png" | "image/webp" | null;
  error: string | null;
};
type Asset = {
  id: string;
  path: string;
  jav_code: string | null;
  title: string | null;
  artwork_url: string | null;
  captured_date: string;
  state: AssetState;
  exception: string | null;
};
type JellyfinAssociation = {
  status:
    | "played"
    | "in_progress"
    | "unplayed"
    | "not_found"
    | "offline"
    | "not_configured";
  confidence?: "certain_path" | "uncertain_metadata";
  reason?: string;
  play_count?: number;
  playback_position_ticks?: number;
  open_url?: string;
  may_authorize_deletion?: boolean;
};
type AssetDetail = {
  id: string;
  path: string;
  jav_code: string | null;
  title: string | null;
  actors: Array<{
    name: string;
    poster_url: string | null;
    actor_folder_url: string | null;
  }>;
  studio: string | null;
  release_date: string | null;
  runtime_minutes: number | null;
  director: string | null;
  tags: string[];
  plot: string | null;
  parse_status: "valid" | "missing" | "invalid";
  source_path: string | null;
  state: AssetState;
  exception: string | null;
  jellyfin?: JellyfinAssociation;
  artwork_url: string | null;
  artwork?: ArtworkProvenance;
  captured_date: string;
};
type AssetTab = "overview" | "nfo";
type AssetHistoryState = {
  asset?: string;
  tab?: AssetTab;
  assetInspectorEntry?: true;
  assetInspectorDepth?: number;
};
type Page = {
  items: Asset[];
  groups: Array<{ date: string; count: number }>;
  page: number;
  total: number;
  total_pages: number;
};
type Health = { state: string; mode: string | null };
type StorageHealth = {
  roots: Array<{
    path: string;
    readable: boolean;
    writable: boolean;
    action: string | null;
    capacity: {
      status: "healthy" | "degraded";
      total_bytes: number | null;
      used_bytes: number | null;
      available_bytes: number | null;
    };
  }>;
  aggregate: {
    status: "healthy" | "degraded";
    filesystem_count: number;
    total_bytes: number | null;
    used_bytes: number | null;
    available_bytes: number | null;
  };
};
type OperationPlan = {
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
type ActorFolder = {
  name: string;
  movie_count: number;
  hard_link_count: number;
  logical_size: number;
  reclaimable_space: number;
  poster_url: string | null;
  derived_file_count?: number;
  unique_inode_count?: number;
  linked_assets?: Asset[];
};
type LoadState = "idle" | "loading" | "ready" | "error";
type ActorSortKey = "name" | "count" | "size";
type SortDirection = "asc" | "desc";
type Task = {
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
  items: Array<{
    id: number;
    kind: string;
    path: string | null;
    status: string;
    message: string | null;
  }>;
};
type TaskDisplayStatus = Task["status"] | "blocked-for-confirmation";
const taskStatusLabels: Record<TaskDisplayStatus, string> = {
  queued: "排队中",
  running: "运行中",
  "blocked-for-confirmation": "等待确认",
  completed: "已完成",
  failed: "失败",
  interrupted: "已中断",
};
function taskDisplayStatus(task: Task): TaskDisplayStatus {
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
function taskProgressPercent(task: Task) {
  const total = task.planned_item_count ?? task.items.length;
  if (total <= 0) return undefined;
  const finished = task.items.filter((item) =>
    ["completed", "applied", "deleted", "changed", "failed", "planned", "skipped"].includes(item.status),
  ).length;
  return Math.min(100, Math.round(finished / total * 100));
}
function isCompleteTask(task: Partial<Task>): task is Task {
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
function isTaskStatus(status: unknown): status is Task["status"] {
  return ["queued", "running", "completed", "failed", "interrupted"].includes(
    String(status),
  );
}
const operations = [
  ["delete_ad_files", "删除广告文件"],
  ["organize_by_code", "按番号整理"],
  ["clean_empty_dirs", "清理空目录"],
  ["standardize_names", "规范文件名"],
  ["extract_codes", "提取番号"],
  ["categorize_files", "分类文件"],
  ["move_origin", "移动到 ORIGIN"],
  ["remove_duplicates", "移除重复文件"],
] as const;

const taskKindLabels: Record<Task["kind"], string> = {
  preview: "预览",
  mutation: "执行",
};

const taskItemStatusLabels: Record<string, string> = {
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
const kindLabels: Record<string, string> = {
  file: "文件",
  directory: "目录",
  symlink: "符号链接",
  other: "其他",
  permanent_deletion: "永久删除",
  remove_actor_folder: "移除演员目录",
  operations: "整理操作",
};
function kindLabel(kind: string) {
  return operations.find(([key]) => key === kind)?.[1] ?? kindLabels[kind] ?? kind;
}
function fileTypeLabel(kind: string) {
  const labels: Record<string, string> = {
    "regular file": "普通文件",
    file: "文件",
    directory: "目录",
    symlink: "符号链接",
    other: "其他",
  };
  return labels[kind] ?? kind;
}
function deletionWarningLabel(warning: string) {
  return warning === "Video file: permanent deletion removes playable media."
    ? "视频文件：永久删除会移除可播放媒体。"
    : warning;
}
function jellyfinReasonLabel(reason: string) {
  const normalized = reason.toLowerCase();
  if (normalized.includes("normalized media asset path")) return "按规范化媒体资产路径关联";
  if (normalized.includes("normalized relative path suffix")) return "按唯一规范化相对路径后缀关联";
  if (normalized.includes("jav code") || normalized.includes("title metadata")) return "按番号或标题元数据关联";
  return reason;
}
function assetExceptionLabel(message: string) {
  const localizedReason = message.replace(
    "the document does not have a root node",
    "文档缺少根节点",
  );
  if (localizedReason.startsWith("NFO metadata is missing.")) {
    return "NFO 元数据缺失。请添加同目录 .nfo 文件并重新核对资产索引。";
  }
  if (localizedReason.startsWith("NFO metadata file is empty. Regenerate it and reconcile the Asset Index:")) {
    return localizedReason
      .replace("NFO metadata file is empty. Regenerate it and reconcile the Asset Index:", "NFO 元数据文件为空，请重新生成并重新核对资产索引：")
      .replace(/ is empty$/, " 为空");
  }
  if (localizedReason.startsWith("NFO metadata file is empty. Regenerate it:")) {
    return localizedReason
      .replace("NFO metadata file is empty. Regenerate it:", "NFO 元数据文件为空，请重新生成：")
      .replace(/ is empty$/, " 为空");
  }
  if (localizedReason.startsWith("Fix invalid NFO metadata and reconcile the Asset Index:")) {
    return localizedReason.replace("Fix invalid NFO metadata and reconcile the Asset Index:", "NFO 元数据无效，请修复后重新核对资产索引：");
  }
  if (localizedReason.startsWith("NFO metadata is no longer safe or valid:")) {
    return localizedReason.replace("NFO metadata is no longer safe or valid:", "NFO 元数据已不安全或无效：");
  }
  return localizedReason;
}
const labels: Record<AssetState, string> = {
  normal: "正常",
  synchronizing: "同步中",
  exception: "异常",
};
const artworkStatusLabels: Record<ArtworkProvenance["status"], string> = {
  missing: "缺失",
  valid: "有效",
  empty: "空文件",
  unrecognized: "无法识别",
  animated: "动态图片",
  truncated_or_corrupt: "截断或损坏",
  too_large: "文件过大",
  unreadable: "无法读取",
};
const jellyfinStatusLabels: Record<NonNullable<AssetDetail["jellyfin"]>["status"], string> = {
  played: "已播放",
  in_progress: "播放中",
  unplayed: "未播放",
  not_found: "未找到",
  offline: "离线",
  not_configured: "未配置",
};
function artworkStatusLabel(status: ArtworkProvenance["status"]) {
  return artworkStatusLabels[status];
}
function jellyfinStatusLabel(status?: NonNullable<AssetDetail["jellyfin"]>["status"]) {
  return status ? jellyfinStatusLabels[status] : "未配置";
}
function parseStatusLabel(status: AssetDetail["parse_status"]) {
  return status === "valid" ? "有效" : status === "missing" ? "缺失" : "无效";
}
function assetIdFromPath(pathname = location.pathname) {
  const match = pathname.match(/^\/assets\/([^/]+)$/);
  if (!match) return null;
  try {
    return decodeURIComponent(match[1]);
  } catch {
    return null;
  }
}
function assetTabFromSearch(search = location.search): AssetTab {
  return new URLSearchParams(search).get("tab") === "nfo" ? "nfo" : "overview";
}
function assetRoute(id: string, tab: AssetTab) {
  const path = `/assets/${encodeURIComponent(id)}`;
  return tab === "nfo" ? `${path}?tab=nfo` : path;
}
function useMobileBreakpoint() {
  const [mobile, setMobile] = useState(() => window.innerWidth <= 760);
  useEffect(() => {
    const update = () => setMobile(window.innerWidth <= 760);
    addEventListener("resize", update);
    return () => removeEventListener("resize", update);
  }, []);
  return mobile;
}
function normalizeLibraryIds(value: string) {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean)
    .join(", ");
}
function actorRoute(name: string) {
  const bytes = new TextEncoder().encode(name);
  let binary = "";
  bytes.forEach((byte) => (binary += String.fromCharCode(byte)));
  return `/actors/${btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "")}`;
}
function actorNameFromPath(pathname = location.pathname) {
  const match = pathname.match(/^\/actors\/([^/]+)$/);
  if (!match) return null;
  try {
    const encoded = match[1].replaceAll("-", "+").replaceAll("_", "/");
    const binary = atob(encoded.padEnd(Math.ceil(encoded.length / 4) * 4, "="));
    return new TextDecoder().decode(Uint8Array.from(binary, (char) => char.charCodeAt(0)));
  } catch {
    return null;
  }
}
function isActorListPath(pathname = location.pathname) {
  return pathname === "/actors" || pathname === "/actors/";
}
type ActorHistoryState = {
  actor?: string;
  actorInspectorEntry?: true;
};
export function App() {
  const initialGalleryState = useRef(galleryStateFromUrl()).current;
  const token = new URLSearchParams(location.search).get("token"),
    [view, setView] = useState<View>(token ? "initialize" : "loading"),
    [password, setPassword] = useState(""),
    [message, setMessage] = useState(""),
    [submitting, setSubmitting] = useState(false);
  const submittingRef = useRef(false);
  const [assets, setAssets] = useState<Page>({
      items: [],
      groups: [],
      page: 1,
      total: 0,
      total_pages: 0,
    }),
    [libraryTotal, setLibraryTotal] = useState(0),
    [query, setQuery] = useState(initialGalleryState.query),
    [filter, setFilter] = useState<AssetState | "">(initialGalleryState.filter),
    [health, setHealth] = useState<Health | null>(null),
    [storage, setStorage] = useState<StorageHealth | null>(null),
    [storageOpen, setStorageOpen] = useState(false),
    [page, setPage] = useState(initialGalleryState.page),
    [nav, setNav] = useState<Navigation>(
      actorNameFromPath() || isActorListPath() ? "actors" : "assets",
    );
  const [tasks, setTasks] = useState<Task[]>([]),
    [taskTotal, setTaskTotal] = useState(0),
    [hasMoreTasks, setHasMoreTasks] = useState(false),
    [historyPageLoading, setHistoryPageLoading] = useState(false),
    [mediaRoot, setMediaRoot] = useState("/media"),
    [selectedOps, setSelectedOps] = useState<string[]>(
      operations.map(([key]) => key),
    );
  const [planToConfirm, setPlanToConfirm] = useState<Task | null>(null);
  const [inspectedAsset, setInspectedAsset] = useState<Asset | null>(null);
  const [assetDetail, setAssetDetail] = useState<AssetDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [assetTab, setAssetTab] = useState<AssetTab>(assetTabFromSearch);
  const inspectedAssetRef = useRef<Asset | null>(null);
  const assetOpenerRef = useRef<HTMLElement | null>(null);
  const assetDismissRef = useRef(false);
  const [galleryLoading, setGalleryLoading] = useState(true);
  const [galleryError, setGalleryError] = useState(false);
  const [galleryRetry, setGalleryRetry] = useState(0);
  const assetRequest = useRef(0);
  const detailRequest = useRef(0);
  const [yaml, setYaml] = useState("");
  const [activeYaml, setActiveYaml] = useState("");
  const [sourceUrl, setSourceUrl] = useState(DEFAULT_RULE_SOURCE);
  const [editing, setEditing] = useState(false);
  const [validation, setValidation] = useState<Validation>(null);
  const [rulesMessage, setRulesMessage] = useState("");
  const [rulesError, setRulesError] = useState("");
  const [rulesPending, setRulesPending] = useState<
    "download" | "validate" | "activate" | null
  >(null);
  const [ruleActivation, setRuleActivation] = useState<RuleActivation>(null);
  const ruleHeadingRef = useRef<HTMLHeadingElement>(null);
  const focusRuleHeadingAfterActivationRef = useRef(false);
  const [jfUrl, setJfUrl] = useState("");
  const [jfLibraries, setJfLibraries] = useState("");
  const [jfKey, setJfKey] = useState("");
  const [jfKeyConfigured, setJfKeyConfigured] = useState(false);
  const [jfBaseline, setJfBaseline] = useState<JellyfinBaseline>(EMPTY_JELLYFIN_BASELINE);
  const [jfLoadState, setJfLoadState] = useState<JellyfinLoadState>("idle");
  const [jfSaving, setJfSaving] = useState(false);
  const [jfError, setJfError] = useState("");
  const [jellyfinAction, setJellyfinAction] = useState<"test" | "refresh" | null>(null);
  const [pendingNavigation, setPendingNavigation] = useState<{
    run: () => void;
  } | null>(null);
  const rulesLoadGeneration = useRef(0);
  const rulesChangeGeneration = useRef(0);
  const jellyfinLoadGeneration = useRef(0);
  const jellyfinChangeGeneration = useRef(0);
  const navRef = useRef<Navigation>(nav);
  const settingsDirtyRef = useRef(false);
  const restoringSettingsPopRef = useRef(false);
  const allowSettingsPopRef = useRef(false);
  const [candidates, setCandidates] = useState<Candidate[]>([]),
    [selected, setSelected] = useState<string[]>([]),
    [plan, setPlan] = useState<DeletionPlan | null>(null),
    [confirmText, setConfirmText] = useState("");
  const [deletionPending, setDeletionPending] = useState<"planning" | "executing" | null>(null);
  const [deletionPlanInvalid, setDeletionPlanInvalid] = useState(false);
  const [deletionError, setDeletionError] = useState("");
  const [deletionOutcome, setDeletionOutcome] = useState<DeletionExecutionTask | null>(null);
  const deletionPlanGeneration = useRef(0);
  const [actors, setActors] = useState<ActorFolder[]>([]);
  const [actorListState, setActorListState] = useState<LoadState>("idle");
  const [confirmActor, setConfirmActor] = useState<ActorFolder | null>(null);
  const [actorRemovalNotice, setActorRemovalNotice] = useState<string | null>(null);
  const [actorRemovalFailure, setActorRemovalFailure] = useState<string | null>(null);
  const [actorBusy, setActorBusy] = useState(false);
  const [inspectedActor, setInspectedActor] = useState<ActorFolder | null>(null);
  const [actorDetailLoading, setActorDetailLoading] = useState(false);
  const [actorDetailError, setActorDetailError] = useState<string | null>(null);
  const [assetBackActor, setAssetBackActor] = useState<ActorFolder | null>(null);
  const actorOpenerRef = useRef<HTMLElement | null>(null);
  const actorLinkedFocusRef = useRef<string | null>(null);
  const actorRequest = useRef(0);
  const inspectedActorRef = useRef<ActorFolder | null>(null);
  const actorRemovalTasksRef = useRef(new Map<string, string>());
  const taskSourcesRef = useRef(new Map<string, EventSource>());
  const loadedHistoryCountRef = useRef(0);
  const historyPageLoadingRef = useRef(false);
  const historyRequestSequenceRef = useRef(0);
  const taskSnapshotGenerationRef = useRef(new Map<string, number>());
  const taskRecoverySequenceRef = useRef(new Map<string, number>());
  const notifiedTaskStatesRef = useRef(new Set<string>());
  useEffect(() => {
    inspectedAssetRef.current = inspectedAsset;
  }, [inspectedAsset]);
  useEffect(() => {
    inspectedActorRef.current = inspectedActor;
  }, [inspectedActor]);
  useEffect(() => {
    if (ruleActivation || !focusRuleHeadingAfterActivationRef.current) return;
    focusRuleHeadingAfterActivationRef.current = false;
    const frame = requestAnimationFrame(() => ruleHeadingRef.current?.focus());
    return () => cancelAnimationFrame(frame);
  }, [ruleActivation]);
  useEffect(() => () => {
    taskSourcesRef.current.forEach((source) => source.close());
    taskSourcesRef.current.clear();
  }, []);
  useEffect(() => {
    if (!token)
      fetch("/api/v1/status").then((r) => {
        if (r.ok) setView("ready");
        else {
          setView("login");
          if (r.status === 503)
            setMessage("请先在服务器本地运行 rust-jav administrator init。");
        }
      });
  }, [token]);
  useEffect(() => {
    if (view !== "ready") return;
    void loadAssets();
    return () => {
      assetRequest.current += 1;
    };
  }, [view, query, filter, page, galleryRetry]);
  useEffect(() => {
    if (nav === "tasks") void loadTasks();
    if (nav === "settings") {
      void loadRules();
      void loadJellyfinConfig();
    }
    if (nav === "deletion") void loadCandidates();
    if (nav === "actors" && !actorNameFromPath()) void loadActors();
  }, [nav]);
  useEffect(() => {
    if (view !== "ready") return;
    const actorName = actorNameFromPath();
    const selectedId = assetIdFromPath();
    if (actorName) void openActor(actorName, false);
    else if (selectedId) {
      setAssetTab(assetTabFromSearch());
      void inspectById(selectedId);
    }
    const onPopState = () => {
      if (restoringSettingsPopRef.current) {
        restoringSettingsPopRef.current = false;
        return;
      }
      if (allowSettingsPopRef.current) {
        allowSettingsPopRef.current = false;
      } else if (navRef.current === "settings" && settingsDirtyRef.current) {
        restoringSettingsPopRef.current = true;
        setPendingNavigation({
          run: () => {
            allowSettingsPopRef.current = true;
            history.back();
          },
        });
        history.forward();
        return;
      }
      const poppedActor = actorNameFromPath();
      const poppedAssetId = assetIdFromPath();
      setAssetBackActor(null);
      if (poppedActor) {
        void openActor(poppedActor, false);
        if (assetDismissRef.current) {
          assetDismissRef.current = false;
          history.pushState({ actor: poppedActor }, "", actorRoute(poppedActor));
        }
      }
      else if (isActorListPath()) {
        actorRequest.current += 1;
        detailRequest.current += 1;
        setInspectedActor(null);
        setActorDetailError(null);
        setInspectedAsset(null);
        setAssetDetail(null);
        setNav("actors");
      }
      else if (poppedAssetId) {
        setAssetTab(assetTabFromSearch());
        setInspectedActor(null);
        setNav("assets");
        if (inspectedAssetRef.current?.id !== poppedAssetId)
          void inspectById(poppedAssetId);
      }
      else {
        detailRequest.current += 1;
        const restored = galleryStateFromUrl();
        setQuery(restored.query);
        setFilter(restored.filter);
        setPage(restored.page);
        setInspectedAsset(null);
        setAssetDetail(null);
        setInspectedActor(null);
        setNav("assets");
        if (assetDismissRef.current) {
          assetDismissRef.current = false;
          history.pushState({}, "", galleryUrl(restored.query, restored.filter, restored.page));
        }
      }
    };
    addEventListener("popstate", onPopState);
    return () => removeEventListener("popstate", onPopState);
  }, [view]);
  async function loadJellyfinConfig() {
    const generation = ++jellyfinLoadGeneration.current;
    const changeGeneration = jellyfinChangeGeneration.current;
    const mayHydrateDraft = !jfDirty;
    setJfLoadState("loading");
    setJfError("");
    try {
      const response = await fetch("/api/v1/jellyfin/config");
      if (!response.ok) throw new Error(await response.text());
      const config = (await response.json().catch(() => null)) as {
        url?: string | null;
        library_ids?: string[];
        api_key_configured?: boolean;
      } | null;
      if (!config) throw new Error("无法加载 Jellyfin 配置。");
      if (
        generation !== jellyfinLoadGeneration.current ||
        changeGeneration !== jellyfinChangeGeneration.current
      ) return;
      const baseline = {
        url: config.url ?? "",
        libraries: (config.library_ids ?? []).join(", "),
      };
      setJfBaseline(baseline);
      setJfKeyConfigured(config.api_key_configured ?? false);
      if (mayHydrateDraft && changeGeneration === jellyfinChangeGeneration.current) {
        setJfUrl(baseline.url);
        setJfLibraries(baseline.libraries);
        setJfKey("");
      }
      setJfLoadState("ready");
    } catch (error) {
      if (generation !== jellyfinLoadGeneration.current) return;
      setJfLoadState("error");
      setJfError(
        error instanceof Error && error.message
          ? error.message
          : "无法加载 Jellyfin 配置。",
      );
    }
  }
  async function saveJellyfin(event: FormEvent) {
    event.preventDefault();
    if (jfSaving) return;
    const snapshot = {
      url: jfUrl,
      libraries: normalizeLibraryIds(jfLibraries),
      apiKey: jfKey,
    };
    const changeGeneration = jellyfinChangeGeneration.current;
    setJfSaving(true);
    setJfError("");
    try {
      const response = await fetch("/api/v1/jellyfin/config", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          url: snapshot.url,
          library_ids: snapshot.libraries.split(", ").filter(Boolean),
          api_key: snapshot.apiKey,
        }),
      });
      if (!response.ok) {
        throw new Error((await response.text()) || "无法保存 Jellyfin 配置。");
      }
      setJfBaseline({ url: snapshot.url, libraries: snapshot.libraries });
      setJfKeyConfigured(jfKeyConfigured || Boolean(snapshot.apiKey));
      setJfLoadState("ready");
      if (changeGeneration === jellyfinChangeGeneration.current) {
        setJfLibraries(snapshot.libraries);
        setJfKey("");
      }
      setMessage("Jellyfin 配置已保存。");
    } catch (error) {
      setJfError(
        error instanceof Error && error.message
          ? error.message
          : "无法保存 Jellyfin 配置。",
      );
    } finally {
      setJfSaving(false);
    }
  }
  async function testJellyfin() {
    if (jellyfinAction || jfSaving) return;
    setJellyfinAction("test");
    setJfError("");
    try {
      const response = await fetch("/api/v1/jellyfin/test", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: "{}",
      });
      if (!response.ok) throw new Error(await response.text());
      setMessage(`已连接到 ${((await response.json()) as { server_name: string }).server_name}。`);
    } catch (error) {
      setJfError(error instanceof Error && error.message ? error.message : "无法测试 Jellyfin 连接。");
    } finally {
      setJellyfinAction(null);
    }
  }
  async function refreshJellyfin() {
    if (jellyfinAction || jfSaving) return;
    setJellyfinAction("refresh");
    setJfError("");
    try {
      const response = await fetch("/api/v1/jellyfin/refresh", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: "{}",
      });
      if (!response.ok) throw new Error(await response.text());
      setMessage("Jellyfin 媒体库刷新完成。");
    } catch (error) {
      setJfError(error instanceof Error && error.message ? error.message : "无法刷新 Jellyfin。");
    } finally {
      setJellyfinAction(null);
    }
  }
  async function loadActors() {
    setActorListState("loading");
    try {
      const response = await fetch("/api/v1/actors");
      if (!response.ok) throw new Error("无法加载演员目录。");
      const folders = (await response.json().catch(() => null)) as ActorFolder[] | null;
      if (!Array.isArray(folders)) throw new Error("无法加载演员目录。");
      setActors(folders);
      setActorListState("ready");
    } catch {
      setActorListState("error");
    }
  }
  async function openActor(actor: ActorFolder | string, push = true) {
    const name = typeof actor === "string" ? actor : actor.name;
    const active = document.activeElement;
    if (push && active instanceof HTMLElement)
      actorOpenerRef.current = active;
    const request = ++actorRequest.current;
    detailRequest.current += 1;
    setNav("actors");
    setActorDetailLoading(true);
    setActorDetailError(null);
    setInspectedActor(null);
    setInspectedAsset(null);
    setAssetDetail(null);
    if (push) {
      const state: ActorHistoryState = {
        actor: name,
        actorInspectorEntry: true,
      };
      history.pushState(state, "", actorRoute(name));
    }
    try {
      const response = await fetch(`/api/v1/actors/${encodeURIComponent(name)}`);
      if (request !== actorRequest.current) return;
      if (!response.ok) throw new Error("无法加载演员目录。");
      const detail = (await response.json()) as ActorFolder;
      if (request !== actorRequest.current) return;
      setInspectedActor(detail);
    } catch {
      if (request === actorRequest.current)
        setActorDetailError("无法加载演员目录。");
    } finally {
      if (request === actorRequest.current) setActorDetailLoading(false);
    }
  }
  function showActorFolders() {
    actorRequest.current += 1;
    detailRequest.current += 1;
    setActorDetailLoading(false);
    setInspectedActor(null);
    setActorDetailError(null);
    setInspectedAsset(null);
    setAssetDetail(null);
    setNav("actors");
    if (!isActorListPath()) history.pushState({}, "", "/actors");
  }
  function closeActor() {
    actorRequest.current += 1;
    setActorDetailLoading(false);
    setInspectedActor(null);
    setActorDetailError(null);
    setNav("actors");
    const state = history.state as ActorHistoryState | null;
    if (state?.actorInspectorEntry && actorNameFromPath()) history.back();
    else history.replaceState({}, "", "/actors");
  }
  async function openLinkedAsset(asset: Asset) {
    if (inspectedActor) {
      setAssetBackActor(inspectedActor);
      actorLinkedFocusRef.current = asset.id;
    }
    setInspectedActor(null);
    await inspect(asset);
  }
  async function requestActorRemoval(actor: ActorFolder) {
    setActorBusy(true);
    const response = await fetch(
      `/api/v1/actors/${encodeURIComponent(actor.name)}`,
    );
    if (response.ok) setConfirmActor((await response.json()) as ActorFolder);
    else
      setMessage(
        (await response.text()) || "无法重新验证演员目录。",
      );
    setActorBusy(false);
  }
  async function removeActor() {
    if (!confirmActor) return;
    const actorName = confirmActor.name;
    setActorBusy(true);
    setActorRemovalFailure(null);
    const response = await fetch(
      `/api/v1/actors/${encodeURIComponent(confirmActor.name)}`,
      { method: "DELETE" },
    );
    if (!response.ok) {
      setMessage(
        (await response.text()) || "移除演员目录的请求被拒绝。",
      );
      setActorBusy(false);
      return;
    }
    const task = (await response.json()) as Task;
    setTasks((current) => [task, ...current]);
    actorRemovalTasksRef.current.set(task.id, actorName);
    watchTask(task.id);
    setConfirmActor(null);
    setActorRemovalNotice("已创建移除演员目录的管理任务。");
    setActorBusy(false);
  }
  async function loadCandidates() {
    const r = await fetch("/api/v1/deletion-candidates");
    if (r.ok) setCandidates(((await r.json()) as { items: Candidate[] }).items);
  }
  async function previewDeletion(selection: "selected" | "unified") {
    const generation = ++deletionPlanGeneration.current;
    setDeletionPending("planning");
    setDeletionError("");
    setConfirmText("");
    try {
      const r = await fetch("/api/v1/deletion-plans", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ paths: selected, selection }),
      });
      if (generation !== deletionPlanGeneration.current) return;
      if (r.ok) {
        setPlan((await r.json()) as DeletionPlan);
        setDeletionOutcome(null);
        setDeletionPlanInvalid(false);
      } else {
        const error = await r.text();
        setDeletionError(error);
      }
    } finally {
      if (generation === deletionPlanGeneration.current) setDeletionPending(null);
    }
  }
  async function executeDeletion() {
    if (!plan || deletionPending || deletionPlanInvalid) return;
    setDeletionPending("executing");
    setDeletionError("");
    try {
      const r = await fetch(`/api/v1/deletion-plans/${plan.id}/execute`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ irreversible: true, confirmation: confirmText }),
      });
      if (r.ok) {
        const response = (await r.json()) as Partial<DeletionExecutionTask> & Partial<Task>;
        if (!isTaskStatus(response.status)) {
          setConfirmText("");
          setDeletionPlanInvalid(true);
          setDeletionError("服务器未返回持久化删除任务状态。请重新创建操作计划后再试。");
          return;
        }
        const task: DeletionExecutionTask = {
          id: response.id ?? plan.id,
          task_type: response.task_type ?? "permanent_deletion",
          status: response.status,
          error: response.error ?? null,
          items: Array.isArray(response.items) ? response.items : [],
        };
        setDeletionOutcome(task);
        setPlan(null);
        setConfirmText("");
        setSelected([]);
        if (isCompleteTask(response)) {
          setTasks((current) => [
            response,
            ...current.filter((item) => item.id !== response.id),
          ]);
        }
        await loadCandidates();
      } else {
        const error = await r.text();
        setConfirmText("");
        setDeletionError(error);
        // Execution consumes the server plan before mutation starts. Any
        // non-success response therefore requires a fresh plan; retrying the
        // same client snapshot must never be presented as authorized.
        setDeletionPlanInvalid(true);
      }
    } finally {
      setDeletionPending(null);
    }
  }
  function closeDeletionReview() {
    if (deletionPending) return;
    deletionPlanGeneration.current += 1;
    setPlan(null);
    setDeletionOutcome(null);
    setDeletionPlanInvalid(false);
    setDeletionError("");
    setConfirmText("");
  }
  async function loadRules() {
    const generation = ++rulesLoadGeneration.current;
    const changeGeneration = rulesChangeGeneration.current;
    setRulesError("");
    try {
      const response = await fetch("/api/v1/rules/active");
      if (!response.ok) throw new Error("无法加载当前规则集。");
      const body = (await response.json().catch(() => null)) as { yaml?: string } | null;
      if (typeof body?.yaml !== "string")
        throw new Error("无法加载当前规则集。");
      if (
        generation !== rulesLoadGeneration.current ||
        changeGeneration !== rulesChangeGeneration.current
      ) return;
      setYaml(body.yaml);
      setActiveYaml(body.yaml);
    } catch (error) {
      if (generation !== rulesLoadGeneration.current) return;
      setRulesError(
        error instanceof Error ? error.message : "无法加载当前规则集。",
      );
    }
  }
  function updateYaml(value: string) {
    rulesChangeGeneration.current += 1;
    setYaml(value);
    setValidation(null);
    setRulesMessage("");
    setRulesError("");
  }
  async function downloadProposal() {
    if (rulesPending) return;
    setRulesPending("download");
    setRulesError("");
    setRulesMessage("正在下载规则草案…");
    try {
      const response = await fetch("/api/v1/rules/download", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ url: sourceUrl }),
      });
      const body = (await response.json().catch(() => null)) as {
        yaml?: string;
        error?: string;
      } | null;
      if (!response.ok || !body?.yaml) {
        throw new Error(body?.error ?? "下载失败。");
      }
      updateYaml(body.yaml);
      setEditing(true);
      setRulesMessage("规则草案已下载，请验证后再保存。");
    } catch (error) {
      setRulesMessage("");
      setRulesError(error instanceof Error ? error.message : "下载失败。");
    } finally {
      setRulesPending(null);
    }
  }
  async function validateRules() {
    if (rulesPending) return;
    const candidate = yaml;
    const changeGeneration = rulesChangeGeneration.current;
    setRulesPending("validate");
    setRulesError("");
    setRulesMessage("正在验证…");
    try {
      const response = await fetch("/api/v1/rules/validate", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ yaml: candidate }),
      });
      const body = (await response.json().catch(() => null)) as {
        valid?: boolean;
        empty?: boolean;
        error?: string;
      } | null;
      if (!response.ok || !body?.valid) throw new Error(body?.error ?? "验证失败。");
      if (changeGeneration !== rulesChangeGeneration.current) return;
      setValidation({ valid: true, empty: Boolean(body.empty), yaml: candidate });
      setRulesMessage(
        body.empty
          ? "规则有效但为空，需要单独确认。"
          : "规则草案有效，可以保存。",
      );
    } catch (error) {
      setValidation(null);
      setRulesMessage("");
      setRulesError(error instanceof Error ? error.message : "验证失败。");
    } finally {
      setRulesPending(null);
    }
  }
  function reviewRuleActivation() {
    if (!validation || validation.yaml !== yaml || rulesPending) return;
    setRuleActivation({ yaml: validation.yaml, empty: validation.empty });
  }
  async function saveRules(candidate: Exclude<RuleActivation, null>) {
    if (rulesPending) return;
    setRulesPending("activate");
    setRulesError("");
    try {
      const response = await fetch("/api/v1/rules/active", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ yaml: candidate.yaml, confirm_empty: candidate.empty }),
      });
      if (!response.ok) {
        const body = (await response.json().catch(() => null)) as {
          error?: string;
        } | null;
        throw new Error(
          body?.error ?? "保存失败；原有当前规则集保持不变。",
        );
      }
      setActiveYaml(candidate.yaml);
      setEditing(false);
      setValidation(null);
      focusRuleHeadingAfterActivationRef.current = true;
      setRuleActivation(null);
      setRulesMessage("当前规则集已原子保存。");
    } catch (error) {
      setRulesError(
        error instanceof Error && error.message
          ? error.message
          : "保存失败；原有当前规则集保持不变。",
      );
      setRuleActivation(null);
    } finally {
      setRulesPending(null);
    }
  }
  async function loadAssets() {
    const request = ++assetRequest.current;
    setGalleryLoading(true);
    setGalleryError(false);
    const p = new URLSearchParams({ page: String(page), per_page: "48" });
    if (query) p.set("q", query);
    if (filter) p.set("state", filter);
    try {
      const [a, h, storageResponse] = await Promise.all([
        fetch(`/api/v1/assets?${p}`),
        fetch("/api/v1/assets/health"),
        fetch("/api/v1/media-roots/storage"),
      ]);
      if (request !== assetRequest.current) return;
      if (!a.ok) throw new Error("资产请求失败");
      const body = (await a.json().catch(() => null)) as Page | null;
      if (!body) throw new Error("资产响应无效");
      setAssets(body);
      if (body.page !== page) {
        setPage(body.page);
        history.replaceState(
          {},
          "",
          galleryUrl(query, filter, body.page, assetIdFromPath()),
        );
      }
      if (!query && !filter) setLibraryTotal(body.total);
      if (h.ok) {
        const healthBody = (await h.json().catch(() => null)) as Health | null;
        if (healthBody) setHealth(healthBody);
      }
      if (storageResponse.ok) {
        const storageBody = (await storageResponse.json().catch(() => null)) as StorageHealth | null;
        if (storageBody?.aggregate && Array.isArray(storageBody.roots)) setStorage(storageBody);
      }
    } catch {
      if (request === assetRequest.current) setGalleryError(true);
    } finally {
      if (request === assetRequest.current) setGalleryLoading(false);
    }
  }
  async function scan() {
    setMessage("正在核对文件系统…");
    const r = await fetch("/api/v1/assets/scan", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ mode: "manual" }),
    });
    setMessage(r.ok ? "资产索引核对完成。" : await r.text());
    await loadAssets();
  }
  async function inspect(asset: Asset, navigate = true) {
    const active = document.activeElement;
    if (active instanceof HTMLElement && !active.closest(".asset-inspector"))
      assetOpenerRef.current = active;
    setAssetTab("overview");
    if (navigate) {
      const current = history.state as AssetHistoryState | null;
      const next: AssetHistoryState = {
        asset: asset.id,
        tab: "overview",
        assetInspectorEntry: true,
        assetInspectorDepth: current?.assetInspectorEntry
          ? current.assetInspectorDepth ?? 1
          : 1,
      };
      if (current?.assetInspectorEntry)
        history.replaceState(next, "", assetRoute(asset.id, "overview"));
      else history.pushState(next, "", assetRoute(asset.id, "overview"));
    }
    const request = ++detailRequest.current;
    setInspectedAsset(asset);
    setAssetDetail(null);
    setDetailLoading(true);
    try {
      const response = await fetch(`/api/v1/assets/${encodeURIComponent(asset.id)}`);
      if (request !== detailRequest.current) return;
      if (!response.ok) throw new Error("资产详情请求失败");
      const detail = (await response.json()) as AssetDetail;
      if (request === detailRequest.current) setAssetDetail(detail);
    } catch {
      if (request === detailRequest.current)
        setMessage("无法加载资产详情。");
    } finally {
      if (request === detailRequest.current) setDetailLoading(false);
    }
  }
  async function inspectById(id: string) {
    const request = ++detailRequest.current;
    setAssetDetail(null);
    setDetailLoading(true);
    try {
      const response = await fetch(`/api/v1/assets/${encodeURIComponent(id)}`);
      if (request !== detailRequest.current) return;
      if (!response.ok) throw new Error("资产详情请求失败");
      const detail = (await response.json()) as AssetDetail & Partial<Asset>;
      if (request !== detailRequest.current) return;
      setInspectedAsset({
        id: detail.id,
        path: detail.path,
        jav_code: detail.jav_code,
        title: detail.title,
        artwork_url: detail.artwork_url,
        captured_date: detail.captured_date,
        state: detail.state,
        exception: detail.exception,
      });
      setAssetDetail(detail);
    } catch {
      if (request === detailRequest.current) {
        setInspectedAsset(null);
        setMessage("无法加载资产详情。");
      }
    } finally {
      if (request === detailRequest.current) setDetailLoading(false);
    }
  }
  function changeAssetTab(tab: AssetTab) {
    if (tab === assetTab) return;
    setAssetTab(tab);
    const asset = inspectedAssetRef.current;
    if (!asset) return;
    const current = history.state as AssetHistoryState | null;
    const next: AssetHistoryState = {
      asset: asset.id,
      tab,
      assetInspectorEntry: current?.assetInspectorEntry,
      assetInspectorDepth: current?.assetInspectorEntry
        ? (current.assetInspectorDepth ?? 1) + 1
        : undefined,
    };
    if (current?.assetInspectorEntry)
      history.pushState(next, "", assetRoute(asset.id, tab));
    else history.replaceState(next, "", assetRoute(asset.id, tab));
  }
  function closeInspector() {
    detailRequest.current += 1;
    setInspectedAsset(null);
    setAssetDetail(null);
    const current = history.state as AssetHistoryState | null;
    if (current?.assetInspectorEntry) {
      const depth = current.assetInspectorDepth ?? 1;
      setAssetBackActor(null);
      assetDismissRef.current = true;
      history.go(-depth);
    } else if (assetIdFromPath()) {
      history.replaceState({}, "", galleryUrl(query, filter, page));
    }
  }
  async function submit(e: FormEvent) {
    e.preventDefault();
    if (submittingRef.current) return;
    submittingRef.current = true;
    setSubmitting(true);
    setMessage("");
    const init = view === "initialize";
    try {
      const r = await fetch(`/api/v1/auth/${init ? "initialize" : "login"}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(init ? { token, password } : { password }),
      });
      setPassword("");
      if (init && r.status === 409) {
        history.replaceState({}, "", "/");
        setView("login");
        setMessage("管理员已初始化，请登录后继续。");
        return;
      }
      if (!r.ok) {
        setMessage(
          !init && r.status === 401
            ? "密码错误。"
            : init && r.status === 400
              ? "密码至少需要 4 个字符。"
              : init && r.status === 403
                ? "初始化链接无效或已过期，请在服务器本地重新生成。"
                : "服务器无法完成请求，请重试。",
        );
        return;
      }
      if (init) {
        history.replaceState({}, "", "/");
        setView("login");
        setMessage("管理员初始化完成，请登录后继续。");
      } else location.assign("/");
    } finally {
      submittingRef.current = false;
      setSubmitting(false);
    }
  }
  async function logout() {
    await fetch("/api/v1/auth/logout", { method: "POST" });
    setView("login");
  }
  async function loadTasks(append = false) {
    if (append && historyPageLoadingRef.current) return;
    if (append) {
      historyPageLoadingRef.current = true;
      setHistoryPageLoading(true);
    } else {
      historyPageLoadingRef.current = false;
      setHistoryPageLoading(false);
    }
    const requestSequence = ++historyRequestSequenceRef.current;
    const offset = append ? loadedHistoryCountRef.current : 0;
    try {
      const [response, activeResponse] = await Promise.all([
        fetch(`/api/v1/tasks?limit=20&offset=${offset}`),
        append ? Promise.resolve(null) : fetch("/api/v1/tasks?active=true"),
      ]);
      if (historyRequestSequenceRef.current !== requestSequence) return;
      if (response.ok) {
      const page = (await response.json().catch(() => null)) as
        Task[] | null;
      if (!page) return;
      const active = activeResponse?.ok
        ? (await activeResponse.json().catch(() => [])) as Task[]
        : [];
      const recovered = append
        ? page
        : [...page, ...active.filter((task) => !page.some((item) => item.id === task.id))];
      const total = Number(response.headers.get("X-Total-Count") ?? page.length);
      loadedHistoryCountRef.current = append
        ? loadedHistoryCountRef.current + page.length
        : page.length;
      setTaskTotal(total);
      setHasMoreTasks(offset + page.length < total);
      setTasks((current) => append
        ? [...current, ...recovered.filter((task) => !current.some((item) => item.id === task.id))]
        : recovered);
      if (append) {
        recovered
          .filter((task) => ["queued", "running"].includes(task.status))
          .forEach((task) => watchTask(task.id));
        return;
      }
      const recoveredById = new Map(recovered.map((task) => [task.id, task]));
      taskSourcesRef.current.forEach((source, id) => {
        const task = recoveredById.get(id);
        if (!task || ["completed", "failed", "interrupted"].includes(task.status)) {
          source.close();
          taskSourcesRef.current.delete(id);
        }
      });
      recovered
        .filter((task) => ["queued", "running"].includes(task.status))
        .forEach((task) => watchTask(task.id));
      }
    } finally {
      if (append && historyRequestSequenceRef.current === requestSequence) {
        historyPageLoadingRef.current = false;
        setHistoryPageLoading(false);
      }
    }
  }
  function notifyTerminalTask(task: Task) {
    const key = `${task.id}:${task.status}`;
    if (notifiedTaskStatesRef.current.has(key)) return;
    notifiedTaskStatesRef.current.add(key);
    if (task.status === "completed") {
      setMessage(taskDisplayStatus(task) === "blocked-for-confirmation"
        ? "操作计划已准备好，等待确认。"
        : "管理任务已完成。");
    } else if (task.status === "failed") {
      setMessage(task.error ?? "管理任务失败，请检查执行结果。");
    } else if (task.status === "interrupted") {
      setMessage(task.error ?? "管理任务已中断。");
    }
  }
  function watchTask(id: string) {
    if (taskSourcesRef.current.has(id)) return;
    const source = new EventSource(`/api/v1/tasks/${id}/events`);
    taskSourcesRef.current.set(id, source);
    source.addEventListener("task", (event) => {
      const task = JSON.parse((event as MessageEvent).data) as Task;
      taskSnapshotGenerationRef.current.set(
        task.id,
        (taskSnapshotGenerationRef.current.get(task.id) ?? 0) + 1,
      );
      setTasks((current) => [
        task,
        ...current.filter((item) => item.id !== task.id),
      ]);
      if (["completed", "failed", "interrupted"].includes(task.status)) {
        notifyTerminalTask(task);
        source.close();
        taskSourcesRef.current.delete(task.id);
        const actorName = actorRemovalTasksRef.current.get(task.id);
        if (actorName) {
          actorRemovalTasksRef.current.delete(task.id);
          if (task.status === "completed") {
            setActorRemovalNotice("移除演员目录的管理任务已完成。");
            void loadActors().then(() => {
              if (inspectedActorRef.current?.name !== actorName) return;
              actorRequest.current += 1;
              setInspectedActor(null);
              setActorDetailError(null);
              setNav("actors");
              history.replaceState({}, "", "/actors");
            });
          } else {
            setActorRemovalNotice(null);
            setActorRemovalFailure(
              `移除演员目录的任务${task.status === "failed" ? "失败" : "已中断"}，演员目录已保留。`,
            );
          }
        }
      }
    });
    source.addEventListener("error", () => {
      const generation = taskSnapshotGenerationRef.current.get(id) ?? 0;
      const recoverySequence = (taskRecoverySequenceRef.current.get(id) ?? 0) + 1;
      taskRecoverySequenceRef.current.set(id, recoverySequence);
      void fetch(`/api/v1/tasks/${id}`)
        .then(async (response) => response.ok ? (await response.json()) as Task : null)
        .then((task) => {
          if (!task) return;
          if ((taskSnapshotGenerationRef.current.get(id) ?? 0) !== generation) return;
          if (taskRecoverySequenceRef.current.get(id) !== recoverySequence) return;
          setTasks((current) => [
            task,
            ...current.filter((item) => item.id !== task.id),
          ]);
          if (["completed", "failed", "interrupted"].includes(task.status)) {
            notifyTerminalTask(task);
            source.close();
            taskSourcesRef.current.delete(task.id);
          }
        })
        .catch(() => undefined);
    });
  }
  async function createTask(event: FormEvent) {
    event.preventDefault();
    setMessage("");
    const response = await fetch("/api/v1/tasks", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        task_type: "operations",
        media_root: mediaRoot,
        mode: "preview",
        operations: operations
          .map(([key]) => key)
          .filter((key) => selectedOps.includes(key)),
      }),
    });
    if (!response.ok) {
      setMessage((await response.text()) || "任务请求被拒绝。");
      return;
    }
    const task = (await response.json()) as Task;
    setTaskTotal((total) => total + 1);
    setTasks((current) => [task, ...current.filter((item) => item.id !== task.id)]);
    watchTask(task.id);
  }
  async function confirmPlan(planId: string) {
    setMessage("");
    const response = await fetch("/api/v1/tasks", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        task_type: "operations",
        mode: "apply",
        plan_id: planId,
        confirmed: true,
      }),
    });
    if (!response.ok) {
      setMessage((await response.text()) || "确认请求被拒绝。");
      return;
    }
    const task = (await response.json()) as Task;
    setTaskTotal((total) => total + 1);
    setTasks((current) => [
      task,
      ...current
        .filter((item) => item.id !== task.id)
        .map((item) => item.id === planId
          ? { ...item, plan_consumed_at: Math.floor(Date.now() / 1000) }
          : item),
    ]);
    setPlanToConfirm(null);
    watchTask(task.id);
  }
  const jfDirty =
    jfUrl !== jfBaseline.url ||
    normalizeLibraryIds(jfLibraries) !== jfBaseline.libraries ||
    jfKey !== "";
  const settingsDirty = yaml !== activeYaml || jfDirty;
  navRef.current = nav;
  settingsDirtyRef.current = settingsDirty;
  function requestNavigation(run: () => void) {
    if (nav === "settings" && settingsDirty) {
      setPendingNavigation({ run });
      return;
    }
    run();
  }
  useEffect(() => {
    if (!settingsDirty) return;
    const warnBeforeUnload = (event: BeforeUnloadEvent) => event.preventDefault();
    addEventListener("beforeunload", warnBeforeUnload);
    return () => removeEventListener("beforeunload", warnBeforeUnload);
  }, [settingsDirty]);
  if (view === "loading") return <div className="auth ui-foundation">正在检查登录状态…</div>;
  if (view === "initialize" || view === "login")
    return (
      <motion.main
        className="auth ui-foundation"
        initial={{ opacity: 0, y: 8 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.24, ease: EASE_OUT }}
      >
        <div className="brand-mark"><ImageIcon aria-hidden="true" /></div>
        <p className="eyebrow">RUST—JAV</p>
        <h1>
          {view === "initialize" ? "初始化管理员" : "欢迎回来"}
        </h1>
        <form className="ui-panel" onSubmit={submit}>
          <label htmlFor="password">密码</label>
          <Input
            id="password"
            type="password"
            minLength={4}
            autoComplete={view === "initialize" ? "new-password" : "current-password"}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            autoFocus
          />
          <Button className="ui-primary-button" type="submit" disabled={submitting}>
            {submitting
              ? "请稍候…"
              : view === "initialize"
                ? "初始化"
                : "登录"}
          </Button>
        </form>
        {message && <p role="status">{message}</p>}
      </motion.main>
    );
  return (
    <motion.div
      className={`shell ui-foundation ${inspectedAsset ? "inspecting" : ""}`}
      data-design="beui-photos"
      initial={false}
      animate={{ opacity: 1 }}
      transition={{ duration: 0.2, ease: EASE_OUT }}
    >
      <aside className="sidebar">
        <div className="logo">
          <span><ImageIcon aria-hidden="true" /></span>
          <div>
            <b>rust-jav</b>
            <small>媒体资产管理</small>
          </div>
        </div>
        <nav aria-label="桌面主导航">
          <p>图库</p>
          <Button
            aria-label="所有资产"
            title="所有资产"
            aria-current={nav === "assets" ? "page" : undefined}
            className={nav === "assets" ? "active" : ""}
            onClick={() => {
              requestNavigation(() => {
                setNav("assets");
                setFilter("");
                setPage(1);
                history.pushState({}, "", galleryUrl(query, "", 1));
              });
            }}
          >
            <span><Grid2X2 aria-hidden="true" /></span> 所有资产 <em>{libraryTotal || assets.total}</em>
          </Button>
          <Button
            aria-label="最近入库"
            aria-current={nav === "recent" ? "page" : undefined}
            className={nav === "recent" ? "active" : ""}
            onClick={() => {
              requestNavigation(() => {
                setNav("recent");
                setFilter("");
                setPage(1);
                history.pushState({}, "", galleryUrl(query, "", 1));
              });
            }}
          >
            <span><Clock3 aria-hidden="true" /></span> 最近入库
          </Button>
          <Button
            aria-label="演员"
            aria-current={nav === "actors" ? "page" : undefined}
            className={nav === "actors" ? "active" : ""}
            onClick={() => requestNavigation(showActorFolders)}
          >
            <span><Users aria-hidden="true" /></span> 演员
            <em>{actors.length}</em>
          </Button>
          <Button
            aria-label="删除候选"
            aria-current={nav === "deletion" ? "page" : undefined}
            className={nav === "deletion" ? "active" : ""}
            onClick={() => requestNavigation(() => setNav("deletion"))}
          >
            <span><Trash2 aria-hidden="true" /></span> 删除候选
          </Button>
          <p>管理</p>
          <Button
            aria-label="整理任务"
            aria-current={nav === "tasks" ? "page" : undefined}
            className={nav === "tasks" ? "active" : ""}
            onClick={() => requestNavigation(() => setNav("tasks"))}
          >
            <span><ListTodo aria-hidden="true" /></span> 整理任务
          </Button>
          <Button
            aria-label="异常资产"
            aria-current={nav === "exceptions" ? "page" : undefined}
            className={nav === "exceptions" ? "active" : ""}
            onClick={() => {
              requestNavigation(() => {
                setNav("exceptions");
                setFilter("exception");
                setPage(1);
                history.pushState({}, "", galleryUrl(query, "exception", 1));
              });
            }}
          >
            <span><AlertTriangle aria-hidden="true" /></span> 异常资产
          </Button>
          <Button
            aria-label="设置"
            aria-current={nav === "settings" ? "page" : undefined}
            className={nav === "settings" ? "active" : ""}
            onClick={() => setNav("settings")}
          >
            <span><Settings aria-hidden="true" /></span> 设置
          </Button>
        </nav>
        {storage && <MediaStorageStatus storage={storage} />}
        <Card className="root-card">
          <small>资产索引</small>
          <b>
            <i className={`health-dot ${health?.state}`} />
            {health?.state === "healthy" ? "健康" : health?.state ? "异常" : "加载中"}
          </b>
          <span>
            {health?.mode
              ? `最近一次${health.mode === "startup" ? "启动" : health.mode === "manual" ? "手动" : "增量"}扫描`
              : "以文件系统为准"}
          </span>
        </Card>
        <Button
          className="signout"
          onClick={() => requestNavigation(() => void logout())}
          aria-label="退出登录"
        >
          <LogOut aria-hidden="true" /> 退出登录
        </Button>
      </aside>
      <main className="content">
        <header>
          <div>
            <p className="eyebrow">媒体图库</p>
            <h1>
              {nav === "assets"
                ? "所有资产"
                : nav === "recent"
                  ? "最近入库"
                  : nav === "exceptions"
                    ? "异常资产"
                : nav === "actors"
                  ? "演员"
                  : nav === "deletion"
                    ? "删除候选"
                    : nav === "tasks"
                      ? "整理任务"
                      : "设置"}
            </h1>
            <small>
              {nav === "actors"
                ? `${actors.length} 个可重建演员视图`
                : nav === "recent"
                  ? `${assets.total} 个最近观察到的资产`
                  : nav === "exceptions"
                    ? `${assets.total} 个需要处理的异常资产`
                : nav === "deletion"
                  ? `${candidates.length} 个路径命中当前规则`
                  : nav === "tasks"
                    ? `${taskTotal} 个持久化任务`
                    : `${libraryTotal || assets.total} 个项目 · 文件系统为准`}
            </small>
          </div>
          {storage && (
            <Button
              className="mobile-storage-entry"
              aria-label="媒体存储"
              onClick={() => setStorageOpen(true)}
            >
              <HardDrive aria-hidden="true" />
              <span>媒体存储</span>
            </Button>
          )}
          {(nav === "assets" || nav === "recent" || nav === "exceptions") && (
            <Button className="scan" onClick={scan} aria-label="重新扫描">
              <RefreshCw aria-hidden="true" /> <span>重新扫描</span>
            </Button>
          )}
        </header>
        {(nav === "assets" || nav === "recent" || nav === "exceptions") && (
          <MediaAssetGallery
            assets={assets}
            query={query}
            filter={filter}
            loading={galleryLoading}
            error={galleryError}
            inspectedAssetId={inspectedAsset?.id ?? null}
            renderArtwork={(asset) => <AssetArtwork asset={asset} />}
            formatDate={formatDate}
            onQueryChange={(nextQuery) => {
              setQuery(nextQuery);
              setPage(1);
              history.replaceState({}, "", galleryUrl(nextQuery, filter, 1, assetIdFromPath()));
            }}
            onFilterChange={(next) => {
              setFilter(next);
              setPage(1);
              history.pushState({}, "", galleryUrl(query, next, 1, assetIdFromPath()));
            }}
            onPageChange={(nextPage) => {
              setPage(nextPage);
              history.pushState({}, "", galleryUrl(query, filter, nextPage, assetIdFromPath()));
            }}
            onInspect={(asset) => void inspect(asset)}
            onRetry={() => setGalleryRetry((value) => value + 1)}
          />
        )}
        {nav === "tasks" && (
          <TaskPanel
            tasks={tasks}
            taskTotal={taskTotal}
            hasMoreTasks={hasMoreTasks}
            historyPageLoading={historyPageLoading}
            mediaRoot={mediaRoot}
            setMediaRoot={setMediaRoot}
            selectedOps={selectedOps}
            setSelectedOps={setSelectedOps}
            createTask={createTask}
            requestPlanConfirmation={setPlanToConfirm}
            refresh={loadTasks}
            loadMore={() => loadTasks(true)}
          />
        )}
        {nav === "actors" && (
          <ActorFolders
            actors={actors}
            state={actorListState}
            inspect={(actor) => void openActor(actor)}
            retry={() => void loadActors()}
          />
        )}
        {nav === "deletion" && (
          <DeletionCandidateBrowser
            candidates={candidates}
            selected={selected}
            setSelected={setSelected}
            preview={(selection) => void previewDeletion(selection)}
            planning={deletionPending === "planning"}
            formatBytes={formatBytes}
            fileTypeLabel={fileTypeLabel}
            warningLabel={deletionWarningLabel}
          />
        )}
        {nav === "settings" && (
          <SettingsPage
            sourceUrl={sourceUrl}
            setSourceUrl={(value) => { setSourceUrl(value); setRulesError(""); }}
            yaml={yaml}
            editing={editing}
            validation={validation}
            rulesMessage={rulesMessage}
            rulesError={rulesError}
            rulesPending={rulesPending}
            downloadProposal={downloadProposal}
            updateYaml={updateYaml}
            beginEditing={() => { setEditing(true); setValidation(null); }}
            validateRules={validateRules}
            reviewRuleActivation={reviewRuleActivation}
            jfUrl={jfUrl}
            setJfUrl={(value) => { jellyfinChangeGeneration.current += 1; setJfUrl(value); setJfError(""); }}
            jfLibraries={jfLibraries}
            setJfLibraries={(value) => { jellyfinChangeGeneration.current += 1; setJfLibraries(value); setJfError(""); }}
            jfKey={jfKey}
            setJfKey={(value) => { jellyfinChangeGeneration.current += 1; setJfKey(value); setJfError(""); }}
            jfKeyConfigured={jfKeyConfigured}
            jfDirty={jfDirty}
            jfLoadState={jfLoadState}
            jfSaving={jfSaving}
            jfError={jfError}
            jellyfinAction={jellyfinAction}
            saveJellyfin={saveJellyfin}
            loadJellyfinConfig={() => void loadJellyfinConfig()}
            testJellyfin={() => void testJellyfin()}
            refreshJellyfin={() => void refreshJellyfin()}
            ruleHeadingRef={ruleHeadingRef}
          />
        )}
        {false && (
          <div className="settings-stack">
            <section className="rules-settings">
              <p className="eyebrow">删除规则</p>
              <h2 ref={ruleHeadingRef} tabIndex={-1}>当前规则集</h2>
              <p>
                远程 YAML 只是规则草案。服务器验证后原子启用；规则不能选择根目录或授权删除。
              </p>
              <label htmlFor="rule-source">规则来源 URL</label>
              <div className="rule-actions">
                <Input
                  id="rule-source"
                  type="url"
                  placeholder="https://raw.githubusercontent.com/…"
                  value={sourceUrl}
                  disabled={rulesPending !== null}
                  onChange={(event) => {
                    setSourceUrl(event.target.value);
                    setRulesError("");
                  }}
                />
                <Button
                  type="button"
                  disabled={!sourceUrl || rulesPending !== null}
                  onClick={downloadProposal}
                >
                  {rulesPending === "download" ? "正在下载草案…" : "下载草案"}
                </Button>
              </div>
              <label htmlFor="rules-yaml">当前规则集 YAML</label>
              <textarea
                id="rules-yaml"
                rows={18}
                readOnly={!editing}
                disabled={rulesPending !== null}
                value={yaml}
                onChange={(event) => updateYaml(event.target.value)}
              />
              <div className="rule-actions">
                {!editing && (
                  <Button
                    type="button"
                    onClick={() => {
                      setEditing(true);
                      setValidation(null);
                    }}
                  >
                    编辑
                  </Button>
                )}
                {editing && (
                  <Button type="button" disabled={rulesPending !== null} onClick={validateRules}>
                    {rulesPending === "validate" ? "正在验证…" : "验证"}
                  </Button>
                )}
                {editing && !validation?.empty && (
                  <Button
                    type="button"
                    disabled={!validation || validation?.yaml !== yaml}
                    onClick={reviewRuleActivation}
                  >
                    保存当前规则集
                  </Button>
                )}
                {editing && validation?.empty && (
                  <Button
                    type="button"
                    className="danger"
                    disabled={validation?.yaml !== yaml}
                    onClick={reviewRuleActivation}
                  >
                    确认空规则并保存
                  </Button>
                )}
              </div>
              {rulesMessage && (
                <p role="status" className="notice">
                  {rulesMessage}
                </p>
              )}
              {rulesError && (
                <p role="alert" className="notice settings-error">
                  {rulesError}
                </p>
              )}
            </section>
            <section
              className="task-create jellyfin-settings"
              aria-busy={jfLoadState === "loading" ? "true" : undefined}
            >
              <p className="eyebrow">媒体服务器</p>
              <h2>Jellyfin</h2>
              <p>
                连接一个服务器并选择多个媒体库 ID。API 密钥只保存在本服务器。
              </p>
              <form className="task-form" onSubmit={saveJellyfin}>
                {jfDirty && <p className="settings-dirty">有未保存的更改</p>}
                <label htmlFor="jellyfin-url">服务器 URL</label>
                <Input
                  id="jellyfin-url"
                  type="url"
                  value={jfUrl}
                  disabled={jfSaving}
                  onChange={(event) => {
                    jellyfinChangeGeneration.current += 1;
                    setJfUrl(event.target.value);
                    setJfError("");
                  }}
                  placeholder="http://jellyfin:8096"
                  required
                />
                <label htmlFor="jellyfin-libraries">媒体库 ID</label>
                <Input
                  id="jellyfin-libraries"
                  value={jfLibraries}
                  disabled={jfSaving}
                  onChange={(event) => {
                    jellyfinChangeGeneration.current += 1;
                    setJfLibraries(event.target.value);
                    setJfError("");
                  }}
                  placeholder="movies, jav"
                  required
                />
                <label htmlFor="jellyfin-key">服务器 API 密钥</label>
                <Input
                  id="jellyfin-key"
                  type="password"
                  autoComplete="off"
                  value={jfKey}
                  disabled={jfSaving}
                  onChange={(event) => {
                    jellyfinChangeGeneration.current += 1;
                    setJfKey(event.target.value);
                    setJfError("");
                  }}
                  required={!jfKeyConfigured}
                />
                <Button type="submit" disabled={!jfDirty || jfSaving}>
                  {jfSaving ? "正在保存 Jellyfin…" : "保存 Jellyfin"}
                </Button>
                {jfError && (
                  <p role="alert" className="notice settings-error">
                    {jfError}
                  </p>
                )}
                {jfLoadState === "error" && (
                  <Button type="button" className="settings-retry" onClick={() => void loadJellyfinConfig()}>
                    重新加载 Jellyfin 设置
                  </Button>
                )}
              </form>
              <div className="jellyfin-actions">
                <Button type="button" disabled={jfDirty || jfSaving} onClick={() => void testJellyfin()}>
                  测试连接
                </Button>
                <Button type="button" disabled={jfDirty || jfSaving} onClick={() => void refreshJellyfin()}>
                  刷新 Jellyfin
                </Button>
              </div>
            </section>
          </div>
        )}
      </main>
      {inspectedAsset && (
        <AssetInspector
          asset={inspectedAsset}
          detail={assetDetail}
          loading={detailLoading}
          close={closeInspector}
          backLabel={assetBackActor ? `返回 ${assetBackActor.name}` : undefined}
          tab={assetTab}
          onTabChange={changeAssetTab}
          restoreFocusRef={assetOpenerRef}
        />
      )}
      <AnimatePresence>
        {(inspectedActor || actorDetailLoading || actorDetailError) && (
          <ActorInspector
            actor={inspectedActor}
            loading={actorDetailLoading}
            error={actorDetailError}
            close={closeActor}
            openAsset={(asset) => void openLinkedAsset(asset)}
            remove={(actor) => void requestActorRemoval(actor)}
            retry={() => {
              const name = inspectedActor?.name ?? actorNameFromPath();
              if (name) void openActor(name, false);
            }}
            restoreFocusRef={actorOpenerRef}
            linkedFocusRef={actorLinkedFocusRef}
            suspended={Boolean(confirmActor)}
          />
        )}
      </AnimatePresence>
      <MorphingModal
        viewId={confirmActor ? "actor-removal" : null}
        placement="center"
        className="actor-removal-modal"
        onClose={() => setConfirmActor(null)}
      >
        {confirmActor && (
          <ActorRemovalDialog
            actor={confirmActor}
            busy={actorBusy}
            cancel={() => setConfirmActor(null)}
            remove={() => void removeActor()}
          />
        )}
      </MorphingModal>
      <OperationPlanDialog
        task={planToConfirm}
        close={() => setPlanToConfirm(null)}
        confirm={(planId) => void confirmPlan(planId)}
      />
      {actorRemovalNotice && (
        <div className="shell-notice" role="status">
          <p>{actorRemovalNotice}</p>
          <Button className="ui-icon-button" onClick={() => setActorRemovalNotice(null)} aria-label="关闭演员目录移除通知">
            <X aria-hidden="true" />
          </Button>
        </div>
      )}
      {actorRemovalFailure && (
        <div className="shell-notice actor-removal-failure" role="alert">
          <p>{actorRemovalFailure}</p>
          <Button className="ui-icon-button" onClick={() => setActorRemovalFailure(null)} aria-label="关闭演员目录移除错误">
            <X aria-hidden="true" />
          </Button>
        </div>
      )}
      <Toast
        fixed
        toasts={
          message
            ? [{ id: "shell-status", title: message, status: "info", duration: 0 }]
            : []
        }
        onDismiss={() => setMessage("")}
      />
      <RuleActivationDialog
        activation={ruleActivation}
        pending={rulesPending === "activate"}
        close={() => { if (!rulesPending) setRuleActivation(null); }}
        activate={() => { if (ruleActivation) void saveRules(ruleActivation); }}
      />
      <DiscardSettingsDialog
        open={Boolean(pendingNavigation)}
        keepEditing={() => setPendingNavigation(null)}
        discard={() => {
          const navigation = pendingNavigation?.run;
          setPendingNavigation(null);
          navigation?.();
        }}
      />
      <MorphingModal
        viewId={storageOpen && storage ? "media-storage" : null}
        placement="center"
        className="storage-modal"
        onClose={() => setStorageOpen(false)}
      >
        {storage && (
          <section
            className="storage-dialog"
            role="dialog"
            aria-modal="true"
            aria-label="媒体存储"
          >
            <Button
              density="compact"
              className="storage-dialog-close"
              aria-label="关闭媒体存储"
              onClick={() => setStorageOpen(false)}
            >
              <X aria-hidden="true" />
            </Button>
            <MediaStorageStatus storage={storage} compact />
          </section>
        )}
      </MorphingModal>
      <nav className="bottom-nav" aria-label="移动端主导航">
        <Button
          aria-label="图库"
          aria-current={nav === "assets" || nav === "recent" || nav === "exceptions" ? "page" : undefined}
          className={nav === "assets" || nav === "recent" || nav === "exceptions" ? "active" : ""}
          onClick={() => {
            requestNavigation(() => {
              setNav("assets");
              setFilter("");
              setPage(1);
              history.pushState({}, "", galleryUrl(query, "", 1));
            });
          }}
        >
          <span><Grid2X2 aria-hidden="true" /></span>图库
        </Button>
        <Button
          aria-label="演员"
          aria-current={nav === "actors" ? "page" : undefined}
          className={nav === "actors" ? "active" : ""}
          onClick={() => requestNavigation(showActorFolders)}
        >
          <span><Users aria-hidden="true" /></span>演员
        </Button>
        <Button
          aria-label="删除"
          aria-current={nav === "deletion" ? "page" : undefined}
          className={nav === "deletion" ? "active" : ""}
          onClick={() => requestNavigation(() => setNav("deletion"))}
        >
          <span><Trash2 aria-hidden="true" /></span>删除
        </Button>
        <Button
          aria-label="任务"
          aria-current={nav === "tasks" ? "page" : undefined}
          className={nav === "tasks" ? "active" : ""}
          onClick={() => requestNavigation(() => setNav("tasks"))}
        >
          <span><ListTodo aria-hidden="true" /></span>任务
        </Button>
        <Button
          aria-label="设置"
          aria-current={nav === "settings" ? "page" : undefined}
          className={nav === "settings" ? "active" : ""}
          onClick={() => setNav("settings")}
        >
          <span><Settings aria-hidden="true" /></span>设置
        </Button>
      </nav>
      <PermanentDeletionReview
        plan={plan}
        outcome={deletionOutcome}
        pending={deletionPending}
        invalid={deletionPlanInvalid}
        error={deletionError}
        confirmText={confirmText}
        onConfirmTextChange={setConfirmText}
        onPreview={(selection) => void previewDeletion(selection)}
        onExecute={() => void executeDeletion()}
        onClose={closeDeletionReview}
        formatBytes={formatBytes}
        fileTypeLabel={fileTypeLabel}
        warningLabel={deletionWarningLabel}
        taskStatusLabel={(status) => taskStatusLabels[status as TaskDisplayStatus] ?? status}
      />
    </motion.div>
  );
}
function AssetInspector({
  asset,
  detail,
  loading,
  close,
  backLabel,
  tab,
  onTabChange,
  restoreFocusRef,
}: {
  asset: Asset;
  detail: AssetDetail | null;
  loading: boolean;
  close: () => void;
  backLabel?: string;
  tab: AssetTab;
  onTabChange: (tab: AssetTab) => void;
  restoreFocusRef: { current: HTMLElement | null };
}) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const closeRef = useRef(close);
  closeRef.current = close;
  const mobile = useMobileBreakpoint();
  useEffect(() => {
    const background = Array.from(
      document.querySelectorAll<HTMLElement>(
        ".shell > .sidebar, .shell > main, .shell > .bottom-nav",
      ),
    ).map((element) => ({
      element,
      inert: element.inert,
      attribute: element.hasAttribute("inert"),
    }));
    const scrollY = window.scrollY;
    const returnFocus = restoreFocusRef.current;
    background.forEach(({ element }) => {
      element.inert = true;
      element.setAttribute("inert", "");
    });
    if (mobile) {
      document.body.classList.add("asset-inspector-open");
      document.body.style.setProperty("--asset-inspector-scroll-y", `${scrollY}px`);
    }
    closeButtonRef.current?.focus();

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        closeRef.current();
        return;
      }
      if (event.key !== "Tab" || !dialogRef.current) return;
      const focusable = Array.from(
        dialogRef.current.querySelectorAll<HTMLElement>(
          'button:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])',
        ),
      );
      if (!focusable.length) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      background.forEach(({ element, inert, attribute }) => {
        element.inert = inert;
        if (attribute) element.setAttribute("inert", "");
        else element.removeAttribute("inert");
      });
      if (mobile) {
        document.body.classList.remove("asset-inspector-open");
        document.body.style.removeProperty("--asset-inspector-scroll-y");
        window.scrollTo(0, scrollY);
      }
      if (returnFocus?.isConnected) returnFocus.focus();
    };
  }, [mobile, restoreFocusRef]);
  return (
    <Sheet open onClose={close} aria-label={asset.jav_code ?? "媒体资产"} className="asset-inspector-sheet" contentClassName="asset-inspector">
      <div ref={dialogRef}>
      <div className="sheet-handle" aria-hidden="true" />
      <Button
        ref={closeButtonRef}
        className="inspector-close ui-icon-button"
        onClick={close}
        aria-label="关闭资产详情"
      >
        <X aria-hidden="true" />
      </Button>
      {backLabel && (
        <Button className="inspector-back" onClick={close} aria-label={backLabel}>
          <ArrowLeft aria-hidden="true" /> <span>{backLabel}</span>
        </Button>
      )}
      <div className="inspector-hero">
        <AssetArtwork
          asset={{
            ...asset,
            artwork_url: detail ? detail.artwork_url : asset.artwork_url,
          }}
        />
        <div>
          <h2 id="asset-detail-title">
            {asset.jav_code ?? "媒体资产"}
          </h2>
          <span>
            {detail?.title ?? asset.title ?? asset.path.split("/").pop()}
          </span>
        </div>
      </div>
      <Tabs defaultValue="overview" value={tab} onValueChange={(value) => onTabChange(value as AssetTab)} variant="underline" className="detail-tabs">
        <TabsList label="资产详情">
          <TabsTrigger value="overview">概览</TabsTrigger>
          <TabsTrigger value="nfo">NFO</TabsTrigger>
        </TabsList>
      {loading ? (
        <p className="detail-loading" role="status">
          正在加载资产详情…
        </p>
      ) : detail && tab === "overview" ? (
        <TabsContent value="overview">
          <DetailSection title="状态">
            <div className="detail-status-list">
              <InspectorStatus
                name="本地资产"
                label={labels[detail.state]}
                tone={detail.state}
                description={detail.exception ? assetExceptionLabel(detail.exception) : (detail.state === "synchronizing"
                  ? "正在自动核对文件系统。"
                  : "本地索引资产仍为权威来源。")}
              />
              {detail.artwork && (
                <InspectorStatus
                  name="本地封面"
                  label={artworkStatusLabel(detail.artwork.status)}
                  tone={detail.artwork.status === "valid"
                    ? "normal"
                    : detail.artwork.status === "missing"
                      ? "synchronizing"
                      : "exception"}
                  description={detail.artwork.error ?? (detail.artwork.status === "valid"
                    ? "已验证的本地 JPEG、PNG 或 WebP 封面为权威来源。"
                    : "核对时未发现本地封面候选。")}
                  action={<DataList items={[
                    ["封面来源", detail.artwork.source_path],
                    ["检测到的媒体类型", detail.artwork.content_type],
                  ]} />}
                />
              )}
              <InspectorStatus
                name="Jellyfin"
                label={jellyfinStatusLabel(detail.jellyfin?.status)}
                tone={detail.jellyfin?.status === "offline" || detail.jellyfin?.status === "not_found"
                  ? "exception"
                  : detail.jellyfin?.status === "not_configured"
                    ? "synchronizing"
                    : "normal"}
                description={detail.jellyfin?.reason ? jellyfinReasonLabel(detail.jellyfin.reason) : (detail.jellyfin?.status === "not_configured"
                  ? "尚未配置 Jellyfin。"
                  : detail.jellyfin?.status === "offline"
                    ? "Jellyfin 当前不可用。"
                    : detail.jellyfin?.status === "not_found"
                      ? "未找到 Jellyfin 关联。"
                      : detail.jellyfin?.confidence === "uncertain_metadata"
                        ? "元数据关联不确定；此关联绝不会授权删除。"
                        : "只读播放关联；本地文件仍为权威来源。")}
                action={detail.jellyfin?.open_url ? (
                  <a href={detail.jellyfin.open_url} target="_blank" rel="noreferrer">
                    在 Jellyfin 中打开 ↗
                  </a>
                ) : undefined}
              />
              {detail.jellyfin && (
                <DataList items={[
                  ["播放次数", detail.jellyfin.play_count === undefined
                    ? null : `${detail.jellyfin.play_count} 次`],
                  ["播放位置", detail.jellyfin.playback_position_ticks === undefined
                    ? null : `${detail.jellyfin.playback_position_ticks} 刻度`],
                  ["删除权限", detail.jellyfin.may_authorize_deletion
                    ? "确定的路径关联" : "此关联不能授权删除"],
                ]} />
              )}
            </div>
          </DetailSection>
          <DetailSection title="演员">
            {detail.actors.length ? (
              <div className="actor-grid">
                {detail.actors.map((actor) =>
                  actor.actor_folder_url ? (
                    <a className="actor-poster" href={actor.actor_folder_url} key={actor.name}>
                      {actor.poster_url ? (
                        <img src={actor.poster_url} alt={`${actor.name} 海报`} />
                      ) : (
                        <span className="actor-silhouette"><UserRound aria-hidden="true" /></span>
                      )}
                      <span><b>{actor.name}</b><small>打开派生演员目录 →</small></span>
                    </a>
                  ) : (
                    <div className="actor-poster" key={actor.name}>
                      <span className="actor-silhouette"><UserRound aria-hidden="true" /></span>
                      <span><b>{actor.name}</b><small>演员目录不可用</small></span>
                    </div>
                  ),
                )}
              </div>
            ) : (
              <p className="muted">此 NFO 中没有演员信息。</p>
            )}
          </DetailSection>
          <DetailSection title="资产信息">
            <DataList items={[
              ["片商", detail.studio],
              ["发行日期", detail.release_date],
              ["收录日期", detail.captured_date],
              ["源视频", detail.path],
            ]} />
          </DetailSection>
        </TabsContent>
      ) : (
        detail && (
          <TabsContent value="nfo">
            <p className="plot">{detail.plot ?? "此 NFO 中没有简介。"}</p>
            <DetailSection title="NFO 元数据">
              <DataList items={[
                ["标题", detail.title],
                ["片商", detail.studio],
                ["发行日期", detail.release_date],
                ["时长", detail.runtime_minutes === null ? null : `${detail.runtime_minutes} 分钟`],
                ["导演", detail.director],
                ["解析状态", parseStatusLabel(detail.parse_status)],
                ["NFO 路径", detail.source_path],
              ]} />
            </DetailSection>
            <div className="tags">
              {detail.tags.map((tag) => (
                <span key={tag}>{tag}</span>
              ))}
            </div>
          </TabsContent>
        )
      )}
      </Tabs>
      </div>
    </Sheet>
  );
}
function ActorFolders({
  actors,
  state,
  inspect,
  retry,
}: {
  actors: ActorFolder[];
  state: LoadState;
  inspect: (actor: ActorFolder) => void;
  retry: () => void;
}) {
  const [sortKey, setSortKey] = useState<ActorSortKey>("name");
  const [sortDirection, setSortDirection] = useState<SortDirection>("asc");
  const sortedActors = useMemo(() => {
    const nameOrder = (left: ActorFolder, right: ActorFolder) =>
      left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
    return [...actors].sort((left, right) => {
      const order =
        sortKey === "count"
          ? left.movie_count - right.movie_count
          : sortKey === "size"
            ? left.logical_size - right.logical_size
            : nameOrder(left, right);
      if (order === 0) return nameOrder(left, right);
      return sortDirection === "asc" ? order : -order;
    });
  }, [actors, sortDirection, sortKey]);
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
      <div className="actor-sort-toolbar" aria-label="演员排序">
        <label>
          <span>排序</span>
          <select
            aria-label="演员排序字段"
            value={sortKey}
            onChange={(event) => {
              const nextKey = event.target.value as ActorSortKey;
              setSortKey(nextKey);
              setSortDirection(nextKey === "name" ? "asc" : "desc");
            }}
          >
            <option value="name">演员名</option>
            <option value="count">资产数量</option>
            <option value="size">逻辑大小</option>
          </select>
        </label>
        <Button
          className="actor-sort-direction"
          aria-label={sortDirection === "asc" ? "切换为降序" : "切换为升序"}
          onClick={() => setSortDirection((current) => current === "asc" ? "desc" : "asc")}
        >
          {sortDirection === "asc" ? <ArrowUp aria-hidden="true" /> : <ArrowDown aria-hidden="true" />}
          {sortDirection === "asc" ? "升序" : "降序"}
        </Button>
      </div>
      <div className="actor-folder-grid">
      {sortedActors.map((actor) => (
        <article className="actor-folder-card" key={actor.name}>
          <Button className="actor-folder-open" aria-label={`打开演员 ${actor.name}`} onClick={() => inspect(actor)}>
            <div className="actor-folder-poster" style={{ aspectRatio: "2 / 3" }}>
              <ActorPortrait actor={actor} loading="lazy" />
              <div>
                <b>{actor.name}</b>
                <p>{actor.movie_count} 个媒体资产 · {formatBytes(actor.logical_size)}</p>
              </div>
            </div>
          </Button>
        </article>
      ))}
      </div>
    </>
  );
}
function ActorInspector({
  actor,
  loading,
  error,
  close,
  openAsset,
  remove,
  retry,
  restoreFocusRef,
  linkedFocusRef,
  suspended,
}: {
  actor: ActorFolder | null;
  loading: boolean;
  error: string | null;
  close: () => void;
  openAsset: (asset: Asset) => void;
  remove: (actor: ActorFolder) => void;
  retry: () => void;
  restoreFocusRef: { current: HTMLElement | null };
  linkedFocusRef: { current: string | null };
  suspended: boolean;
}) {
  const dialogRef = useRef<HTMLElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const actionMenuRef = useRef<HTMLDivElement>(null);
  const actionMenuButtonRef = useRef<HTMLButtonElement>(null);
  const actionMenuOpenRef = useRef(false);
  const [actionMenuOpen, setActionMenuOpen] = useState(false);
  const closeRef = useRef(close);
  closeRef.current = close;
  actionMenuOpenRef.current = actionMenuOpen;
  const prefersReducedMotion = useReducedMotion();
  const reduce = prefersReducedMotion
    || (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false);
  const mobile = useMobileBreakpoint();
  useEffect(() => {
    if (suspended) return;
    const background = Array.from(
      document.querySelectorAll<HTMLElement>(
        ".shell > .sidebar, .shell > main, .shell > .bottom-nav",
      ),
    ).map((element) => ({
      element,
      inert: element.inert,
      attribute: element.hasAttribute("inert"),
    }));
    const scrollY = window.scrollY;
    const returnFocus = restoreFocusRef.current;
    background.forEach(({ element }) => {
      element.inert = true;
      element.setAttribute("inert", "");
    });
    if (mobile) {
      document.body.classList.add("asset-inspector-open");
      document.body.style.setProperty("--asset-inspector-scroll-y", `${scrollY}px`);
    }
    closeButtonRef.current?.focus();

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        if (actionMenuOpenRef.current) {
          setActionMenuOpen(false);
          actionMenuButtonRef.current?.focus();
          return;
        }
        closeRef.current();
        return;
      }
      if (event.key !== "Tab" || !dialogRef.current) return;
      const focusable = Array.from(
        dialogRef.current.querySelectorAll<HTMLElement>(
          'button:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])',
        ),
      );
      if (!focusable.length) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      background.forEach(({ element, inert, attribute }) => {
        element.inert = inert;
        if (attribute) element.setAttribute("inert", "");
        else element.removeAttribute("inert");
      });
      if (mobile) {
        document.body.classList.remove("asset-inspector-open");
        document.body.style.removeProperty("--asset-inspector-scroll-y");
        window.scrollTo(0, scrollY);
      }
      if (returnFocus?.isConnected) returnFocus.focus();
    };
  }, [mobile, restoreFocusRef, suspended]);
  useEffect(() => {
    if (!actionMenuOpen) return;
    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (!actionMenuRef.current?.contains(event.target as Node)) {
        setActionMenuOpen(false);
      }
    };
    document.addEventListener("pointerdown", closeOnOutsidePointer);
    return () => document.removeEventListener("pointerdown", closeOnOutsidePointer);
  }, [actionMenuOpen]);
  useEffect(() => {
    if (suspended || !actor || !linkedFocusRef.current || !dialogRef.current) return;
    const target = Array.from(
      dialogRef.current.querySelectorAll<HTMLElement>("[data-asset-id]"),
    ).find((element) => element.dataset.assetId === linkedFocusRef.current);
    if (target) {
      target.focus();
      linkedFocusRef.current = null;
    }
  }, [actor, linkedFocusRef, suspended]);
  return (
    <motion.aside
      ref={dialogRef}
      className="asset-inspector actor-inspector"
      role="dialog"
      aria-modal={suspended ? undefined : "true"}
      inert={suspended || undefined}
      aria-labelledby={actor ? "actor-detail-title" : undefined}
      aria-label={actor ? undefined : "演员目录详情"}
      initial={reduce ? false : mobile ? { y: 28 } : { x: 40 }}
      animate={mobile ? { y: 0 } : { x: 0 }}
      exit={reduce ? undefined : mobile ? { y: 28 } : { x: 40 }}
      transition={reduce ? { duration: 0 } : undefined}
    >
      <div className="sheet-handle" aria-hidden="true" />
      <Button ref={closeButtonRef} className="inspector-close ui-icon-button" onClick={close} aria-label="关闭演员详情"><X aria-hidden="true" /></Button>
      {actor && (
        <div className="actor-action-menu" ref={actionMenuRef}>
          <Button
            ref={actionMenuButtonRef}
            className="actor-action-menu-trigger ui-icon-button"
            aria-label="更多操作"
            aria-haspopup="menu"
            aria-expanded={actionMenuOpen}
            onClick={() => setActionMenuOpen((open) => !open)}
          >
            <Ellipsis aria-hidden="true" />
          </Button>
          {actionMenuOpen && (
            <div className="actor-action-menu-popover" role="menu" aria-label="演员操作">
              <Button
                role="menuitem"
                onClick={() => {
                  setActionMenuOpen(false);
                  remove(actor);
                }}
              >
                <Trash2 aria-hidden="true" /> 删除演员目录…
              </Button>
            </div>
          )}
        </div>
      )}
      {loading && !actor ? <p role="status">正在加载演员目录…</p> : actor && (
        <>
          <div className="actor-detail-hero">
            <ActorPortrait actor={actor} />
            <div><p className="eyebrow">演员视图</p><h2 id="actor-detail-title">{actor.name}</h2></div>
          </div>
          <dl className="actor-metrics">
            <Info k="派生路径" v={String(actor.derived_file_count ?? actor.hard_link_count)} />
            <Info k="去重文件" v={String(actor.unique_inode_count ?? actor.movie_count)} />
            <Info k="逻辑大小" v={formatBytes(actor.logical_size)} />
            <Info k="可回收空间" v={formatBytes(actor.reclaimable_space)} />
          </dl>
          <span className="sr-only" aria-hidden="true">引用的逻辑大小</span>
          <span className="sr-only" aria-hidden="true">移除后可回收空间</span>
          <section className="linked-assets">
            <div className="section-title"><h3>关联媒体资产</h3><span>{actor.linked_assets?.length ?? 0}</span></div>
            {(actor.linked_assets ?? []).length ? (
              <div className="linked-asset-grid">
                {(actor.linked_assets ?? []).map((asset) => (
                  <Button key={asset.id} data-asset-id={asset.id} aria-label={`打开资产 ${asset.jav_code ?? asset.title ?? "媒体资产"}`} onClick={() => openAsset(asset)}>
                    <LinkedAssetArtwork asset={asset} />
                    <span><b>{asset.jav_code ?? "媒体资产"}</b><small>{asset.title ?? asset.path}</small></span>
                  </Button>
                ))}
              </div>
            ) : <p className="muted">暂无关联媒体资产。</p>}
          </section>
        </>
      )}
      {!loading && error && (
        <div className="actor-feedback actor-detail-error" role="alert">
          <AlertTriangle aria-hidden="true" />
          <h2>{error}</h2>
          <p>演员目录仍然存在，请重新读取当前文件系统状态。</p>
          <Button onClick={retry}>重试演员目录</Button>
        </div>
      )}
    </motion.aside>
  );
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
function ActorRemovalDialog({
  actor,
  busy,
  cancel,
  remove,
}: {
  actor: ActorFolder;
  busy: boolean;
  cancel: () => void;
  remove: () => void;
}) {
  return (
    <section
      className="confirm-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="remove-actor-title"
    >
        <p className="eyebrow">安全移除派生路径</p>
        <h2 id="remove-actor-title">移除 {actor.name}？</h2>
        <p>
          只会解除此演员目录下的派生演员视图路径。源媒体资产、NFO 元数据和 Jellyfin 项目都不会被删除。
        </p>
        <dl>
          <div>
            <dt>演员目录</dt>
            <dd>{actor.name}</dd>
          </div>
          <div>
            <dt>影片</dt>
            <dd>{actor.movie_count}</dd>
          </div>
          <div>
            <dt>派生路径</dt>
            <dd>{actor.derived_file_count ?? actor.hard_link_count}</dd>
          </div>
          <div>
            <dt>去重文件</dt>
            <dd>{actor.unique_inode_count ?? 0}</dd>
          </div>
          <div>
            <dt>引用的逻辑大小</dt>
            <dd>{formatBytes(actor.logical_size)}</dd>
          </div>
          <div>
            <dt>移除后可回收空间</dt>
            <dd>{formatBytes(actor.reclaimable_space)}</dd>
          </div>
        </dl>
        <p className="regenerate-note">
          之后可根据源 NFO 元数据重新生成演员链接。硬链接要求演员视图和媒体根目录位于同一文件系统。
        </p>
        <div className="dialog-actions">
          <Button disabled={busy} onClick={cancel}>
            取消
          </Button>
          <Button className="danger" disabled={busy} onClick={remove}>
            通过管理任务移除
          </Button>
        </div>
    </section>
  );
}
function MediaStorageStatus({
  storage,
  compact = false,
}: {
  storage: StorageHealth;
  compact?: boolean;
}) {
  const { aggregate } = storage;
  const total = aggregate.total_bytes;
  const used = aggregate.used_bytes;
  const available = aggregate.available_bytes;
  const healthy =
    aggregate.status === "healthy" &&
    total !== null &&
    used !== null &&
    available !== null;
  const percentage = healthy && total > 0
    ? Math.round((used / total) * 100)
    : 0;
  const action = storage.roots.find((root) => root.action)?.action;

  return (
    <section
      className={`media-storage-card${compact ? " compact" : ""}`}
      role="region"
      aria-label="媒体存储"
    >
      <div className="media-storage-title">
        <HardDrive aria-hidden="true" />
        <span>媒体存储</span>
      </div>
      {healthy ? (
        <>
          <b>{formatBytes(total!)} 总量</b>
          <span>{formatBytes(used!)} 已用</span>
          <span>{formatBytes(available!)} 剩余</span>
          <Progress
            className="storage-progress"
            aria-label={`媒体存储已使用 ${percentage}%`}
            value={percentage}
          />
        </>
      ) : (
        <>
          <b>容量不可用</b>
          <span>{action ?? "无法读取媒体根目录容量"}</span>
        </>
      )}
    </section>
  );
}

function Empty() {
  return (
    <div className="empty">
      <span>◇</span>
      <h2>暂无媒体资产</h2>
      <p>配置媒体根目录后，重新扫描文件系统。</p>
    </div>
  );
}

function AssetArtwork({ asset }: { asset: Asset }) {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [asset.artwork_url]);
  const label = asset.jav_code ?? asset.title ?? "媒体资产";
  if (asset.artwork_url && !failed) {
    return (
      <img
        loading="lazy"
        src={asset.artwork_url}
        alt={`${label} 封面`}
        onError={() => setFailed(true)}
      />
    );
  }
  return (
    <div className="placeholder" role="img" aria-label={`${label} 暂无封面`}>
      <span>◇</span>
      <small>暂无封面</small>
    </div>
  );
}

function LinkedAssetArtwork({ asset }: { asset: Asset }) {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [asset.artwork_url]);
  if (!asset.artwork_url || failed) return <Film aria-hidden="true" />;
  return (
    <img
      src={asset.artwork_url}
      alt=""
      loading="lazy"
      onError={() => setFailed(true)}
    />
  );
}
function LegacyTaskPanel({
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
      <section className="task-create">
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
              onClick={() => setSelectedOps(operations.map(([key]) => key))}
            >
              完整流程
            </Button>
          </div>
          <div className="operation-list">
            {operations.map(([key, label]) => (
              <label key={key}>
                <Input
                  type="checkbox"
                  checked={selectedOps.includes(key)}
                  onChange={() => toggle(key)}
                />
                <span>{label}</span>
              </label>
            ))}
          </div>
          <Button type="submit" disabled={!selectedOps.length}>
            预览 15 分钟有效的计划
          </Button>
        </form>
      </section>
      <section className="task-history">
        <div className="task-title">
          <div>
            <h2>任务生命周期</h2>
            <p>持久化历史、实时进度、报告与验证</p>
            <p className="task-count">{taskTotal} 个任务</p>
          </div>
          <Button className="refresh" onClick={() => void refresh()}>
            刷新
          </Button>
        </div>
        {tasks.length === 0 ? (
          <p className="task-empty">暂无管理任务。</p>
        ) : (
          <ol className="tasks">
            {tasks.map((task) => (
              <li key={task.id}>
                <div className="task-summary">
                  <span className={`status status-${taskDisplayStatus(task)}`}>
                    {taskStatusLabels[taskDisplayStatus(task)]}
                  </span>
                  <strong>{taskKindLabels[task.kind]}</strong>
                  <span className="task-root">{task.media_root}</span>
                </div>
                <small>
                  {task.items.length} 个项目结果 · {task.id}
                </small>
                {(task.status === "queued" || task.status === "running") && (
                  <div
                    className="task-progress"
                    role="progressbar"
                    aria-label="任务进度"
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={taskProgressPercent(task)}
                  >
                    <span style={{ width: taskProgressPercent(task) === undefined
                      ? undefined
                      : `${taskProgressPercent(task)}%` }} />
                  </div>
                )}
                {task.error && (
                  <p className="task-error" role={task.status === "failed" ? "alert" : undefined}>
                    {task.error}
                  </p>
                )}
                {task.operation_plan && (
                  <div className="plan">
                    <b>检查最终路径</b>
                    <small>
                      到期时间{" "}
                      {new Date(
                        task.plan_expires_at! * 1000,
                      ).toLocaleTimeString()}
                    </small>
                    {task.operation_plan.warnings.map((warning) => (
                      <p className="task-error" key={warning}>
                        {warning}
                      </p>
                    ))}
                    <ul>
                      {task.operation_plan.actions.slice(0, 50).map((action, index) => (
                        <li
                          className={action.destructive ? "destructive" : ""}
                          key={index}
                        >
                          <span>
                            {action.destructive ? "破坏性操作" : kindLabel(action.kind)}
                          </span>
                          <code>{action.path ?? "—"}</code>
                        </li>
                      ))}
                      {task.operation_plan.actions.length > 50 && (
                        <li className="task-truncated">
                          最终报告中还有 {task.operation_plan.actions.length - 50} 个计划操作
                        </li>
                      )}
                    </ul>
                    {task.status === "completed" &&
                      !task.plan_consumed_at &&
                      Date.now() / 1000 <= task.plan_expires_at! && (
                        <Button onClick={() => requestPlanConfirmation(task)}>
                          确认并执行
                        </Button>
                      )}
                  </div>
                )}
                {task.items.length > 0 && (
                  <ul className="task-items">
                    {task.items.slice(0, 50).map((item) => (
                      <li key={item.id}>
                        <span>{taskItemStatusLabels[item.status] ?? item.status}</span>
                        <b>{kindLabel(item.kind)}</b>
                        <span className="task-item-path">
                          <code>{item.path ?? "—"}</code>
                          {item.path && (
                            <Button
                              type="button"
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
                    {task.items.length > 50 && (
                      <li className="task-truncated">
                        最终报告中还有 {task.items.length - 50} 个项目结果
                      </li>
                    )}
                  </ul>
                )}
                {task.report && (
                  <details>
                    <summary>最终报告和迁移验证</summary>
                    <pre>{JSON.stringify(task.report, null, 2)}</pre>
                  </details>
                )}
              </li>
            ))}
          </ol>
        )}
        {hasMoreTasks && (
          <Button
            type="button"
            className="show-more-tasks"
            disabled={historyPageLoading}
            onClick={() => void loadMore()}
          >
            {historyPageLoading ? "正在加载任务…" : "再加载 20 个任务"}
          </Button>
        )}
      </section>
    </div>
  );
}
function formatDate(v: string) {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "long",
    day: "numeric",
    year: "numeric",
  }).format(new Date(`${v}T00:00:00`));
}
function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  const units = ["KiB", "MiB", "GiB", "TiB"];
  let size = value / 1024,
    index = 0;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index++;
  }
  return `${size.toFixed(size >= 10 ? 0 : 1)} ${units[index]}`;
}
const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
