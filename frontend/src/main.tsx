import React, { FormEvent, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import "./style.css";

type View = "loading" | "initialize" | "login" | "ready";
type Validation = { valid: true; empty: boolean; yaml: string } | null;

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

  useEffect(() => {
    if (token) return;
    fetch("/api/v1/status").then(async (response) => {
      if (response.ok) {
        const status = (await response.json()) as { version: string };
        setVersion(status.version);
        setView("ready");
        const rules = await fetch("/api/v1/rules/active");
        if (rules.ok) setYaml(((await rules.json()) as { yaml: string }).yaml);
      } else {
        setView("login");
        if (response.status === 503) setMessage("Run rust-jav administrator init locally first.");
      }
    });
  }, [token]);

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
