import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import { App } from "./main";
import { productionStyle, productionValue } from "./test-css";

const asset = {
  id: "asset-direct-route",
  path: "/media/a-very-long-library-name/performer/ABC-123/ABC-123.2160p.remux.mp4",
  jav_code: "ABC-123",
  title: "Blue Room",
  artwork_url: "/api/v1/assets/asset-direct-route/artwork",
  captured_date: "2026-08-30",
  state: "normal",
  exception: null,
};

const detail = {
  ...asset,
  actors: [
    {
      name: "miru",
      poster_url: "/api/v1/actors/miru/poster",
      actor_folder_url: "/actors/bWlydQ",
    },
  ],
  studio: null,
  release_date: "2026-08-24",
  runtime_minutes: null,
  director: null,
  tags: ["Drama", "4K", "中文字幕"],
  plot: null,
  parse_status: "valid",
  source_path:
    "/media/a-very-long-library-name/performer/ABC-123/metadata/ABC-123.release.nfo",
  jellyfin: {
    status: "played",
    confidence: "certain_path",
    reason: "Matched by normalized Media Asset path",
    play_count: 2,
    playback_position_ticks: 120000000,
    open_url: "http://jellyfin.test/web/#/details?id=jf-1",
    may_authorize_deletion: true,
  },
};

function stubInspectorApi(options: { detailResponse?: Promise<Response> } = {}) {
  const requests: Array<{ url: string; signal?: AbortSignal | null }> = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      requests.push({ url, signal: init?.signal });
      if (url === "/api/v1/status") return Response.json({ state: "healthy" });
      if (url.startsWith("/api/v1/assets?"))
        return Response.json({
          items: [asset],
          groups: [{ date: asset.captured_date, count: 1 }],
          page: 1,
          total: 1,
          total_pages: 1,
        });
      if (url === "/api/v1/assets/health")
        return Response.json({ state: "healthy", mode: "manual" });
      if (url === "/api/v1/media-roots/storage") return new Response(null, { status: 204 });
      if (url === `/api/v1/assets/${asset.id}`)
        return options.detailResponse ?? Response.json(detail);
      return new Response(null, { status: 204 });
    }),
  );
  return requests;
}

async function openFromGallery() {
  const trigger = await screen.findByRole("button", {
    name: /^(?:Inspect|查看资产) ABC-123$/,
  });
  await userEvent.click(trigger);
  return { trigger, dialog: await screen.findByRole("dialog", { name: "ABC-123" }) };
}

function setViewport(width: number) {
  Object.defineProperty(window, "innerWidth", { configurable: true, value: width });
  window.dispatchEvent(new Event("resize"));
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  document.body.className = "";
  document.body.removeAttribute("style");
  Object.defineProperty(window, "scrollY", { configurable: true, value: 0 });
  setViewport(1024);
  history.replaceState({}, "", "/");
});

describe("Issue #39 AssetInspector 直达路由与历史", () => {
  it("当直达资产的 NFO URL 时，应只凭详情 API 恢复封面、NFO tab 与真实元数据", async () => {
    history.replaceState({}, "", `/assets/${asset.id}?tab=nfo`);
    const requests = stubInspectorApi();
    render(<App />);

    const dialog = await screen.findByRole("dialog", { name: "ABC-123" });
    expect(requests.some((request) => request.url === `/api/v1/assets/${asset.id}`)).toBe(true);
    expect(within(dialog).getByRole("tab", { name: "NFO" }).getAttribute("aria-selected")).toBe("true");
    expect(within(dialog).getByText(detail.source_path)).not.toBeNull();
    expect(dialog.querySelector(".inspector-hero img")?.getAttribute("src")).toBe(asset.artwork_url);
  });

  it("当从 Overview 切换 NFO 后浏览器 Back 时，应同步 tab URL 并恢复 Overview", async () => {
    stubInspectorApi();
    render(<App />);
    const { dialog } = await openFromGallery();

    await userEvent.click(within(dialog).getByRole("tab", { name: "NFO" }));
    expect(location.pathname + location.search).toBe(`/assets/${asset.id}?tab=nfo`);

    const popped = new Promise<void>((resolve) =>
      addEventListener("popstate", () => resolve(), { once: true }),
    );
    history.back();
    await popped;
    await waitFor(() =>
      expect(within(dialog).getByRole("tab", { name: "概览" }).getAttribute("aria-selected")).toBe("true"),
    );
    expect(location.pathname).toBe(`/assets/${asset.id}`);
  });

  it("当关闭从图库打开的详情后再 Back 时，应停留在先前页面且不重新打开详情", async () => {
    history.replaceState({}, "", "/before-gallery");
    history.pushState({}, "", "/");
    stubInspectorApi();
    render(<App />);
    const { dialog } = await openFromGallery();

    await userEvent.click(within(dialog).getByRole("button", { name: "关闭资产详情" }));
    await waitFor(() => expect(location.pathname).toBe("/"));
    history.forward();
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(location.pathname).toBe("/");
    expect(screen.queryByRole("dialog", { name: "ABC-123" })).toBeNull();
    const popped = new Promise<void>((resolve) =>
      addEventListener("popstate", () => resolve(), { once: true }),
    );
    history.back();
    await popped;
    expect(location.pathname).toBe("/");
    expect(screen.queryByRole("dialog", { name: "ABC-123" })).toBeNull();
    const previous = new Promise<void>((resolve) =>
      addEventListener("popstate", () => resolve(), { once: true }),
    );
    history.back();
    await previous;
    expect(location.pathname).toBe("/before-gallery");
    expect(screen.queryByRole("dialog", { name: "ABC-123" })).toBeNull();
  });
});

describe("Issue #39 AssetInspector 模态可访问性", () => {
  it("当打开桌面 Inspector 时，应把焦点移入 Close", async () => {
    setViewport(1280);
    stubInspectorApi();
    render(<App />);
    const { dialog } = await openFromGallery();

    expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "关闭资产详情" }));
  });

  it("当打开模态 Inspector 时，应让 sidebar、main、bottom-nav 全部 inert", async () => {
    stubInspectorApi();
    const { container } = render(<App />);
    await openFromGallery();

    expect([
      container.querySelector(".sidebar")?.hasAttribute("inert"),
      container.querySelector("main")?.hasAttribute("inert"),
      container.querySelector(".bottom-nav")?.hasAttribute("inert"),
    ]).toEqual([true, true, true]);
  });

  it.each([1280, 390])("当视口宽度为 %i 时，应提供可访问的模态详情", async (width) => {
    setViewport(width);
    stubInspectorApi();
    render(<App />);
    const { dialog } = await openFromGallery();

    expect(dialog.getAttribute("aria-modal")).toBe("true");
  });

  it("应通过真实生产级联提供 44px 圆形 Close 控件", async () => {
    stubInspectorApi();
    render(<App />);
    const { dialog } = await openFromGallery();

    const close = within(dialog).getByRole("button", { name: "关闭资产详情" });
    expect(close.classList.contains("inspector-close")).toBe(true);
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

  it("当按 Escape 关闭 Inspector 时，应移除背景 inert 并把焦点恢复到触发卡", async () => {
    stubInspectorApi();
    const { container } = render(<App />);
    const { trigger } = await openFromGallery();

    screen.getByRole("button", { name: "关闭资产详情" }).focus();
    await userEvent.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByRole("dialog", { name: "ABC-123" })).toBeNull());
    expect(document.activeElement).toBe(trigger);
    expect(container.querySelector(".sidebar")?.hasAttribute("inert")).toBe(false);
    expect(container.querySelector("main")?.hasAttribute("inert")).toBe(false);
    expect(container.querySelector(".bottom-nav")?.hasAttribute("inert")).toBe(false);
  });

  it("当点击 Close 关闭 Inspector 时，应把焦点恢复到触发卡", async () => {
    stubInspectorApi();
    render(<App />);
    const { trigger, dialog } = await openFromGallery();

    await userEvent.click(within(dialog).getByRole("button", { name: "关闭资产详情" }));

    await waitFor(() => expect(screen.queryByRole("dialog", { name: "ABC-123" })).toBeNull());
    expect(document.activeElement).toBe(trigger);
  });

  it("当在移动视口打开和关闭 sheet 时，应锁定页面并恢复原滚动位置", async () => {
    setViewport(390);
    Object.defineProperty(window, "scrollY", { configurable: true, value: 640 });
    const scrollTo = vi.fn();
    vi.stubGlobal("scrollTo", scrollTo);
    stubInspectorApi();
    render(<App />);
    const { dialog } = await openFromGallery();

    expect(document.body.classList.contains("asset-inspector-open")).toBe(true);
    expect(document.body.style.getPropertyValue("--asset-inspector-scroll-y")).toBe("640px");

    await userEvent.click(within(dialog).getByRole("button", { name: "关闭资产详情" }));
    await waitFor(() => expect(document.body.classList.contains("asset-inspector-open")).toBe(false));
    expect(scrollTo).toHaveBeenCalledWith(0, 640);
  });

  it("当打开后跨越移动断点时，应同步启用和移除页面滚动锁", async () => {
    setViewport(1280);
    stubInspectorApi();
    render(<App />);
    await openFromGallery();
    expect(document.body.classList.contains("asset-inspector-open")).toBe(false);

    setViewport(390);
    await waitFor(() =>
      expect(document.body.classList.contains("asset-inspector-open")).toBe(true),
    );
    setViewport(1280);
    await waitFor(() =>
      expect(document.body.classList.contains("asset-inspector-open")).toBe(false),
    );
  });
});

describe("Issue #39 AssetInspector prototype 内容结构", () => {
  it("当查看 Overview 与 NFO 时，应共享 detail section/data-list 并完整呈现长路径、空字段与多个 tags", async () => {
    stubInspectorApi();
    const { container } = render(<App />);
    const { dialog } = await openFromGallery();

    const overviewSections = container.querySelectorAll(
      '.asset-inspector [role="tabpanel"] section.detail-section',
    );
    expect(overviewSections.length).toBeGreaterThanOrEqual(2);
    expect(within(dialog).getByText(asset.path)).not.toBeNull();
    expect(within(dialog).getByRole("link", { name: /miru.*演员目录/ }).getAttribute("href")).toBe("/actors/bWlydQ");
    expect(within(dialog).getByText("已播放")).not.toBeNull();
    expect(within(dialog).getByText("按规范化媒体资产路径关联")).not.toBeNull();
    expect(within(dialog).getByText("2 次")).not.toBeNull();
    expect(within(dialog).getByText("120000000 刻度")).not.toBeNull();
    expect(within(dialog).getByText("确定的路径关联")).not.toBeNull();

    await userEvent.click(within(dialog).getByRole("tab", { name: "NFO" }));
    const nfoPanel = within(dialog).getByRole("tabpanel");
    expect(nfoPanel.querySelector("section.detail-section dl.detail-list")).not.toBeNull();
    expect(within(nfoPanel).getByText(detail.source_path)).not.toBeNull();
    expect(within(nfoPanel).getAllByText("未提供")).toHaveLength(3);
    for (const tag of detail.tags) expect(within(nfoPanel).getByText(tag)).not.toBeNull();
  });
});

describe("Issue #39 AssetInspector 请求竞态", () => {
  it("当旧资产请求在新资产之后返回时，应保持新资产详情且旧请求不得覆盖", async () => {
    const newer = { ...asset, id: "asset-newer", jav_code: "NEW-200", title: "Newer" };
    let resolveOld: ((response: Response) => void) | undefined;
    let resolveNew: ((response: Response) => void) | undefined;
    const oldResponse = new Promise<Response>((resolve) => { resolveOld = resolve; });
    const newResponse = new Promise<Response>((resolve) => { resolveNew = resolve; });
    vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url === "/api/v1/status") return Response.json({ state: "healthy" });
      if (url.startsWith("/api/v1/assets?")) return Response.json({
        items: [asset, newer], groups: [{ date: asset.captured_date, count: 2 }],
        page: 1, total: 2, total_pages: 1,
      });
      if (url === "/api/v1/assets/health") return Response.json({ state: "healthy", mode: "manual" });
      if (url === "/api/v1/media-roots/storage") return new Response(null, { status: 204 });
      if (url === `/api/v1/assets/${asset.id}`) return oldResponse;
      if (url === `/api/v1/assets/${newer.id}`) return newResponse;
      return new Response(null, { status: 204 });
    }));
    render(<App />);
    const cards = await screen.findAllByRole("button", { name: /^(?:Inspect|查看资产)/ });

    await userEvent.click(cards[0]);
    await userEvent.click(cards[1]);
    resolveNew?.(Response.json({ ...detail, ...newer, source_path: "/media/NEW-200.nfo" }));
    expect(await screen.findByRole("dialog", { name: "NEW-200" })).not.toBeNull();
    resolveOld?.(Response.json(detail));
    await userEvent.click(screen.getByRole("tab", { name: "NFO" }));

    expect(screen.getByText("/media/NEW-200.nfo")).not.toBeNull();
    expect(screen.queryByText(detail.source_path)).toBeNull();
  });

  it("当详情请求失败时，应退出 loading 并显示可恢复错误", async () => {
    let rejectDetail: ((reason: Error) => void) | undefined;
    const failedDetail = new Promise<Response>((_resolve, reject) => {
      rejectDetail = reject;
    });
    stubInspectorApi({ detailResponse: failedDetail });
    render(<App />);
    const { dialog } = await openFromGallery();
    rejectDetail?.(new Error("network down"));

    expect(await screen.findByText("无法加载资产详情。")).not.toBeNull();
    expect(within(dialog).queryByRole("status")).toBeNull();
  });
});
