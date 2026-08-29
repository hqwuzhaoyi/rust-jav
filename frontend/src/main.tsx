import React, { FormEvent, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import "./style.css";

type View = "loading" | "initialize" | "login" | "ready";

function App() {
  const token = new URLSearchParams(location.search).get("token");
  const [view, setView] = useState<View>(token ? "initialize" : "loading");
  const [password, setPassword] = useState("");
  const [message, setMessage] = useState("");
  const [version, setVersion] = useState("");

  useEffect(() => {
    if (token) return;
    fetch("/api/v1/status").then(async (response) => {
      if (response.ok) {
        const status = (await response.json()) as { version: string };
        setVersion(status.version);
        setView("ready");
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
      {view === "ready" && <section><h2>Service ready</h2><p>Authenticated against API v1 · rust-jav {version}</p><button onClick={logout}>Sign out</button></section>}
      {message && <p role="status" className="message">{message}</p>}
    </main>
  );
}

createRoot(document.getElementById("root")!).render(<React.StrictMode><App /></React.StrictMode>);
