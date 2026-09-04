import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import { App } from "./main";
import { productionStyle, productionValue } from "./test-css";

const actorName = "Alice #100% / 雪";
const longActorName =
  "A performer name long enough to wrap without hiding its final words";
const actorSlug = "QWxpY2UgIzEwMCUgLyDpm6o";
const longActorSlug = btoa(unescape(encodeURIComponent(longActorName)))
  .replaceAll("+", "-")
  .replaceAll("/", "_")
  .replace(/=+$/, "");
const encodedActorName = "Alice%20%23100%25%20%2F%20%E9%9B%AA";
const encodedLongActorName = encodeURIComponent(longActorName);
const posterUrl = `/api/v1/actors/${encodedActorName}/poster`;

const linkedAsset = {
  id: "asset/from actor?#1",
  path: "/media/ABC-123/ABC-123.mp4",
  jav_code: "ABC-123",
  title: "Linked Film",
  artwork_url: "/api/v1/assets/asset%2Ffrom%20actor%3F%231/artwork",
  captured_date: "2026-08-30",
  state: "normal",
  exception: null,
};

const actor = {
  name: actorName,
  movie_count: 2,
  derived_file_count: 13,
  hard_link_count: 13,
  unique_inode_count: 7,
  logical_size: 5_261_336_576,
  reclaimable_space: 0,
  poster_url: posterUrl,
  linked_assets: [linkedAsset],
};

const fallbackActor = {
  ...actor,
  name: longActorName,
  movie_count: 0,
  derived_file_count: 0,
  hard_link_count: 0,
  unique_inode_count: 0,
  logical_size: 0,
  poster_url: null,
  linked_assets: [],
};

const assetDetail = {
  ...linkedAsset,
  actors: [
    {
      name: actorName,
      poster_url: posterUrl,
      actor_folder_url: `/actors/${actorSlug}`,
    },
  ],
  studio: "Studio",
  release_date: "2026-08-30",
  runtime_minutes: 90,
  director: null,
  tags: [],
  plot: null,
  parse_status: "valid",
  source_path: "/media/ABC-123/ABC-123.nfo",
};

type StubOptions = {
  actors?: typeof actor[];
  actorListResponse?: Promise<Response> | Response;
  actorDetailResponse?: Promise<Response> | Response;
};

function stubActorApi(options: StubOptions = {}) {
  const requests: Array<{ url: string; method: string }> = [];
  const actors = options.actors ?? [actor, fallbackActor];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      const method = init?.method ?? "GET";
      requests.push({ url, method });
      if (url === "/api/v1/status") return Response.json({ state: "healthy" });
      if (url.startsWith("/api/v1/assets?"))
        return Response.json({
          items: [linkedAsset],
          groups: [{ date: linkedAsset.captured_date, count: 1 }],
          page: 1,
          total: 1,
          total_pages: 1,
        });
      if (url === "/api/v1/assets/health")
        return Response.json({ state: "healthy", mode: "manual" });
      if (url === "/api/v1/media-roots/storage")
        return new Response(null, { status: 204 });
      if (url === "/api/v1/actors")
        return options.actorListResponse ?? Response.json(actors);
      if (url === `/api/v1/actors/${encodedActorName}`) {
        if (method === "DELETE")
          return Response.json(
            {
              id: "task-actor-remove-1",
              task_type: "remove_actor_folder",
              kind: "mutation",
              status: "queued",
              items: [],
            },
            { status: 202 },
          );
        return options.actorDetailResponse ?? Response.json(actor);
      }
      if (url === `/api/v1/actors/${encodedLongActorName}`)
        return Response.json(fallbackActor);
      if (url === "/api/v1/assets/asset%2Ffrom%20actor%3F%231")
        return Response.json(assetDetail);
      return new Response(null, { status: 204 });
    }),
  );
  return requests;
}

function setViewport(width: number) {
  Object.defineProperty(window, "innerWidth", { configurable: true, value: width });
  window.dispatchEvent(new Event("resize"));
}

function setReducedMotion(reduce: boolean) {
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: vi.fn((query: string): MediaQueryList => ({
      matches: reduce && query.includes("prefers-reduced-motion"),
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(() => true),
    })),
  });
}

async function openActors() {
  await userEvent.click(
    (await screen.findAllByRole("button", { name: /Actors/ }))[0],
  );
}

async function openActorFromCard() {
  await openActors();
  const trigger = await screen.findByRole("button", { name: `Open ${actorName}` });
  await userEvent.click(trigger);
  return {
    trigger,
    dialog: await screen.findByRole("dialog", { name: actorName }),
  };
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  document.body.className = "";
  document.body.removeAttribute("style");
  setViewport(1024);
  setReducedMotion(false);
  history.replaceState({}, "", "/");
});

describe("Issue #40 Actor Folder prototype cards", () => {
  it("renders 2:3 cards with encoded portraits and an explicit poster fallback", async () => {
    stubActorApi();
    render(<App />);
    await openActors();

    const portraitCard = await screen.findByRole("button", {
      name: `Open ${actorName}`,
    });
    const portrait = within(portraitCard).getByRole("img", {
      name: `${actorName} portrait`,
    });
    expect(portrait).toHaveAttribute("src", posterUrl);
    expect(portrait).toHaveAttribute("loading", "lazy");
    expect(within(portraitCard).getByText("2 Media Assets · 4.9 GiB")).toBeVisible();
    expect(
      getComputedStyle(portraitCard.querySelector(".actor-folder-poster") as HTMLElement)
        .aspectRatio,
    ).toBe("2 / 3");

    const fallbackCard = screen.getByRole("button", {
      name: `Open ${longActorName}`,
    });
    expect(within(fallbackCard).getByText("0 Media Assets · 0 B")).toBeVisible();
    expect(
      within(fallbackCard).getByRole("img", {
        name: `${longActorName} portrait unavailable`,
      }),
    ).toBeVisible();
  });

  it("keeps a long Actor Folder name available without hover", async () => {
    setReducedMotion(true);
    stubActorApi();
    render(<App />);
    await openActors();

    const card = await screen.findByRole("button", {
      name: `Open ${longActorName}`,
    });
    card.focus();
    expect(within(card).getByText(longActorName)).toBeVisible();
    await userEvent.keyboard("{Enter}");
    expect(await screen.findByRole("dialog", { name: longActorName })).toBeVisible();
  });
});

describe("Issue #40 responsive ActorInspector", () => {
  it.each([1280, 390])(
    "moves focus into a modal Actor detail at %ipx",
    async (width) => {
      setViewport(width);
      stubActorApi();
      render(<App />);
      const { trigger, dialog } = await openActorFromCard();

      expect(dialog).toHaveAttribute("aria-modal", "true");
      expect(document.activeElement).toBe(
        within(dialog).getByRole("button", { name: "Close actor details" }),
      );

      await userEvent.keyboard("{Escape}");
      await waitFor(() => expect(dialog).not.toBeInTheDocument());
      expect(trigger).toHaveFocus();
    },
  );

  it("uses the real production cascade for a 44px circular Close control", async () => {
    stubActorApi();
    render(<App />);
    const { dialog } = await openActorFromCard();

    const close = within(dialog).getByRole("button", { name: "Close actor details" });
    expect(close).toHaveClass("inspector-close", "ui-touch-target", "ui-icon-button");
    const closeStyle = productionStyle(close);
    expect([
      productionValue(close, "width"),
      productionValue(close, "height"),
      productionValue(close, "min-width"),
      productionValue(close, "min-height"),
      closeStyle.borderRadius,
    ]).toEqual(["44px", "44px", "44px", "44px", "50%"]);
    const iconStyle = productionStyle(close.querySelector("svg")!);
    expect([iconStyle.width, iconStyle.height, iconStyle.flexShrink]).toEqual([
      "16px",
      "16px",
      "0",
    ]);
  });

  it("restores focus to the Actor Folder card when desktop detail closes", async () => {
    setViewport(1280);
    stubActorApi();
    render(<App />);
    const { trigger, dialog } = await openActorFromCard();

    await userEvent.click(
      within(dialog).getByRole("button", { name: "Close actor details" }),
    );

    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: actorName })).not.toBeInTheDocument(),
    );
    expect(document.activeElement).toBe(trigger);
  });
});

describe("Issue #40 Actor Folder routes and linked Asset history", () => {
  it("ignores a stale Actor detail that finishes JSON parsing after a newer route", async () => {
    let resolveStaleJson: ((value: typeof actor) => void) | undefined;
    let markJsonStarted: (() => void) | undefined;
    const jsonStarted = new Promise<void>((resolve) => {
      markJsonStarted = resolve;
    });
    const staleJson = new Promise<typeof actor>((resolve) => {
      resolveStaleJson = resolve;
    });
    history.replaceState({}, "", `/actors/${actorSlug}`);
    stubActorApi({
      actorDetailResponse: {
        ok: true,
        json: () => {
          markJsonStarted?.();
          return staleJson;
        },
      } as Response,
    });
    render(<App />);

    await jsonStarted;
    history.replaceState({}, "", `/actors/${longActorSlug}`);
    window.dispatchEvent(new PopStateEvent("popstate"));
    expect(await screen.findByRole("dialog", { name: longActorName })).toBeVisible();

    await act(async () => resolveStaleJson?.(actor));
    expect(screen.getByRole("dialog", { name: longActorName })).toBeVisible();
    expect(screen.queryByRole("dialog", { name: actorName })).not.toBeInTheDocument();
  });

  it("uses /actors as the explicit Actor Folder list route", async () => {
    stubActorApi();
    render(<App />);

    await openActors();
    expect(location.pathname).toBe("/actors");
  });

  it("backs out of a card-opened Actor detail to /actors", async () => {
    stubActorApi();
    render(<App />);
    const { dialog } = await openActorFromCard();

    await userEvent.click(
      within(dialog).getByRole("button", { name: "Close actor details" }),
    );
    await waitFor(() => expect(location.pathname).toBe("/actors"));
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: actorName })).not.toBeInTheDocument(),
    );
  });

  it("replaces a directly loaded Actor detail with /actors when closed", async () => {
    history.replaceState({}, "", `/actors/${actorSlug}`);
    const historyLength = history.length;
    stubActorApi();
    render(<App />);
    const dialog = await screen.findByRole("dialog", { name: actorName });

    await userEvent.click(
      within(dialog).getByRole("button", { name: "Close actor details" }),
    );
    expect(location.pathname).toBe("/actors");
    expect(history.length).toBe(historyLength);
  });

  it("loads a base64url Actor Folder route directly without putting the name in the URL", async () => {
    history.replaceState({}, "", `/actors/${actorSlug}`);
    const requests = stubActorApi();
    render(<App />);

    const dialog = await screen.findByRole("dialog", { name: actorName });
    await waitFor(() => expect(dialog).toBeVisible());
    expect(location.pathname).toBe(`/actors/${actorSlug}`);
    expect(requests).toContainEqual({
      url: `/api/v1/actors/${encodedActorName}`,
      method: "GET",
    });
    expect(location.pathname).not.toContain("Alice");
  });

  it("closes a linked Media Asset back to its Actor and restores the linked-card focus", async () => {
    history.replaceState({}, "", `/actors/${actorSlug}`);
    stubActorApi();
    render(<App />);
    const actorDialog = await screen.findByRole("dialog", { name: actorName });
    const linkedCard = within(actorDialog).getByRole("button", {
      name: "Open ABC-123",
    });

    await userEvent.click(linkedCard);
    const assetDialog = await screen.findByRole("dialog", { name: "ABC-123" });
    await userEvent.click(
      within(assetDialog).getByRole("button", { name: "Close asset details" }),
    );

    const restoredActor = await screen.findByRole("dialog", { name: actorName });
    expect(location.pathname).toBe(`/actors/${actorSlug}`);
    expect(document.activeElement).toBe(
      within(restoredActor).getByRole("button", { name: "Open ABC-123" }),
    );
  });

  it("does not reopen a dismissed linked Asset during later Back or Forward navigation", async () => {
    history.replaceState({}, "", `/actors/${actorSlug}`);
    stubActorApi();
    render(<App />);
    await userEvent.click(
      within(await screen.findByRole("dialog", { name: actorName })).getByRole(
        "button",
        { name: "Open ABC-123" },
      ),
    );
    await userEvent.click(
      within(await screen.findByRole("dialog", { name: "ABC-123" })).getByRole(
        "button",
        { name: "Close asset details" },
      ),
    );
    await screen.findByRole("dialog", { name: actorName });

    history.back();
    await new Promise((resolve) => setTimeout(resolve, 20));
    history.forward();
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(screen.queryByRole("dialog", { name: "ABC-123" })).not.toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: actorName })).toBeVisible();
  });
});

describe("Issue #40 Actor Folder storage semantics", () => {
  it("presents derived paths, unique files, logical size, and reclaimable space separately", async () => {
    history.replaceState({}, "", `/actors/${actorSlug}`);
    stubActorApi();
    render(<App />);
    const dialog = await screen.findByRole("dialog", { name: actorName });

    const metrics = Array.from(dialog.querySelectorAll(".actor-metrics > div")).map(
      (row) => [row.querySelector("dt")?.textContent, row.querySelector("dd")?.textContent],
    );
    expect(metrics).toEqual([
      ["Derived paths", "13"],
      ["Unique files", "7"],
      ["Logical Size", "4.9 GiB"],
      ["Reclaimable Space", "0 B"],
    ]);
  });
});

describe("Issue #40 safe Actor Folder removal", () => {
  it("suspends ActorInspector modal behavior while removal confirmation owns focus", async () => {
    stubActorApi();
    render(<App />);
    const { dialog: actorDialog } = await openActorFromCard();
    await userEvent.click(
      within(actorDialog).getByRole("button", { name: "Remove Actor Folder…" }),
    );

    const confirmation = await screen.findByRole("dialog", {
      name: `Remove ${actorName}?`,
    });
    expect(document.querySelectorAll('[role="dialog"][aria-modal="true"]')).toHaveLength(1);
    expect(actorDialog).toHaveAttribute("inert");
    expect(document.activeElement && confirmation.contains(document.activeElement)).toBe(true);

    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(confirmation).not.toBeInTheDocument());
    expect(screen.getByRole("dialog", { name: actorName })).toBeVisible();
  });

  it("confirms only derived paths are removed and starts a durable Management Task", async () => {
    const eventSources: string[] = [];
    class MockEventSource {
      constructor(url: string | URL) {
        eventSources.push(String(url));
      }
      addEventListener() {}
      close() {}
    }
    vi.stubGlobal("EventSource", MockEventSource);
    const requests = stubActorApi();
    render(<App />);
    const { dialog } = await openActorFromCard();
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Remove Actor Folder…" }),
    );

    const confirmation = await screen.findByRole("dialog", {
      name: `Remove ${actorName}?`,
    });
    expect(confirmation).toHaveTextContent("Only derived Actor View paths");
    expect(confirmation).toHaveTextContent("Source Media Assets");
    expect(confirmation).toHaveTextContent("NFO metadata");
    expect(confirmation).toHaveTextContent("Jellyfin items");
    await userEvent.click(
      within(confirmation).getByRole("button", {
        name: "Remove via Management Task",
      }),
    );

    expect(requests).toContainEqual({
      url: `/api/v1/actors/${encodedActorName}`,
      method: "DELETE",
    });
    expect(eventSources).toEqual([
      "/api/v1/tasks/task-actor-remove-1/events",
    ]);
    expect(
      await screen.findByText("Actor Folder removal started as a Management Task."),
    ).toBeVisible();
  });

  it("keeps the Shell Notice dismiss control a keyboard-operable 44px circle", async () => {
    class MockEventSource {
      addEventListener() {}
      close() {}
    }
    vi.stubGlobal("EventSource", MockEventSource);
    stubActorApi();
    render(<App />);
    const { dialog } = await openActorFromCard();
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Remove Actor Folder…" }),
    );
    await userEvent.click(
      within(await screen.findByRole("dialog", { name: `Remove ${actorName}?` }))
        .getByRole("button", { name: "Remove via Management Task" }),
    );

    const notice = await screen.findByRole("status");
    const close = within(notice).getByRole("button", {
      name: "Dismiss Actor removal notification",
    });
    const closeStyle = productionStyle(close);
    expect([
      productionValue(close, "width"),
      productionValue(close, "height"),
      productionValue(close, "min-width"),
      productionValue(close, "min-height"),
      closeStyle.borderRadius,
      closeStyle.flexShrink,
    ]).toEqual(["44px", "44px", "44px", "44px", "50%", "0"]);
    const iconStyle = productionStyle(close.querySelector("svg")!);
    expect([iconStyle.width, iconStyle.height, iconStyle.flexShrink]).toEqual([
      "14px",
      "14px",
      "0",
    ]);

    close.focus();
    await userEvent.keyboard("{Enter}");
    expect(notice).not.toBeInTheDocument();
  });

  it("refreshes Actor Folders and closes the removed detail only when its task completes", async () => {
    let taskListener: ((event: MessageEvent) => void) | undefined;
    class MockEventSource {
      addEventListener(type: string, listener: EventListener) {
        if (type === "task") taskListener = listener as (event: MessageEvent) => void;
      }
      close() {}
    }
    vi.stubGlobal("EventSource", MockEventSource);
    const requests = stubActorApi();
    render(<App />);
    const { dialog } = await openActorFromCard();
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Remove Actor Folder…" }),
    );
    await userEvent.click(
      within(await screen.findByRole("dialog", { name: `Remove ${actorName}?` }))
        .getByRole("button", { name: "Remove via Management Task" }),
    );

    expect(requests.filter(({ url }) => url === "/api/v1/actors")).toHaveLength(1);
    await act(async () => {
      taskListener?.(new MessageEvent("task", {
        data: JSON.stringify({ id: "task-actor-remove-1", status: "completed" }),
      }));
    });

    await waitFor(() =>
      expect(requests.filter(({ url }) => url === "/api/v1/actors")).toHaveLength(2),
    );
    expect(location.pathname).toBe("/actors");
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: actorName })).not.toBeInTheDocument(),
    );
  });

  it.each(["failed", "interrupted"])(
    "keeps Actor detail open and reports a %s removal task",
    async (status) => {
      let taskListener: ((event: MessageEvent) => void) | undefined;
      class MockEventSource {
        addEventListener(type: string, listener: EventListener) {
          if (type === "task") taskListener = listener as (event: MessageEvent) => void;
        }
        close() {}
      }
      vi.stubGlobal("EventSource", MockEventSource);
      stubActorApi();
      render(<App />);
      const { dialog } = await openActorFromCard();
      await userEvent.click(
        within(dialog).getByRole("button", { name: "Remove Actor Folder…" }),
      );
      await userEvent.click(
        within(await screen.findByRole("dialog", { name: `Remove ${actorName}?` }))
          .getByRole("button", { name: "Remove via Management Task" }),
      );

      await act(async () => {
        taskListener?.(new MessageEvent("task", {
          data: JSON.stringify({ id: "task-actor-remove-1", status }),
        }));
      });

      expect(screen.getByRole("dialog", { name: actorName })).toBeVisible();
      expect(location.pathname).toBe(`/actors/${actorSlug}`);
      expect(await screen.findByRole("alert")).toHaveTextContent(status);
    },
  );
});

describe("Issue #40 Actor Folder feedback states", () => {
  it("shows loading without flashing the empty state", async () => {
    let resolveActors: ((response: Response) => void) | undefined;
    const pending = new Promise<Response>((resolve) => {
      resolveActors = resolve;
    });
    stubActorApi({ actorListResponse: pending });
    render(<App />);
    await openActors();

    expect(
      await screen.findByRole("status", { name: "Loading Actor Folders" }),
    ).toBeVisible();
    expect(
      screen.queryByRole("heading", { name: "No Actor Folders" }),
    ).not.toBeInTheDocument();
    resolveActors?.(Response.json([actor]));
  });

  it("shows a dedicated empty state after a successful empty response", async () => {
    stubActorApi({ actors: [] });
    render(<App />);
    await openActors();

    expect(
      await screen.findByRole("heading", { name: "No Actor Folders" }),
    ).toBeVisible();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows a retryable Actor Folder error instead of the empty state", async () => {
    stubActorApi({
      actorListResponse: new Response("actor service unavailable", { status: 503 }),
    });
    render(<App />);
    await openActors();

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Actor Folders could not be loaded",
    );
    expect(screen.getByRole("button", { name: "Retry" })).toBeVisible();
    expect(
      screen.queryByRole("heading", { name: "No Actor Folders" }),
    ).not.toBeInTheDocument();
  });

  it("leaves detail loading and exposes a recoverable error when direct detail fails", async () => {
    history.replaceState({}, "", `/actors/${actorSlug}`);
    stubActorApi({
      actorDetailResponse: new Response("detail unavailable", { status: 503 }),
    });
    render(<App />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Actor Folder could not be loaded",
    );
    expect(screen.queryByText("Loading Actor Folder…")).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Retry Actor Folder" }),
    ).toBeVisible();
  });
});
