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
  durationMs?: number | null;
}

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
