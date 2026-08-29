import React, { FormEvent, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import "./style.css";

type View = "loading" | "initialize" | "login" | "ready";
type Task = {
  id: string;
  task_type: string;
  media_root: string;
  kind: "preview" | "mutation";
  status: "queued" | "running" | "completed" | "failed" | "interrupted";
  created_at: number;
  items: Array<{ id: number; kind: string; path: string | null; status: string }>;
};

function App() {
  const token = new URLSearchParams(location.search).get("token");
  const [view, setView] = useState<View>(token ? "initialize" : "loading");
  const [password, setPassword] = useState("");
  const [message, setMessage] = useState("");
  const [version, setVersion] = useState("");
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
        void loadTasks();
      } else {
        setView("login");
        if (response.status === 503) setMessage("Run rust-jav administrator init locally first.");
      }
    });
  }, [token]);

  async function loadTasks() {
    const response = await fetch("/api/v1/tasks");
    if (response.ok) setTasks((await response.json()) as Task[]);
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
      {view === "ready" && <div className="dashboard">
        <section>
          <h2>Management Tasks</h2>
          <p>Authenticated against API v1 · rust-jav {version}</p>
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
          <button className="secondary" onClick={logout}>Sign out</button>
        </section>
      </div>}
      {message && <p role="status" className="message">{message}</p>}
    </main>
  );
}

createRoot(document.getElementById("root")!).render(<React.StrictMode><App /></React.StrictMode>);
