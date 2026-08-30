import { afterEach, describe, expect, it, vi } from "vitest";

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
