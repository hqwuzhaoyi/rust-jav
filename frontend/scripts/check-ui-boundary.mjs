#!/usr/bin/env node
/**
 * Prevent feature code from silently reintroducing generic interaction
 * primitives. This deliberately uses the TypeScript parser rather than regex
 * so JSX aliases, self-closing elements, and role attributes are audited.
 */
import path from "node:path";
import process from "node:process";
import { API } from "typescript/unstable/sync";
import * as ts from "typescript/unstable/ast";

const frontendRoot = process.cwd();
const sourceRoot = path.join(frontendRoot, "src");
const RAW_INTERACTIVE = new Set(["button", "input", "select", "textarea"]);
const GENERIC_ROLES = new Set([
  "alertdialog", "button", "checkbox", "combobox", "dialog", "menu",
  "menuitem", "menuitemcheckbox", "menuitemradio", "option", "progressbar",
  "radio", "separator", "switch", "tab", "tablist", "tabpanel",
]);

function jsxTagName(node) {
  return ts.isIdentifier(node.tagName)
    ? node.tagName.text
    : node.tagName.getText();
}

function literalAttribute(node, name) {
  const attribute = node.attributes.properties.find(
    (property) => ts.isJsxAttribute(property) && property.name.text === name,
  );
  if (!attribute || !ts.isJsxAttribute(attribute)) return undefined;
  if (attribute.initializer && ts.isStringLiteral(attribute.initializer)) return attribute.initializer.text;
  return attribute.initializer ? "<dynamic>" : "";
}

export function collectFindings(root = frontendRoot) {
  const findings = [];
  const api = new API();
  const snapshot = api.updateSnapshot({ openProjects: [path.join(root, "tsconfig.json")] });
  const project = snapshot.getProjects().find(
    (candidate) => path.resolve(candidate.configFileName) === path.join(root, "tsconfig.json"),
  );
  if (!project) throw new Error("UI boundary lint could not load frontend/tsconfig.json");
  for (const file of project.rootFiles) {
    if (!file.endsWith(".tsx") || file.endsWith(".test.tsx")) continue;
    const relative = path.relative(root, file).split(path.sep).join("/");
    // Registry source owns the generic primitive implementation.
    if (
      relative.startsWith("src/components/ui/") ||
      relative.startsWith("src/components/motion/")
    ) continue;
    const source = project.program.getSourceFile(file);
    if (!source) throw new Error(`UI boundary lint could not parse ${relative}`);
    const visit = (node) => {
      if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
        const tag = jsxTagName(node);
        if (RAW_INTERACTIVE.has(tag) || tag === "motion.button") {
          const position = source.getLineAndCharacterOfPosition(node.getStart(source));
          findings.push({ file: relative, key: `raw:${tag}`, line: position.line + 1, column: position.character + 1 });
        }
        const role = literalAttribute(node, "role");
        // A computed role is also rejected: its runtime value cannot be proven
        // to be a semantic-content exception by this static boundary check.
        if (role && (role === "<dynamic>" || GENERIC_ROLES.has(role))) {
          const position = source.getLineAndCharacterOfPosition(node.getStart(source));
          findings.push({ file: relative, key: `role:${role}`, line: position.line + 1, column: position.character + 1 });
        }
      }
      node.forEachChild(visit);
    };
    visit(source);
  }
  snapshot.dispose();
  api.close();
  return findings;
}

export function auditUiBoundary(root = frontendRoot) {
  const findings = collectFindings(root);
  return { findings, violations: findings };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const { findings, violations } = auditUiBoundary();
  if (violations.length) {
    console.error("UI boundary violations: generic controls and interaction roles belong in src/components/ui.");
    for (const violation of violations) {
      console.error(`  ${violation.file}:${violation.line}:${violation.column} ${violation.key}`);
    }
    process.exitCode = 1;
  } else {
    console.log("UI boundary check passed (zero application-level generic control violations).");
  }
}
