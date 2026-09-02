import { defineConfig } from "vitest/config";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { loadEnv, type Plugin } from "vite";
import { frontendSourceDigest } from "./source-digest.ts";

function frontendProvenancePlugin(): Plugin {
  const javascriptPath = "/assets/app.js";
  const stylesheetPath = "/assets/app.css";
  const javascriptPlaceholder = "__RUST_JAV_APP_JS_SHA256__";
  const stylesheetPlaceholder = "__RUST_JAV_APP_CSS_SHA256__";
  const indexPlaceholder = "__RUST_JAV_INDEX_SHA256__";
  let javascriptHash = javascriptPlaceholder;
  let stylesheetHash = stylesheetPlaceholder;
  let outputDirectory = resolve(process.cwd(), "dist");
  return {
    name: "rust-jav-frontend-provenance",
    configResolved(config) {
      outputDirectory = resolve(config.root, config.build.outDir);
    },
    transformIndexHtml(html: string) {
      return html.replace(
        "<head>",
        `<head>
    <meta name="rust-jav-source-digest" content="${frontendSourceDigest()}" />
    <meta name="rust-jav-asset-manifest" content="/assets/asset-manifest.json" />
    <meta name="rust-jav-index-sha256" content="${indexPlaceholder}" />
    <meta name="rust-jav-app-js-sha256" content="${javascriptHash}" />
    <meta name="rust-jav-app-css-sha256" content="${stylesheetHash}" />`,
      );
    },
    generateBundle: {
      order: "post" as const,
      handler(_options, bundle) {
        const javascript = bundle[javascriptPath.slice(1)];
        const stylesheet = bundle[stylesheetPath.slice(1)];
        if (javascript?.type !== "chunk" || stylesheet?.type !== "asset") {
          throw new Error("production frontend output is missing app.js or app.css");
        }
        javascriptHash = createHash("sha256").update(javascript.code).digest("hex");
        stylesheetHash = createHash("sha256").update(stylesheet.source).digest("hex");
        const sourceDigest = frontendSourceDigest();
        const manifest = {
          version: 1,
          source_digest: sourceDigest,
          index: { path: "/index.html", normalized_sha256: indexPlaceholder },
          assets: {
            javascript: { path: javascriptPath, sha256: javascriptHash },
            stylesheet: { path: stylesheetPath, sha256: stylesheetHash },
          },
        };
        this.emitFile({
          type: "asset",
          fileName: "assets/asset-manifest.json",
          source: `${JSON.stringify(manifest, null, 2)}\n`,
        });
      },
    },
    writeBundle() {
      const shellPath = resolve(outputDirectory, "index.html");
      let shell = readFileSync(shellPath, "utf8")
        .replace(javascriptPlaceholder, javascriptHash)
        .replace(stylesheetPlaceholder, stylesheetHash);
      const provenanceMeta = /(<meta name="rust-jav-(?:source-digest|asset-manifest|index-sha256|app-js-sha256|app-css-sha256)" content=")[^"]*(")/g;
      const normalizedShell = shell.replace(provenanceMeta, "$1$2");
      const normalizedIndexHash = createHash("sha256").update(normalizedShell).digest("hex");
      shell = shell.replace(indexPlaceholder, normalizedIndexHash);
      writeFileSync(shellPath, shell);
      const manifestPath = resolve(outputDirectory, "assets/asset-manifest.json");
      const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
      manifest.index.normalized_sha256 = normalizedIndexHash;
      writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
    },
  };
}

export default defineConfig(({ command, mode, isPreview }) => {
  const backendOrigin =
    process.env.VITE_BACKEND_ORIGIN ??
    loadEnv(mode, process.cwd(), "").VITE_BACKEND_ORIGIN;
  const proxy =
    command === "serve" && !isPreview && backendOrigin
      ? {
          "/api": { target: backendOrigin, changeOrigin: true },
          "/health": { target: backendOrigin, changeOrigin: true },
        }
      : undefined;

  return {
    plugins: [frontendProvenancePlugin(), react(), tailwindcss()],
    resolve: { alias: { "@": new URL("./src", import.meta.url).pathname } },
    server: { proxy },
    test: { environment: "jsdom", setupFiles: ["./src/test-setup.ts"] },
    build: {
      rollupOptions: {
        output: {
          entryFileNames: "assets/app.js",
          assetFileNames: "assets/app.[ext]",
        },
      },
    },
  };
});
