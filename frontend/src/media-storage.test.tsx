import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import { App } from "./main";

type CapacityResponse = {
  roots: Array<{
    path: string;
    readable: boolean;
    writable: boolean;
    action: string | null;
    capacity: {
      status: "healthy" | "degraded";
      total_bytes: number | null;
      used_bytes: number | null;
      available_bytes: number | null;
    };
  }>;
  aggregate: {
    status: "healthy" | "degraded";
    filesystem_count: number;
    total_bytes: number | null;
    used_bytes: number | null;
    available_bytes: number | null;
  };
};

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  history.replaceState({}, "", "/");
});

function stubShell(capacity: CapacityResponse) {
  const requests: string[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      requests.push(url);
      if (url === "/api/v1/status") return Response.json({ version: "test" });
      if (url === "/api/v1/media-roots/storage") return Response.json(capacity);
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
  return requests;
}

const healthyCapacity: CapacityResponse = {
  roots: [
    {
      path: "/media",
      readable: true,
      writable: true,
      action: null,
      capacity: {
        status: "healthy",
        total_bytes: 1024 ** 4,
        used_bytes: 768 * 1024 ** 3,
        available_bytes: 256 * 1024 ** 3,
      },
    },
  ],
  aggregate: {
    status: "healthy",
    filesystem_count: 1,
    total_bytes: 1024 ** 4,
    used_bytes: 768 * 1024 ** 3,
    available_bytes: 256 * 1024 ** 3,
  },
};

describe("真实 Management Interface shell 的媒体存储状态", () => {
  it("当容量健康时，应在桌面侧栏显示格式化总量_剩余量与使用比例", async () => {
    const requests = stubShell(healthyCapacity);
    render(<App />);

    const storage = await screen.findByRole("region", { name: "媒体存储" });
    expect(requests).toContain("/api/v1/media-roots/storage");
    expect(within(storage).getByText("1.0 TiB 总量")).toBeInTheDocument();
    expect(within(storage).getByText("768 GiB 已用")).toBeInTheDocument();
    expect(within(storage).getByText("256 GiB 剩余")).toBeInTheDocument();
    expect(
      within(storage).getByRole("progressbar", { name: "媒体存储已使用 75%" }),
    ).toHaveAttribute("aria-valuenow", "75");
  });

  it("当容量降级时，应显示明确的容量不可用文案而不伪造数值", async () => {
    stubShell({
      roots: [
        {
          path: "/media/offline",
          readable: false,
          writable: false,
          action: "检查媒体根目录权限或挂载状态",
          capacity: {
            status: "degraded",
            total_bytes: null,
            used_bytes: null,
            available_bytes: null,
          },
        },
      ],
      aggregate: {
        status: "degraded",
        filesystem_count: 0,
        total_bytes: null,
        used_bytes: null,
        available_bytes: null,
      },
    });
    render(<App />);

    const storage = await screen.findByRole("region", { name: "媒体存储" });
    expect(within(storage).getByText("容量不可用")).toBeInTheDocument();
    expect(
      within(storage).getByText("检查媒体根目录权限或挂载状态"),
    ).toBeInTheDocument();
    expect(within(storage).queryByText(/0 B/)).not.toBeInTheDocument();
    expect(within(storage).queryByRole("progressbar")).not.toBeInTheDocument();
  });

  it("当使用移动布局时，应提供可访问的媒体存储入口并展示同一容量状态", async () => {
    stubShell(healthyCapacity);
    render(<App />);

    const mobileEntry = await screen.findByRole("button", { name: "媒体存储" });
    await userEvent.click(mobileEntry);

    const mobileStatus = screen.getByRole("dialog", { name: "媒体存储" });
    const close = within(mobileStatus).getByRole("button", { name: "关闭媒体存储" });
    expect(close).toHaveFocus();
    expect(within(mobileStatus).getByText("1.0 TiB 总量")).toBeInTheDocument();
    expect(within(mobileStatus).getByText("768 GiB 已用")).toBeInTheDocument();
    expect(within(mobileStatus).getByText("256 GiB 剩余")).toBeInTheDocument();
    expect(
      within(mobileStatus).getByRole("progressbar", {
        name: "媒体存储已使用 75%",
      }),
    ).toHaveAttribute("aria-valuenow", "75");
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "媒体存储" })).not.toBeInTheDocument();
    expect(mobileEntry).toHaveFocus();
  });
});
