import type { ActionInvokeResponse, PluginConfigResponse, PluginUiEvent } from "./types";

export interface PluginUiContext { pluginId: string; origin: string }
export function resolvePluginUiContext(): PluginUiContext {
  return { pluginId: "dev.stellatune.source.netease", origin: window.location.origin };
}
export async function getPluginConfig(ctx: PluginUiContext): Promise<PluginConfigResponse> {
  return requestJson(ctx, "GET", "/api/config");
}
export async function getPlayerState(ctx: PluginUiContext): Promise<Record<string, unknown>> {
  return requestJson(ctx, "GET", "/api/player/state");
}
export async function putPluginConfig(ctx: PluginUiContext, config: unknown): Promise<PluginConfigResponse> {
  return requestJson(ctx, "PUT", "/api/config", config);
}
export async function invokePluginAction(ctx: PluginUiContext, action: string, payload: Record<string, unknown> = {}): Promise<ActionInvokeResponse> {
  return requestJson(ctx, "POST", `/api/actions/${encodeURIComponent(action)}`, payload);
}
export function subscribePluginEvents(ctx: PluginUiContext, onEvent: (event: PluginUiEvent) => void, onError?: (error: Event) => void): () => void {
  const stream = new EventSource(new URL("/api/events", ctx.origin));
  stream.addEventListener("event", raw => {
    try { onEvent(JSON.parse((raw as MessageEvent<string>).data) as PluginUiEvent); }
    catch { /* A malformed event must not stop reconnects. */ }
  });
  if (onError) stream.onerror = onError;
  return () => stream.close();
}
async function requestJson<T>(ctx: PluginUiContext, method: string, path: string, body?: unknown): Promise<T> {
  const response = await fetch(new URL(path, ctx.origin), { method, headers: { "content-type": "application/json" }, body: body === undefined ? undefined : JSON.stringify(body) });
  const result = await response.json();
  if (!response.ok) throw new Error(result.message ?? `${response.status} ${response.statusText}`);
  return result as T;
}
