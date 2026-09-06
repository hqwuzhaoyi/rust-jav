import { describe, expect, it, vi } from "vitest";
import {
  createActorDeletionPlan,
  deletionPlanExecutionPayload,
  executeDeletionPlan,
} from "./actor-deletion-api";

const impact = {
  asset_id: "asset-55",
  metadata_actors: ["Alice", "Beatrice"],
  affected_actor_folders: ["Alice", "Beatrice"],
  other_actor_folders: ["Beatrice"],
  requires_multi_actor_confirmation: true,
};

function fetchResponse(response: Response) {
  return vi.fn<typeof fetch>().mockResolvedValue(response);
}

describe("Actor Folder permanent-deletion API contract", () => {
  it("sends only selected Asset Index ids and per-asset confirmation ids, never browser paths", async () => {
    const fetcher = fetchResponse(
      Response.json(
        {
          id: "actor-plan-55",
          selection: "unified",
          logical_size: 123,
          reclaimable_space: 123,
          created_at: 100,
          expires_at: 1000,
          hard_link_search_roots: ["/media", "/actors"],
          paths: [],
          discovered_hard_links: [],
          origin: {
            type: "actor_folder",
            actor_folder: "Alice / 雪",
            selected_asset_ids: ["asset-55"],
          },
          actor_folder_impacts: [impact],
        },
        { status: 201 },
      ),
    );

    const result = await createActorDeletionPlan(
      "Alice / 雪",
      {
        asset_ids: ["asset-55"],
        confirmed_multi_actor_asset_ids: ["asset-55"],
      },
      fetcher,
    );

    expect(result).toMatchObject({ status: "created", plan: { selection: "unified" } });
    expect(fetcher).toHaveBeenCalledWith(
      "/api/v1/actors/Alice%20%2F%20%E9%9B%AA/permanent-deletion-plans",
      expect.objectContaining({ method: "POST" }),
    );
    const request = fetcher.mock.calls[0]?.[1] as RequestInit;
    expect(JSON.parse(request.body as string)).toEqual({
      asset_ids: ["asset-55"],
      confirmed_multi_actor_asset_ids: ["asset-55"],
    });
    expect(request.body).not.toContain("/media/");
    expect(JSON.parse(request.body as string)).not.toHaveProperty("paths");
  });

  it("preserves the server's 409 per-asset multi-actor impact response", async () => {
    const result = await createActorDeletionPlan(
      "Alice",
      { asset_ids: ["asset-55"] },
      fetchResponse(
        Response.json(
          {
            error: "every multi-actor Media Asset requires its own confirmation before permanent deletion",
            unconfirmed_multi_actor_asset_ids: ["asset-55"],
            actor_folder_impacts: [impact],
          },
          { status: 409 },
        ),
      ),
    );

    expect(result).toEqual({
      status: "multi_actor_confirmation_required",
      error: "every multi-actor Media Asset requires its own confirmation before permanent deletion",
      unconfirmed_multi_actor_asset_ids: ["asset-55"],
      actor_folder_impacts: [impact],
    });
  });

  it("reuses the permanent-deletion execute payload without exposing source paths", async () => {
    expect(deletionPlanExecutionPayload("PERMANENTLY DELETE")).toEqual({
      irreversible: true,
      confirmation: "PERMANENTLY DELETE",
    });
    const fetcher = fetchResponse(Response.json({ id: "task-55" }, { status: 202 }));
    await executeDeletionPlan("actor-plan-55", "PERMANENTLY DELETE", fetcher);

    expect(fetcher).toHaveBeenCalledWith(
      "/api/v1/deletion-plans/actor-plan-55/execute",
      expect.objectContaining({ method: "POST" }),
    );
    const request = fetcher.mock.calls[0]?.[1] as RequestInit;
    expect(JSON.parse(request.body as string)).toEqual({
      irreversible: true,
      confirmation: "PERMANENTLY DELETE",
    });
    expect(JSON.parse(request.body as string)).not.toHaveProperty("paths");
  });
});
