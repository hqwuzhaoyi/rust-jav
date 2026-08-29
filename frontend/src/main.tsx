import React, { FormEvent, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import "./style.css";

type View = "loading" | "initialize" | "login" | "ready";
type Validation = { valid: true; empty: boolean; yaml: string } | null;
type Task = {
  id: string;
  task_type: string;
  media_root: string;
  kind: "preview" | "mutation";
  status: "queued" | "running" | "completed" | "failed" | "interrupted";
  created_at: number;
  items: Array<{ id: number; kind: string; path: string | null; status: string }>;
};

export function App() {
  const token = new URLSearchParams(location.search).get("token");
  const [view, setView] = useState<View>(token ? "initialize" : "loading");
  const [password, setPassword] = useState("");
  const [message, setMessage] = useState("");
  const [version, setVersion] = useState("");
  const [yaml, setYaml] = useState("");
  const [sourceUrl, setSourceUrl] = useState("");
  const [editing, setEditing] = useState(false);
  const [validation, setValidation] = useState<Validation>(null);
  const [rulesMessage, setRulesMessage] = useState("");
  const [tasks, setTasks] = useState<Task[]>([]);
  const [mediaRoot, setMediaRoot] = useState("");
  const [mode, setMode] = useState<"preview" | "apply">("preview");
  const [operation, setOperation] = useState("delete_ad_files");

  useEffect(() => {
    if (token) return;
    fetch("/api/v1/status").then(async (response) => {
      if (response.ok) {
        const status = (await response.json()) as { version: string };
        setVersion(status.version);
        setView("ready");
        const rules = await fetch("/api/v1/rules/active");
        if (rules.ok) setYaml(((await rules.json()) as { yaml: string }).yaml);
        void loadTasks();
      } else {
        setView("login");
        if (response.status === 503) setMessage("Run rust-jav administrator init locally first.");
      }
    });
  }, [token]);

  async function loadTasks() {
    const response = await fetch("/api/v1/tasks");
    if (response.ok) {
      const body = await response.json().catch(() => null) as Task[] | null;
      if (body) setTasks(body);
    }
  }

  function watchTask(id: string) {
    const events = new EventSource(`/api/v1/tasks/${id}/events`);
    events.addEventListener("task", (event) => {
      const task = JSON.parse((event as MessageEvent).data) as Task;
      setTasks((current) => [task, ...current.filter((item) => item.id !== task.id)]);
      if (["completed", "failed", "interrupted"].includes(task.status)) events.close();
    });
  }

  async function createTask(event: FormEvent) {
    event.preventDefault();
    setMessage("");
    const response = await fetch("/api/v1/tasks", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ task_type: "operations", media_root: mediaRoot, mode, operations: [operation] }),
    });
    if (!response.ok) {
      setMessage(await response.text() || "Task request rejected.");
      return;
    }
    const task = (await response.json()) as Task;
    setTasks((current) => [task, ...current]);
    watchTask(task.id);
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    setMessage("");
    const initialize = view === "initialize";
    const response = await fetch(`/api/v1/auth/${initialize ? "initialize" : "login"}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(initialize ? { token, password } : { password }),
    });
    setPassword("");
    if (!response.ok) {
      setMessage(response.status === 401 ? "Incorrect password." : "Request rejected.");
      return;
    }
    if (initialize) {
      history.replaceState({}, "", "/");
      setView("login");
      setMessage("Administrator initialized. Sign in to continue.");
    } else {
      location.assign("/");
    }
  }

  async function logout() {
    await fetch("/api/v1/auth/logout", { method: "POST" });
    setView("login");
  }

  function updateYaml(value: string) {
    setYaml(value);
    setValidation(null);
    setRulesMessage("");
  }

  async function downloadProposal() {
    setRulesMessage("Downloading proposal…");
    const response = await fetch("/api/v1/rules/download", {
      method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ url: sourceUrl }),
    });
    const body = await response.json() as { yaml?: string; error?: string };
    if (!response.ok || !body.yaml) { setRulesMessage(body.error ?? "Download failed."); return; }
    updateYaml(body.yaml);
    setEditing(true);
    setRulesMessage("Proposal downloaded. Validate it before saving.");
  }

  async function validateRules() {
    const candidate = yaml;
    setRulesMessage("Validating…");
    const response = await fetch("/api/v1/rules/validate", {
      method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ yaml: candidate }),
    });
    const body = await response.json() as { valid?: boolean; empty?: boolean; error?: string };
    if (!response.ok || !body.valid) { setValidation(null); setRulesMessage(body.error ?? "Validation failed."); return; }
    setValidation({ valid: true, empty: Boolean(body.empty), yaml: candidate });
    setRulesMessage(body.empty ? "Valid, but empty. A separate confirmation is required." : "Valid proposal. Ready to save.");
  }

  async function saveRules(confirmEmpty = false) {
    const response = await fetch("/api/v1/rules/active", {
      method: "PUT", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ yaml, confirm_empty: confirmEmpty }),
    });
    if (!response.ok) {
      const body = await response.json() as { error?: string };
      setRulesMessage(body.error ?? "Save failed; the previous Active Rule Set remains active.");
      return;
    }
    setEditing(false); setValidation(null); setRulesMessage("Active Rule Set saved atomically.");
  }

  return (
    <main>
      <p className="eyebrow">RUST-JAV</p>
      <h1>Management Interface</h1>
      {view === "loading" && <p>Checking session…</p>}
      {(view === "initialize" || view === "login") && (
        <form onSubmit={submit}>
          <h2>{view === "initialize" ? "Initialize Administrator" : "Administrator login"}</h2>
          <label htmlFor="password">Password</label>
          <input id="password" type="password" minLength={12} autoComplete="current-password" value={password} onChange={(event) => setPassword(event.target.value)} required autoFocus />
          <button type="submit">{view === "initialize" ? "Initialize" : "Sign in"}</button>
        </form>
      )}
      {view === "ready" && <>
        <section className="service"><p>Authenticated against API v1 · rust-jav {version}</p><button className="secondary" onClick={logout}>Sign out</button></section>
        <div className="dashboard">
          <section>
            <h2>Management Tasks</h2>
            <form className="task-form" onSubmit={createTask}>
              <label htmlFor="media-root">Media Root</label>
              <input id="media-root" value={mediaRoot} onChange={(event) => setMediaRoot(event.target.value)} placeholder="/media/library" required />
              <label htmlFor="operation">Operation</label>
              <select id="operation" value={operation} onChange={(event) => setOperation(event.target.value)}>
                <option value="delete_ad_files">Delete ad files</option>
                <option value="standardize_names">Standardize names</option>
                <option value="clean_empty_dirs">Clean empty directories</option>
                <option value="remove_duplicates">Remove duplicates</option>
              </select>
              <label htmlFor="mode">Mode</label>
              <select id="mode" value={mode} onChange={(event) => setMode(event.target.value as "preview" | "apply")}>
                <option value="preview">Preview</option>
                <option value="apply">Apply changes</option>
              </select>
              <button type="submit">Start task</button>
            </form>
          </section>
          <section>
            <h2>Lifecycle</h2>
            {tasks.length === 0 ? <p>No Management Tasks yet.</p> : <ol className="tasks">{tasks.map((task) =>
              <li key={task.id}>
                <span className={`status status-${task.status}`}>{task.status}</span>
                <strong>{task.kind}</strong> · {task.media_root}
                <small>{task.items.length} item outcome{task.items.length === 1 ? "" : "s"} · {task.id}</small>
              </li>
            )}</ol>}
            <button className="secondary" onClick={() => void loadTasks()}>Refresh</button>
          </section>
        </div>
        <section>
          <p className="eyebrow">SETTINGS</p>
          <h2>Active Rule Set</h2>
          <p>Remote YAML is only a proposal. The server validates and atomically activates it; rules cannot select roots or authorize deletion.</p>
          <label htmlFor="rule-source">Rule Source URL</label>
          <div className="action-row"><input id="rule-source" type="url" placeholder="https://raw.githubusercontent.com/…" value={sourceUrl} onChange={(event) => setSourceUrl(event.target.value)} /><button type="button" disabled={!sourceUrl} onClick={downloadProposal}>Download proposal</button></div>
          <label htmlFor="rules-yaml">Active Rule Set YAML</label>
          <textarea id="rules-yaml" rows={18} readOnly={!editing} value={yaml} onChange={(event) => updateYaml(event.target.value)} />
          <div className="action-row">
            {!editing && <button type="button" onClick={() => { setEditing(true); setValidation(null); }}>Edit</button>}
            {editing && <button type="button" onClick={validateRules}>Validate</button>}
            {editing && !validation?.empty && <button type="button" disabled={!validation || validation.yaml !== yaml} onClick={() => saveRules(false)}>Save Active Rule Set</button>}
            {editing && validation?.empty && <button type="button" className="danger" disabled={validation.yaml !== yaml} onClick={() => saveRules(true)}>Confirm empty and save</button>}
          </div>
          {rulesMessage && <p role="status" className="message">{rulesMessage}</p>}
        </section>
      </>}
      {message && <p role="status" className="message">{message}</p>}
    </main>
  );
}

const root = document.getElementById("root");
if (root) createRoot(root).render(<React.StrictMode><App /></React.StrictMode>);
