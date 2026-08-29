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
