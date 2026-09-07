import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const source = (path: string) => readFileSync(join(process.cwd(), "src", path), "utf8");
const main = source("main.tsx");
const gallery = source("features/assets/media-asset-gallery.tsx");
const inspector = source("features/assets/inspector-details.tsx");

describe("Issues #49 and #51 registry migration", () => {
  it("composes the Gallery from registry controls instead of page-local controls", () => {
    for (const primitive of ["Button", "Card", "Input", "Badge", "Tabs"]) {
      expect(gallery).toContain(`{ ${primitive}`);
    }
    expect(main).toContain("<MediaAssetGallery");
    expect(main).not.toContain('from "./beui-tabs"');
  });

  it("uses the registry Sheet and shares Inspector data presentation", () => {
    expect(main).toContain("<Sheet open");
    expect(main).toContain("<Tabs defaultValue=\"overview\"");
    expect(inspector).toContain("export function DetailSection");
    expect(inspector).toContain("export function DataList");
    expect(inspector).toContain("export function InspectorStatus");
  });
});
