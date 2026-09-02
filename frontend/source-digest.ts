import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const frontendRoot = dirname(fileURLToPath(import.meta.url));
const fixedProductionInputs = [
  "index.html",
  "package-lock.json",
  "package.json",
  "source-digest.ts",
  "tsconfig.json",
  "vite.config.ts",
];

function sourceInputs(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return sourceInputs(path);
    if (entry.name.includes(".test.") || entry.name === "test-setup.ts" || entry.name === "test-css.ts") {
      return [];
    }
    return /\.(css|ts|tsx)$/.test(entry.name) ? [relative(frontendRoot, path)] : [];
  });
}

export function frontendProductionInputs() {
  return [...fixedProductionInputs, ...sourceInputs(join(frontendRoot, "src"))]
    .map((path) => path.replaceAll("\\", "/"))
    .sort();
}

export function frontendSourceDigest() {
  const digest = createHash("sha256");
  for (const path of frontendProductionInputs()) {
    digest.update(path);
    digest.update("\0");
    digest.update(readFileSync(join(frontendRoot, path)));
    digest.update("\0");
  }
  return digest.digest("hex");
}
