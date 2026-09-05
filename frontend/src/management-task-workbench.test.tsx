import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import { App } from "./main";
import { productionValue } from "./test-css";

type TaskStatus =
  | "queued"
  | "running"
  | "completed"
  | "failed"
  | "interrupted";

type TaskItem = {
  id: number;
  kind: string;
  path: string | null;
  status: string;
  message: string | null;
};

type TaskFixture = {
  id: string;
  task_type: string;
  media_root: string;
  kind: "preview" | "mutation";
  status: TaskStatus;
  created_at: number;
  error: string | null;
  plan_expires_at: number | null;
  operation_plan: {
    operations: string[];
    actions: Array<{
      kind: string;
      path: string | null;
      source?: string | null;
      target?: string | null;
      destructive: boolean;
      warning: string | null;
    }>;
    warnings: string[];
    requires_confirmation: boolean;
  } | null;
  report: Record<string, unknown> | null;
  planned_item_count?: number | null;
  items: TaskItem[];
};

type RecordedRequest = {
  url: string;
  method: string;
  body: Record<string, unknown> | null;
};

type EventListener = (event: Event | MessageEvent) => void;

class ControlledEventSource {
  static instances: ControlledEventSource[] = [];

  readonly url: string;
  readonly close = vi.fn();
  private readonly listeners = new Map<string, EventListener[]>();

  constructor(url: string | URL) {
    this.url = String(url);
    ControlledEventSource.instances.push(this);
  }

  addEventListener(type: string, listener: EventListener) {
    const listeners = this.listeners.get(type) ?? [];
    listeners.push(listener);
    this.listeners.set(type, listeners);
  }

  emitTask(task: TaskFixture) {
    const event = new MessageEvent("task", { data: JSON.stringify(task) });
    this.listeners.get("task")?.forEach((listener) => listener(event));
  }

  emitError() {
    const event = new Event("error");
    this.listeners.get("error")?.forEach((listener) => listener(event));
  }
}

function task(
  id: string,
  status: TaskStatus,
  overrides: Partial<TaskFixture> = {},
): TaskFixture {
  return {
    id,
    task_type: "operations",
    media_root: "/media/library",
    kind: "mutation",
    status,
    created_at: 1_777_777_777,
    error: null,
    plan_expires_at: null,
    operation_plan: null,
    report: null,
    planned_item_count: null,
    items: [],
    ...overrides,
  };
}

function confirmationPlan() {
  return {
    operations: [
      "delete_ad_files",
      "categorize_files",
      "remove_duplicates",
    ],
    actions: [
      {
        kind: "delete_ad_files",
        path: "/media/library/ABC-001/ad.txt",
        source: "/media/library/ABC-001/ad.txt",
        target: null,
        destructive: true,
        warning: "This file will be removed",
      },
      {
        kind: "categorize_files",
        path: "/media/library/ABC-001/ABC-001.mp4",
        source: "/media/library/incoming/ABC-001.mp4",
        target: "/media/library/ABC-001/ABC-001.mp4",
        destructive: false,
        warning: null,
      },
    ],
    warnings: ["Review destructive paths before applying"],
    requires_confirmation: true,
  };
}

function stubTaskApi(options: {
  tasks?: TaskFixture[];
  createdTask?: TaskFixture;
  recoveredTask?: TaskFixture | Promise<TaskFixture>;
  recoveredTasks?: Array<TaskFixture | Promise<TaskFixture>>;
  loadMoreGate?: Promise<void>;
} = {}) {
  let persistedTasks = options.tasks ?? [];
  const requests: RecordedRequest[] = [];
  let recoveryIndex = 0;

  vi.stubGlobal("EventSource", ControlledEventSource);
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      const method = init?.method ?? "GET";
      const body =
        typeof init?.body === "string"
          ? (JSON.parse(init.body) as Record<string, unknown>)
          : null;
      requests.push({ url, method, body });

      if (url === "/api/v1/status")
        return Response.json({ state: "healthy" });
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
      if (new URL(url, "http://localhost").pathname === "/api/v1/tasks" && method === "GET") {
        const query = new URL(url, "http://localhost").searchParams;
        const sourceTasks = query.get("active") === "true"
          ? persistedTasks.filter((task) => task.status === "queued" || task.status === "running")
          : persistedTasks;
        const limit = Number(query.get("limit") ?? sourceTasks.length);
        const offset = Number(query.get("offset") ?? 0);
        if (offset > 0) await options.loadMoreGate;
        return Response.json(sourceTasks.slice(offset, offset + limit), {
          headers: { "X-Total-Count": String(persistedTasks.length) },
        });
      }
      if (url === "/api/v1/tasks" && method === "POST") {
        const created =
          options.createdTask ?? task("task-created", "queued", { kind: "preview" });
        persistedTasks = [created, ...persistedTasks];
        return Response.json(created, { status: 202 });
      }
      if (url.startsWith("/api/v1/tasks/") && method === "GET") {
        const recovered = await (options.recoveredTasks?.[recoveryIndex++] ?? options.recoveredTask);
        return recovered
          ? Response.json(recovered)
          : new Response(null, { status: 404 });
      }
      return new Response(null, { status: 204 });
    }),
  );

  return {
    requests,
    replaceTasks(next: TaskFixture[]) {
      persistedTasks = next;
    },
  };
}

async function openTasks() {
  render(<App />);
  await userEvent.click(
    await screen.findByRole("button", { name: "整理任务" }),
  );
  await screen.findByRole("heading", { name: "新建操作计划" });
}

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  ControlledEventSource.instances = [];
  history.replaceState({}, "", "/");
  Object.defineProperty(window, "innerWidth", {
    configurable: true,
    value: 1024,
  });
});

describe("Issue #41 Management Task 创建与确认", () => {
  it("当管理员按任意顺序选择操作并创建预览时，应按 canonical ordering 提交", async () => {
    const { requests } = stubTaskApi();
    await openTasks();

    const fullPipeline = screen.getByRole("button", { name: "完整流程" });
    expect(fullPipeline).toHaveClass("ui-touch-target");
    expect(productionValue(fullPipeline, "min-height")).toBe("44px");

    for (const checkbox of screen.getAllByRole("checkbox")) {
      await userEvent.click(checkbox);
    }
    await userEvent.click(screen.getByLabelText("移除重复文件"));
    await userEvent.click(screen.getByLabelText("分类文件"));
    await userEvent.click(screen.getByLabelText("删除广告文件"));
    await userEvent.click(
      screen.getByRole("button", { name: "预览 15 分钟有效的计划" }),
    );

    const preview = requests.find(
      (request) =>
        request.url === "/api/v1/tasks" && request.method === "POST",
    );
    expect(preview?.body).toMatchObject({
      mode: "preview",
      operations: [
        "delete_ad_files",
        "categorize_files",
        "remove_duplicates",
      ],
    });
  });

  it("当预览计划等待应用时，应先展示明确确认对话框再提交 apply", async () => {
    const preview = task("plan-41", "completed", {
      kind: "preview",
      plan_expires_at: Math.floor(Date.now() / 1000) + 900,
      operation_plan: confirmationPlan(),
    });
    const { requests } = stubTaskApi({ tasks: [preview] });
    await openTasks();

    await userEvent.click(
      await screen.findByRole("button", { name: "确认并执行" }),
    );

    expect(
      requests.filter(
        (request) =>
          request.url === "/api/v1/tasks" && request.method === "POST",
      ),
    ).toHaveLength(0);
    const confirmation = await screen.findByRole("dialog", {
      name: "确认操作计划",
    });
    expect(confirmation).toHaveTextContent("plan-41");
    expect(confirmation).toHaveTextContent("Review destructive paths");
    expect(confirmation).toHaveTextContent("删除广告文件");
    expect(confirmation).toHaveTextContent("/media/library/ABC-001/ad.txt");
    expect(confirmation).toHaveTextContent("This file will be removed");
    expect(confirmation).toHaveTextContent("/media/library/ABC-001/ABC-001.mp4");
    expect(confirmation).toHaveTextContent("来源 /media/library/incoming/ABC-001.mp4");
    expect(confirmation).toHaveTextContent("目标 /media/library/ABC-001/ABC-001.mp4");

    await userEvent.click(
      within(confirmation).getByRole("button", { name: "执行已确认计划" }),
    );
    expect(
      requests.find(
        (request) =>
          request.url === "/api/v1/tasks" && request.method === "POST",
      )?.body,
    ).toEqual({
      task_type: "operations",
      mode: "apply",
      plan_id: "plan-41",
      confirmed: true,
    });
  });
});

describe("Issue #41 Management Task 统一生命周期", () => {
  it("当历史包含所有生命周期阶段时，应以同一状态词汇呈现六种状态", async () => {
    stubTaskApi({
      tasks: [
        task("queued-1", "queued"),
        task("running-1", "running"),
        task("blocked-1", "completed", {
          kind: "preview",
          plan_expires_at: Math.floor(Date.now() / 1000) + 900,
          operation_plan: confirmationPlan(),
        }),
        task("completed-1", "completed"),
        task("failed-1", "failed", { error: "permission denied" }),
        task("interrupted-1", "interrupted", { error: "service restarted" }),
      ],
    });
    await openTasks();

    for (const label of [
      "排队中",
      "运行中",
      "等待确认",
      "已完成",
      "失败",
      "已中断",
    ]) {
      expect(screen.getAllByText(label, { exact: true })[0]).toBeVisible();
    }
  });

  it("当创建请求被接受时，应立即显示持久任务卡并开始监听进度", async () => {
    const created = task("preview-created-41", "queued", {
      kind: "preview",
      media_root: "/media/new-library",
    });
    stubTaskApi({ createdTask: created });
    await openTasks();
    await screen.findByText("暂无管理任务。");

    await userEvent.clear(screen.getByLabelText("媒体根目录"));
    await userEvent.type(screen.getByLabelText("媒体根目录"), "/media/new-library");
    await userEvent.click(
      screen.getByRole("button", { name: "预览 15 分钟有效的计划" }),
    );

    expect(await screen.findByText(/preview-created-41/)).toBeVisible();
    expect(screen.getByText("/media/new-library")).toBeVisible();
    expect(ControlledEventSource.instances.map((source) => source.url)).toContain(
      "/api/v1/tasks/preview-created-41/events",
    );
  });

  it("当 SSE 报告逐项进度时，应更新任务卡与可访问进度", async () => {
    const running = task("running-progress-41", "running", {
      items: [
        {
          id: 1,
          kind: "delete_ad_files",
          path: "/media/one/ad.txt",
          status: "completed",
          message: null,
        },
        {
          id: 2,
          kind: "categorize_files",
          path: "/media/two/movie.mp4",
          status: "running",
          message: null,
        },
      ],
    });
    stubTaskApi({ tasks: [task("running-progress-41", "queued")] });
    await openTasks();

    act(() => ControlledEventSource.instances[0].emitTask(running));

    expect((await screen.findAllByText("运行中", { exact: true }))[0]).toBeVisible();
    expect(
      screen.getByRole("progressbar", { name: "任务进度" }),
    ).toHaveAttribute("aria-valuenow", "50");
    expect(screen.getByText("/media/one/ad.txt")).toBeVisible();
    expect(screen.getByText("/media/two/movie.mp4")).toBeVisible();
  });

  it("应使用 mutation 复制的计划动作总数计算真实进度", async () => {
    const running = task("running-denominator-41", "running", {
      planned_item_count: 4,
      items: [{ id: 1, kind: "move", path: "/media/one.mp4", status: "applied", message: null }],
    });
    stubTaskApi({ tasks: [task("running-denominator-41", "queued", { planned_item_count: 4 })] });
    await openTasks();
    act(() => ControlledEventSource.instances[0].emitTask(running));
    expect(screen.getByRole("progressbar", { name: "任务进度" })).toHaveAttribute("aria-valuenow", "25");
  });

  it("当任务部分失败时，应同时保留成功与失败 outcome 并给出明确失败反馈", async () => {
    stubTaskApi({
      tasks: [
        task("partial-failure-41", "failed", {
          error: "1 of 2 operations failed",
          items: [
            {
              id: 1,
              kind: "delete_ad_files",
              path: "/media/ok/ad.txt",
              status: "completed",
              message: "Removed",
            },
            {
              id: 2,
              kind: "categorize_files",
              path: "/media/readonly/movie.mp4",
              status: "failed",
              message: "Permission denied",
            },
          ],
        }),
      ],
    });
    await openTasks();

    const failure = await screen.findByRole("alert");
    expect(failure).toHaveTextContent("1 of 2 operations failed");
    expect(screen.getByText("/media/ok/ad.txt")).toBeVisible();
    expect(screen.getByText("Removed")).toBeVisible();
    expect(screen.getByText("/media/readonly/movie.mp4")).toBeVisible();
    expect(screen.getByText("Permission denied")).toBeVisible();
  });
});

describe("Issue #41 SSE 恢复与跨导航持久结果", () => {
  it("当 SSE 连接报错时，应读取持久快照并恢复终态 outcome", async () => {
    const running = task("recover-41", "running");
    const recovered = task("recover-41", "completed", {
      items: [
        {
          id: 1,
          kind: "remove_duplicates",
          path: "/media/recovered/duplicate.mp4",
          status: "completed",
          message: "Recovered after reconnect",
        },
      ],
    });
    const { requests } = stubTaskApi({
      tasks: [running],
      recoveredTask: recovered,
    });
    await openTasks();

    act(() => ControlledEventSource.instances[0].emitError());

    await waitFor(() =>
      expect(requests).toContainEqual({
        url: "/api/v1/tasks/recover-41",
        method: "GET",
        body: null,
      }),
    );
    expect(await screen.findByText("Recovered after reconnect")).toBeVisible();
    expect(screen.getAllByText("已完成", { exact: true })[0]).toBeVisible();
  });

  it("迟到的 REST recovery 不得覆盖期间到达的 SSE completed", async () => {
    let resolveRecovery!: (task: TaskFixture) => void;
    const recovery = new Promise<TaskFixture>((resolve) => { resolveRecovery = resolve; });
    stubTaskApi({ tasks: [task("monotonic-41", "running")], recoveredTask: recovery });
    await openTasks();
    const source = ControlledEventSource.instances[0];
    act(() => source.emitError());
    const completed = task("monotonic-41", "completed", {
      items: [{ id: 1, kind: "move", path: "/media/final.mp4", status: "applied", message: "SSE final" }],
    });
    act(() => source.emitTask(completed));
    expect(await screen.findByText("SSE final")).toBeVisible();
    await act(async () => resolveRecovery(task("monotonic-41", "running")));
    expect(screen.getByText("已完成", { exact: true })).toBeVisible();
    expect(screen.getByText("SSE final")).toBeVisible();
  });

  it("同一 SSE generation 的较旧 recovery 不得覆盖较新 recovery", async () => {
    let resolveFirst!: (task: TaskFixture) => void;
    let resolveSecond!: (task: TaskFixture) => void;
    const first = new Promise<TaskFixture>((resolve) => { resolveFirst = resolve; });
    const second = new Promise<TaskFixture>((resolve) => { resolveSecond = resolve; });
    stubTaskApi({
      tasks: [task("recovery-sequence-41", "running")],
      recoveredTasks: [first, second],
    });
    await openTasks();
    const source = ControlledEventSource.instances[0];
    act(() => { source.emitError(); source.emitError(); });
    await act(async () => resolveSecond(task("recovery-sequence-41", "completed", {
      items: [{ id: 1, kind: "move", path: "/media/final.mp4", status: "applied", message: "newer recovery" }],
    })));
    expect(await screen.findByText("newer recovery")).toBeVisible();
    await act(async () => resolveFirst(task("recovery-sequence-41", "running")));
    expect(screen.getByText("已完成", { exact: true })).toBeVisible();
    expect(screen.getByText("newer recovery")).toBeVisible();
  });

  it("当离开再返回任务页时，应从持久历史恢复导航期间完成的结果", async () => {
    const running = task("navigation-41", "running");
    const completed = task("navigation-41", "completed", {
      items: [
        {
          id: 1,
          kind: "standardize_names",
          path: "/media/navigation/renamed.mp4",
          status: "completed",
          message: "Persisted outcome",
        },
      ],
    });
    const api = stubTaskApi({ tasks: [running] });
    await openTasks();

    await userEvent.click(screen.getByRole("button", { name: "所有资产" }));
    api.replaceTasks([completed]);
    await userEvent.click(screen.getByRole("button", { name: "整理任务" }));

    expect(await screen.findByText("Persisted outcome")).toBeVisible();
    expect(
      api.requests.filter(
        (request) =>
            request.url === "/api/v1/tasks?limit=20&offset=0" && request.method === "GET",
      ),
    ).toHaveLength(2);
  });

  it("应发现首个分页窗口之外的 active task 并维持 watcher", async () => {
    const history = Array.from({ length: 25 }, (_, index) => task(`completed-${index}`, "completed"));
    history.push(task("active-outside-page-41", "running", { created_at: 1 }));
    const api = stubTaskApi({ tasks: history });
    await openTasks();
    expect(api.requests.some((request) => request.url === "/api/v1/tasks?active=true")).toBe(true);
    expect(ControlledEventSource.instances.map((source) => source.url)).toContain(
      "/api/v1/tasks/active-outside-page-41/events",
    );
    await userEvent.click(screen.getByRole("button", { name: "再加载 20 个任务" }));
    expect(await screen.findByText(/completed-20/)).toBeVisible();
    expect(api.requests.some((request) => request.url === "/api/v1/tasks?limit=20&offset=20")).toBe(true);
  });
});

describe("Issue #41 响应式极端数据", () => {
  it("Load more 应为 single-flight 且只在接受响应后推进 history offset", async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    const history = Array.from({ length: 25 }, (_, index) => task(`single-flight-${index}`, "completed"));
    const api = stubTaskApi({ tasks: history, loadMoreGate: gate });
    await openTasks();
    const button = screen.getByRole("button", { name: "再加载 20 个任务" });
    await Promise.all([userEvent.click(button), userEvent.click(button)]);
    expect(api.requests.filter((request) => request.url === "/api/v1/tasks?limit=20&offset=20")).toHaveLength(1);
    expect(button).toBeDisabled();
    await act(async () => release());
    expect(await screen.findByText(/single-flight-20/)).toBeVisible();
  });

  it.each([390, 1280])(
    "当视口宽度为 %ipx 且路径很长_历史很大时，应仍可访问完整路径与失败结果",
    async (width) => {
      Object.defineProperty(window, "innerWidth", {
        configurable: true,
        value: width,
      });
      const longPath = `/media/${"nested-library/".repeat(12)}readonly-movie.mp4`;
      const largeHistory = Array.from({ length: 75 }, (_, index) =>
        task(`history-${String(index + 1).padStart(3, "0")}`, "completed"),
      );
      largeHistory[0] = task("history-partial-001", "failed", {
        error: "Completed with partial failures",
        items: [
          {
            id: 1,
            kind: "move_origin",
            path: longPath,
            status: "failed",
            message: "Destination is read-only",
          },
        ],
      });
      const api = stubTaskApi({ tasks: largeHistory });
      await openTasks();

      expect(screen.getByText("75 个任务", { exact: true })).toBeVisible();
      expect(screen.getByText("Completed with partial failures")).toBeVisible();
      expect(screen.getByText("Destination is read-only")).toBeVisible();
      const copyPath = screen.getByRole("button", {
        name: `复制完整路径 ${longPath}`,
      });
      expect(copyPath).toBeVisible();
      expect(copyPath).toHaveClass("ui-touch-target");
      expect(productionValue(copyPath, "min-height")).toBe("44px");
      await userEvent.click(screen.getByRole("button", { name: "再加载 20 个任务" }));
      expect(await screen.findByText(/history-021/)).toBeVisible();
      expect(api.requests.some((request) => request.url === "/api/v1/tasks?limit=20&offset=20")).toBe(true);
    },
  );
});
