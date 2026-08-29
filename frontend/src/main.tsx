import React, { FormEvent, useEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import "./style.css";
type View = "loading" | "initialize" | "login" | "ready";
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
type AssetDetail = {
  id: string;
  path: string;
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
};
type Page = {
  items: Asset[];
  groups: Array<{ date: string; count: number }>;
  page: number;
  total: number;
  total_pages: number;
};
type Health = { state: string; mode: string | null };
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
  normal: "Normal",
  synchronizing: "Synchronizing",
  exception: "Exception",
};
export function App() {
  const token = new URLSearchParams(location.search).get("token"),
    [view, setView] = useState<View>(token ? "initialize" : "loading"),
    [password, setPassword] = useState(""),
    [message, setMessage] = useState("");
  const [assets, setAssets] = useState<Page>({
      items: [],
      groups: [],
      page: 1,
      total: 0,
      total_pages: 0,
    }),
    [query, setQuery] = useState(""),
    [filter, setFilter] = useState<AssetState | "">(""),
    [health, setHealth] = useState<Health | null>(null),
    [page, setPage] = useState(1),
    [nav, setNav] = useState<"assets" | "deletion" | "tasks" | "settings">(
      "assets",
    );
  const [tasks, setTasks] = useState<Task[]>([]),
    [mediaRoot, setMediaRoot] = useState(""),
    [selectedOps, setSelectedOps] = useState<string[]>(
      operations.map(([key]) => key),
    );
  const [inspectedAsset, setInspectedAsset] = useState<Asset | null>(null);
  const [assetDetail, setAssetDetail] = useState<AssetDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [yaml, setYaml] = useState("");
  const [sourceUrl, setSourceUrl] = useState("");
  const [editing, setEditing] = useState(false);
  const [validation, setValidation] = useState<Validation>(null);
  const [rulesMessage, setRulesMessage] = useState("");
  const [candidates, setCandidates] = useState<Candidate[]>([]),
    [selected, setSelected] = useState<string[]>([]),
    [plan, setPlan] = useState<DeletionPlan | null>(null),
    [confirmText, setConfirmText] = useState("");
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
    const timer = setTimeout(() => void loadAssets(), 180);
    return () => clearTimeout(timer);
  }, [view, query, filter, page]);
  useEffect(() => {
    if (nav === "tasks") void loadTasks();
    if (nav === "settings") void loadRules();
    if (nav === "deletion") void loadCandidates();
  }, [nav]);
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
    if (response.ok)
      setYaml(((await response.json()) as { yaml: string }).yaml);
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
    const p = new URLSearchParams({ page: String(page), per_page: "48" });
    if (query) p.set("q", query);
    if (filter) p.set("state", filter);
    const [a, h] = await Promise.all([
      fetch(`/api/v1/assets?${p}`),
      fetch("/api/v1/assets/health"),
    ]);
    if (a.ok) {
      const body = (await a.json().catch(() => null)) as Page | null;
      if (body) setAssets(body);
    }
    if (h.ok) {
      const body = (await h.json().catch(() => null)) as Health | null;
      if (body) setHealth(body);
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
  async function inspect(asset: Asset) {
    setInspectedAsset(asset);
    setAssetDetail(null);
    setDetailLoading(true);
    const response = await fetch(`/api/v1/assets/${asset.id}`);
    if (response.ok) setAssetDetail((await response.json()) as AssetDetail);
    else setMessage("Asset details could not be loaded.");
    setDetailLoading(false);
  }
  function closeInspector() {
    setInspectedAsset(null);
    setAssetDetail(null);
  }
  async function submit(e: FormEvent) {
    e.preventDefault();
    const init = view === "initialize",
      r = await fetch(`/api/v1/auth/${init ? "initialize" : "login"}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(init ? { token, password } : { password }),
      });
    setPassword("");
    if (!r.ok) {
      setMessage(
        r.status === 401 ? "Incorrect password." : "Request rejected.",
      );
      return;
    }
    if (init) {
      history.replaceState({}, "", "/");
      setView("login");
      setMessage("Administrator initialized. Sign in to continue.");
    } else location.assign("/");
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
      if (["completed", "failed", "interrupted"].includes(task.status))
        source.close();
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
  if (view === "loading") return <div className="auth">Checking session…</div>;
  if (view === "initialize" || view === "login")
    return (
      <main className="auth">
        <div className="brand-mark">◆</div>
        <p className="eyebrow">RUST—JAV</p>
        <h1>
          {view === "initialize" ? "Initialize Administrator" : "Welcome back"}
        </h1>
        <form onSubmit={submit}>
          <label htmlFor="password">Password</label>
          <input
            id="password"
            type="password"
            minLength={12}
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            autoFocus
          />
          <button type="submit">
            {view === "initialize" ? "Initialize" : "Sign in"}
          </button>
        </form>
        {message && <p role="status">{message}</p>}
      </main>
    );
  return (
    <div className={`shell ${inspectedAsset ? "inspecting" : ""}`}>
      <aside className="sidebar">
        <div className="logo">
          <span>◆</span>
          <div>
            <b>rust-jav</b>
            <small>Media library</small>
          </div>
        </div>
        <nav>
          <p>LIBRARY</p>
          <button
            className={nav === "assets" ? "active" : ""}
            onClick={() => setNav("assets")}
          >
            <span>▦</span> All Assets <em>{assets.total}</em>
          </button>
          <button>
            <span>◷</span> Recently Added
          </button>
          <button>
            <span>♙</span> Actors
          </button>
          <button
            className={nav === "deletion" ? "active" : ""}
            onClick={() => setNav("deletion")}
          >
            <span>⌫</span> Deletion Candidates
          </button>
          <p>MANAGE</p>
          <button
            className={nav === "tasks" ? "active" : ""}
            onClick={() => setNav("tasks")}
          >
            <span>☷</span> Management Tasks
          </button>
          <button>
            <span>⚠</span> Exceptions
          </button>
          <button
            className={nav === "settings" ? "active" : ""}
            onClick={() => setNav("settings")}
          >
            <span>⚙</span> Settings
          </button>
        </nav>
        <div className="root-card">
          <small>ASSET INDEX</small>
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
        <button className="signout" onClick={logout}>
          Sign out
        </button>
      </aside>
      <main className="content">
        <header>
          <div>
            <p className="eyebrow">MEDIA LIBRARY</p>
            <h1>
              {nav === "assets"
                ? "All Assets"
                : nav === "deletion"
                  ? "Deletion Candidates"
                  : nav === "tasks"
                    ? "Management Tasks"
                    : "Settings"}
            </h1>
            <small>
              {nav === "deletion"
                ? `${candidates.length} paths matched by the Active Rule Set`
                : nav === "tasks"
                  ? `${tasks.length} durable tasks · live lifecycle recovery`
                  : `${assets.total} indexed Media Assets · filesystem authoritative`}
            </small>
          </div>
          {nav === "assets" && (
            <button className="scan" onClick={scan}>
              ↻ <span>Reconcile</span>
            </button>
          )}
        </header>
        {nav === "assets" && (
          <>
            <div className="toolbar">
              <label className="search">
                ⌕
                <input
                  aria-label="Search assets"
                  placeholder="Search code, title, or path"
                  value={query}
                  onChange={(e) => {
                    setQuery(e.target.value);
                    setPage(1);
                  }}
                />
              </label>
              <div className="filters">
                <button
                  className={!filter ? "selected" : ""}
                  onClick={() => setFilter("")}
                >
                  All
                </button>
                {(["normal", "synchronizing", "exception"] as AssetState[]).map(
                  (s) => (
                    <button
                      key={s}
                      className={filter === s ? "selected" : ""}
                      onClick={() => {
                        setFilter(s);
                        setPage(1);
                      }}
                    >
                      {labels[s]}
                    </button>
                  ),
                )}
              </div>
            </div>
            {message && (
              <p className="notice" role="status">
                {message}
              </p>
            )}
            <div className="library">
              {grouped.length === 0 ? (
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
                        <article
                          className={`asset-card ${inspectedAsset?.id === a.id ? "selected" : ""}`}
                          key={a.id}
                        >
                          <button
                            className="asset-select"
                            onClick={() => void inspect(a)}
                            aria-label={`Inspect ${a.jav_code ?? a.title ?? "asset"}`}
                          >
                            <div className="poster">
                              {a.artwork_url ? (
                                <img
                                  loading="lazy"
                                  src={a.artwork_url}
                                  alt=""
                                />
                              ) : (
                                <div className="placeholder">
                                  <span>◇</span>
                                  <small>NO ARTWORK</small>
                                </div>
                              )}
                              <span
                                className={`state ${a.state}`}
                                title={a.exception ?? labels[a.state]}
                              />
                            </div>
                            <div className="meta">
                              <b>{a.jav_code ?? a.title ?? "Unidentified"}</b>
                              <span>{a.title ?? a.path.split("/").pop()}</span>
                              <small className={`state-label ${a.state}`}>
                                {labels[a.state]}
                              </small>
                            </div>
                          </button>
                        </article>
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
                  onClick={() => setPage((p) => p - 1)}
                >
                  Previous
                </button>
                <span>
                  {page} / {assets.total_pages}
                </span>
                <button
                  disabled={page === assets.total_pages}
                  onClick={() => setPage((p) => p + 1)}
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
            message={message}
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
            {message && (
              <p className="notice" role="status">
                {message}
              </p>
            )}
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
        )}
      </main>
      {inspectedAsset && (
        <AssetInspector
          asset={inspectedAsset}
          detail={assetDetail}
          loading={detailLoading}
          close={closeInspector}
        />
      )}
      <nav className="bottom-nav">
        <button
          className={nav === "assets" ? "active" : ""}
          onClick={() => setNav("assets")}
        >
          <span>▦</span>Library
        </button>
        <button
          className={nav === "deletion" ? "active" : ""}
          onClick={() => setNav("deletion")}
        >
          <span>⌫</span>Delete
        </button>
        <button
          className={nav === "tasks" ? "active" : ""}
          onClick={() => setNav("tasks")}
        >
          <span>☷</span>Tasks
        </button>
        <button
          className={nav === "settings" ? "active" : ""}
          onClick={() => setNav("settings")}
        >
          <span>⚙</span>Settings
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
    </div>
  );
}
function AssetInspector({
  asset,
  detail,
  loading,
  close,
}: {
  asset: Asset;
  detail: AssetDetail | null;
  loading: boolean;
  close: () => void;
}) {
  const [tab, setTab] = useState<"overview" | "nfo">("overview");
  useEffect(() => setTab("overview"), [asset.id]);
  useEffect(() => {
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    addEventListener("keydown", escape);
    return () => removeEventListener("keydown", escape);
  }, [close]);
  return (
    <aside
      className="asset-inspector"
      role="dialog"
      aria-modal="false"
      aria-labelledby="asset-detail-title"
    >
      <div className="sheet-handle" />
      <button
        autoFocus
        className="inspector-close"
        onClick={close}
        aria-label="Close asset details"
      >
        ×
      </button>
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
          <strong id="asset-detail-title">
            {asset.jav_code ?? "Media Asset"}
          </strong>
          <span>
            {detail?.title ?? asset.title ?? asset.path.split("/").pop()}
          </span>
        </div>
      </div>
      <div className="detail-tabs" role="tablist" aria-label="Asset details">
        <button
          role="tab"
          aria-selected={tab === "overview"}
          onClick={() => setTab("overview")}
        >
          Overview
        </button>
        <button
          role="tab"
          aria-selected={tab === "nfo"}
          onClick={() => setTab("nfo")}
        >
          NFO
        </button>
      </div>
      {loading ? (
        <p className="detail-loading" role="status">
          Loading asset details…
        </p>
      ) : detail && tab === "overview" ? (
        <div role="tabpanel" className="detail-panel">
          <StateBanner detail={detail} />
          <h2>Actors</h2>
          {detail.actors.length ? (
            <div className="actor-grid">
              {detail.actors.map((actor) =>
                actor.actor_folder_url ? (
                  <a
                    className="actor-poster"
                    href={actor.actor_folder_url}
                    key={actor.name}
                  >
                    {actor.poster_url ? (
                      <img
                        src={actor.poster_url}
                        alt={`${actor.name} poster`}
                      />
                    ) : (
                      <span className="actor-silhouette">♙</span>
                    )}
                    <span>
                      <b>{actor.name}</b>
                      <small>Actor Folder →</small>
                    </span>
                  </a>
                ) : (
                  <div className="actor-poster" key={actor.name}>
                    <span className="actor-silhouette">♙</span>
                    <span>
                      <b>{actor.name}</b>
                      <small>Actor Folder unavailable</small>
                    </span>
                  </div>
                ),
              )}
            </div>
          ) : (
            <p className="muted">No actors in this NFO.</p>
          )}
          <dl className="detail-list">
            <Info k="Studio" v={detail.studio} />
            <Info k="Release" v={detail.release_date} />
            <Info k="Source video" v={detail.path} />
          </dl>
        </div>
      ) : (
        detail && (
          <div role="tabpanel" className="detail-panel">
            <p className="plot">{detail.plot ?? "No plot in this NFO."}</p>
            <dl className="detail-list">
              <Info k="Title" v={detail.title} />
              <Info k="Studio" v={detail.studio} />
              <Info k="Release date" v={detail.release_date} />
              <Info
                k="Runtime"
                v={
                  detail.runtime_minutes
                    ? `${detail.runtime_minutes} minutes`
                    : null
                }
              />
              <Info k="Director" v={detail.director} />
              <Info k="Parse status" v={detail.parse_status} />
              <Info k="NFO path" v={detail.source_path} />
            </dl>
            <div className="tags">
              {detail.tags.map((tag) => (
                <span key={tag}>{tag}</span>
              ))}
            </div>
          </div>
        )
      )}
    </aside>
  );
}
function StateBanner({ detail }: { detail: AssetDetail }) {
  return (
    <div className={`state-banner ${detail.state}`}>
      <b>{labels[detail.state]} Asset</b>
      <span>
        {detail.exception ??
          (detail.state === "synchronizing"
            ? "Automatic reconciliation is in progress."
            : "Local metadata is valid.")}
      </span>
    </div>
  );
}
function Info({ k, v }: { k: string; v: string | null }) {
  return (
    <div>
      <dt>{k}</dt>
      <dd>{v ?? "Not provided"}</dd>
    </div>
  );
}
function Empty() {
  return (
    <div className="empty">
      <span>◇</span>
      <h2>No Media Assets</h2>
      <p>Configure a Media Root, then reconcile the filesystem.</p>
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
  message,
}: {
  tasks: Task[];
  mediaRoot: string;
  setMediaRoot: (v: string) => void;
  selectedOps: string[];
  setSelectedOps: (v: string[]) => void;
  createTask: (e: FormEvent) => void;
  confirmPlan: (id: string) => Promise<void>;
  refresh: () => Promise<void>;
  message: string;
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
        {message && (
          <p className="notice" role="status">
            {message}
          </p>
        )}
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
                      {task.operation_plan.actions.map((action, index) => (
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
                    {task.items.map((item) => (
                      <li key={item.id}>
                        <span>{item.status}</span>
                        <b>{item.kind}</b>
                        <code>{item.path ?? "—"}</code>
                        {item.message && <small>{item.message}</small>}
                      </li>
                    ))}
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
  const units = ["KB", "MB", "GB", "TB"];
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
