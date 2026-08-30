import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import { App } from "./main";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  history.replaceState({}, "", "/");
});

describe("Administrator initialization", () => {
  it("uses new-password semantics and permits only one in-flight submission", async () => {
    history.replaceState({}, "", "/initialize?token=one-use-token");
    let resolveRequest: ((response: Response) => void) | undefined;
    const fetchMock = vi.fn(
      () =>
        new Promise<Response>((resolve) => {
          resolveRequest = resolve;
        }),
    );
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);
    const password = screen.getByLabelText("Password");
    const submit = screen.getByRole("button", { name: "Initialize" });
    expect(password).toHaveAttribute("autocomplete", "new-password");

    await userEvent.type(password, "4827");
    await userEvent.dblClick(submit);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(submit).toBeDisabled();
    resolveRequest?.(new Response(null, { status: 204 }));
    expect(
      await screen.findByText("Administrator initialized. Sign in to continue."),
    ).toBeInTheDocument();
  });

  it("reports an already initialized Administrator instead of a generic rejection", async () => {
    history.replaceState({}, "", "/initialize?token=used-token");
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response(null, { status: 409 })),
    );

    render(<App />);
    await userEvent.type(screen.getByLabelText("Password"), "4827");
    await userEvent.click(screen.getByRole("button", { name: "Initialize" }));

    expect(
      await screen.findByText(
        "Administrator is already initialized. Sign in to continue.",
      ),
    ).toBeInTheDocument();
  });
});

describe("Administrator 登录", () => {
  it("当凭据有效且会话建立时，应进入真实 Management Interface shell", async () => {
    const requests: Array<{ url: string; method: string }> = [];
    let statusChecks = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = String(input);
        const method = init?.method ?? "GET";
        requests.push({ url, method });
        if (url === "/api/v1/status") {
          statusChecks += 1;
          return statusChecks === 1
            ? new Response(null, { status: 401 })
            : Response.json({ state: "healthy" });
        }
        if (url === "/api/v1/auth/login")
          return new Response(null, { status: 204 });
        if (url.startsWith("/api/v1/assets?"))
          return Response.json({
            items: [],
            groups: [],
            page: 1,
            total: 0,
            total_pages: 1,
          });
        if (url === "/api/v1/assets/health")
          return Response.json({ state: "healthy", mode: "manual" });
        return new Response(null, { status: 204 });
      }),
    );

    const firstVisit = render(<App />);
    await userEvent.type(
      await screen.findByLabelText("Password"),
      "correct horse battery staple",
    );
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
    await waitFor(() =>
      expect(requests).toContainEqual({
        url: "/api/v1/auth/login",
        method: "POST",
      }),
    );

    firstVisit.unmount();
    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "所有资产" }),
    ).toBeInTheDocument();
    expect(document.querySelector(".shell")).toHaveAttribute(
      "data-design",
      "beui-photos",
    );
  });
});

describe("Active Rule Set settings", () => {
  it("downloads into the editor, validates, and saves only after explicit activation", async () => {
    const requests: Array<{ url: string; method: string; body?: string }> = [];
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = String(input);
        const method = init?.method ?? "GET";
        requests.push({ url, method, body: init?.body?.toString() });
        if (url === "/api/v1/status")
          return new Response('{"version":"test"}', { status: 200 });
        if (url === "/api/v1/rules/active")
          return new Response('{"yaml":"version: 1\\nrules: []\\n"}', {
            status: 200,
          });
        if (url === "/api/v1/rules/download")
          return new Response(
            '{"yaml":"version: 1\\nrules:\\n  - pattern: \'*.ad\'\\n"}',
            { status: 200 },
          );
        if (url === "/api/v1/rules/validate")
          return new Response('{"valid":true,"empty":false}', { status: 200 });
        return new Response(null, { status: 204 });
      }),
    );

    render(<App />);
    await userEvent.click(
      (await screen.findAllByRole("button", { name: /Settings/ }))[0],
    );
    expect(
      await screen.findByRole("heading", { name: "Active Rule Set" }),
    ).toBeInTheDocument();
    await userEvent.type(
      screen.getByLabelText("Rule Source URL"),
      "https://raw.githubusercontent.com/acme/rules/main/rules.yaml",
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Download proposal" }),
    );
    expect(await screen.findByDisplayValue(/\*\.ad/)).toBeInTheDocument();
    expect(
      requests.some(
        (request) =>
          request.url === "/api/v1/rules/active" && request.method === "PUT",
      ),
    ).toBe(false);
    await userEvent.click(screen.getByRole("button", { name: "Validate" }));
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Save Active Rule Set" }),
      ).toBeEnabled(),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Save Active Rule Set" }),
    );
    expect(
      requests.some(
        (request) =>
          request.url === "/api/v1/rules/active" && request.method === "PUT",
      ),
    ).toBe(true);
  });
});

describe("permanent deletion", () => {
  it("requires an explicit irreversible phrase after choosing selected or unified paths", async () => {
    const requests: Array<{url:string;body?:string}> = [];
    vi.stubGlobal("fetch",vi.fn(async(input:RequestInfo|URL,init?:RequestInit)=>{
      const url=String(input); requests.push({url,body:init?.body?.toString()});
      if(url==="/api/v1/status")return new Response("{}",{status:200});
      if(url==="/api/v1/deletion-candidates")return Response.json({items:[{path:"/media/movie/video.mp4",matching_rule:"*.mp4",type:"file",video_warning:"Permanent deletion removes video content",logical_size:1024,reclaimable_space:4096}]});
      if(url==="/api/v1/deletion-plans")return new Response(JSON.stringify({id:"plan-1",selection:JSON.parse(init?.body as string).selection,logical_size:1024,reclaimable_space:4096,expires_at:999,paths:[{path:"/media/movie/video.mp4",type:"file",video_warning:"warning"}],discovered_hard_links:[{path:"/links/video.mp4"}]}),{status:201});
      if(url==="/api/v1/deletion-plans/plan-1/execute")return Response.json({id:"task-1"},{status:202});
      return new Response(null,{status:204});
    }));
    render(<App/>);
    await userEvent.click((await screen.findAllByRole("button",{name:/Deletion Candidates/}))[0]);
    await userEvent.click(await screen.findByRole("checkbox",{name:/video.mp4/}));
    await userEvent.click(screen.getByRole("button",{name:"Review 1"}));
    expect(await screen.findByText("⚠ This plan permanently removes video content.")).toBeInTheDocument();
    const deleteButton=screen.getByRole("button",{name:"Permanently delete"});
    expect(deleteButton).toBeDisabled();
    await userEvent.type(screen.getByLabelText(/Type/),"PERMANENTLY DELETE");
    expect(deleteButton).toBeEnabled();
    await userEvent.click(deleteButton);
    expect(requests.some(request=>request.url.endsWith("/execute")&&request.body?.includes('"irreversible":true'))).toBe(true);
  });
});

const actorSummary = {
  name: "Alice Aoki",
  movie_count: 2,
  derived_file_count: 7,
  hard_link_count: 7,
  unique_inode_count: 3,
  logical_size: 5242880,
  reclaimable_space: 1048576,
  poster_url: "/api/v1/actors/Alice%20Aoki/poster",
};

const linkedAsset = {
  id: "asset-1",
  path: "/media/ABC-123/ABC-123.mp4",
  jav_code: "ABC-123",
  title: "Linked Film",
  artwork_url: "/art/ABC-123.jpg",
  captured_date: "2026-08-30",
  state: "normal",
  exception: null,
};

function stubActorApi() {
  const requests: string[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      requests.push(url);
      if (url === "/api/v1/status") return Response.json({ state: "healthy" });
      if (url.startsWith("/api/v1/assets?"))
        return Response.json({
          items: [linkedAsset],
          groups: [{ date: "2026-08-30", count: 1 }],
          page: 1,
          total: 1,
          total_pages: 1,
        });
      if (url === "/api/v1/assets/health")
        return Response.json({ state: "healthy", mode: "manual" });
      if (url === "/api/v1/actors") return Response.json([actorSummary]);
      if (url === "/api/v1/actors/Alice%20Aoki")
        return Response.json({ ...actorSummary, linked_assets: [linkedAsset] });
      if (url === "/api/v1/assets/asset-1")
        return Response.json({
          ...linkedAsset,
          actors: [{ name: "Alice Aoki", poster_url: actorSummary.poster_url, actor_folder_url: "/actors/QWxpY2UgQW9raQ" }],
          studio: "Studio",
          release_date: "2026-08-30",
          runtime_minutes: 90,
          director: null,
          tags: [],
          plot: null,
          parse_status: "valid",
          source_path: "/media/ABC-123/ABC-123.nfo",
        });
      return new Response(null, { status: 204 });
    }),
  );
  return requests;
}

describe("Actor detail navigation", () => {
  it("opens an actor card without invoking its separate Remove action", async () => {
    const requests = stubActorApi();
    render(<App />);
    await userEvent.click((await screen.findAllByRole("button", { name: /Actors/ }))[0]);
    await userEvent.click(await screen.findByRole("button", { name: "Open Alice Aoki" }));

    expect(await screen.findByRole("heading", { name: "Alice Aoki" })).toBeInTheDocument();
    expect(location.pathname).toBe("/actors/QWxpY2UgQW9raQ");
    expect(requests).toContain("/api/v1/actors/Alice%20Aoki");
    expect(requests.filter((url) => url === "/api/v1/actors/Alice%20Aoki")).toHaveLength(1);
  });

  it("loads a base64url actor route directly and exposes the required metrics", async () => {
    history.replaceState({}, "", "/actors/QWxpY2UgQW9raQ");
    stubActorApi();
    render(<App />);

    expect(await screen.findByText("Derived paths")).toBeInTheDocument();
    expect(screen.getByText("Unique files")).toBeInTheDocument();
    expect(screen.getByText("Referenced logical size")).toBeInTheDocument();
    expect(screen.getByText("Reclaimable if removed")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove Actor Folder…" })).toBeInTheDocument();
  });

  it("opens a linked Media Asset and returns to its actor through browser history", async () => {
    history.replaceState({}, "", "/actors/QWxpY2UgQW9raQ");
    stubActorApi();
    render(<App />);

    await userEvent.click(await screen.findByRole("button", { name: /Open ABC-123/ }));
    expect(await screen.findByRole("heading", { name: "ABC-123" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Back to Alice Aoki" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Back to Alice Aoki" }));
    expect(await screen.findByText("Referenced logical size")).toBeInTheDocument();
    expect(location.pathname).toBe("/actors/QWxpY2UgQW9raQ");
  });
});

describe("BeUI Photos presentation", () => {
  it("makes prototype navigation and configured defaults operational", async () => {
    stubActorApi();
    render(<App />);

    await screen.findByText("所有资产", { selector: "h1" });
    await userEvent.click(screen.getByRole("button", { name: "Recently Added" }));
    expect(screen.getByText("最近入库", { selector: "h1" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Exceptions" }));
    expect(screen.getByText("异常资产", { selector: "h1" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Management Tasks" }));
    expect(screen.getByLabelText("Media Root")).toHaveValue("/media");
    await userEvent.click(screen.getAllByRole("button", { name: "Settings" })[0]);
    expect(screen.getByLabelText("Rule Source URL")).toHaveValue(
      "https://raw.githubusercontent.com/hqwuzhaoyi/rust-jav/feature/web-jellyfin-truenas/rules.yaml",
    );
  });

  it("uses the prototype information architecture and Chinese gallery labels", async () => {
    stubActorApi();
    render(<App />);

    expect(await screen.findByText("所有资产", { selector: "h1" })).toBeInTheDocument();
    expect(screen.getAllByText("图库").length).toBeGreaterThan(0);
    expect(screen.getAllByText("演员").length).toBeGreaterThan(0);
    expect(screen.getAllByText("删除候选").length).toBeGreaterThan(0);
    expect(screen.getAllByText("整理任务").length).toBeGreaterThan(0);
    expect(document.querySelector('.shell')).toHaveAttribute("data-design", "beui-photos");
  });

  it("renders a dense 4:3 gallery with overlays and accessible motion tabs", async () => {
    stubActorApi();
    render(<App />);

    const tile = await screen.findByRole("button", { name: "查看资产 ABC-123" });
    expect(tile.closest(".asset-card")).toHaveClass("photos-tile");
    expect(tile.querySelector(".asset-overlay")).toBeInTheDocument();
    expect(tile.querySelector("svg")).toBeInTheDocument();
    await userEvent.click(tile);
    const overview = await screen.findByRole("tab", { name: "Overview" });
    expect(overview).toHaveAttribute(
      "aria-controls",
    );
    expect(overview.closest('[role="tablist"]')).toHaveAttribute(
      "data-variant",
      "underline",
    );
  });
});
