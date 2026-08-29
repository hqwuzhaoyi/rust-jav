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
type Page = {
  items: Asset[];
  groups: Array<{ date: string; count: number }>;
  page: number;
  total: number;
  total_pages: number;
};
type Health = { state: string; mode: string | null };
type Task = {
  id: string;
  task_type: string;
  media_root: string;
  kind: "preview" | "mutation";
  status: "queued" | "running" | "completed" | "failed" | "interrupted";
  created_at: number;
  error: string | null;
  items: Array<{
    id: number;
    kind: string;
    path: string | null;
    status: string;
    message: string | null;
  }>;
};
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
    [nav, setNav] = useState<"assets" | "tasks" | "settings">("assets");
  const [tasks, setTasks] = useState<Task[]>([]),
    [mediaRoot, setMediaRoot] = useState(""),
    [mode, setMode] = useState<"preview" | "apply">("preview"),
    [operation, setOperation] = useState("delete_ad_files");
  const [yaml, setYaml] = useState("");
  const [sourceUrl, setSourceUrl] = useState("");
  const [editing, setEditing] = useState(false);
  const [validation, setValidation] = useState<Validation>(null);
  const [rulesMessage, setRulesMessage] = useState("");
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
  }, [nav]);
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
        mode,
        operations: [operation],
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
    <div className="shell">
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
                : nav === "tasks"
                  ? "Management Tasks"
                  : "Settings"}
            </h1>
            <small>
              {nav === "tasks"
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
                        <article className="asset-card" key={a.id}>
                          <div className="poster">
                            {a.artwork_url ? (
                              <img loading="lazy" src={a.artwork_url} alt="" />
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
                          </div>
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
            mode={mode}
            setMode={setMode}
            operation={operation}
            setOperation={setOperation}
            createTask={createTask}
            refresh={loadTasks}
            message={message}
          />
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
      <nav className="bottom-nav">
        <button
          className={nav === "assets" ? "active" : ""}
          onClick={() => setNav("assets")}
        >
          <span>▦</span>Library
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
  mode,
  setMode,
  operation,
  setOperation,
  createTask,
  refresh,
  message,
}: {
  tasks: Task[];
  mediaRoot: string;
  setMediaRoot: (v: string) => void;
  mode: "preview" | "apply";
  setMode: (v: "preview" | "apply") => void;
  operation: string;
  setOperation: (v: string) => void;
  createTask: (e: FormEvent) => void;
  refresh: () => Promise<void>;
  message: string;
}) {
  return (
    <div className="task-dashboard">
      <section className="task-create">
        <h2>New task</h2>
        <form className="task-form" onSubmit={createTask}>
          <label htmlFor="media-root">Media Root</label>
          <input
            id="media-root"
            value={mediaRoot}
            onChange={(e) => setMediaRoot(e.target.value)}
            placeholder="/media/library"
            required
          />
          <label htmlFor="operation">Operation</label>
          <select
            id="operation"
            value={operation}
            onChange={(e) => setOperation(e.target.value)}
          >
            <option value="delete_ad_files">Delete ad files</option>
            <option value="standardize_names">Standardize names</option>
            <option value="clean_empty_dirs">Clean empty directories</option>
            <option value="remove_duplicates">Remove duplicates</option>
          </select>
          <label htmlFor="mode">Mode</label>
          <select
            id="mode"
            value={mode}
            onChange={(e) => setMode(e.target.value as "preview" | "apply")}
          >
            <option value="preview">Preview</option>
            <option value="apply">Apply changes</option>
          </select>
          <button type="submit">Start task</button>
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
            <p>Durable history and per-item outcomes</p>
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
const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
