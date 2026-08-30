import React from "react";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./main";

function baseResponse(url: string, method: string) {
  if (url === "/api/v1/status") return Response.json({ state: "healthy" });
  if (url.startsWith("/api/v1/assets?"))
    return Response.json({ items: [], groups: [], page: 1, total: 0, total_pages: 1 });
  if (url === "/api/v1/assets/health")
    return Response.json({ state: "healthy", mode: "manual" });
  if (url === "/api/v1/media-roots/storage") return new Response(null, { status: 204 });
  if (url === "/api/v1/rules/active" && method === "GET")
    return Response.json({ yaml: "version: 1\nrules: []\n" });
  if (url === "/api/v1/jellyfin/config" && method === "GET")
    return Response.json({
      url: "http://jellyfin:8096",
      library_ids: ["jav"],
      api_key_configured: true,
    });
  return new Response(null, { status: 204 });
}

async function enterSettings() {
  render(<App />);
  const buttons = await screen.findAllByRole("button", { name: "Settings" });
  await userEvent.click(buttons[0]);
  await screen.findByRole("heading", { name: "Jellyfin" });
}

function jellyfinSection() {
  const section = screen.getByRole("heading", { name: "Jellyfin" }).closest("section");
  if (!section) throw new Error("Jellyfin section is missing");
  return section;
}

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  history.replaceState({}, "", "/");
});

describe("Issue #42 P1 Settings review", () => {
  it("keeps an edit made during initial Jellyfin loading dirty without letting the late baseline overwrite it", async () => {
    let resolveConfig!: (response: Response) => void;
    const pendingConfig = new Promise<Response>((resolve) => {
      resolveConfig = resolve;
    });
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = String(input);
        const method = init?.method ?? "GET";
        if (url === "/api/v1/jellyfin/config" && method === "GET") return pendingConfig;
        return baseResponse(url, method);
      }),
    );
    await enterSettings();

    const section = jellyfinSection();
    await userEvent.type(within(section).getByLabelText("Server URL"), "http://draft:8096");
    await userEvent.type(within(section).getByLabelText("Library IDs"), "draft-library");
    await userEvent.type(within(section).getByLabelText("Server API key"), "draft-key");
    expect(within(section).getByText("Unsaved changes")).toBeVisible();
    expect(within(section).getByRole("button", { name: "Save Jellyfin" })).toBeEnabled();

    await act(async () => {
      resolveConfig(
        Response.json({
          url: "http://persisted:8096",
          library_ids: ["persisted-library"],
          api_key_configured: true,
        }),
      );
    });

    await waitFor(() =>
      expect(within(section).getByLabelText("Server URL")).toHaveValue("http://draft:8096"),
    );
    expect(within(section).getByLabelText("Library IDs")).toHaveValue("draft-library");
    expect(within(section).getByLabelText("Server API key")).toHaveValue("draft-key");
    expect(within(section).getByRole("button", { name: "Save Jellyfin" })).toBeEnabled();
  });

  it("allows edits after a failed Jellyfin load and retries without replacing the dirty draft", async () => {
    let configAttempts = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = String(input);
        const method = init?.method ?? "GET";
        if (url === "/api/v1/jellyfin/config" && method === "GET") {
          configAttempts += 1;
          if (configAttempts === 1)
            return new Response("Jellyfin settings are unavailable.", { status: 503 });
          return Response.json({
            url: "http://persisted:8096",
            library_ids: ["persisted-library"],
            api_key_configured: true,
          });
        }
        return baseResponse(url, method);
      }),
    );
    await enterSettings();

    const section = jellyfinSection();
    expect(await within(section).findByRole("alert")).toHaveTextContent(
      "Jellyfin settings are unavailable.",
    );
    const url = within(section).getByLabelText("Server URL");
    await userEvent.type(url, "http://draft-after-error:8096");
    await userEvent.type(within(section).getByLabelText("Library IDs"), "draft-library");
    await userEvent.type(within(section).getByLabelText("Server API key"), "draft-key");
    expect(within(section).getByRole("button", { name: "Save Jellyfin" })).toBeEnabled();

    await userEvent.click(
      within(section).getByRole("button", { name: "Retry Jellyfin settings" }),
    );
    await waitFor(() =>
      expect(
        within(section).queryByRole("button", { name: "Retry Jellyfin settings" }),
      ).not.toBeInTheDocument(),
    );
    expect(url).toHaveValue("http://draft-after-error:8096");
    expect(within(section).getByLabelText("Library IDs")).toHaveValue("draft-library");
    expect(within(section).getByLabelText("Server API key")).toHaveValue("draft-key");
    expect(within(section).getByText("Unsaved changes")).toBeVisible();
  });

  it("restores a dirty Settings history pop until discard is confirmed", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) =>
        baseResponse(String(input), init?.method ?? "GET"),
      ),
    );
    history.replaceState({ page: "previous" }, "", "/previous");
    history.pushState({ page: "settings" }, "", "/?page=1");
    const settingsUrl = location.pathname + location.search;
    const previousUrl = "/previous";
    const forward = vi.spyOn(history, "forward").mockImplementation(() => {
      history.replaceState({ page: "settings" }, "", settingsUrl);
      dispatchEvent(new PopStateEvent("popstate", { state: { page: "settings" } }));
    });
    const back = vi.spyOn(history, "back").mockImplementation(() => {
      history.replaceState({ page: "previous" }, "", previousUrl);
      dispatchEvent(new PopStateEvent("popstate", { state: { page: "previous" } }));
    });
    await enterSettings();
    const section = jellyfinSection();
    await waitFor(() =>
      expect(within(section).getByLabelText("Server URL")).toHaveValue("http://jellyfin:8096"),
    );
    await userEvent.clear(within(section).getByLabelText("Server URL"));
    await userEvent.type(within(section).getByLabelText("Server URL"), "http://draft:8096");

    history.replaceState({ page: "previous" }, "", previousUrl);
    dispatchEvent(new PopStateEvent("popstate", { state: { page: "previous" } }));
    const firstDialog = await screen.findByRole("dialog", {
      name: "Discard unsaved changes?",
    });
    expect(forward).toHaveBeenCalledTimes(1);
    await userEvent.click(within(firstDialog).getByRole("button", { name: "Keep editing" }));
    expect(location.pathname + location.search).toBe(settingsUrl);
    expect(within(section).getByLabelText("Server URL")).toHaveValue("http://draft:8096");

    history.replaceState({ page: "previous" }, "", previousUrl);
    dispatchEvent(new PopStateEvent("popstate", { state: { page: "previous" } }));
    const secondDialog = await screen.findByRole("dialog", {
      name: "Discard unsaved changes?",
    });
    await userEvent.click(within(secondDialog).getByRole("button", { name: "Discard changes" }));
    expect(back).toHaveBeenCalledTimes(1);
    await waitFor(() =>
      expect(screen.queryByRole("heading", { name: "Jellyfin" })).not.toBeInTheDocument(),
    );
    expect(location.pathname).toBe(previousUrl);
  });

  it("restores focus to the stable Rule Settings heading after activation removes its opener", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = String(input);
        const method = init?.method ?? "GET";
        if (url === "/api/v1/rules/validate" && method === "POST")
          return Response.json({ valid: true, empty: false });
        return baseResponse(url, method);
      }),
    );
    await enterSettings();
    const heading = screen.getByRole("heading", { name: "Active Rule Set" });
    await userEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByLabelText("Active Rule Set YAML"), {
      target: { value: "version: 1\nrules:\n  - pattern: '*.review'\n" },
    });
    await userEvent.click(screen.getByRole("button", { name: "Validate" }));
    await userEvent.click(
      await screen.findByRole("button", { name: "Save Active Rule Set" }),
    );
    const dialog = await screen.findByRole("dialog", { name: "Activate Rule Set" });
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Activate Rule Set" }),
    );

    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Activate Rule Set" })).not.toBeInTheDocument(),
    );
    await waitFor(() => expect(heading).toHaveFocus());
  });
});
