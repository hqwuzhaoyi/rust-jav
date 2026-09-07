import { execFileSync } from "node:child_process";
import { describe, expect, it } from "vitest";

describe("UI registry boundary lint", () => {
  it("accepts only approved registry interaction implementations", () => {
    const output = execFileSync("node", ["./scripts/check-ui-boundary.mjs"], {
      cwd: process.cwd(),
      encoding: "utf8",
    });
    expect(output).toMatch(/UI boundary check passed/);
  });
});
