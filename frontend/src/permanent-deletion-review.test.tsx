import React from "react";
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./main";

const selectedPath =
  "/media/JAV/ORIGIN/a-very-long-library-name/ABC-123/ABC-123.mp4";
const hardLinkPath =
  "/actors/Alice Aoki/a-very-long-derived-view/ABC-123.mp4";

type RecordedRequest = {
  url: string;
  method: string;
  body: Record<string, unknown> | null;
};

type DeletionApiOptions = {
  execute?: () => Response;
};

function candidate() {
  return {
    path: selectedPath,
    matching_rule: "*.mp4",
    type: "regular file",
    video_warning: "Video file: permanent deletion removes playable media.",
    logical_size: 1536,
    reclaimable_space: 4096,
  };
}

function plan(selection: "selected" | "unified", sequence: number) {
  const unified = selection === "unified";
  return {
    id: `plan-${selection}-${sequence}`,
    selection,
    logical_size: unified ? 3072 : 1536,
    reclaimable_space: unified ? 4096 : 0,
    created_at: 2_000_000_000,
    expires_at: 2_000_000_600,
    hard_link_search_roots: ["/media/JAV/ORIGIN", "/actors"],
    paths: [
      {
        path: selectedPath,
        type: "regular file",
        video_warning: "Video file: permanent deletion removes playable media.",
      },
      ...(unified
        ? [{ path: hardLinkPath, type: "regular file", video_warning: null }]
        : []),
    ],
    discovered_hard_links: [
      { path: hardLinkPath, type: "regular file" },
    ],
  };
}

function stubDeletionApi(options: DeletionApiOptions = {}) {
  const requests: RecordedRequest[] = [];
  let planSequence = 0;
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
      if (url === "/api/v1/deletion-candidates")
        return Response.json({ items: [candidate()] });
      if (url === "/api/v1/deletion-plans" && method === "POST") {
        planSequence += 1;
        return Response.json(
          plan(body?.selection as "selected" | "unified", planSequence),
          { status: 201 },
        );
      }
      if (/^\/api\/v1\/deletion-plans\/[^/]+\/execute$/.test(url))
        return options.execute?.() ??
          Response.json(
            {
              id: "deletion-task-1",
              task_type: "permanent_deletion",
              status: "completed",
              error: null,
              items: [
                {
                  path: selectedPath,
                  status: "deleted",
                  message: null,
                },
              ],
            },
            { status: 202 },
          );
      return new Response(null, { status: 204 });
    }),
  );
  return requests;
}

async function openDeletionReview() {
  render(<App />);
  const navigation = await screen.findAllByRole("button", {
    name: "Deletion Candidates",
  });
  await userEvent.click(navigation[0]);
  await screen.findByRole("heading", { name: "Review permanent deletion" });
}

async function selectAndReview() {
  const checkbox = await screen.findByRole("checkbox", {
    name: `Select ${selectedPath}`,
  });
  await userEvent.click(checkbox);
  const opener = screen.getByRole("button", { name: "Review 1" });
  await userEvent.click(opener);
  const dialog = await screen.findByRole("dialog", {
    name: "Permanently delete 1 paths?",
  });
  return { dialog, opener };
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

describe("Issue #43 permanent-deletion review", () => {
  it("shows the complete candidate path, exact type, video warning, Logical Size, and Reclaimable Space", async () => {
    stubDeletionApi();
    await openDeletionReview();

    const checkbox = await screen.findByRole("checkbox", {
      name: `Select ${selectedPath}`,
    });
    const row = checkbox.closest("label");
    expect(row).not.toBeNull();
    expect(within(row as HTMLElement).getByText(selectedPath)).toBeVisible();
    expect(within(row as HTMLElement).getByText(/regular file/)).toBeVisible();
    expect(
      within(row as HTMLElement).getByText(
        "Video file: permanent deletion removes playable media.",
      ),
    ).toBeVisible();
    expect(within(row as HTMLElement).getByText("Logical Size")).toBeVisible();
    expect(within(row as HTMLElement).getByText("1.5 KiB")).toBeVisible();
    expect(
      within(row as HTMLElement).getByText("Reclaimable Space"),
    ).toBeVisible();
    expect(within(row as HTMLElement).getByText("4.0 KiB")).toBeVisible();
  });

  it("keeps selected-only and unified hard-link scope explicit and obtains a new plan for each choice", async () => {
    const requests = stubDeletionApi();
    await openDeletionReview();
    const { dialog } = await selectAndReview();

    expect(
      within(dialog).getByRole("button", { name: "Selected paths only" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(within(dialog).getByText(selectedPath)).toBeVisible();
    expect(within(dialog).getByText("regular file")).toBeVisible();
    expect(within(dialog).getByText("/media/JAV/ORIGIN")).toBeVisible();
    expect(within(dialog).getByText("/actors")).toBeVisible();
    expect(within(dialog).getByText(hardLinkPath)).toBeVisible();

    await userEvent.click(
      within(dialog).getByRole("button", {
        name: "All discovered hard links (1)",
      }),
    );
    const unifiedDialog = await screen.findByRole("dialog", {
      name: "Permanently delete 2 paths?",
    });
    expect(
      within(unifiedDialog).getByRole("button", {
        name: "All discovered hard links (1)",
      }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(within(unifiedDialog).getByText(hardLinkPath)).toBeVisible();

    expect(
      requests
        .filter((request) => request.url === "/api/v1/deletion-plans")
        .map((request) => request.body),
    ).toEqual([
      { paths: [selectedPath], selection: "selected" },
      { paths: [selectedPath], selection: "unified" },
    ]);
  });

  it("invalidates the phrase after an expired plan and requires a fresh plan before retrying", async () => {
    const requests = stubDeletionApi({
      execute: () =>
        new Response(
          "Operation Plan expired; create a fresh plan before permanent deletion.",
          { status: 409 },
        ),
    });
    await openDeletionReview();
    const { dialog } = await selectAndReview();
    const phrase = within(dialog).getByLabelText(/PERMANENTLY DELETE/);
    const execute = within(dialog).getByRole("button", {
      name: "Permanently delete",
    });

    await userEvent.type(phrase, "PERMANENTLY DELETE");
    await userEvent.click(execute);

    expect(
      await screen.findByText(
        "Operation Plan expired; create a fresh plan before permanent deletion.",
      ),
    ).toBeInTheDocument();
    expect(phrase).toHaveValue("");
    expect(execute).toBeDisabled();
    expect(
      within(dialog).getByRole("button", {
        name: "Create fresh Operation Plan",
      }),
    ).toHaveClass("ui-touch-target");
    expect(
      requests.filter((request) => request.url.endsWith("/execute")),
    ).toHaveLength(1);
  });

  it("presents deleted, replaced, and failed paths as a partial outcome without claiming rollback", async () => {
    stubDeletionApi({
      execute: () =>
        Response.json(
          {
            id: "deletion-task-partial",
            task_type: "permanent_deletion",
            status: "failed",
            error: "permanent deletion completed with partial failures",
            items: [
              {
                path: selectedPath,
                status: "deleted",
                message: null,
              },
              {
                path: hardLinkPath,
                status: "changed",
                message: "File was replaced after the Operation Plan was created",
              },
              {
                path: "/media/JAV/ORIGIN/ABC-124/locked.nfo",
                status: "failed",
                message: "Permission denied",
              },
            ],
          },
          { status: 202 },
        ),
    });
    await openDeletionReview();
    const { dialog } = await selectAndReview();
    await userEvent.type(
      within(dialog).getByLabelText(/PERMANENTLY DELETE/),
      "PERMANENTLY DELETE",
    );
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Permanently delete" }),
    );

    const outcome = await screen.findByRole("dialog", {
      name: "Permanent deletion completed with partial failures",
    });
    expect(within(outcome).getByText(selectedPath)).toBeVisible();
    expect(within(outcome).getByText("Deleted")).toBeVisible();
    expect(within(outcome).getByText(hardLinkPath)).toBeVisible();
    expect(within(outcome).getByText("Replaced after planning")).toBeVisible();
    expect(
      within(outcome).getByText(
        "File was replaced after the Operation Plan was created",
      ),
    ).toBeVisible();
    expect(within(outcome).getByText("Permission denied")).toBeVisible();
    expect(within(outcome).getByText("No rollback was attempted.")).toBeVisible();
    expect(within(outcome).queryByText(/rolled back successfully/i)).toBeNull();
    expect(within(outcome).getByRole("button", { name: "Close" }))
      .toHaveClass("ui-touch-target");
  });

  it("preserves an interrupted durable task instead of coercing it to completed", async () => {
    stubDeletionApi({
      execute: () =>
        Response.json(
          {
            id: "deletion-task-interrupted",
            task_type: "permanent_deletion",
            status: "interrupted",
            error: "outcome persistence was interrupted",
            items: [
              {
                path: selectedPath,
                status: "deleted",
                message: null,
              },
            ],
          },
          { status: 202 },
        ),
    });
    await openDeletionReview();
    const { dialog } = await selectAndReview();
    await userEvent.type(
      within(dialog).getByLabelText(/PERMANENTLY DELETE/),
      "PERMANENTLY DELETE",
    );
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Permanently delete" }),
    );

    const outcome = await screen.findByRole("dialog", {
      name: "Permanent deletion interrupted",
    });
    expect(within(outcome).getByText("outcome persistence was interrupted"))
      .toBeVisible();
    expect(within(outcome).queryByRole("heading", { name: /completed/i }))
      .toBeNull();
  });

  it("keeps a 390px review usable without horizontal scrolling", async () => {
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: 390,
    });
    stubDeletionApi();
    await openDeletionReview();
    const { dialog } = await selectAndReview();

    expect(within(dialog).getByText(selectedPath)).toBeVisible();
    expect(
      within(dialog).getByRole("button", { name: "Selected paths only" }),
    ).toBeVisible();
    expect(
      within(dialog).getByRole("button", {
        name: "All discovered hard links (1)",
      }),
    ).toBeVisible();
    expect(getComputedStyle(dialog).maxWidth).toBe("100%");
    expect(getComputedStyle(within(dialog).getByText(selectedPath)).overflowWrap)
      .toBe("anywhere");
  });

  it("moves focus into the dialog, traps Tab, closes with Escape, and restores the opener", async () => {
    stubDeletionApi();
    await openDeletionReview();
    const { dialog, opener } = await selectAndReview();

    const cancel = within(dialog).getByRole("button", { name: "Cancel" });
    expect(dialog).toContainElement(document.activeElement as HTMLElement);
    cancel.focus();
    await userEvent.keyboard("{Tab}");
    expect(dialog).toContainElement(document.activeElement as HTMLElement);
    await userEvent.keyboard("{Escape}");

    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Permanently delete 1 paths?" }),
      ).not.toBeInTheDocument(),
    );
    expect(opener).toHaveFocus();
  });
});
