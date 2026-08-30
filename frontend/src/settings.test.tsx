import React from "react";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./main";

type RecordedRequest = {
  url: string;
  method: string;
  body: Record<string, unknown> | null;
};

const activeYaml = "version: 1\nrules:\n  - pattern: '*.active'\n";
const proposalYaml = "version: 1\nrules:\n  - pattern: '*.proposal'\n";
const emptyYaml = "version: 1\nrules: []\n";

function stubSettingsApi(
  override?: (
    request: RecordedRequest,
  ) => Response | Promise<Response> | undefined,
) {
  const requests: RecordedRequest[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const request: RecordedRequest = {
        url: String(input),
        method: init?.method ?? "GET",
        body:
          typeof init?.body === "string"
            ? (JSON.parse(init.body) as Record<string, unknown>)
            : null,
      };
      requests.push(request);
      const overridden = override?.(request);
      if (overridden) return await overridden;

      if (request.url === "/api/v1/status")
        return Response.json({ state: "healthy" });
      if (request.url.startsWith("/api/v1/assets?"))
        return Response.json({
          items: [],
          groups: [],
          page: 1,
          total: 0,
          total_pages: 1,
        });
      if (request.url === "/api/v1/assets/health")
        return Response.json({ state: "healthy", mode: "manual" });
      if (request.url === "/api/v1/media-roots/storage")
        return new Response(null, { status: 204 });
      if (request.url === "/api/v1/rules/active")
        return request.method === "GET"
          ? Response.json({ yaml: activeYaml })
          : new Response(null, { status: 204 });
      if (request.url === "/api/v1/rules/download")
        return Response.json({ yaml: proposalYaml });
      if (request.url === "/api/v1/rules/validate")
        return Response.json({
          valid: true,
          empty: request.body?.yaml === emptyYaml,
        });
      if (request.url === "/api/v1/jellyfin/config")
        return request.method === "GET"
          ? Response.json({
              url: "http://jellyfin:8096",
              library_ids: ["movies", "jav"],
              api_key_configured: true,
            })
          : new Response(null, { status: 204 });
      return new Response(null, { status: 204 });
    }),
  );
  return requests;
}

async function openSettings(options: { mobile?: boolean } = {}) {
  if (options.mobile) {
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: 390,
    });
  }
  render(<App />);
  const settingsButtons = await screen.findAllByRole("button", {
    name: "Settings",
  });
  await userEvent.click(
    options.mobile ? settingsButtons.at(-1)! : settingsButtons[0],
  );
  await screen.findByRole("heading", { name: "Active Rule Set" });
  await waitFor(() =>
    expect(screen.getByLabelText("Server URL")).toHaveValue(
      "http://jellyfin:8096",
    ),
  );
  return settingsButtons;
}

function settingsSection(name: "Active Rule Set" | "Jellyfin") {
  const section = screen.getByRole("heading", { name }).closest("section");
  if (!section) throw new Error(`${name} section is missing`);
  return section;
}

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  history.replaceState({}, "", "/");
  Object.defineProperty(window, "innerWidth", {
    configurable: true,
    value: 1024,
  });
});

describe("Issue #42 Rule proposal activation", () => {
  it("keeps a validated download as a proposal until a separate activation dialog is confirmed", async () => {
    const requests = stubSettingsApi();
    await openSettings();

    await userEvent.clear(screen.getByLabelText("Rule Source URL"));
    await userEvent.type(
      screen.getByLabelText("Rule Source URL"),
      "https://raw.githubusercontent.com/acme/rules/main/rules.yaml",
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Download proposal" }),
    );
    expect(await screen.findByDisplayValue("*.proposal", { exact: false })).toBeVisible();
    expect(
      requests.filter(
        (request) =>
          request.url === "/api/v1/rules/active" && request.method === "PUT",
      ),
    ).toHaveLength(0);

    await userEvent.click(screen.getByRole("button", { name: "Validate" }));
    const activate = await screen.findByRole("button", {
      name: "Save Active Rule Set",
    });
    await userEvent.click(activate);

    expect(
      requests.filter(
        (request) =>
          request.url === "/api/v1/rules/active" && request.method === "PUT",
      ),
    ).toHaveLength(0);
    let dialog = await screen.findByRole("dialog", {
      name: "Activate Rule Set",
    });
    expect(dialog).toHaveTextContent("*.proposal");
    expect(dialog).toContainElement(document.activeElement as HTMLElement);

    await userEvent.keyboard("{Escape}");
    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Activate Rule Set" }),
      ).not.toBeInTheDocument(),
    );
    expect(activate).toHaveFocus();

    await userEvent.keyboard("{Enter}");
    dialog = await screen.findByRole("dialog", { name: "Activate Rule Set" });
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Activate Rule Set" }),
    );
    expect(
      requests.find(
        (request) =>
          request.url === "/api/v1/rules/active" && request.method === "PUT",
      )?.body,
    ).toEqual({ yaml: proposalYaml, confirm_empty: false });
  });

  it("uses a distinct warning dialog and confirmation for an intentionally empty Rule Set", async () => {
    const requests = stubSettingsApi();
    await openSettings();

    await userEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByLabelText("Active Rule Set YAML"), {
      target: { value: emptyYaml },
    });
    await userEvent.click(screen.getByRole("button", { name: "Validate" }));
    const reviewEmpty = await screen.findByRole("button", {
      name: "Confirm empty and save",
    });
    await userEvent.click(reviewEmpty);

    expect(
      requests.filter(
        (request) =>
          request.url === "/api/v1/rules/active" && request.method === "PUT",
      ),
    ).toHaveLength(0);
    const dialog = await screen.findByRole("dialog", {
      name: "Activate empty Rule Set",
    });
    expect(dialog).toHaveTextContent(/no enabled rules/i);
    await userEvent.click(
      within(dialog).getByRole("button", {
        name: "Activate empty Rule Set",
      }),
    );
    expect(
      requests.find(
        (request) =>
          request.url === "/api/v1/rules/active" && request.method === "PUT",
      )?.body,
    ).toEqual({ yaml: emptyYaml, confirm_empty: true });
  });

  it("keeps a failed proposal error inline with the Rule form", async () => {
    stubSettingsApi((request) => {
      if (request.url === "/api/v1/rules/download")
        return Response.json(
          { error: "Rule source host is not allowed." },
          { status: 422 },
        );
    });
    await openSettings();

    await userEvent.click(
      screen.getByRole("button", { name: "Download proposal" }),
    );

    expect(
      await within(settingsSection("Active Rule Set")).findByRole("alert"),
    ).toHaveTextContent("Rule source host is not allowed.");
    expect(screen.getByLabelText("Active Rule Set YAML")).toHaveValue(activeYaml);
  });
});

describe("Issue #42 Jellyfin Settings state and security", () => {
  it("marks only changed fields dirty and clears the marker after a successful save", async () => {
    stubSettingsApi();
    await openSettings();

    const section = settingsSection("Jellyfin");
    const save = within(section).getByRole("button", { name: "Save Jellyfin" });
    expect(save).toBeDisabled();
    expect(within(section).queryByText("Unsaved changes")).not.toBeInTheDocument();

    const url = within(section).getByLabelText("Server URL");
    await userEvent.clear(url);
    await userEvent.type(url, "http://jellyfin-new:8096");
    expect(save).toBeEnabled();
    expect(within(section).getByText("Unsaved changes")).toBeVisible();

    await userEvent.click(save);
    await waitFor(() => expect(save).toBeDisabled());
    expect(within(section).queryByText("Unsaved changes")).not.toBeInTheDocument();
  });

  it("disables a pending Jellyfin save and reports failure inline without losing edits", async () => {
    let resolveSave!: (response: Response) => void;
    const pendingSave = new Promise<Response>((resolve) => {
      resolveSave = resolve;
    });
    stubSettingsApi((request) => {
      if (
        request.url === "/api/v1/jellyfin/config" &&
        request.method === "PUT"
      )
        return pendingSave;
    });
    await openSettings();

    const section = settingsSection("Jellyfin");
    const url = within(section).getByLabelText("Server URL");
    await userEvent.clear(url);
    await userEvent.type(url, "http://offline-jellyfin:8096");
    await userEvent.click(
      within(section).getByRole("button", { name: "Save Jellyfin" }),
    );

    expect(
      within(section).getByRole("button", { name: "Saving Jellyfin…" }),
    ).toBeDisabled();
    expect(url).toBeDisabled();

    await act(async () => {
      resolveSave(new Response("Jellyfin is unreachable.", { status: 502 }));
    });
    expect(await within(section).findByRole("alert")).toHaveTextContent(
      "Jellyfin is unreachable.",
    );
    expect(url).toHaveValue("http://offline-jellyfin:8096");
    expect(url).toBeEnabled();
  });

  it("never hydrates returned credential-shaped fields into the browser", async () => {
    const requests = stubSettingsApi((request) => {
      if (
        request.url === "/api/v1/jellyfin/config" &&
        request.method === "GET"
      )
        return Response.json({
          url: "http://jellyfin:8096",
          library_ids: ["movies"],
          api_key_configured: true,
          api_key: "must-never-reach-the-browser",
          server_key: "also-server-only",
        });
    });
    await openSettings();

    const section = settingsSection("Jellyfin");
    expect(within(section).getByLabelText("Server API key")).toHaveValue("");
    expect(document.body).not.toHaveTextContent("must-never-reach-the-browser");
    expect(document.body).not.toHaveTextContent("also-server-only");

    await userEvent.clear(within(section).getByLabelText("Library IDs"));
    await userEvent.type(within(section).getByLabelText("Library IDs"), "movies, tv");
    await userEvent.click(
      within(section).getByRole("button", { name: "Save Jellyfin" }),
    );
    expect(
      requests.find(
        (request) =>
          request.url === "/api/v1/jellyfin/config" && request.method === "PUT",
      )?.body,
    ).toEqual({
      url: "http://jellyfin:8096",
      library_ids: ["movies", "tv"],
      api_key: "",
    });
  });
});

describe("Issue #42 mobile Settings", () => {
  it("keeps both Settings forms keyboard-accessible from the 390px bottom navigation", async () => {
    stubSettingsApi();
    const settingsButtons = await openSettings({ mobile: true });

    expect(settingsButtons.at(-1)).toHaveAttribute("aria-current", "page");
    expect(screen.getByLabelText("Rule Source URL")).toBeVisible();
    expect(screen.getByLabelText("Active Rule Set YAML")).toBeVisible();
    expect(screen.getByLabelText("Server URL")).toBeVisible();
    expect(screen.getByLabelText("Library IDs")).toBeVisible();
    expect(screen.getByLabelText("Server API key")).toBeVisible();

    screen.getByLabelText("Rule Source URL").focus();
    await userEvent.tab();
    expect(
      screen.getByRole("button", { name: "Download proposal" }),
    ).toHaveFocus();
  });
});
