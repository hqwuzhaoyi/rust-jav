export type DeletionPlanPath = {
  path: string;
  type: string;
  filesystem_identity?: { device: number; inode: number };
  logical_size?: number;
  allocated_size?: number;
  observed_link_count?: number;
  video_warning?: string | null;
};

export type ActorFolderDeletionImpact = {
  asset_id: string;
  metadata_actors: string[];
  affected_actor_folders: string[];
  other_actor_folders: string[];
  requires_multi_actor_confirmation: boolean;
};

/** A plan from the Actor Folder entry point always includes every discovered link. */
export type UnifiedDeletionPlan = {
  id: string;
  selection: "unified";
  logical_size: number;
  reclaimable_space: number;
  created_at: number;
  expires_at: number;
  hard_link_search_roots: string[];
  paths: DeletionPlanPath[];
  discovered_hard_links: DeletionPlanPath[];
  origin: {
    type: "actor_folder";
    actor_folder: string;
    selected_asset_ids: string[];
  };
  actor_folder_impacts: ActorFolderDeletionImpact[];
};

export type CreateActorDeletionPlanRequest = {
  asset_ids: string[];
  confirmed_multi_actor_asset_ids?: string[];
};

export type MultiActorConfirmationRequired = {
  status: "multi_actor_confirmation_required";
  error: string;
  unconfirmed_multi_actor_asset_ids: string[];
  actor_folder_impacts: ActorFolderDeletionImpact[];
};

export type ActorDeletionPlanConflict = {
  status: "conflict" | "error";
  http_status: number;
  error: string;
};

export type ActorDeletionPlanResult =
  | { status: "created"; plan: UnifiedDeletionPlan }
  | MultiActorConfirmationRequired
  | ActorDeletionPlanConflict;

export type ExecuteDeletionPlanRequest = {
  irreversible: true;
  confirmation: string;
};

export type ExecuteDeletionPlanResult = {
  http_status: number;
  body: unknown;
};

type Fetch = typeof fetch;

export function actorDeletionPlanPath(actorName: string) {
  return `/api/v1/actors/${encodeURIComponent(actorName)}/permanent-deletion-plans`;
}

export function deletionPlanExecutionPayload(
  confirmation: string,
): ExecuteDeletionPlanRequest {
  return { irreversible: true, confirmation };
}

export async function createActorDeletionPlan(
  actorName: string,
  request: CreateActorDeletionPlanRequest,
  fetcher: Fetch = fetch,
): Promise<ActorDeletionPlanResult> {
  const response = await fetcher(actorDeletionPlanPath(actorName), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    // The endpoint intentionally accepts Asset Index ids only. Browser paths
    // are never part of this contract.
    body: JSON.stringify({
      asset_ids: request.asset_ids,
      ...(request.confirmed_multi_actor_asset_ids
        ? {
            confirmed_multi_actor_asset_ids:
              request.confirmed_multi_actor_asset_ids,
          }
        : {}),
    }),
  });
  const body = await jsonOrText(response);
  if (response.status === 409 && isMultiActorConfirmation(body)) {
    return {
      status: "multi_actor_confirmation_required",
      error: body.error,
      unconfirmed_multi_actor_asset_ids:
        body.unconfirmed_multi_actor_asset_ids,
      actor_folder_impacts: body.actor_folder_impacts,
    };
  }
  if (!response.ok) {
    return {
      status: response.status === 409 ? "conflict" : "error",
      http_status: response.status,
      error: errorMessage(body),
    };
  }
  return { status: "created", plan: body as UnifiedDeletionPlan };
}

export async function executeDeletionPlan(
  planId: string,
  confirmation: string,
  fetcher: Fetch = fetch,
): Promise<ExecuteDeletionPlanResult> {
  const response = await fetcher(
    `/api/v1/deletion-plans/${encodeURIComponent(planId)}/execute`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(deletionPlanExecutionPayload(confirmation)),
    },
  );
  return { http_status: response.status, body: await jsonOrText(response) };
}

async function jsonOrText(response: Response): Promise<unknown> {
  const text = await response.text();
  if (!text) return null;
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return text;
  }
}

function isMultiActorConfirmation(
  value: unknown,
): value is Omit<MultiActorConfirmationRequired, "status"> {
  if (!value || typeof value !== "object") return false;
  const response = value as Record<string, unknown>;
  return (
    typeof response.error === "string" &&
    Array.isArray(response.unconfirmed_multi_actor_asset_ids) &&
    Array.isArray(response.actor_folder_impacts)
  );
}

function errorMessage(value: unknown) {
  if (typeof value === "string") return value;
  if (value && typeof value === "object" && typeof (value as { error?: unknown }).error === "string")
    return (value as { error: string }).error;
  return "Unable to create a permanent-deletion Operation Plan.";
}
