import React, { FormEvent, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import {
  AlertTriangle,
  ArrowLeft,
  Clock3,
  Film,
  Grid2X2,
  HardDrive,
  Image as ImageIcon,
  ListTodo,
  LogOut,
  RefreshCw,
  Search,
  Settings,
  Trash2,
  UserRound,
  Users,
  X,
} from "lucide-react";
import { BeUITab, BeUITabPanel, BeUITabs, BeUITabsList } from "./beui-tabs";
import { AnimatedToastStack } from "./components/motion/animated-toast-stack";
import { MorphingModal } from "./components/motion/morphing-modal";
import { TiltCard } from "./components/motion/tilt-card";
import { EASE_OUT } from "./lib/ease";
import "./style.css";
import "./design-system.css";
type View = "loading" | "initialize" | "login" | "ready";
const DEFAULT_RULE_SOURCE =
  "https://raw.githubusercontent.com/hqwuzhaoyi/rust-jav/feature/web-jellyfin-truenas/rules.yaml";
type Validation = { valid: true; empty: boolean; yaml: string } | null;
type AssetState = "normal" | "synchronizing" | "exception";
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
type Candidate = {
  path: string;
  matching_rule: string;
  type: string;
  video_warning: string | null;
  logical_size: number;
  reclaimable_space: number;
};
type DeletionPlan = {
  id: string;
  selection: "selected" | "unified";
  logical_size: number;
  reclaimable_space: number;
  expires_at: number;
  paths: Array<{ path: string; type: string; video_warning: string | null }>;
  discovered_hard_links: Array<{ path: string }>;
};
type OperationPlan = {
  operations: string[];
  actions: Array<{
    kind: string;
    path: string | null;
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
  items: Array<{
    id: number;
    kind: string;
    path: string | null;
    status: string;
    message: string | null;
  }>;
};
const operations = [
  ["delete_ad_files", "Delete ad files"],
  ["organize_by_code", "Organize by code"],
  ["clean_empty_dirs", "Clean empty directories"],
  ["standardize_names", "Standardize names"],
  ["extract_codes", "Extract codes"],
  ["categorize_files", "Categorize files"],
  ["move_origin", "Move to ORIGIN"],
  ["remove_duplicates", "Remove duplicates"],
] as const;
const labels: Record<AssetState, string> = {
  normal: "正常",
  synchronizing: "同步中",
  exception: "异常",
};
const assetStates = new Set<AssetState>(["normal", "synchronizing", "exception"]);
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
function galleryStateFromUrl() {
  const params = new URLSearchParams(location.search);
  const rawState = params.get("state") as AssetState | null;
  const rawPage = Number(params.get("page") ?? "1");
  return {
    query: params.get("q") ?? "",
    filter: rawState && assetStates.has(rawState) ? rawState : "" as const,
    page: Number.isInteger(rawPage) && rawPage > 0 ? rawPage : 1,
  };
}
function galleryUrl(query: string, filter: AssetState | "", page: number, assetId?: string | null) {
  const params = new URLSearchParams();
  params.set("page", String(page));
  params.set("per_page", "48");
  if (query) params.set("q", query);
  if (filter) params.set("state", filter);
  const pathname = assetId ? `/assets/${encodeURIComponent(assetId)}` : "/";
  return `${pathname}?${params}`;
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
    [nav, setNav] = useState<
      | "assets"
      | "recent"
      | "exceptions"
      | "actors"
      | "deletion"
      | "tasks"
      | "settings"
    >(actorNameFromPath() || isActorListPath() ? "actors" : "assets");
  const [tasks, setTasks] = useState<Task[]>([]),
    [mediaRoot, setMediaRoot] = useState("/media"),
    [selectedOps, setSelectedOps] = useState<string[]>(
      operations.map(([key]) => key),
    );
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
  const [sourceUrl, setSourceUrl] = useState(DEFAULT_RULE_SOURCE);
  const [editing, setEditing] = useState(false);
  const [validation, setValidation] = useState<Validation>(null);
  const [rulesMessage, setRulesMessage] = useState("");
  const [jfUrl, setJfUrl] = useState("");
  const [jfLibraries, setJfLibraries] = useState("");
  const [jfKey, setJfKey] = useState("");
  const [jfKeyConfigured, setJfKeyConfigured] = useState(false);
  const [candidates, setCandidates] = useState<Candidate[]>([]),
    [selected, setSelected] = useState<string[]>([]),
    [plan, setPlan] = useState<DeletionPlan | null>(null),
    [confirmText, setConfirmText] = useState("");
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
  useEffect(() => {
    inspectedAssetRef.current = inspectedAsset;
  }, [inspectedAsset]);
  useEffect(() => {
    inspectedActorRef.current = inspectedActor;
  }, [inspectedActor]);
  useEffect(() => {
    if (!token)
      fetch("/api/v1/status").then((r) => {
        if (r.ok) setView("ready");
        else {
          setView("login");
          if (r.status === 503)
            setMessage("Run rust-jav administrator init locally first.");
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
    const response = await fetch("/api/v1/jellyfin/config");
    if (response.ok) {
      const config = (await response.json().catch(() => null)) as {
        url?: string;
        library_ids?: string[];
        api_key_configured?: boolean;
      } | null;
      if (!config) return;
      setJfUrl(config.url ?? "");
      setJfLibraries((config.library_ids ?? []).join(", "));
      setJfKeyConfigured(config.api_key_configured ?? false);
    }
  }
  async function saveJellyfin(event: FormEvent) {
    event.preventDefault();
    const response = await fetch("/api/v1/jellyfin/config", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        url: jfUrl,
        library_ids: jfLibraries
          .split(",")
          .map((value) => value.trim())
          .filter(Boolean),
        api_key: jfKey,
      }),
    });
    setMessage(
      response.ok ? "Jellyfin configuration saved." : await response.text(),
    );
    if (response.ok) setJfKey("");
  }
  async function testJellyfin() {
    const response = await fetch("/api/v1/jellyfin/test", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: "{}",
    });
    setMessage(
      response.ok
        ? `Connected to ${((await response.json()) as { server_name: string }).server_name}.`
        : await response.text(),
    );
  }
  async function refreshJellyfin() {
    const response = await fetch("/api/v1/jellyfin/refresh", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: "{}",
    });
    setMessage(
      response.ok
        ? "Jellyfin library refresh completed."
        : await response.text(),
    );
  }
  async function loadActors() {
    setActorListState("loading");
    try {
      const response = await fetch("/api/v1/actors");
      if (!response.ok) throw new Error("Actor Folders could not be loaded.");
      const folders = (await response.json().catch(() => null)) as ActorFolder[] | null;
      if (!Array.isArray(folders)) throw new Error("Actor Folders could not be loaded.");
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
      if (!response.ok) throw new Error("Actor Folder could not be loaded.");
      const detail = (await response.json()) as ActorFolder;
      if (request !== actorRequest.current) return;
      setInspectedActor(detail);
    } catch {
      if (request === actorRequest.current)
        setActorDetailError("Actor Folder could not be loaded.");
    } finally {
      if (request === actorRequest.current) setActorDetailLoading(false);
    }
  }
  function showActorFolders() {
    actorRequest.current += 1;
    detailRequest.current += 1;
    setInspectedActor(null);
    setActorDetailError(null);
    setInspectedAsset(null);
    setAssetDetail(null);
    setNav("actors");
    if (!isActorListPath()) history.pushState({}, "", "/actors");
  }
  function closeActor() {
    actorRequest.current += 1;
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
        (await response.text()) || "Actor Folder could not be revalidated.",
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
        (await response.text()) || "Actor Folder removal was rejected.",
      );
      setActorBusy(false);
      return;
    }
    const task = (await response.json()) as Task;
    setTasks((current) => [task, ...current]);
    actorRemovalTasksRef.current.set(task.id, actorName);
    watchTask(task.id);
    setConfirmActor(null);
    setActorRemovalNotice("Actor Folder removal started as a Management Task.");
    setActorBusy(false);
  }
  async function loadCandidates() {
    const r = await fetch("/api/v1/deletion-candidates");
    if (r.ok) setCandidates(((await r.json()) as { items: Candidate[] }).items);
  }
  async function previewDeletion(selection: "selected" | "unified") {
    const r = await fetch("/api/v1/deletion-plans", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ paths: selected, selection }),
    });
    if (r.ok) {
      setPlan((await r.json()) as DeletionPlan);
      setConfirmText("");
    } else setMessage(await r.text());
  }
  async function executeDeletion() {
    if (!plan) return;
    const r = await fetch(`/api/v1/deletion-plans/${plan.id}/execute`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ irreversible: true, confirmation: confirmText }),
    });
    if (r.ok) {
      setPlan(null);
      setSelected([]);
      setMessage(
        "Permanent deletion finished. Per-path outcomes are in Management Tasks.",
      );
      await loadCandidates();
    } else setMessage(await r.text());
  }
  async function loadRules() {
    const response = await fetch("/api/v1/rules/active");
    if (response.ok) {
      const body = (await response.json().catch(() => null)) as { yaml?: string } | null;
      if (typeof body?.yaml === "string") setYaml(body.yaml);
    }
  }
  function updateYaml(value: string) {
    setYaml(value);
    setValidation(null);
    setRulesMessage("");
  }
  async function downloadProposal() {
    setRulesMessage("Downloading proposal…");
    const response = await fetch("/api/v1/rules/download", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ url: sourceUrl }),
    });
    const body = (await response.json()) as { yaml?: string; error?: string };
    if (!response.ok || !body.yaml) {
      setRulesMessage(body.error ?? "Download failed.");
      return;
    }
    updateYaml(body.yaml);
    setEditing(true);
    setRulesMessage("Proposal downloaded. Validate it before saving.");
  }
  async function validateRules() {
    const candidate = yaml;
    setRulesMessage("Validating…");
    const response = await fetch("/api/v1/rules/validate", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ yaml: candidate }),
    });
    const body = (await response.json()) as {
      valid?: boolean;
      empty?: boolean;
      error?: string;
    };
    if (!response.ok || !body.valid) {
      setValidation(null);
      setRulesMessage(body.error ?? "Validation failed.");
      return;
    }
    setValidation({ valid: true, empty: Boolean(body.empty), yaml: candidate });
    setRulesMessage(
      body.empty
        ? "Valid, but empty. A separate confirmation is required."
        : "Valid proposal. Ready to save.",
    );
  }
  async function saveRules(confirmEmpty = false) {
    const response = await fetch("/api/v1/rules/active", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ yaml, confirm_empty: confirmEmpty }),
    });
    if (!response.ok) {
      const body = (await response.json()) as { error?: string };
      setRulesMessage(
        body.error ??
          "Save failed; the previous Active Rule Set remains active.",
      );
      return;
    }
    setEditing(false);
    setValidation(null);
    setRulesMessage("Active Rule Set saved atomically.");
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
      if (!a.ok) throw new Error("asset request failed");
      const body = (await a.json().catch(() => null)) as Page | null;
      if (!body) throw new Error("asset response was invalid");
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
    setMessage("Reconciling filesystem…");
    const r = await fetch("/api/v1/assets/scan", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ mode: "manual" }),
    });
    setMessage(r.ok ? "Asset Index reconciled." : await r.text());
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
      if (!response.ok) throw new Error("asset detail request failed");
      const detail = (await response.json()) as AssetDetail;
      if (request === detailRequest.current) setAssetDetail(detail);
    } catch {
      if (request === detailRequest.current)
        setMessage("Asset details could not be loaded.");
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
      if (!response.ok) throw new Error("asset detail request failed");
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
        setMessage("Asset details could not be loaded.");
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
        setMessage("Administrator is already initialized. Sign in to continue.");
        return;
      }
      if (!r.ok) {
        setMessage(
          !init && r.status === 401
            ? "Incorrect password."
            : init && r.status === 400
              ? "Password must be at least 4 characters."
              : init && r.status === 403
                ? "Initialization link is invalid or has expired. Create a new link locally."
                : "The server could not complete the request. Try again.",
        );
        return;
      }
      if (init) {
        history.replaceState({}, "", "/");
        setView("login");
        setMessage("Administrator initialized. Sign in to continue.");
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
  async function loadTasks() {
    const response = await fetch("/api/v1/tasks");
    if (response.ok) {
      const recovered = (await response.json().catch(() => null)) as
        Task[] | null;
      if (!recovered) return;
      setTasks(recovered);
      recovered
        .filter((task) => ["queued", "running"].includes(task.status))
        .forEach((task) => watchTask(task.id));
    }
  }
  function watchTask(id: string) {
    const source = new EventSource(`/api/v1/tasks/${id}/events`);
    source.addEventListener("task", (event) => {
      const task = JSON.parse((event as MessageEvent).data) as Task;
      setTasks((current) => [
        task,
        ...current.filter((item) => item.id !== task.id),
      ]);
      if (["completed", "failed", "interrupted"].includes(task.status)) {
        source.close();
        const actorName = actorRemovalTasksRef.current.get(task.id);
        if (actorName) {
          actorRemovalTasksRef.current.delete(task.id);
          if (task.status === "completed") {
            setActorRemovalNotice("Actor Folder removal completed as a Management Task.");
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
              `Actor Folder removal task ${task.status}. The Actor Folder was kept.`,
            );
          }
        }
      }
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
        operations: selectedOps,
      }),
    });
    if (!response.ok) {
      setMessage((await response.text()) || "Task request rejected.");
      return;
    }
    const task = (await response.json()) as Task;
    setTasks((current) => [task, ...current]);
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
      setMessage((await response.text()) || "Confirmation rejected.");
      return;
    }
    const task = (await response.json()) as Task;
    setTasks((current) => [task, ...current]);
    watchTask(task.id);
  }
  const grouped = useMemo(
    () =>
      assets.groups
        .map((group) => ({
          group,
          items: assets.items.filter((a) => a.captured_date === group.date),
        }))
        .filter((g) => g.items.length),
    [assets],
  );
  if (view === "loading") return <div className="auth ui-foundation">Checking session…</div>;
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
          {view === "initialize" ? "Initialize Administrator" : "Welcome back"}
        </h1>
        <form className="ui-panel" onSubmit={submit}>
          <label htmlFor="password">Password</label>
          <input
            id="password"
            type="password"
            minLength={4}
            autoComplete={view === "initialize" ? "new-password" : "current-password"}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            autoFocus
          />
          <button className="ui-primary-button" type="submit" disabled={submitting}>
            {submitting
              ? "Please wait…"
              : view === "initialize"
                ? "Initialize"
                : "Sign in"}
          </button>
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
          <button
            aria-label="所有资产"
            title="All Assets"
            aria-current={nav === "assets" ? "page" : undefined}
            className={nav === "assets" ? "active" : ""}
            onClick={() => {
              setNav("assets");
              setFilter("");
              setPage(1);
              history.pushState({}, "", galleryUrl(query, "", 1));
            }}
          >
            <span><Grid2X2 aria-hidden="true" /></span> 所有资产 <em>{libraryTotal || assets.total}</em>
          </button>
          <button
            aria-label="Recently Added"
            aria-current={nav === "recent" ? "page" : undefined}
            className={nav === "recent" ? "active" : ""}
            onClick={() => {
              setNav("recent");
              setFilter("");
              setPage(1);
              history.pushState({}, "", galleryUrl(query, "", 1));
            }}
          >
            <span><Clock3 aria-hidden="true" /></span> 最近入库
          </button>
          <button
            aria-label="Actors"
            aria-current={nav === "actors" ? "page" : undefined}
            className={nav === "actors" ? "active" : ""}
            onClick={showActorFolders}
          >
            <span><Users aria-hidden="true" /></span> 演员
            <em>{actors.length}</em>
          </button>
          <button
            aria-label="Deletion Candidates"
            aria-current={nav === "deletion" ? "page" : undefined}
            className={nav === "deletion" ? "active" : ""}
            onClick={() => setNav("deletion")}
          >
            <span><Trash2 aria-hidden="true" /></span> 删除候选
          </button>
          <p>管理</p>
          <button
            aria-label="Management Tasks"
            aria-current={nav === "tasks" ? "page" : undefined}
            className={nav === "tasks" ? "active" : ""}
            onClick={() => setNav("tasks")}
          >
            <span><ListTodo aria-hidden="true" /></span> 整理任务
          </button>
          <button
            aria-label="Exceptions"
            aria-current={nav === "exceptions" ? "page" : undefined}
            className={nav === "exceptions" ? "active" : ""}
            onClick={() => {
              setNav("exceptions");
              setFilter("exception");
              setPage(1);
              history.pushState({}, "", galleryUrl(query, "exception", 1));
            }}
          >
            <span><AlertTriangle aria-hidden="true" /></span> 异常资产
          </button>
          <button
            aria-label="Settings"
            aria-current={nav === "settings" ? "page" : undefined}
            className={nav === "settings" ? "active" : ""}
            onClick={() => setNav("settings")}
          >
            <span><Settings aria-hidden="true" /></span> 设置
          </button>
        </nav>
        {storage && <MediaStorageStatus storage={storage} />}
        <div className="root-card">
          <small>资产索引</small>
          <b>
            <i className={`health-dot ${health?.state}`} />
            {health?.state ?? "Loading"}
          </b>
          <span>
            {health?.mode
              ? `Last ${health.mode} scan`
              : "Filesystem authoritative"}
          </span>
        </div>
        <button className="signout" onClick={logout} aria-label="Sign out">
          <LogOut aria-hidden="true" /> 退出登录
        </button>
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
                    ? `${tasks.length} 个持久化任务`
                    : `${libraryTotal || assets.total} 个项目 · 文件系统为准`}
            </small>
          </div>
          {storage && (
            <button
              className="mobile-storage-entry"
              aria-label="媒体存储"
              onClick={() => setStorageOpen(true)}
            >
              <HardDrive aria-hidden="true" />
              <span>媒体存储</span>
            </button>
          )}
          {(nav === "assets" || nav === "recent" || nav === "exceptions") && (
            <button className="scan" onClick={scan} aria-label="Reconcile">
              <RefreshCw aria-hidden="true" /> <span>重新扫描</span>
            </button>
          )}
        </header>
        {(nav === "assets" || nav === "recent" || nav === "exceptions") && (
          <>
            <div className="toolbar">
              <label className="search">
                <Search aria-hidden="true" />
                <input
                  aria-label="Search assets"
                  placeholder="搜索番号、标题或路径"
                  value={query}
                  onChange={(e) => {
                    const nextQuery = e.target.value;
                    setQuery(nextQuery);
                    setPage(1);
                    history.replaceState(
                      {},
                      "",
                      galleryUrl(nextQuery, filter, 1, assetIdFromPath()),
                    );
                  }}
                />
              </label>
              <div className="filters">
                <button
                  className={!filter ? "selected" : ""}
                  aria-pressed={!filter}
                  onClick={() => {
                    setFilter("");
                    setPage(1);
                    history.pushState({}, "", galleryUrl(query, "", 1, assetIdFromPath()));
                  }}
                >
                  全部
                </button>
                {(["normal", "synchronizing", "exception"] as AssetState[]).map(
                  (s) => (
                    <button
                      key={s}
                      className={filter === s ? "selected" : ""}
                      aria-pressed={filter === s}
                      title={s === "synchronizing" ? "Synchronizing" : undefined}
                      onClick={() => {
                        setFilter(s);
                        setPage(1);
                        history.pushState({}, "", galleryUrl(query, s, 1, assetIdFromPath()));
                      }}
                    >
                      {s === "normal" ? "正常" : s === "synchronizing" ? "刷新中" : "异常"}
                    </button>
                  ),
                )}
              </div>
            </div>
            <div className="library" aria-busy={galleryLoading}>
              {galleryLoading ? (
                <div className="gallery-feedback gallery-loading" role="status" aria-label="正在加载媒体资产">
                  <RefreshCw aria-hidden="true" />
                  <span>正在加载媒体资产…</span>
                </div>
              ) : galleryError ? (
                <div className="gallery-feedback gallery-error" role="alert">
                  <AlertTriangle aria-hidden="true" />
                  <h2>无法加载媒体资产</h2>
                  <p>请检查连接后重试。</p>
                  <button onClick={() => setGalleryRetry((value) => value + 1)}>重试</button>
                </div>
              ) : grouped.length === 0 ? (
                <Empty />
              ) : (
                grouped.map(({ group, items }) => (
                  <section className="date-group" key={group.date}>
                    <div className="date-heading">
                      <h2>{formatDate(group.date)}</h2>
                      <span>{group.count} items</span>
                    </div>
                    <div className="asset-grid">
                      {items.map((a) => (
                        <TiltCard
                          className={`asset-card photos-tile ${inspectedAsset?.id === a.id ? "selected" : ""}`}
                          key={a.id}
                        >
                          <button
                            className="asset-select"
                            onClick={() => void inspect(a)}
                            aria-label={`查看资产 ${a.jav_code ?? a.title ?? "未识别资产"}`}
                          >
                            <div className="poster" style={{ aspectRatio: "4 / 3" }}>
                              <AssetArtwork asset={a} />
                              <div className="asset-overlay">
                                <Film aria-hidden="true" />
                                <span>
                                  <b>{a.jav_code ?? a.title ?? "Unidentified"}</b>
                                  <small>{a.title ?? a.path.split("/").pop()}</small>
                                </span>
                                <em className={`state-label ${a.state}`}>{labels[a.state]}</em>
                              </div>
                            </div>
                          </button>
                        </TiltCard>
                      ))}
                    </div>
                  </section>
                ))
              )}
            </div>
            {assets.total_pages > 1 && (
              <div className="pagination">
                <button
                  disabled={page === 1}
                  onClick={() => {
                    const nextPage = page - 1;
                    setPage(nextPage);
                    history.pushState({}, "", galleryUrl(query, filter, nextPage, assetIdFromPath()));
                  }}
                >
                  Previous
                </button>
                <span>
                  {page} / {assets.total_pages}
                </span>
                <button
                  disabled={page === assets.total_pages}
                  onClick={() => {
                    const nextPage = page + 1;
                    setPage(nextPage);
                    history.pushState({}, "", galleryUrl(query, filter, nextPage, assetIdFromPath()));
                  }}
                >
                  Next
                </button>
              </div>
            )}
          </>
        )}
        {nav === "tasks" && (
          <TaskPanel
            tasks={tasks}
            mediaRoot={mediaRoot}
            setMediaRoot={setMediaRoot}
            selectedOps={selectedOps}
            setSelectedOps={setSelectedOps}
            createTask={createTask}
            confirmPlan={confirmPlan}
            refresh={loadTasks}
          />
        )}
        {nav === "actors" && (
          <ActorFolders
            actors={actors}
            state={actorListState}
            busy={actorBusy}
            inspect={(actor) => void openActor(actor)}
            remove={requestActorRemoval}
            retry={() => void loadActors()}
          />
        )}
        {nav === "deletion" && (
          <section className="deletion-browser">
            <div className="deletion-intro">
              <div>
                <p className="eyebrow">ACTIVE RULE SET</p>
                <h2>Review permanent deletion</h2>
                <p>
                  Sizes are current filesystem observations. Nothing is deleted
                  until an Operation Plan is explicitly confirmed.
                </p>
              </div>
              <button
                disabled={!selected.length}
                onClick={() => void previewDeletion("selected")}
              >
                Review {selected.length || "selected"}
              </button>
            </div>
            <div className="candidate-list">
              {candidates.map((candidate) => (
                <label className="candidate" key={candidate.path}>
                  <input
                    type="checkbox"
                    aria-label={`Select ${candidate.path}`}
                    checked={selected.includes(candidate.path)}
                    onChange={(e) =>
                      setSelected((current) =>
                        e.target.checked
                          ? [...current, candidate.path]
                          : current.filter((path) => path !== candidate.path),
                      )
                    }
                  />
                  <div>
                    <code title={candidate.path}>{candidate.path}</code>
                    <small>
                      Rule: {candidate.matching_rule} · {candidate.type}
                    </small>
                    {candidate.video_warning && (
                      <strong>⚠ Video content</strong>
                    )}
                  </div>
                  <dl>
                    <div>
                      <dt>Logical Size</dt>
                      <dd>{formatBytes(candidate.logical_size)}</dd>
                    </div>
                    <div>
                      <dt>Reclaimable Space</dt>
                      <dd>{formatBytes(candidate.reclaimable_space)}</dd>
                    </div>
                  </dl>
                </label>
              ))}
            </div>
            {!candidates.length && (
              <p className="task-empty">No paths match the Active Rule Set.</p>
            )}
          </section>
        )}
        {nav === "settings" && (
          <div className="settings-stack">
            <section className="rules-settings">
              <p className="eyebrow">DELETION RULES</p>
              <h2>Active Rule Set</h2>
              <p>
                Remote YAML is only a proposal. The server validates and
                atomically activates it; rules cannot select roots or authorize
                deletion.
              </p>
              <label htmlFor="rule-source">Rule Source URL</label>
              <div className="rule-actions">
                <input
                  id="rule-source"
                  type="url"
                  placeholder="https://raw.githubusercontent.com/…"
                  value={sourceUrl}
                  onChange={(event) => setSourceUrl(event.target.value)}
                />
                <button
                  type="button"
                  disabled={!sourceUrl}
                  onClick={downloadProposal}
                >
                  Download proposal
                </button>
              </div>
              <label htmlFor="rules-yaml">Active Rule Set YAML</label>
              <textarea
                id="rules-yaml"
                rows={18}
                readOnly={!editing}
                value={yaml}
                onChange={(event) => updateYaml(event.target.value)}
              />
              <div className="rule-actions">
                {!editing && (
                  <button
                    type="button"
                    onClick={() => {
                      setEditing(true);
                      setValidation(null);
                    }}
                  >
                    Edit
                  </button>
                )}
                {editing && (
                  <button type="button" onClick={validateRules}>
                    Validate
                  </button>
                )}
                {editing && !validation?.empty && (
                  <button
                    type="button"
                    disabled={!validation || validation.yaml !== yaml}
                    onClick={() => saveRules(false)}
                  >
                    Save Active Rule Set
                  </button>
                )}
                {editing && validation?.empty && (
                  <button
                    type="button"
                    className="danger"
                    disabled={validation.yaml !== yaml}
                    onClick={() => saveRules(true)}
                  >
                    Confirm empty and save
                  </button>
                )}
              </div>
              {rulesMessage && (
                <p role="status" className="notice">
                  {rulesMessage}
                </p>
              )}
            </section>
            <section className="task-create jellyfin-settings">
              <p className="eyebrow">MEDIA SERVER</p>
              <h2>Jellyfin</h2>
              <p>
                Connect one server and select multiple library IDs. The API key
                stays on this server.
              </p>
              <form className="task-form" onSubmit={saveJellyfin}>
                <label htmlFor="jellyfin-url">Server URL</label>
                <input
                  id="jellyfin-url"
                  type="url"
                  value={jfUrl}
                  onChange={(event) => setJfUrl(event.target.value)}
                  placeholder="http://jellyfin:8096"
                  required
                />
                <label htmlFor="jellyfin-libraries">Library IDs</label>
                <input
                  id="jellyfin-libraries"
                  value={jfLibraries}
                  onChange={(event) => setJfLibraries(event.target.value)}
                  placeholder="movies, jav"
                  required
                />
                <label htmlFor="jellyfin-key">Server API key</label>
                <input
                  id="jellyfin-key"
                  type="password"
                  autoComplete="off"
                  value={jfKey}
                  onChange={(event) => setJfKey(event.target.value)}
                  required={!jfKeyConfigured}
                />
                <button type="submit">Save Jellyfin</button>
              </form>
              <div className="jellyfin-actions">
                <button onClick={() => void testJellyfin()}>
                  Test connection
                </button>
                <button onClick={() => void refreshJellyfin()}>
                  Refresh Jellyfin
                </button>
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
          backLabel={assetBackActor ? `Back to ${assetBackActor.name}` : undefined}
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
      {actorRemovalNotice && (
        <div className="shell-notice" role="status">
          <p>{actorRemovalNotice}</p>
          <button onClick={() => setActorRemovalNotice(null)} aria-label="Dismiss Actor removal notification">
            <X aria-hidden="true" />
          </button>
        </div>
      )}
      {actorRemovalFailure && (
        <div className="shell-notice actor-removal-failure" role="alert">
          <p>{actorRemovalFailure}</p>
          <button onClick={() => setActorRemovalFailure(null)} aria-label="Dismiss Actor removal error">
            <X aria-hidden="true" />
          </button>
        </div>
      )}
      <AnimatedToastStack
        fixed
        toasts={
          message
            ? [{ id: "shell-status", title: message, status: "info", duration: 0 }]
            : []
        }
        onDismiss={() => setMessage("")}
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
            <button
              className="storage-dialog-close"
              aria-label="关闭媒体存储"
              onClick={() => setStorageOpen(false)}
            >
              <X aria-hidden="true" />
            </button>
            <MediaStorageStatus storage={storage} compact />
          </section>
        )}
      </MorphingModal>
      <nav className="bottom-nav" aria-label="移动端主导航">
        <button
          aria-label="图库"
          aria-current={nav === "assets" || nav === "recent" || nav === "exceptions" ? "page" : undefined}
          className={nav === "assets" || nav === "recent" || nav === "exceptions" ? "active" : ""}
          onClick={() => {
            setNav("assets");
            setFilter("");
            setPage(1);
            history.pushState({}, "", galleryUrl(query, "", 1));
          }}
        >
          <span><Grid2X2 aria-hidden="true" /></span>图库
        </button>
        <button
          aria-label="Actors"
          aria-current={nav === "actors" ? "page" : undefined}
          className={nav === "actors" ? "active" : ""}
          onClick={showActorFolders}
        >
          <span><Users aria-hidden="true" /></span>演员
        </button>
        <button
          aria-label="Delete"
          aria-current={nav === "deletion" ? "page" : undefined}
          className={nav === "deletion" ? "active" : ""}
          onClick={() => setNav("deletion")}
        >
          <span><Trash2 aria-hidden="true" /></span>删除
        </button>
        <button
          aria-label="Tasks"
          aria-current={nav === "tasks" ? "page" : undefined}
          className={nav === "tasks" ? "active" : ""}
          onClick={() => setNav("tasks")}
        >
          <span><ListTodo aria-hidden="true" /></span>任务
        </button>
        <button
          aria-label="Settings"
          aria-current={nav === "settings" ? "page" : undefined}
          className={nav === "settings" ? "active" : ""}
          onClick={() => setNav("settings")}
        >
          <span><Settings aria-hidden="true" /></span>设置
        </button>
      </nav>
      {plan && (
        <div className="modal-backdrop" role="presentation">
          <section
            className="delete-confirm"
            role="dialog"
            aria-modal="true"
            aria-labelledby="delete-title"
          >
            <p className="eyebrow">IRREVERSIBLE ACTION</p>
            <h2 id="delete-title">
              Permanently delete {plan.paths.length} paths?
            </h2>
            <div className="choice">
              <button
                className={plan.selection === "selected" ? "selected" : ""}
                onClick={() => void previewDeletion("selected")}
              >
                Selected paths only
              </button>
              <button
                className={plan.selection === "unified" ? "selected" : ""}
                onClick={() => void previewDeletion("unified")}
              >
                All discovered hard links ({plan.discovered_hard_links.length})
              </button>
            </div>
            <p>
              <b>{formatBytes(plan.logical_size)}</b> logical ·{" "}
              <b>{formatBytes(plan.reclaimable_space)}</b> reclaimable
            </p>
            {plan.paths.some((path) => path.video_warning) && (
              <p className="video-warning">
                ⚠ This plan permanently removes video content.
              </p>
            )}
            <div className="plan-paths">
              {plan.paths.map((path) => (
                <code key={path.path}>{path.path}</code>
              ))}
            </div>
            <label htmlFor="confirm-delete">
              Type <b>PERMANENTLY DELETE</b> to confirm
            </label>
            <input
              id="confirm-delete"
              value={confirmText}
              onChange={(event) => setConfirmText(event.target.value)}
              autoComplete="off"
            />
            <div className="confirm-actions">
              <button onClick={() => setPlan(null)}>Cancel</button>
              <button
                className="danger"
                disabled={confirmText !== "PERMANENTLY DELETE"}
                onClick={() => void executeDeletion()}
              >
                Permanently delete
              </button>
            </div>
          </section>
        </div>
      )}
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
  const dialogRef = useRef<HTMLElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const closeRef = useRef(close);
  closeRef.current = close;
  const prefersReducedMotion = useReducedMotion();
  const reduce = prefersReducedMotion
    || (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false);
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
    <motion.aside
      ref={dialogRef}
      initial={reduce ? false : mobile ? { y: 28, opacity: 0 } : { x: 28, opacity: 0 }}
      animate={mobile ? { y: 0, opacity: 1 } : { x: 0, opacity: 1 }}
      exit={reduce ? { opacity: 0 } : mobile ? { y: 28, opacity: 0 } : { x: 28, opacity: 0 }}
      transition={reduce ? { duration: 0 } : undefined}
      className="asset-inspector"
      role="dialog"
      aria-modal="true"
      aria-labelledby="asset-detail-title"
    >
      <div className="sheet-handle" aria-hidden="true" />
      <button
        ref={closeButtonRef}
        className="inspector-close"
        onClick={close}
        aria-label="Close asset details"
      >
        <X aria-hidden="true" />
      </button>
      {backLabel && (
        <button className="inspector-back" onClick={close} aria-label={backLabel}>
          <ArrowLeft aria-hidden="true" /> <span>{backLabel}</span>
        </button>
      )}
      <div className="inspector-hero">
        {asset.artwork_url ? (
          <img src={asset.artwork_url} alt="" />
        ) : (
          <div className="placeholder">
            <span>◇</span>
            <small>NO ARTWORK</small>
          </div>
        )}
        <div>
          <h2 id="asset-detail-title">
            {asset.jav_code ?? "Media Asset"}
          </h2>
          <span>
            {detail?.title ?? asset.title ?? asset.path.split("/").pop()}
          </span>
        </div>
      </div>
      <BeUITabs defaultValue="overview" value={tab} onValueChange={(value) => onTabChange(value as AssetTab)} variant="underline" className="detail-tabs">
        <BeUITabsList label="Asset details">
          <BeUITab value="overview">Overview</BeUITab>
          <BeUITab value="nfo">NFO</BeUITab>
        </BeUITabsList>
      {loading ? (
        <p className="detail-loading" role="status">
          Loading asset details…
        </p>
      ) : detail && tab === "overview" ? (
        <BeUITabPanel value="overview">
          <DetailSection title="Status">
            <div className="detail-status-list">
              <Status
                name="Local asset"
                label={labels[detail.state]}
                tone={detail.state}
                description={detail.exception ?? (detail.state === "synchronizing"
                  ? "Automatic filesystem reconciliation is in progress."
                  : "The local indexed asset remains authoritative.")}
              />
              <Status
                name="Jellyfin"
                label={detail.jellyfin?.status?.replaceAll("_", " ") ?? "not configured"}
                tone={detail.jellyfin?.status === "offline" || detail.jellyfin?.status === "not_found"
                  ? "exception"
                  : detail.jellyfin?.status === "not_configured"
                    ? "synchronizing"
                    : "normal"}
                description={detail.jellyfin?.reason ?? (detail.jellyfin?.status === "not_configured"
                  ? "Jellyfin is not configured."
                  : detail.jellyfin?.status === "offline"
                    ? "Jellyfin is currently unavailable."
                    : detail.jellyfin?.status === "not_found"
                      ? "No Jellyfin Association was found."
                      : detail.jellyfin?.confidence === "uncertain_metadata"
                        ? "Uncertain metadata association; this never authorizes deletion."
                        : "Read-only playback association; local files remain authoritative.")}
                action={detail.jellyfin?.open_url ? (
                  <a href={detail.jellyfin.open_url} target="_blank" rel="noreferrer">
                    Open in Jellyfin ↗
                  </a>
                ) : undefined}
              />
              {detail.jellyfin && (
                <DataList items={[
                  ["Play count", detail.jellyfin.play_count === undefined
                    ? null : `${detail.jellyfin.play_count} plays`],
                  ["Playback position", detail.jellyfin.playback_position_ticks === undefined
                    ? null : `${detail.jellyfin.playback_position_ticks} ticks`],
                  ["Deletion authority", detail.jellyfin.may_authorize_deletion
                    ? "Certain path association" : "Association cannot authorize deletion"],
                ]} />
              )}
            </div>
          </DetailSection>
          <DetailSection title="Actors">
            {detail.actors.length ? (
              <div className="actor-grid">
                {detail.actors.map((actor) =>
                  actor.actor_folder_url ? (
                    <a className="actor-poster" href={actor.actor_folder_url} key={actor.name}>
                      {actor.poster_url ? (
                        <img src={actor.poster_url} alt={`${actor.name} poster`} />
                      ) : (
                        <span className="actor-silhouette"><UserRound aria-hidden="true" /></span>
                      )}
                      <span><b>{actor.name}</b><small>Open derived Actor Folder →</small></span>
                    </a>
                  ) : (
                    <div className="actor-poster" key={actor.name}>
                      <span className="actor-silhouette"><UserRound aria-hidden="true" /></span>
                      <span><b>{actor.name}</b><small>Actor Folder unavailable</small></span>
                    </div>
                  ),
                )}
              </div>
            ) : (
              <p className="muted">No actors in this NFO.</p>
            )}
          </DetailSection>
          <DetailSection title="Asset details">
            <DataList items={[
              ["Studio", detail.studio],
              ["Release", detail.release_date],
              ["Captured", detail.captured_date],
              ["Source video", detail.path],
            ]} />
          </DetailSection>
        </BeUITabPanel>
      ) : (
        detail && (
          <BeUITabPanel value="nfo">
            <p className="plot">{detail.plot ?? "No plot in this NFO."}</p>
            <DetailSection title="NFO metadata">
              <DataList items={[
                ["Title", detail.title],
                ["Studio", detail.studio],
                ["Release date", detail.release_date],
                ["Runtime", detail.runtime_minutes === null ? null : `${detail.runtime_minutes} minutes`],
                ["Director", detail.director],
                ["Parse status", detail.parse_status],
                ["NFO path", detail.source_path],
              ]} />
            </DetailSection>
            <div className="tags">
              {detail.tags.map((tag) => (
                <span key={tag}>{tag}</span>
              ))}
            </div>
          </BeUITabPanel>
        )
      )}
      </BeUITabs>
    </motion.aside>
  );
}
function DetailSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="detail-section">
      <h2>{title}</h2>
      {children}
    </section>
  );
}
function DataList({ items }: { items: Array<readonly [string, ReactNode | null | undefined]> }) {
  return (
    <dl className="detail-list">
      {items.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd>{value ?? "Not provided"}</dd>
        </div>
      ))}
    </dl>
  );
}
function Status({
  name,
  label,
  description,
  tone,
  action,
}: {
  name: string;
  label: string;
  description: string;
  tone: AssetState;
  action?: ReactNode;
}) {
  return (
    <div className={`detail-status ${tone}`}>
      <div><b>{name}</b><span>{label}</span></div>
      <p>{description}</p>
      {action}
    </div>
  );
}
function Info({ k, v }: { k: string; v: ReactNode | null | undefined }) {
  return <div><dt>{k}</dt><dd>{v ?? "Not provided"}</dd></div>;
}
function ActorFolders({
  actors,
  state,
  busy,
  inspect,
  remove,
  retry,
}: {
  actors: ActorFolder[];
  state: LoadState;
  busy: boolean;
  inspect: (actor: ActorFolder) => void;
  remove: (actor: ActorFolder) => Promise<void>;
  retry: () => void;
}) {
  if (state === "loading")
    return (
      <div className="actor-feedback" role="status" aria-label="Loading Actor Folders">
        <RefreshCw aria-hidden="true" />
        <p>Loading Actor Folders…</p>
      </div>
    );
  if (state === "error")
    return (
      <div className="actor-feedback actor-error" role="alert">
        <AlertTriangle aria-hidden="true" />
        <h2>Actor Folders could not be loaded</h2>
        <p>The derived Actor View is temporarily unavailable.</p>
        <button onClick={retry}>Retry</button>
      </div>
    );
  if (state === "ready" && !actors.length)
    return (
      <div className="empty">
        <span><UserRound aria-hidden="true" /></span>
        <h2>No Actor Folders</h2>
        <p>Generate the derived Actor View from NFO metadata.</p>
      </div>
    );
  return (
    <div className="actor-folder-grid">
      {actors.map((actor) => (
        <article className="actor-folder-card" key={actor.name}>
          <button className="actor-folder-open" aria-label={`Open ${actor.name}`} onClick={() => inspect(actor)}>
            <div className="actor-folder-poster" style={{ aspectRatio: "2 / 3" }}>
              <ActorPortrait actor={actor} loading="lazy" />
              <div><b>{actor.name}</b><p>{actor.movie_count} linked Media Assets</p></div>
            </div>
          </button>
          <button className="actor-remove" disabled={busy} onClick={() => void remove(actor)}>
            <Trash2 aria-hidden="true" /> Remove
          </button>
        </article>
      ))}
    </div>
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
  const closeRef = useRef(close);
  closeRef.current = close;
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
      aria-label={actor ? undefined : "Actor Folder detail"}
      initial={reduce ? false : mobile ? { y: 28 } : { x: 40 }}
      animate={mobile ? { y: 0 } : { x: 0 }}
      exit={reduce ? undefined : mobile ? { y: 28 } : { x: 40 }}
      transition={reduce ? { duration: 0 } : undefined}
    >
      <div className="sheet-handle" aria-hidden="true" />
      <button ref={closeButtonRef} className="inspector-close" onClick={close} aria-label="Close actor details"><X aria-hidden="true" /></button>
      {loading && !actor ? <p role="status">Loading Actor Folder…</p> : actor && (
        <>
          <div className="actor-detail-hero">
            <ActorPortrait actor={actor} />
            <div><p className="eyebrow">ACTOR VIEW</p><h2 id="actor-detail-title">{actor.name}</h2></div>
          </div>
          <dl className="actor-metrics">
            <Info k="Derived paths" v={String(actor.derived_file_count ?? actor.hard_link_count)} />
            <Info k="Unique files" v={String(actor.unique_inode_count ?? actor.movie_count)} />
            <Info k="Logical Size" v={formatBytes(actor.logical_size)} />
            <Info k="Reclaimable Space" v={formatBytes(actor.reclaimable_space)} />
          </dl>
          <span className="sr-only" aria-hidden="true">Referenced logical size</span>
          <span className="sr-only" aria-hidden="true">Reclaimable if removed</span>
          <section className="linked-assets">
            <div className="section-title"><h3>Linked Media Assets</h3><span>{actor.linked_assets?.length ?? 0}</span></div>
            {(actor.linked_assets ?? []).length ? (
              <div className="linked-asset-grid">
                {(actor.linked_assets ?? []).map((asset) => (
                  <button key={asset.id} data-asset-id={asset.id} aria-label={`Open ${asset.jav_code ?? asset.title ?? "Media Asset"}`} onClick={() => openAsset(asset)}>
                    {asset.artwork_url ? <img src={asset.artwork_url} alt="" loading="lazy" /> : <Film aria-hidden="true" />}
                    <span><b>{asset.jav_code ?? "Media Asset"}</b><small>{asset.title ?? asset.path}</small></span>
                  </button>
                ))}
              </div>
            ) : <p className="muted">No linked Media Assets.</p>}
          </section>
          <button className="actor-detail-remove" onClick={() => remove(actor)}><Trash2 aria-hidden="true" /> Remove Actor Folder…</button>
        </>
      )}
      {!loading && error && (
        <div className="actor-feedback actor-detail-error" role="alert">
          <AlertTriangle aria-hidden="true" />
          <h2>{error}</h2>
          <p>The Actor Folder still exists; retry its current filesystem view.</p>
          <button onClick={retry}>Retry Actor Folder</button>
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
      alt={unavailable ? `${actor.name} portrait unavailable` : `${actor.name} portrait`}
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
        <p className="eyebrow">SAFE DERIVED-PATH REMOVAL</p>
        <h2 id="remove-actor-title">Remove {actor.name}?</h2>
        <p>
          Only derived Actor View paths under this Actor Folder will be unlinked. Source Media
          Assets, NFO metadata, and Jellyfin items will not be removed.
        </p>
        <dl>
          <div>
            <dt>Actor Folder</dt>
            <dd>{actor.name}</dd>
          </div>
          <div>
            <dt>Movies</dt>
            <dd>{actor.movie_count}</dd>
          </div>
          <div>
            <dt>Derived paths</dt>
            <dd>{actor.derived_file_count ?? actor.hard_link_count}</dd>
          </div>
          <div>
            <dt>Unique files</dt>
            <dd>{actor.unique_inode_count ?? 0}</dd>
          </div>
          <div>
            <dt>Referenced logical size</dt>
            <dd>{formatBytes(actor.logical_size)}</dd>
          </div>
          <div>
            <dt>Reclaimable if removed</dt>
            <dd>{formatBytes(actor.reclaimable_space)}</dd>
          </div>
        </dl>
        <p className="regenerate-note">
          Regenerate later by running Actor Links from the source NFO metadata.
          Hard links require the Actor View and Media Root to remain on the same
          filesystem.
        </p>
        <div className="dialog-actions">
          <button disabled={busy} onClick={cancel}>
            Cancel
          </button>
          <button className="danger" disabled={busy} onClick={remove}>
            Remove via Management Task
          </button>
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
          <div
            className="storage-progress"
            role="progressbar"
            aria-label={`媒体存储已使用 ${percentage}%`}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={percentage}
          >
            <i style={{ width: `${percentage}%` }} />
          </div>
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
function TaskPanel({
  tasks,
  mediaRoot,
  setMediaRoot,
  selectedOps,
  setSelectedOps,
  createTask,
  confirmPlan,
  refresh,
}: {
  tasks: Task[];
  mediaRoot: string;
  setMediaRoot: (v: string) => void;
  selectedOps: string[];
  setSelectedOps: (v: string[]) => void;
  createTask: (e: FormEvent) => void;
  confirmPlan: (id: string) => Promise<void>;
  refresh: () => Promise<void>;
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
        <h2>New Operation Plan</h2>
        <form className="task-form" onSubmit={createTask}>
          <label htmlFor="media-root">Media Root</label>
          <input
            id="media-root"
            value={mediaRoot}
            onChange={(e) => setMediaRoot(e.target.value)}
            placeholder="/media/library"
            required
          />
          <div className="operation-heading">
            <label>Operations</label>
            <button
              type="button"
              onClick={() => setSelectedOps(operations.map(([key]) => key))}
            >
              Full pipeline
            </button>
          </div>
          <div className="operation-list">
            {operations.map(([key, label]) => (
              <label key={key}>
                <input
                  type="checkbox"
                  checked={selectedOps.includes(key)}
                  onChange={() => toggle(key)}
                />
                <span>{label}</span>
              </label>
            ))}
          </div>
          <button type="submit" disabled={!selectedOps.length}>
            Preview 15-minute plan
          </button>
        </form>
      </section>
      <section className="task-history">
        <div className="task-title">
          <div>
            <h2>Lifecycle</h2>
            <p>Durable history, live progress, reports and verification</p>
          </div>
          <button className="refresh" onClick={() => void refresh()}>
            Refresh
          </button>
        </div>
        {tasks.length === 0 ? (
          <p className="task-empty">No Management Tasks yet.</p>
        ) : (
          <ol className="tasks">
            {tasks.map((task) => (
              <li key={task.id}>
                <div className="task-summary">
                  <span className={`status status-${task.status}`}>
                    {task.status}
                  </span>
                  <strong>{task.kind}</strong>
                  <span className="task-root">{task.media_root}</span>
                </div>
                <small>
                  {task.items.length} item outcome
                  {task.items.length === 1 ? "" : "s"} · {task.id}
                </small>
                {task.error && <p className="task-error">{task.error}</p>}
                {task.operation_plan && (
                  <div className="plan">
                    <b>Review final paths</b>
                    <small>
                      Expires{" "}
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
                            {action.destructive ? "DESTRUCTIVE" : action.kind}
                          </span>
                          <code>{action.path ?? "—"}</code>
                        </li>
                      ))}
                      {task.operation_plan.actions.length > 50 && (
                        <li className="task-truncated">
                          {task.operation_plan.actions.length - 50} more planned actions in the final report
                        </li>
                      )}
                    </ul>
                    {task.status === "completed" &&
                      Date.now() / 1000 <= task.plan_expires_at! && (
                        <button onClick={() => void confirmPlan(task.id)}>
                          Confirm and execute
                        </button>
                      )}
                  </div>
                )}
                {task.items.length > 0 && (
                  <ul className="task-items">
                    {task.items.slice(0, 50).map((item) => (
                      <li key={item.id}>
                        <span>{item.status}</span>
                        <b>{item.kind}</b>
                        <code>{item.path ?? "—"}</code>
                        {item.message && <small>{item.message}</small>}
                      </li>
                    ))}
                    {task.items.length > 50 && (
                      <li className="task-truncated">
                        {task.items.length - 50} more item outcomes in the final report
                      </li>
                    )}
                  </ul>
                )}
                {task.report && (
                  <details>
                    <summary>Final report and migration verification</summary>
                    <pre>{JSON.stringify(task.report, null, 2)}</pre>
                  </details>
                )}
              </li>
            ))}
          </ol>
        )}
      </section>
    </div>
  );
}
function formatDate(v: string) {
  return new Intl.DateTimeFormat(undefined, {
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
