import { describe, expect, it } from "vitest";
import { applicationCss, foundationCss } from "./test-css";

type SelectorOccurrence = { context: string; selector: string };

function selectors(source: string) {
  const style = document.createElement("style");
  style.textContent = source;
  document.head.append(style);
  const occurrences: SelectorOccurrence[] = [];
  const visit = (rules: CSSRuleList, context = "root") => {
    for (const rule of rules) {
      if (rule instanceof CSSStyleRule) {
        for (const selector of rule.selectorText.split(",")) {
          occurrences.push({ context, selector: selector.trim() });
        }
      } else if ("cssRules" in rule) {
        const condition = "conditionText" in rule ? String(rule.conditionText) : rule.cssText;
        visit((rule as CSSGroupingRule).cssRules, `${context} / ${condition}`);
      }
    }
  };
  visit(style.sheet!.cssRules);
  style.remove();
  return occurrences;
}

describe("Issue #44 canonical CSS ownership", () => {
  it("does not define a selector in both the foundation and application layers", () => {
    const foundation = new Set(
      selectors(foundationCss).map(({ context, selector }) => `${context} :: ${selector}`),
    );
    const overlaps = selectors(applicationCss)
      .map(({ context, selector }) => `${context} :: ${selector}`)
      .filter((selector) => foundation.has(selector));

    expect(overlaps).toEqual([]);
  });

  it("defines every selector once per cascade context", () => {
    const counts = new Map<string, number>();
    for (const { context, selector } of [
      ...selectors(foundationCss),
      ...selectors(applicationCss),
    ]) {
      const key = `${context} :: ${selector}`;
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    const duplicates = [...counts]
      .filter(([, count]) => count > 1)
      .map(([selector, count]) => `${selector} (${count})`)
      .sort();

    expect(duplicates).toEqual([]);
  });
});
