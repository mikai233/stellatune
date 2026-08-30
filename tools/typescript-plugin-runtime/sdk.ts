export const PROTOCOL = "stellatune-capability-rpc/1" as const;

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

export interface StellatunePlugin {
  descriptor: {
    id: string;
    apiVersion: 2;
    capabilities: readonly string[];
  };
  initialize?(context: unknown): Promise<unknown> | unknown;
  invoke(request: CapabilityInvokeRequest): Promise<unknown> | unknown;
  shutdown?(): Promise<void> | void;
}

export function definePlugin(plugin: StellatunePlugin): StellatunePlugin {
  return plugin;
}
