import type {
  ActionInvokeResponse,
  PluginConfigResponse,
  PluginUiEvent
} from "./types";

const TOKEN_HEADER = "x-stellatune-plugin-ui-token";
const SOURCE_KIND = "source";
const SOURCE_TYPE_ID = "netease";

export interface GatewayContext {
  pluginId: string;
  origin: string;
  token: string;
}

export function resolveGatewayContext(): GatewayContext {
  const query = new URLSearchParams(window.location.search);
  const pluginId = resolvePluginId(window.location.pathname, query);
  const token = resolveToken(query);
  const origin = resolveGatewayOrigin(query);
  return {
    pluginId,
    origin,
    token
  };
}

export function createNeteaseConfigRoot(config: unknown): Record<string, unknown> {
  return {
    [SOURCE_KIND]: {
      [SOURCE_TYPE_ID]: config
    }
  };
}

export function extractNeteaseConfig(root: Record<string, unknown>): unknown | null {
  const byKind = root[SOURCE_KIND];
  if (typeof byKind !== "object" || byKind === null) {
    return null;
  }
  const byKindRecord = byKind as Record<string, unknown>;
  return byKindRecord[SOURCE_TYPE_ID] ?? null;
}

export async function getPluginConfig(ctx: GatewayContext): Promise<PluginConfigResponse> {
  return await requestJson<PluginConfigResponse>(
    ctx,
    "GET",
    `/api/plugins/${encodeURIComponent(ctx.pluginId)}/config`
  );
}

export async function putPluginConfig(
  ctx: GatewayContext,
  configRoot: Record<string, unknown>
): Promise<PluginConfigResponse> {
  return await requestJson<PluginConfigResponse>(
    ctx,
    "PUT",
    `/api/plugins/${encodeURIComponent(ctx.pluginId)}/config`,
    configRoot
  );
}

export async function invokePluginAction(
  ctx: GatewayContext,
  action: string,
  payload: Record<string, unknown> = {}
): Promise<ActionInvokeResponse> {
  return await requestJson<ActionInvokeResponse>(
    ctx,
    "POST",
    `/api/plugins/${encodeURIComponent(ctx.pluginId)}/actions/${encodeURIComponent(action)}`,
    payload
  );
}

export function subscribePluginEvents(
  ctx: GatewayContext,
  onEvent: (event: PluginUiEvent) => void,
  onError?: (error: Event) => void
): () => void {
  const endpoint = new URL(
    `/api/plugins/${encodeURIComponent(ctx.pluginId)}/events`,
    ctx.origin
  );
  if (ctx.token) {
    endpoint.searchParams.set("token", ctx.token);
  }
  const stream = new EventSource(endpoint.toString());
  stream.addEventListener("event", (raw) => {
    const message = raw as MessageEvent<string>;
    try {
      const parsed = JSON.parse(message.data) as PluginUiEvent;
      onEvent(parsed);
    } catch {
      // ignore invalid payloads to keep stream alive
    }
  });
  stream.addEventListener("lagged", (raw) => {
    const message = raw as MessageEvent<string>;
    onEvent({
      plugin_id: ctx.pluginId,
      name: "lagged",
      payload: safeParse(message.data),
      ts_ms: Date.now()
    });
  });
  if (onError) {
    stream.onerror = onError;
  }
  return () => stream.close();
}

async function requestJson<T>(
  ctx: GatewayContext,
  method: "GET" | "PUT" | "POST",
  path: string,
  body?: unknown
): Promise<T> {
  const endpoint = new URL(path, ctx.origin);
  const headers = new Headers();
  headers.set("content-type", "application/json");
  if (ctx.token) {
    headers.set(TOKEN_HEADER, ctx.token);
  }

  const response = await fetch(endpoint, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body)
  });

  const payloadText = await response.text();
  if (!response.ok) {
    const reason = payloadText.trim() || `${response.status} ${response.statusText}`;
    throw new Error(reason);
  }
  if (!payloadText.trim()) {
    return {} as T;
  }
  return JSON.parse(payloadText) as T;
}

function resolvePluginId(pathname: string, query: URLSearchParams): string {
  const pluginFromQuery = query.get("plugin_id")?.trim();
  if (pluginFromQuery) {
    return pluginFromQuery;
  }

  const parts = pathname.split("/").filter((item) => item.length > 0);
  const uiIndex = parts.indexOf("ui");
  if (uiIndex >= 0 && parts.length > uiIndex + 1) {
    return decodeURIComponent(parts[uiIndex + 1]);
  }
  return "dev.stellatune.source.netease";
}

function resolveToken(query: URLSearchParams): string {
  return query.get("token")?.trim() ?? "";
}

function resolveGatewayOrigin(query: URLSearchParams): string {
  const raw = query.get("gateway_origin")?.trim();
  if (!raw) {
    return window.location.origin;
  }
  try {
    const parsed = new URL(raw);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return window.location.origin;
    }
    return parsed.origin;
  } catch {
    return window.location.origin;
  }
}

function safeParse(raw: string): unknown {
  try {
    return JSON.parse(raw);
  } catch {
    return raw;
  }
}
