export type AssetState = "normal" | "synchronizing" | "exception";

export type ArtworkProvenance = {
  status: "missing" | "valid" | "empty" | "unrecognized" | "animated" | "truncated_or_corrupt" | "too_large" | "unreadable";
  source_path: string | null;
  content_type: "image/jpeg" | "image/png" | "image/webp" | null;
  error: string | null;
};

export type MediaAsset = {
  id: string;
  path: string;
  jav_code: string | null;
  title: string | null;
  artwork_url: string | null;
  captured_date: string;
  state: AssetState;
  exception: string | null;
};

export type JellyfinAssociation = {
  status: "played" | "in_progress" | "unplayed" | "not_found" | "offline" | "not_configured";
  confidence?: "certain_path" | "uncertain_metadata";
  reason?: string;
  play_count?: number;
  playback_position_ticks?: number;
  open_url?: string;
  may_authorize_deletion?: boolean;
};

export type MediaAssetDetail = MediaAsset & {
  actors: Array<{ name: string; poster_url: string | null; actor_folder_url: string | null }>;
  studio: string | null;
  release_date: string | null;
  runtime_minutes: number | null;
  director: string | null;
  tags: string[];
  plot: string | null;
  parse_status: "valid" | "missing" | "invalid";
  source_path: string | null;
  jellyfin?: JellyfinAssociation;
  artwork?: ArtworkProvenance;
};

export type MediaAssetPage = {
  items: MediaAsset[];
  groups: Array<{ date: string; count: number }>;
  page: number;
  total: number;
  total_pages: number;
};
