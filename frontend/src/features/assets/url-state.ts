import type { AssetState } from "./model";

const states = new Set<AssetState>(["normal", "synchronizing", "exception"]);

export function galleryStateFromUrl(search = location.search) {
  const params = new URLSearchParams(search);
  const state = params.get("state") as AssetState | null;
  const page = Number(params.get("page") ?? "1");
  return {
    query: params.get("q") ?? "",
    filter: state && states.has(state) ? state : "" as const,
    page: Number.isInteger(page) && page > 0 ? page : 1,
  };
}

export function galleryUrl(query: string, filter: AssetState | "", page: number, assetId?: string | null) {
  const params = new URLSearchParams({ page: String(page), per_page: "48" });
  if (query) params.set("q", query);
  if (filter) params.set("state", filter);
  return `${assetId ? `/assets/${encodeURIComponent(assetId)}` : "/"}?${params}`;
}
