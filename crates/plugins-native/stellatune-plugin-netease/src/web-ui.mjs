import { join } from "node:path";
import { startUiServer, readJson, sendJson } from "../../../../tools/typescript-plugin-runtime/ui-server.mjs";
import { createHostClient } from "../../../../tools/typescript-plugin-runtime/host-client.mjs";

export async function openWebUi(context, config, business) {
  const host = createHostClient(context.hostApiBaseUrl);
  const subscribers = new Set();
  const pluginId = context.pluginId;
  const authActions = new Set(["netease.auth.login_status", "netease.auth.login_refresh", "netease.auth.logout", "netease.auth.session", "netease.auth.qr.start", "netease.auth.qr.status"]);
  const control = { "playback.play": "play", "playback.pause": "pause", "playback.stop": "stop", "playback.next": "next", "playback.previous": "previous" };
  const server = await startUiServer({ root: join(context.packageRoot, "ui"), handleApi: async (request, response, url) => {
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
      const publish = event => {
        if (response.destroyed) return;
        if (!response.write(`event: event\ndata: ${JSON.stringify({ plugin_id: pluginId, name: event.type, payload: event, ts_ms: Date.now() })}\n\n`)) response.destroy();
      };
      const unsubscribe = host.subscribe(publish, error => publish({ type: "connectionError", message: error.message }));
      subscribers.add(unsubscribe);
      const heartbeat = setInterval(() => response.write(": keepalive\n\n"), 15000);
      response.on("close", () => { clearInterval(heartbeat); unsubscribe(); subscribers.delete(unsubscribe); });
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
          pluginId, capabilityId: "netease-source", providerId: String(input.provider_id ?? "netease"), providerKey: String(input.provider_track_key),
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
      sendJson(response, { action, message: "操作完成", data });
      return true;
    }
    sendJson(response, { message: "not found" }, 404);
    return true;
  } });
  const devUrl = process.env.STELLATUNE_NETEASE_UI_DEV_URL;
  if (devUrl) {
    const url = new URL(devUrl);
    if (!["http:", "https:"].includes(url.protocol)) { await server.close(); throw new Error("UI development URL must use HTTP"); }
    process.stderr.write(`Netease UI API: ${server.url} (set STELLATUNE_NETEASE_UI_API for Vite)\n`);
  }
  return { url: devUrl || server.url, close: async () => { for (const stop of subscribers) stop(); await server.close(); } };
}
