import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const styleCss = readFileSync(resolve(process.cwd(), "src/style.css"), "utf8");
const designSystemCss = readFileSync(resolve(process.cwd(), "src/design-system.css"), "utf8");

function unlayered(source: string) {
  return source
    .replace(/^@(import|source)\s+[^;]+;\s*$/gm, "")
    .replace(/@theme\s+inline\s*\{[^{}]*\}/g, "");
}

export const productionCss = [styleCss, designSystemCss]
  .map(unlayered)
  .join("\n");

export function installProductionStyles() {
  let style = document.querySelector<HTMLStyleElement>("style[data-production-css]");
  if (!style) {
    style = document.createElement("style");
    style.dataset.productionCss = "true";
    style.textContent = productionCss;
    document.head.append(style);
  }
  return style;
}

export function productionStyle(element: Element) {
  installProductionStyles();
  return getComputedStyle(element);
}

export function productionValue(element: Element, property: string) {
  const style = productionStyle(element);
  let value = style.getPropertyValue(property).trim();
  for (let depth = 0; depth < 4 && value.includes("var("); depth += 1) {
    value = value.replace(
      /var\((--[\w-]+)(?:,\s*([^)]+))?\)/g,
      (_match, name: string, fallback: string | undefined) =>
        style.getPropertyValue(name).trim() ||
        getComputedStyle(document.documentElement).getPropertyValue(name).trim() ||
        fallback?.trim() ||
        "",
    );
  }
  return value;
}
