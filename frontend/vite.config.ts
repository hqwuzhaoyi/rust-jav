import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { loadEnv } from "vite";

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
    plugins: [react(), tailwindcss()],
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
