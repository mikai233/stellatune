export interface TrackPresentation {
  title?: string | null; artist?: string | null; album?: string | null;
  durationMs?: number | null;
  cover?: { kind: "url" | "file" | "data"; value: string; mime?: string | null } | null;
}
export interface ProviderTrack { pluginId: string; capabilityId: string; catalogCapabilityId?: string; providerId: string; providerKey: string; metadata?: TrackPresentation }
export type PlayerCommand =
  | { command: "play" | "pause" | "stop" | "next" | "previous" }
  | { command: "seek"; positionMs: number }
  | { command: "playTrack"; trackId: string }
  | { command: "appendQueue" | "replaceQueue"; trackIds: string[] }
  | { command: "removeQueueItems"; itemIds: string[] }
  | { command: "selectItem"; itemId: string }
  | { command: "setQueueMode"; repeat: "off" | "all" | "one"; shuffle: boolean }
  | { command: "playProviderTrack" | "enqueueProviderTrack"; track: ProviderTrack };
export interface PlayerState {
  state: "idle" | "preparing" | "recovering" | "ready" | "playing" | "paused" | "buffering" | "failed";
  itemId: string | null; trackId: string | null; positionMs: number; durationMs: number | null;
}
export interface PlayerQueue {
  items: { itemId: string; trackId: string; providerTrack?: ProviderTrack | null; metadata?: TrackPresentation | null }[]; order: string[];
  currentItemId: string | null; requestedItemId: string | null;
  repeat: "off" | "all" | "one"; shuffle: boolean; revision: string;
}
export type PlayerEvent =
  | { type: "snapshot"; state: PlayerState; queue: PlayerQueue }
  | { type: "queueChanged"; queue: PlayerQueue }
  | { type: "stateChanged"; state: PlayerState["state"] }
  | { type: "trackChanged" | "playbackEnded"; itemId: string }
  | { type: "position"; itemId: string; positionMs: number }
  | { type: "buffering"; itemId: string; active: boolean }
  | { type: "failed"; message: string };
export interface HostClient {
  command(command: PlayerCommand): Promise<Record<string, unknown>>;
  getState(signal?: AbortSignal): Promise<PlayerState>;
  getQueue(signal?: AbortSignal): Promise<PlayerQueue>;
  subscribe(onEvent: (event: PlayerEvent) => void, onError?: (error: Error) => void): () => void;
  play(): Promise<Record<string, unknown>>;
  pause(): Promise<Record<string, unknown>>;
  stop(): Promise<Record<string, unknown>>;
  seek(positionMs: number): Promise<Record<string, unknown>>;
}
export function createHostClient(baseUrl: string): HostClient;
