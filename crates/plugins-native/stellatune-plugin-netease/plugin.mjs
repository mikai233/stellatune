import { spawn } from "node:child_process";

const PLUGIN_ID = "dev.stellatune.source.netease";
const DEFAULT_CONFIG = Object.freeze({
  sidecarBaseUrl: "http://127.0.0.1:46321",
  sidecarPath: null,
  sidecarArgs: [],
  requestTimeoutMs: 8000,
  defaultLevel: "standard",
});

let sidecarProcess = null;
let sidecarSignature = "";

function pluginError(code, message, retryable = false, details) {
  return Object.assign(new Error(message), { code, retryable, details });
}

function configFrom(input) {
  const raw = input?.config ?? input?.sourceConfig ?? {};
  return {
    ...DEFAULT_CONFIG,
    ...raw,
    sidecarBaseUrl: String(raw.sidecarBaseUrl ?? raw.sidecar_base_url ?? DEFAULT_CONFIG.sidecarBaseUrl).replace(/\/+$/, ""),
    sidecarPath: raw.sidecarPath ?? raw.sidecar_path ?? DEFAULT_CONFIG.sidecarPath,
    sidecarArgs: raw.sidecarArgs ?? raw.sidecar_args ?? DEFAULT_CONFIG.sidecarArgs,
    requestTimeoutMs: Number(raw.requestTimeoutMs ?? raw.api_request_timeout_ms ?? DEFAULT_CONFIG.requestTimeoutMs),
    defaultLevel: String(raw.defaultLevel ?? raw.default_level ?? DEFAULT_CONFIG.defaultLevel),
  };
}

async function health(config) {
  try {
    const response = await fetch(`${config.sidecarBaseUrl}/health`, {
      signal: AbortSignal.timeout(Math.max(500, config.requestTimeoutMs)),
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
      true,
    );
  }
  const signature = JSON.stringify([executable, config.sidecarArgs, config.sidecarBaseUrl]);
  if (!sidecarProcess || sidecarSignature !== signature) {
    await stopSidecar();
    sidecarProcess = spawn(executable, config.sidecarArgs, {
      stdio: ["ignore", "ignore", "inherit"],
      windowsHide: true,
      env: { ...process.env, STELLATUNE_NCM_OWNER_PID: String(process.pid) },
    });
    sidecarProcess.once("exit", () => {
      sidecarProcess = null;
      sidecarSignature = "";
    });
    sidecarSignature = signature;
  }
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    if (await health(config)) return;
    await new Promise((resolve) => setTimeout(resolve, 150));
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
    new Promise((resolve) => child.once("exit", resolve)),
    new Promise((resolve) => setTimeout(resolve, 500)),
  ]);
}

async function getJson(config, path, params = {}) {
  await ensureSidecar(config);
  const url = new URL(path, `${config.sidecarBaseUrl}/`);
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && String(value).length > 0) {
      url.searchParams.set(key, String(value));
    }
  }
  const response = await fetch(url, {
    signal: AbortSignal.timeout(Math.max(500, config.requestTimeoutMs)),
  });
  if (!response.ok) {
    throw pluginError("sidecar_http_error", `${path} returned HTTP ${response.status}`, response.status >= 500);
  }
  return await response.json();
}

function songId(input) {
  const value = input?.song_id ?? input?.songId ?? input?.track_id ?? input?.trackId
    ?? input?.track?.song_id ?? input?.track?.songId
    ?? input?.playlist_ref?.song_id ?? input?.playlist_ref?.track_id ?? input?.playlist_ref?.id;
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
  const cookie = String(input?.cookie ?? "").trim();
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
    duration_ms: item.duration_ms ?? null,
  };
  return {
    kind: "track",
    item_id: String(id),
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
    track,
  };
}

function controlResult(action, payload) {
  return [{
    kind: "control_result", item_id: action, source_id: "netease",
    source_label: "Netease Cloud Music", track_id: null, playlist_id: null,
    title: action, subtitle: "source control action result", artist: null, album: null,
    duration_ms: null, track_count: null, cover: null, ext_hint: null, path_hint: null,
    playlist_ref: payload, track: null,
  }];
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
    capabilities: { seekable: true, durationMs: track?.duration_ms ?? track?.durationMs ?? null },
  };
}

async function listItems(input) {
  const config = configFrom(input);
  const action = String(input?.action ?? "search").toLowerCase();
  const limit = Math.max(1, Math.min(Number(input?.limit ?? 30), action === "list_playlists" ? 200 : 1000));
  const offset = Math.max(0, Number(input?.offset ?? 0));
  if (action === "list_playlists") {
    const result = await getJson(config, "/v1/playlists", { limit, offset, source_label: "Netease Cloud Music", ...cookieParams(input) });
    return (result.items ?? []).map((item) => ({
      kind: "playlist", item_id: String(item.playlist_id), source_id: item.source_id ?? "netease",
      source_label: item.source_label ?? "Netease Cloud Music", track_id: null,
      playlist_id: String(item.playlist_id), title: item.title, subtitle: null, artist: null,
      album: null, duration_ms: null, track_count: item.track_count ?? null, cover: item.cover ?? null,
      ext_hint: null, path_hint: null, playlist_ref: item.playlist_ref ?? { playlist_id: Number(item.playlist_id) }, track: null,
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
    "netease.auth.login_status": "/v1/auth/login_status", "auth.login_status": "/v1/auth/login_status",
    "netease.auth.login_refresh": "/v1/auth/login_refresh", "auth.login_refresh": "/v1/auth/login_refresh",
    "netease.auth.logout": "/v1/auth/logout", "auth.logout": "/v1/auth/logout",
    "netease.auth.session": "/v1/auth/session", "auth.session": "/v1/auth/session",
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
  if (request.capabilityId === "netease-auth") return auth(input, request.operation);
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

export default {
  descriptor: {
    id: PLUGIN_ID,
    apiVersion: 2,
    capabilities: ["netease-source", "netease-search", "netease-auth", "netease-lyrics"],
  },
  invoke,
  shutdown: stopSidecar,
};
