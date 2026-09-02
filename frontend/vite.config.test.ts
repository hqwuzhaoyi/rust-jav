import { afterEach, describe, expect, it, vi } from "vitest";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { frontendSourceDigest } from "./source-digest.ts";

afterEach(() => {
  delete process.env.VITE_BACKEND_ORIGIN;
  vi.resetModules();
});

describe("本地 NAS backend proxy", () => {
  it("当配置后端来源时，应让 /api 与 /health 开发请求代理到同一来源", async () => {
    process.env.VITE_BACKEND_ORIGIN = "http://nas.test:8848";
    const configModulePath = "./vite.config";
    const { default: exportedConfig } = await import(configModulePath);
    const config =
      typeof exportedConfig === "function"
        ? await exportedConfig({
            command: "serve",
            mode: "development",
            isSsrBuild: false,
            isPreview: false,
          })
        : await exportedConfig;

    expect(config.server?.proxy?.["/api"]).toMatchObject({
      target: "http://nas.test:8848",
    });
    expect(config.server?.proxy?.["/health"]).toMatchObject({
      target: "http://nas.test:8848",
    });
  });
});

describe("embedded production asset provenance", () => {
  it("binds the tracked shell and manifest to the current source and bundle bytes", () => {
    const dist = resolve(process.cwd(), "dist");
    const shell = readFileSync(resolve(dist, "index.html"), "utf8");
    const manifest = JSON.parse(
      readFileSync(resolve(dist, "assets/asset-manifest.json"), "utf8"),
    ) as {
      source_digest: string;
      index: { path: string; normalized_sha256: string };
      assets: Record<"javascript" | "stylesheet", { path: string; sha256: string }>;
    };
    const hash = (path: string) =>
      createHash("sha256")
        .update(readFileSync(resolve(dist, path.replace(/^\//, ""))))
        .digest("hex");

    expect(manifest.source_digest).toBe(frontendSourceDigest());
    expect(manifest.index.path).toBe("/index.html");
    const provenanceMeta = /(<meta name="rust-jav-(?:source-digest|asset-manifest|index-sha256|app-js-sha256|app-css-sha256)" content=")[^"]*(")/g;
    const matches = [...shell.matchAll(provenanceMeta)];
    expect(matches).toHaveLength(5);
    const normalizedShell = shell.replace(provenanceMeta, "$1$2");
    expect(manifest.index.normalized_sha256).toBe(
      createHash("sha256").update(normalizedShell).digest("hex"),
    );
    expect(manifest.assets.javascript.path).toBe("/assets/app.js");
    expect(manifest.assets.stylesheet.path).toBe("/assets/app.css");
    expect(manifest.assets.javascript.sha256).toBe(hash("/assets/app.js"));
    expect(manifest.assets.stylesheet.sha256).toBe(hash("/assets/app.css"));
    expect(shell).toContain(`<meta name="rust-jav-source-digest" content="${manifest.source_digest}"`);
    expect(shell).toContain('<meta name="rust-jav-asset-manifest" content="/assets/asset-manifest.json"');
    expect(shell).toContain(`<meta name="rust-jav-index-sha256" content="${manifest.index.normalized_sha256}"`);
    expect(shell).toContain(`<meta name="rust-jav-app-js-sha256" content="${manifest.assets.javascript.sha256}"`);
    expect(shell).toContain(`<meta name="rust-jav-app-css-sha256" content="${manifest.assets.stylesheet.sha256}"`);
    expect(shell).toContain(`src="${manifest.assets.javascript.path}"`);
    expect(shell).toContain(`href="${manifest.assets.stylesheet.path}"`);
    expect(shell.match(new RegExp(`src="${manifest.assets.javascript.path}"`, "g"))).toHaveLength(1);
    expect(shell.match(new RegExp(`href="${manifest.assets.stylesheet.path}"`, "g"))).toHaveLength(1);
  });
});
