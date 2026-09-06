import { execFileSync } from "node:child_process";
import { describe, expect, it } from "vitest";

describe("UI registry boundary lint", () => {
  it("accepts only the explicit temporary legacy baseline", () => {
    const output = execFileSync("node", ["./scripts/check-ui-boundary.mjs"], {
      cwd: process.cwd(),
      encoding: "utf8",
    });
    expect(output).toMatch(/UI boundary check passed/);
  });
});
