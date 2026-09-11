export const PROTOCOL = "stellatune-capability-rpc/1" as const;
export { createHostClient } from "./host-client.mjs";
export type { PlayerCommand, PlayerState, PlayerQueue, PlayerEvent, HostClient } from "./host-client.mjs";

export interface PluginContext {
  pluginId: string;
  generation: number;
  hostApiBaseUrl: string;
  dataDir: string;
  packageRoot: string;
}

export interface PluginErrorShape {
  code: string;
  message: string;
  retryable: boolean;
  details?: unknown;
}

export interface CapabilityInvokeRequest {
  capabilityId: string;
  instanceId?: string;
  operation: string;
  input: unknown;
}

export interface SourcePlan {
  source:
    | { kind: "file"; path: string }
    | { kind: "http"; url: string; headers?: Record<string, string> };
  media?: { mimeType?: string; codecHint?: string };
  capabilities: { seekable: boolean; durationMs?: number };
  requirements?: { decoderCapabilityId?: string };
}

/** Input for a source resolver declaring manifest local_extensions. */
export interface LocalFileRequest { path: string }
/** inspect-file response; resolve-file returns a file or HTTP SourcePlan. */
export interface LocalFileMetadata {
  title?: string | null;
  artist?: string | null;
  album?: string | null;
  albumArtist?: string | null;
  discNumber?: number | null;
  trackNumber?: number | null;
  artists?: string[];
  durationMs?: number | null;
  /** Optional artwork URL; host downloads up to 12 MiB into its cover cache. */
  coverUrl?: string | null;
}

export type MediaKind = "track" | "album" | "artist" | "folder" | "playlist";
export type CatalogSort = "default" | "title";
export interface MediaRef { sourceInstanceId: string; kind: MediaKind; id: string }
export interface CatalogItem {
  reference: MediaRef; title: string; artist?: string | null; album?: string | null;
  durationMs?: number | null; trackCount?: number | null; artworkUrl?: string | null;
  albumRef?: MediaRef | null; artistRefs?: MediaRef[];
}
export interface CatalogPage { items: CatalogItem[]; nextCursor: string | null; total?: number | null }
export interface CatalogQuery {
  sourceInstanceId: string; kind: MediaKind; parent?: MediaRef | null;
  search: string; sort: CatalogSort; cursor?: string | null; limit: number;
}
export interface LibraryInstance {
  instanceId: string; name: string; resolverCapabilityId: string;
  browseKinds: MediaKind[]; searchKinds: MediaKind[]; sorts: CatalogSort[]; error?: string | null;
}
export interface PluginLibraries { protocolVersion: 1; instances: LibraryInstance[] }
/** media-library: list-sources -> PluginLibraries; browse -> CatalogPage;
 * get-detail -> CatalogItem. Browse, detail and resolve receive instanceId.
 * resolve receives {trackId: string}. Never parse provider IDs as numbers.
 */

export interface StellatunePlugin {
  descriptor: {
    id: string;
    apiVersion: 2;
    capabilities: readonly string[];
  };
  initialize?(context: PluginContext): Promise<unknown> | unknown;
  openUi?(): Promise<{ url: string }>;
  invoke(request: CapabilityInvokeRequest): Promise<unknown> | unknown;
  shutdown?(): Promise<void> | void;
}

export function definePlugin(plugin: StellatunePlugin): StellatunePlugin {
  return plugin;
}
