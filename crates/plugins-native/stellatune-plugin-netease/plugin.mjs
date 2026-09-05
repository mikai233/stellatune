// ../src/config.mjs
import { mkdir, readFile, writeFile, rename, rm } from "node:fs/promises";
import { join } from "node:path";
import { randomUUID } from "node:crypto";
var defaults = Object.freeze({
  sidecar_base_url: "http://127.0.0.1:46321",
  sidecar_path: null,
  sidecar_args: [],
  api_request_timeout_ms: 8e3,
  default_level: "standard",
  cookie: ""
});
function validate(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("plugin config must be an object");
  const config = { ...defaults, ...value };
  const url = new URL(config.sidecar_base_url);
  if (!["http:", "https:"].includes(url.protocol)) throw new Error("sidecar_base_url must use HTTP");
  if (config.sidecar_path !== null && typeof config.sidecar_path !== "string") throw new Error("sidecar_path must be a string or null");
  if (!Array.isArray(config.sidecar_args) || config.sidecar_args.some((v) => typeof v !== "string")) throw new Error("sidecar_args must be strings");
  if (!Number.isSafeInteger(config.api_request_timeout_ms) || config.api_request_timeout_ms < 500) throw new Error("API timeout must be at least 500 ms");
  if (typeof config.default_level !== "string" || !config.default_level.trim()) throw new Error("default_level is required");
  if (typeof config.cookie !== "string") throw new Error("cookie must be a string");
  return config;
}
async function openConfig(dataDir) {
  await mkdir(dataDir, { recursive: true });
  const file = join(dataDir, "config.json");
  let current;
  try {
    current = validate(JSON.parse(await readFile(file, "utf8")));
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
    current = validate({});
  }
  let pending = Promise.resolve();
  return {
    get: () => structuredClone(current),
    save(patch) {
      const operation = pending.then(async () => {
        if (!patch || typeof patch !== "object" || Array.isArray(patch)) throw new Error("plugin config must be an object");
        const next = validate({ ...current, ...patch });
        const temp = `${file}.${randomUUID()}.tmp`;
        try {
          await writeFile(temp, JSON.stringify(next, null, 2));
          await rename(temp, file);
        } finally {
          await rm(temp, { force: true });
        }
        current = next;
        return structuredClone(current);
      });
      pending = operation.catch(() => {
      });
      return operation;
    }
  };
}

// ../src/web-ui.mjs
import { join as join2 } from "node:path";

// ../../../../tools/typescript-plugin-runtime/ui-server.mjs
import { createServer } from "node:http";
import { readFile as readFile2, realpath } from "node:fs/promises";
import { resolve, relative, isAbsolute, extname } from "node:path";
function sendJson(response, value, status = 200) {
  response.writeHead(status, { "content-type": "application/json; charset=utf-8" });
  response.end(JSON.stringify(value));
}
async function readJson(request) {
  const chunks = [];
  let size = 0;
  for await (const chunk of request) {
    size += chunk.length;
    if (size > 1024 * 1024) throw new Error("request exceeds 1 MiB");
    chunks.push(chunk);
  }
  return JSON.parse(Buffer.concat(chunks).toString("utf8") || "{}");
}
async function startUiServer({ root, handleApi }) {
  const filesRoot = await realpath(root);
  const sockets = /* @__PURE__ */ new Set();
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url, "http://localhost");
      if (await handleApi(request, response, url)) return;
      if (request.method !== "GET" && request.method !== "HEAD") return sendJson(response, { message: "not found" }, 404);
      let filename;
      try {
        filename = await realpath(resolve(filesRoot, `.${decodeURIComponent(url.pathname === "/" ? "/index.html" : url.pathname)}`));
      } catch {
        return sendJson(response, { message: "not found" }, 404);
      }
      const rel = relative(filesRoot, filename);
      if (rel.startsWith("..") || isAbsolute(rel)) return sendJson(response, { message: "not found" }, 404);
      const bytes = await readFile2(filename);
      const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".json": "application/json", ".svg": "image/svg+xml", ".png": "image/png", ".ico": "image/x-icon" };
      response.writeHead(200, { "content-type": types[extname(filename)] ?? "application/octet-stream", "cache-control": "no-cache" });
      response.end(request.method === "HEAD" ? void 0 : bytes);
    } catch (error) {
      if (!response.headersSent) sendJson(response, { message: error.message, code: error.code ?? "invalidRequest" }, 400);
      else response.destroy();
    }
  });
  server.on("connection", (socket) => {
    sockets.add(socket);
    socket.on("close", () => sockets.delete(socket));
  });
  await new Promise((resolve2, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve2);
  });
  return {
    url: `http://127.0.0.1:${server.address().port}/`,
    close: () => new Promise((resolve2) => {
      server.close(resolve2);
      for (const socket of sockets) socket.destroy();
    })
  };
}

// ../../../../tools/typescript-plugin-runtime/host-client.mjs
import { setTimeout as delay } from "node:timers/promises";
function createHostClient(baseUrl) {
  async function request(path, body, signal) {
    const response = await fetch(new URL(path, baseUrl), {
      method: body === void 0 ? "GET" : "POST",
      headers: { "content-type": "application/json" },
      body: body === void 0 ? void 0 : JSON.stringify(body),
      signal
    });
    const value = await response.json();
    if (!response.ok) throw Object.assign(new Error(value.message ?? response.statusText), { code: value.code });
    return value;
  }
  const getState = (signal) => request("/player/state", void 0, signal);
  const getQueue = (signal) => request("/player/queue", void 0, signal);
  function subscribe(onEvent, onError = () => {
  }) {
    const controller = new AbortController();
    const { signal } = controller;
    void (async () => {
      while (!signal.aborted) {
        try {
          const response = await fetch(new URL("/player/events", baseUrl), { signal });
          if (!response.ok || !response.body) throw new Error(`events: HTTP ${response.status}`);
          const reader = response.body.getReader();
          const decoder = new TextDecoder();
          let buffer = "";
          try {
            while (!signal.aborted) {
              const { value, done } = await reader.read();
              if (done) break;
              buffer = (buffer + decoder.decode(value, { stream: true })).replace(/\r\n/g, "\n");
              let end;
              while ((end = buffer.indexOf("\n\n")) >= 0) {
                const frame = buffer.slice(0, end);
                buffer = buffer.slice(end + 2);
                const data = frame.split("\n").filter((line) => line.startsWith("data:")).map((line) => line.slice(5).trimStart()).join("\n");
                if (!data) continue;
                const event = JSON.parse(data);
                if (event.type === "resync") {
                  const [state, queue] = await Promise.all([getState(signal), getQueue(signal)]);
                  onEvent({ type: "snapshot", state, queue });
                } else onEvent(event);
              }
            }
          } finally {
            await reader.cancel().catch(() => {
            });
          }
        } catch (error) {
          if (!signal.aborted) onError(error);
        }
        if (!signal.aborted) await delay(1e3, void 0, { signal }).catch(() => {
        });
      }
    })();
    return () => controller.abort();
  }
  const command = (body) => request("/player/commands", body);
  return {
    command,
    getState,
    getQueue,
    subscribe,
    play: () => command({ command: "play" }),
    pause: () => command({ command: "pause" }),
    stop: () => command({ command: "stop" }),
    seek: (positionMs) => command({ command: "seek", positionMs })
  };
}

// ../src/web-ui.mjs
async function openWebUi(context2, config, business) {
  const host = createHostClient(context2.hostApiBaseUrl);
  const subscribers = /* @__PURE__ */ new Set();
  const pluginId = context2.pluginId;
  const authActions = /* @__PURE__ */ new Set(["netease.auth.login_status", "netease.auth.login_refresh", "netease.auth.logout", "netease.auth.session", "netease.auth.qr.start", "netease.auth.qr.status"]);
  const control = { "playback.play": "play", "playback.pause": "pause", "playback.stop": "stop", "playback.next": "next", "playback.previous": "previous" };
  const server = await startUiServer({ root: join2(context2.packageRoot, "ui"), handleApi: async (request, response, url) => {
    if (!url.pathname.startsWith("/api/")) return false;
    if (url.pathname === "/api/player/state" && request.method === "GET") {
      sendJson(response, await host.getState());
      return true;
    }
    if (url.pathname === "/api/config") {
      if (request.method === "GET") sendJson(response, { config: config.get() });
      else if (request.method === "PUT") sendJson(response, { config: await config.save(await readJson(request)) });
      else sendJson(response, { message: "method not allowed" }, 405);
      return true;
    }
    if (url.pathname === "/api/events" && request.method === "GET") {
      response.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache" });
      const publish = (event) => {
        if (response.destroyed) return;
        if (!response.write(`event: event
data: ${JSON.stringify({ plugin_id: pluginId, name: event.type, payload: event, ts_ms: Date.now() })}

`)) response.destroy();
      };
      const unsubscribe = host.subscribe(publish, (error) => publish({ type: "connectionError", message: error.message }));
      subscribers.add(unsubscribe);
      const heartbeat = setInterval(() => response.write(": keepalive\n\n"), 15e3);
      response.on("close", () => {
        clearInterval(heartbeat);
        unsubscribe();
        subscribers.delete(unsubscribe);
      });
      return true;
    }
    if (url.pathname.startsWith("/api/actions/") && request.method === "POST") {
      const action = decodeURIComponent(url.pathname.slice("/api/actions/".length));
      const payload = await readJson(request);
      const input = payload.request ?? payload;
      let data;
      if (control[action]) data = await host.command({ command: control[action] });
      else if (action === "playback.seek") data = await host.seek(input.positionMs);
      else if (action === "playback.play_provider_track" || action === "playback.enqueue_provider_track") {
        data = await host.command({ command: action === "playback.play_provider_track" ? "playProviderTrack" : "enqueueProviderTrack", track: {
          pluginId,
          capabilityId: "netease-source",
          providerId: String(input.provider_id ?? "netease"),
          providerKey: String(input.provider_track_key)
        } });
      } else if (action === "playback.play_track" || action === "playback.enqueue_track") {
        const trackId = String(input.track_id);
        data = await host.command(action === "playback.play_track" ? { command: "playTrack", trackId } : { command: "appendQueue", trackIds: [trackId] });
      } else if (authActions.has(action)) {
        data = await business("netease-auth", action, input);
        data = { response: [{ playlist_ref: data }] };
      } else if (action === "netease.song.lyric") {
        data = { response: [{ playlist_ref: await business("netease-lyrics", "fetch", input) }] };
      } else if (["search", "list_playlists", "playlist_tracks", "netease.song.url", "song.url"].includes(action)) {
        const value = await business("netease-search", "invoke", { ...input, action });
        data = { response: Array.isArray(value) ? value : [{ playlist_ref: value }] };
      } else throw new Error(`unsupported plugin action: ${action}`);
      sendJson(response, { action, message: "\u64CD\u4F5C\u5B8C\u6210", data });
      return true;
    }
    sendJson(response, { message: "not found" }, 404);
    return true;
  } });
  const devUrl = process.env.STELLATUNE_NETEASE_UI_DEV_URL;
  if (devUrl) {
    const url = new URL(devUrl);
    if (!["http:", "https:"].includes(url.protocol)) {
      await server.close();
      throw new Error("UI development URL must use HTTP");
    }
    process.stderr.write(`Netease UI API: ${server.url} (set STELLATUNE_NETEASE_UI_API for Vite)
`);
  }
  return { url: devUrl || server.url, close: async () => {
    for (const stop of subscribers) stop();
    await server.close();
  } };
}

// ../src/plugin.mjs
import { spawn } from "node:child_process";
var PLUGIN_ID = "dev.stellatune.source.netease";
var DEFAULT_CONFIG = Object.freeze({
  sidecarBaseUrl: "http://127.0.0.1:46321",
  sidecarPath: null,
  sidecarArgs: [],
  requestTimeoutMs: 8e3,
  defaultLevel: "standard"
});
var context;
var configStore;
var ui;
var uiStarting;
var sidecarProcess = null;
var sidecarSignature = "";
function pluginError(code, message, retryable = false, details) {
  return Object.assign(new Error(message), { code, retryable, details });
}
function configFrom(input) {
  const raw = configStore.get();
  return {
    ...DEFAULT_CONFIG,
    ...raw,
    sidecarBaseUrl: String(raw.sidecarBaseUrl ?? raw.sidecar_base_url ?? DEFAULT_CONFIG.sidecarBaseUrl).replace(/\/+$/, ""),
    sidecarPath: raw.sidecarPath ?? raw.sidecar_path ?? DEFAULT_CONFIG.sidecarPath,
    sidecarArgs: raw.sidecarArgs ?? raw.sidecar_args ?? DEFAULT_CONFIG.sidecarArgs,
    requestTimeoutMs: Number(raw.requestTimeoutMs ?? raw.api_request_timeout_ms ?? DEFAULT_CONFIG.requestTimeoutMs),
    defaultLevel: String(raw.defaultLevel ?? raw.default_level ?? DEFAULT_CONFIG.defaultLevel)
  };
}
async function health(config) {
  try {
    const response = await fetch(`${config.sidecarBaseUrl}/health`, {
      signal: AbortSignal.timeout(Math.max(500, config.requestTimeoutMs))
    });
    return response.ok && (await response.json())?.ok === true;
  } catch (_) {
    return false;
  }
}
async function ensureSidecar(config) {
  if (await health(config)) return;
  const executable = String(config.sidecarPath ?? "").trim();
  if (!executable) {
    throw pluginError(
      "sidecar_unavailable",
      `Netease service is unavailable at ${config.sidecarBaseUrl}; configure sidecarPath or start it separately`,
      true
    );
  }
  const signature = JSON.stringify([executable, config.sidecarArgs, config.sidecarBaseUrl]);
  if (!sidecarProcess || sidecarSignature !== signature) {
    await stopSidecar();
    sidecarProcess = spawn(executable, config.sidecarArgs, {
      stdio: ["ignore", "ignore", "inherit"],
      windowsHide: true,
      env: { ...process.env, STELLATUNE_NCM_OWNER_PID: String(process.pid) }
    });
    sidecarProcess.once("exit", () => {
      sidecarProcess = null;
      sidecarSignature = "";
    });
    sidecarSignature = signature;
  }
  const deadline = Date.now() + 1e4;
  while (Date.now() < deadline) {
    if (await health(config)) return;
    await new Promise((resolve2) => setTimeout(resolve2, 150));
  }
  throw pluginError("sidecar_start_timeout", "Netease service did not become ready", true);
}
async function stopSidecar() {
  const child = sidecarProcess;
  sidecarProcess = null;
  sidecarSignature = "";
  if (!child || child.exitCode !== null) return;
  child.kill();
  await Promise.race([
    new Promise((resolve2) => child.once("exit", resolve2)),
    new Promise((resolve2) => setTimeout(resolve2, 500))
  ]);
}
async function getJson(config, path, params = {}) {
  await ensureSidecar(config);
  const url = new URL(path, `${config.sidecarBaseUrl}/`);
  for (const [key, value] of Object.entries(params)) {
    if (value !== void 0 && value !== null && String(value).length > 0) {
      url.searchParams.set(key, String(value));
    }
  }
  const response = await fetch(url, {
    signal: AbortSignal.timeout(Math.max(500, config.requestTimeoutMs))
  });
  if (!response.ok) {
    throw pluginError("sidecar_http_error", `${path} returned HTTP ${response.status}`, response.status >= 500);
  }
  return await response.json();
}
function songId(input) {
  const value = input?.song_id ?? input?.songId ?? input?.track_id ?? input?.trackId ?? input?.track?.song_id ?? input?.track?.songId ?? input?.playlist_ref?.song_id ?? input?.playlist_ref?.track_id ?? input?.playlist_ref?.id;
  const id = Number(value);
  if (!Number.isSafeInteger(id) || id <= 0) throw pluginError("invalid_input", "song id is required");
  return id;
}
function playlistId(input) {
  const value = input?.playlist_id ?? input?.playlistId ?? input?.playlist_ref?.playlist_id ?? input?.playlist_ref?.id;
  const id = Number(value);
  if (!Number.isSafeInteger(id) || id <= 0) throw pluginError("invalid_input", "playlist id is required");
  return id;
}
function level(input, config) {
  return String(input?.level ?? input?.track?.level ?? config.defaultLevel).trim() || "standard";
}
function cookieParams(input) {
  const cookie = String(configStore.get().cookie ?? "").trim();
  return cookie ? { cookie } : {};
}
function normalizeTrack(item, selectedLevel) {
  const id = Number(item.song_id);
  const ext = String(item.ext_hint ?? "mp3").replace(/^\./, "").toLowerCase() || "mp3";
  const track = {
    song_id: id,
    level: item.level ?? selectedLevel,
    stream_url: item.stream_url ?? null,
    ext_hint: ext,
    cover: item.cover ?? null,
    title: item.title ?? null,
    artist: item.artist ?? null,
    album: item.album ?? null,
    duration_ms: item.duration_ms ?? null
  };
  return {
    kind: "track",
    item_id: String(id),
    source_resolver_capability_id: "netease-source",
    source_id: "netease",
    source_label: "Netease Cloud Music",
    track_id: String(id),
    playlist_id: null,
    title: item.title || `Song ${id}`,
    subtitle: null,
    artist: item.artist ?? null,
    album: item.album ?? null,
    duration_ms: item.duration_ms ?? null,
    track_count: null,
    cover: item.cover ?? null,
    ext_hint: ext,
    path_hint: `netease:${id}.${ext}`,
    playlist_ref: null,
    track
  };
}
async function resolveSource(input) {
  const config = configFrom(input);
  const track = input?.track ?? input;
  let url = String(track?.stream_url ?? track?.streamUrl ?? "").trim();
  let ext = String(track?.ext_hint ?? track?.extHint ?? "").trim().replace(/^\./, "").toLowerCase();
  if (!url) {
    const result = await getJson(config, "/v1/song/url", { song_id: songId(input), level: level(input, config), ...cookieParams(input) });
    url = String(result.url ?? "").trim();
    ext = String(result.ext_hint ?? ext ?? "mp3").replace(/^\./, "").toLowerCase();
  }
  if (!/^https?:\/\//i.test(url)) throw pluginError("invalid_source_url", "Netease returned a non-HTTP media URL");
  return {
    source: { kind: "http", url, headers: {} },
    media: { codecHint: ext || "mp3" },
    capabilities: { seekable: true, durationMs: track?.duration_ms ?? track?.durationMs ?? null }
  };
}
async function listItems(input) {
  const config = configFrom(input);
  const action = String(input?.action ?? "search").toLowerCase();
  const limit = Math.max(1, Math.min(Number(input?.limit ?? 30), action === "list_playlists" ? 200 : 1e3));
  const offset = Math.max(0, Number(input?.offset ?? 0));
  if (action === "list_playlists") {
    const result = await getJson(config, "/v1/playlists", { limit, offset, source_label: "Netease Cloud Music", ...cookieParams(input) });
    return (result.items ?? []).map((item) => ({
      kind: "playlist",
      item_id: String(item.playlist_id),
      source_id: item.source_id ?? "netease",
      source_label: item.source_label ?? "Netease Cloud Music",
      track_id: null,
      playlist_id: String(item.playlist_id),
      title: item.title,
      subtitle: null,
      artist: null,
      album: null,
      duration_ms: null,
      track_count: item.track_count ?? null,
      cover: item.cover ?? null,
      ext_hint: null,
      path_hint: null,
      playlist_ref: item.playlist_ref ?? { playlist_id: Number(item.playlist_id) },
      track: null
    }));
  }
  if (action === "playlist_tracks") {
    const result = await getJson(config, "/v1/playlist/tracks", { playlist_id: playlistId(input), limit, offset, level: level(input, config), ...cookieParams(input) });
    return (result.items ?? []).map((item) => normalizeTrack(item, level(input, config)));
  }
  if (action === "search") {
    const keywords = String(input?.keywords ?? "").trim();
    if (!keywords) return [];
    const result = await getJson(config, "/v1/search", { keywords, limit, offset, level: level(input, config), ...cookieParams(input) });
    return (result.items ?? []).map((item) => normalizeTrack(item, level(input, config)));
  }
  return null;
}
async function auth(input, operation) {
  const config = configFrom(input);
  const action = operation === "invoke" ? String(input?.action ?? "") : operation;
  const paths = {
    "netease.auth.login_status": "/v1/auth/login_status",
    "auth.login_status": "/v1/auth/login_status",
    "netease.auth.login_refresh": "/v1/auth/login_refresh",
    "auth.login_refresh": "/v1/auth/login_refresh",
    "netease.auth.logout": "/v1/auth/logout",
    "auth.logout": "/v1/auth/logout",
    "netease.auth.session": "/v1/auth/session",
    "auth.session": "/v1/auth/session"
  };
  if (paths[action]) return getJson(config, paths[action], cookieParams(input));
  if (action === "netease.auth.qr.start" || action === "auth.qr.start") {
    const keyPayload = await getJson(config, "/v1/auth/qr/key", cookieParams(input));
    const key = keyPayload?.body?.data?.unikey ?? keyPayload?.body?.data?.key ?? keyPayload?.body?.data?.qrkey;
    if (!key) throw pluginError("invalid_sidecar_response", "QR key response is missing a key");
    const createPayload = await getJson(config, "/v1/auth/qr/create", { ...cookieParams(input), key, qrimg: input?.qrimg ?? true });
    return { key, key_payload: keyPayload, create_payload: createPayload };
  }
  if (action === "netease.auth.qr.status" || action === "auth.qr.status") {
    const key = input?.key ?? input?.playlist_ref?.key;
    if (!key) throw pluginError("invalid_input", "QR status requires a key");
    return getJson(config, "/v1/auth/qr/check", { ...cookieParams(input), key });
  }
  throw pluginError("unsupported_operation", `unsupported auth operation ${action}`);
}
async function invoke(request) {
  const input = request.input ?? {};
  if (request.capabilityId === "netease-source" && request.operation === "resolve") return resolveSource(input);
  if (request.capabilityId === "netease-search") {
    const listed = await listItems(input);
    if (listed !== null) return listed;
    if (request.operation === "list-items") return [];
  }
  if (request.capabilityId === "netease-auth") {
    const result = await auth(input, request.operation);
    const action = request.operation === "invoke" ? input.action : request.operation;
    if (action === "netease.auth.logout" || action === "auth.logout") await configStore.save({ cookie: "" });
    else if (typeof result?.cookie === "string" && result.cookie) await configStore.save({ cookie: result.cookie });
    return result;
  }
  if (request.capabilityId === "netease-lyrics") {
    const config = configFrom(input);
    return getJson(config, "/v1/lyric", { song_id: songId(input), ...cookieParams(input) });
  }
  if (request.capabilityId === "netease-search" && (input.action === "netease.song.url" || input.action === "song.url")) {
    const config = configFrom(input);
    return getJson(config, "/v1/song/url", { song_id: songId(input), level: level(input, config), ...cookieParams(input) });
  }
  throw pluginError("unsupported_operation", `${request.capabilityId}.${request.operation} is unsupported`);
}
var plugin_default = {
  descriptor: {
    id: PLUGIN_ID,
    apiVersion: 2,
    capabilities: ["netease-source", "netease-search", "netease-auth", "netease-lyrics"]
  },
  async initialize(value) {
    context = value;
    if (!context.dataDir || !context.hostApiBaseUrl) throw new Error("plugin host context is missing");
    configStore = await openConfig(context.dataDir);
  },
  invoke,
  async openUi() {
    if (!ui) {
      uiStarting ??= openWebUi(context, configStore, (capabilityId, operation, input) => invoke({ capabilityId, operation, input }));
      try {
        ui = await uiStarting;
      } finally {
        uiStarting = null;
      }
    }
    return { url: ui.url };
  },
  async shutdown() {
    if (ui) await ui.close();
    ui = null;
    await stopSidecar();
  }
};
export {
  plugin_default as default
};
