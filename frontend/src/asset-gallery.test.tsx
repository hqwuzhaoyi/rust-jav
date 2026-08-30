import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import { App } from "./main";

const indexedAssets = [
  {
    id: "stable-asset-a",
    path: "/media/duplicate-a/ABC-123.mp4",
    jav_code: "ABC-123",
    title: "相同标题",
    artwork_url: "/api/v1/assets/stable-asset-a/artwork",
    captured_date: "2026-08-30",
    state: "normal",
    exception: null,
  },
  {
    id: "stable-asset-b",
    path: "/media/duplicate-b/ABC-123.mp4",
    jav_code: "ABC-123",
    title: "相同标题",
    artwork_url: null,
    captured_date: "2026-08-30",
    state: "synchronizing",
    exception: null,
  },
  {
    id: "stable-asset-c",
    path: "/media/XYZ-789/XYZ-789.mp4",
    jav_code: "XYZ-789",
    title: "异常资产",
    artwork_url: "/api/v1/assets/stable-asset-c/artwork",
    captured_date: "2026-08-29",
    state: "exception",
    exception: "NFO 无法解析",
  },
];

function assetPage(page = 1, totalPages = 4) {
  return {
    items: indexedAssets,
    groups: [
      { date: "2026-08-30", count: 2 },
      { date: "2026-08-29", count: 1 },
    ],
    page,
    total: 3,
    total_pages: totalPages,
  };
}

function assetDetail(id: string) {
  const asset = indexedAssets.find((item) => item.id === id) ?? indexedAssets[0];
  return {
    ...asset,
    actors: [],
    studio: id,
    release_date: null,
    runtime_minutes: null,
    director: null,
    tags: [],
    plot: null,
    parse_status: "valid",
    source_path: `${asset.path}.nfo`,
  };
}

function stubGalleryApi(
  assetsResponse: (url: URL, requestNumber: number) => Promise<Response> = async (url) =>
    Response.json(assetPage(Number(url.searchParams.get("page") ?? 1))),
) {
  const requests: string[] = [];
  let assetRequestNumber = 0;
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const request = String(input);
      requests.push(request);
      if (request === "/api/v1/status") return Response.json({ state: "healthy" });
      if (request.startsWith("/api/v1/assets?")) {
        assetRequestNumber += 1;
        return assetsResponse(new URL(request, location.origin), assetRequestNumber);
      }
      if (request === "/api/v1/assets/health")
        return Response.json({ state: "healthy", mode: "manual" });
      if (request === "/api/v1/media-roots/storage") return new Response(null, { status: 204 });
      const detail = request.match(/^\/api\/v1\/assets\/(stable-asset-[abc])$/);
      if (detail) return Response.json(assetDetail(detail[1]));
      return new Response(null, { status: 204 });
    }),
  );
  return requests;
}

function galleryRequests(requests: string[]) {
  return requests
    .filter((request) => request.startsWith("/api/v1/assets?"))
    .map((request) => new URL(request, location.origin));
}

function setMediaPreferences({ reduce = false, hover = true } = {}) {
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: vi.fn((query: string): MediaQueryList => ({
      matches: query.includes("prefers-reduced-motion") ? reduce : hover,
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

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  history.replaceState({}, "", "/");
});

describe("Issue #38 媒体资产图库 URL 契约", () => {
  it("当携带 q、state、page 直达图库时，应恢复控件并发送完全一致的服务端参数", async () => {
    history.replaceState({}, "", "/?q=ABC-123&state=exception&page=3");
    const requests = stubGalleryApi();
    render(<App />);

    expect(await screen.findByLabelText(/^(?:Search assets|搜索资产)$/)).toHaveValue("ABC-123");
    expect(screen.getByRole("button", { name: "异常" })).toHaveAttribute("aria-pressed", "true");
    expect(await screen.findByText("3 / 4")).toBeInTheDocument();
    const request = galleryRequests(requests).at(-1);
    expect(Object.fromEntries(request?.searchParams ?? [])).toEqual({
      page: "3",
      per_page: "48",
      q: "ABC-123",
      state: "exception",
    });
  });

  it("当搜索、筛选和翻页时，应把状态写入 URL 且请求参数始终与 URL 一致", async () => {
    const requests = stubGalleryApi();
    render(<App />);
    const search = await screen.findByLabelText(/^(?:Search assets|搜索资产)$/);

    await userEvent.type(search, "XYZ-789");
    await userEvent.click(screen.getByRole("button", { name: /^(?:刷新中|同步中)$/ }));
    await userEvent.click(await screen.findByRole("button", { name: /^(?:Next|下一页)$/ }));

    await waitFor(() => expect(new URLSearchParams(location.search).get("page")).toBe("2"));
    const currentUrl = new URL(location.href);
    const request = galleryRequests(requests).at(-1);
    expect(Object.fromEntries(request?.searchParams ?? [])).toEqual(
      Object.fromEntries(currentUrl.searchParams),
    );
  });

  it("当浏览器 Back 触发 popstate 时，应恢复 q、state、page 及对应请求", async () => {
    history.replaceState({}, "", "/?q=first&state=normal&page=2");
    const requests = stubGalleryApi();
    render(<App />);
    await screen.findByText("2 / 4");

    history.pushState({}, "", "/?q=second&state=exception&page=3");
    window.dispatchEvent(new PopStateEvent("popstate"));

    await waitFor(() =>
      expect(screen.getByLabelText(/^(?:Search assets|搜索资产)$/)).toHaveValue("second"),
    );
    expect(screen.getByRole("button", { name: "异常" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText("3 / 4")).toBeInTheDocument();
    expect(Object.fromEntries(galleryRequests(requests).at(-1)?.searchParams ?? [])).toMatchObject({
      q: "second",
      state: "exception",
      page: "3",
    });
  });

  it("当服务端修正越界页码时，应同步显示状态与 URL", async () => {
    history.replaceState({}, "", "/?page=999");
    stubGalleryApi(async () => Response.json(assetPage(4, 4)));
    render(<App />);

    expect(await screen.findByText("4 / 4")).toBeInTheDocument();
    await waitFor(() => expect(new URLSearchParams(location.search).get("page")).toBe("4"));
  });
});

describe("Issue #38 真实 Asset Index 卡片", () => {
  it("当两个资产番号相同时，应由稳定 id 分别驱动详情请求与选中 URL", async () => {
    const requests = stubGalleryApi();
    render(<App />);
    const duplicateCards = await screen.findAllByRole("button", {
      name: /^(?:Inspect|查看资产) ABC-123$/,
    });

    await userEvent.click(duplicateCards[1]);

    expect(requests).toContain("/api/v1/assets/stable-asset-b");
    expect(await screen.findByText("stable-asset-b")).toBeInTheDocument();
    expect(location.pathname).toBe("/assets/stable-asset-b");
  });

  it("当 Asset Index 同时返回有图和无图资产时，应呈现 4:3 overlay、lazy artwork 和明确无图状态", async () => {
    stubGalleryApi();
    render(<App />);
    const cards = await screen.findAllByRole("button", { name: /^(?:Inspect|查看资产)/ });
    const poster = cards[0].querySelector(".poster") as HTMLElement;

    expect(getComputedStyle(poster).aspectRatio).toBe("4 / 3");
    expect(poster.querySelector(".asset-overlay")).toBeVisible();
    expect(within(cards[0]).getByText("ABC-123")).toBeVisible();
    const artwork = within(cards[0]).getByRole("img", { name: "ABC-123 封面" });
    expect(artwork).toHaveAttribute("loading", "lazy");
    expect(artwork).toHaveAttribute("src", "/api/v1/assets/stable-asset-a/artwork");
    expect(within(cards[1]).getByRole("img", { name: "ABC-123 暂无封面" })).toBeVisible();

    fireEvent.error(artwork);
    expect(within(cards[0]).getByRole("img", { name: "ABC-123 暂无封面" })).toBeVisible();
  });

  it("当 Asset Index 返回三种领域状态时，应以正常、同步中、异常中文语义区分且不只依赖颜色", async () => {
    stubGalleryApi();
    render(<App />);
    const cards = await screen.findAllByRole("button", { name: /^(?:Inspect|查看资产)/ });

    expect(within(cards[0]).getByText("正常")).toBeVisible();
    expect(within(cards[1]).getByText("同步中")).toBeVisible();
    expect(within(cards[2]).getByText("异常")).toBeVisible();
  });

  it("当从图库打开再关闭详情时，Back 不应重新打开刚关闭的资产", async () => {
    history.replaceState({}, "", "/before-gallery");
    history.pushState({}, "", "/?page=1&per_page=48");
    stubGalleryApi();
    render(<App />);
    await userEvent.click((await screen.findAllByRole("button", { name: /^(?:Inspect|查看资产)/ }))[0]);
    await userEvent.click(await screen.findByRole("button", { name: "Close asset details" }));
    await waitFor(() => expect(location.pathname).toBe("/"));

    const popped = new Promise<void>((resolve) =>
      addEventListener("popstate", () => resolve(), { once: true }),
    );
    history.back();
    await popped;
    expect(location.pathname).toBe("/");
    expect(screen.queryByRole("button", { name: "Close asset details" })).not.toBeInTheDocument();
    const previous = new Promise<void>((resolve) =>
      addEventListener("popstate", () => resolve(), { once: true }),
    );
    history.back();
    await previous;
    expect(location.pathname).toBe("/before-gallery");
    expect(screen.queryByRole("button", { name: "Close asset details" })).not.toBeInTheDocument();
  });

  it("当较早的详情请求最后返回时，应保持最新资产的 URL 与详情", async () => {
    let resolveA: ((response: Response) => void) | undefined;
    let resolveB: ((response: Response) => void) | undefined;
    const detailA = new Promise<Response>((resolve) => { resolveA = resolve; });
    const detailB = new Promise<Response>((resolve) => { resolveB = resolve; });
    const requests = stubGalleryApi();
    const originalFetch = globalThis.fetch;
    vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url === "/api/v1/assets/stable-asset-a") return detailA;
      if (url === "/api/v1/assets/stable-asset-b") return detailB;
      return originalFetch(input);
    }));
    render(<App />);
    const cards = await screen.findAllByRole("button", { name: /^(?:Inspect|查看资产) ABC-123$/ });
    await userEvent.click(cards[0]);
    await userEvent.click(cards[1]);

    resolveB?.(Response.json(assetDetail("stable-asset-b")));
    expect(await screen.findByText("stable-asset-b")).toBeInTheDocument();
    resolveA?.(Response.json(assetDetail("stable-asset-a")));
    await userEvent.click(screen.getByRole("tab", { name: "NFO" }));

    expect(location.pathname).toBe("/assets/stable-asset-b");
    expect(screen.getByText("/media/duplicate-b/ABC-123.mp4.nfo")).toBeInTheDocument();
    expect(screen.queryByText("/media/duplicate-a/ABC-123.mp4.nfo")).not.toBeInTheDocument();
    expect(requests).toContain("/api/v1/assets?" + new URLSearchParams({ page: "1", per_page: "48" }));
  });
});

describe("Issue #38 图库反馈与可访问性", () => {
  it("当资产请求仍在进行时，应显示加载状态且不得闪现空态", async () => {
    let resolveAssets: ((response: Response) => void) | undefined;
    stubGalleryApi(
      () => new Promise<Response>((resolve) => {
        resolveAssets = resolve;
      }),
    );
    render(<App />);

    expect(await screen.findByRole("status", { name: "正在加载媒体资产" })).toBeVisible();
    expect(screen.queryByRole("heading", { name: "暂无媒体资产" })).not.toBeInTheDocument();
    resolveAssets?.(Response.json(assetPage()));
  });

  it("当服务端成功返回空页时，应显示独立中文空态", async () => {
    stubGalleryApi(async () => Response.json({ ...assetPage(), items: [], groups: [], total: 0, total_pages: 0 }));
    render(<App />);

    expect(await screen.findByRole("heading", { name: "暂无媒体资产" })).toBeVisible();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("当服务端失败后重试成功时，应先显示错误而非空态，再恢复真实卡片", async () => {
    stubGalleryApi(async (_url, requestNumber) =>
      requestNumber === 1
        ? new Response("boom", { status: 500 })
        : Response.json(assetPage()),
    );
    render(<App />);

    expect(await screen.findByRole("alert")).toHaveTextContent("无法加载媒体资产");
    expect(screen.queryByRole("heading", { name: "暂无媒体资产" })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "重试" }));
    expect(await screen.findAllByRole("button", { name: /^(?:Inspect|查看资产)/ })).toHaveLength(3);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("当桌面侧栏和手机 bottom nav 同时存在于响应式 DOM 时，应各自有名称和当前页语义", async () => {
    stubGalleryApi();
    render(<App />);

    const desktop = await screen.findByRole("navigation", { name: "桌面主导航" });
    const mobile = screen.getByRole("navigation", { name: "移动端主导航" });
    expect(within(desktop).getByRole("button", { name: "所有资产" })).toHaveAttribute("aria-current", "page");
    expect(within(mobile).getByRole("button", { name: "图库" })).toHaveAttribute("aria-current", "page");
  });

  it("当用户启用 reduced-motion 且设备不支持 hover 时，应无需 hover 即可读卡片并用键盘打开详情", async () => {
    setMediaPreferences({ reduce: true, hover: false });
    stubGalleryApi();
    render(<App />);
    const cards = await screen.findAllByRole("button", { name: /^(?:Inspect|查看资产)/ });

    expect(within(cards[0]).getByText("ABC-123")).toBeVisible();
    cards[0].focus();
    await userEvent.keyboard("{Enter}");
    expect(await screen.findByRole("dialog", { name: "ABC-123" })).toBeVisible();
  });
});
