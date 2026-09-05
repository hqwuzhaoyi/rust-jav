import { afterEach, describe, expect, it } from "vitest";
import {
  installProductionStyles,
  productionCss,
  productionStyle,
  productionValue,
} from "./test-css";

function control(className: string, parentClass?: string) {
  const parent = document.createElement("div");
  if (parentClass) parent.className = parentClass;
  const button = document.createElement("button");
  button.className = className;
  parent.append(button);
  document.body.append(parent);
  return button;
}

afterEach(() => {
  document.body.replaceChildren();
});

describe("Issue #47 explicit touch target contracts", () => {
  it("does not size every button or every Toast button through a global descendant rule", () => {
    installProductionStyles();
    expect(productionCss).not.toMatch(
      /(?:^|\})\s*button\s*\{[^}]*min-height\s*:\s*44px/,
    );
    expect(productionCss).not.toMatch(/\bbutton\s*\{\s*touch-action\s*:/);
    expect(productionCss).not.toMatch(/\.ui-toast-stack\s+button\s*\{/);
    expect(productionCss).not.toMatch(/@(import|source|theme)\b/);
  });

  it("keeps intentional compact and 44px controls explicit after global coupling is removed", () => {
    const touchTarget = control("ui-touch-target");
    const filter = control("ui-touch-target", "beui-tabs-list");
    filter.parentElement?.setAttribute("data-variant", "segment");
    const actorMenu = control("actor-action-menu-trigger ui-touch-target ui-icon-button");
    const pagination = control("", "pagination");
    const tabs = control("", "beui-tabs-list");
    tabs.parentElement?.setAttribute("data-variant", "underline");
    const bottomNav = control("ui-touch-target", "bottom-nav");

    expect(productionValue(touchTarget, "min-height")).toBe("44px");
    expect(productionStyle(filter).minHeight).toBe("38px");
    expect(productionValue(actorMenu, "min-height")).toBe("44px");
    expect(productionStyle(pagination).minHeight).toBe("44px");
    expect(productionValue(tabs, "min-height")).toBe("44px");
    expect(productionValue(bottomNav, "min-height")).toBe("44px");
  });

  it("keeps the compact Storage modal close control square, circular, and non-shrinking", () => {
    const close = control("storage-dialog-close");
    const icon = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    close.append(icon);

    const closeStyle = productionStyle(close);
    expect([
      closeStyle.width,
      closeStyle.height,
      closeStyle.minWidth,
      closeStyle.minHeight,
      closeStyle.borderRadius,
      closeStyle.flex,
    ]).toEqual(["36px", "36px", "36px", "36px", "50%", "0 0 36px"]);
    const iconStyle = productionStyle(icon);
    expect([iconStyle.width, iconStyle.height, iconStyle.flexShrink]).toEqual([
      "16px",
      "16px",
      "0",
    ]);
  });
});
