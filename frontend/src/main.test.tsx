import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import { App } from "./main";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
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
