import test from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { mkdtemp, cp, mkdir, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createHostClient } from "../../../../tools/typescript-plugin-runtime/host-client.mjs";

async function listen(handler) {
  const server = createServer(handler);
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  return { url: `http://127.0.0.1:${server.address().port}`, close: () => new Promise(resolve => { server.close(resolve); server.closeAllConnections(); }) };
}
const json = (res, value) => { res.writeHead(200, { "content-type": "application/json" }); res.end(JSON.stringify(value)); };

test("host SDK resynchronizes after lag and reconnects with exact decimal identities", { timeout: 5000 }, async t => {
  let connections = 0;
  const state = { state: "paused", itemId: "9007199254740993", trackId: "9007199254740994", positionMs: 40, durationMs: 1000 };
  const queue = { items: [], revision: "9007199254740995" };
  const host = await listen((req, res) => {
    if (req.url === "/player/state") return json(res, state);
    if (req.url === "/player/queue") return json(res, queue);
    res.writeHead(200, { "content-type": "text/event-stream" });
    if (++connections === 1) {
      res.write("event: event\r\n");
      res.end('data: {"type":"resync"}\r\n\r\n');
    } else res.write(`event: event\ndata: ${JSON.stringify({ type: "snapshot", state, queue })}\n\n`);
  });
  let stop;
  t.after(async () => { stop?.(); await host.close(); });
  const received = [];
  await new Promise(resolve => {
    stop = createHostClient(host.url).subscribe(event => {
      received.push(event);
      if (received.length === 2) resolve();
    });
  });
  assert.equal(connections, 2);
  assert.deepEqual(received, [{ type: "snapshot", state, queue }, { type: "snapshot", state, queue }]);
});

test("installed bundle owns UI, persistent configuration, login and host event forwarding", { timeout: 15000 }, async t => {
  const source = process.env.STELLATUNE_TEST_PLUGIN_PACKAGE ?? fileURLToPath(new URL("../", import.meta.url));
  const temporary = await mkdtemp(join(tmpdir(), "stellatune-hosted-ui-"));
  const packageRoot = join(temporary, "package");
  const dataDir = join(temporary, "plugin-data");
  await mkdir(packageRoot);
  await cp(join(source, "plugin.mjs"), join(packageRoot, "plugin.mjs"));
  await cp(join(source, "ui"), join(packageRoot, "ui"), { recursive: true });
  const commands = [];
  let lastCookie;
  let selectedLevel;
  const sidecar = await listen((req, res) => {
    const url = new URL(req.url, "http://localhost");
    lastCookie = url.searchParams.get("cookie");
    switch (url.pathname) {
      case "/health": return json(res, { ok: true });
      case "/v1/search": return json(res, { items: [{ song_id: 42, title: "Fixture song", ext_hint: "flac" }] });
      case "/v1/playlists": return json(res, { items: [{ playlist_id: 1, title: "Fixture playlist" }] });
      case "/v1/playlist/tracks": return json(res, { items: [{ song_id: 42, title: "Fixture song" }] });
      case "/v1/song/url": selectedLevel = url.searchParams.get("level"); return json(res, { url: "http://example.test/song.flac", ext_hint: "flac" });
      case "/v1/lyric": return json(res, { body: { data: { lrc: { lyric: "[00:00]fixture" } } } });
      case "/v1/auth/qr/key": return json(res, { body: { data: { unikey: "fixture-key" } } });
      case "/v1/auth/qr/create": return json(res, { body: { data: { qrurl: "https://example.test/login" } } });
      case "/v1/auth/qr/check": return json(res, { cookie: "fixture-session", body: { code: 803 } });
      case "/v1/auth/login_status": return json(res, { cookie: "fixture-session", body: { code: 200, data: { profile: { nickname: "Fixture" } } } });
      default: res.writeHead(404); res.end();
    }
  });
  const host = await listen(async (req, res) => {
    if (req.url === "/player/events") {
      res.writeHead(200, { "content-type": "text/event-stream" });
      res.write('event: event\ndata: {"type":"snapshot","state":{"state":"paused","positionMs":40},"queue":{"items":[]}}\n\n');
      return;
    }
    if (req.url === "/player/commands") {
      let body = ""; for await (const chunk of req) body += chunk;
      commands.push(JSON.parse(body));
      return json(res, { itemId: "9007199254740993", trackId: "9007199254740994" });
    }
    json(res, {});
  });
  const plugin = (await import(pathToFileURL(join(packageRoot, "plugin.mjs")).href)).default;
  t.after(async () => {
    await plugin.shutdown(); await host.close(); await sidecar.close();
    assert.ok(resolve(temporary).startsWith(resolve(join(tmpdir(), "stellatune-hosted-ui-"))));
    await rm(temporary, { recursive: true, force: true });
  });
  const context = { pluginId: "dev.stellatune.source.netease", generation: 1, packageRoot, dataDir, hostApiBaseUrl: host.url };
  await plugin.initialize(context);
  const [first, second] = await Promise.all([plugin.openUi(), plugin.openUi()]);
  assert.equal(first.url, second.url);
  assert.match(await (await fetch(first.url)).text(), /<html/);
  const call = async (path, body, method = "POST") => {
    const response = await fetch(new URL(path, first.url), { method, headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
    assert.equal(response.status, 200, await response.clone().text());
    return response.json();
  };
  await call("/api/config", { sidecar_base_url: sidecar.url, default_level: "lossless" }, "PUT");
  const search = await call("/api/actions/search", { request: { keywords: "fixture", config: { sidecar_base_url: "http://ignored.invalid" } } });
  assert.equal(search.data.response[0].track_id, "42");
  assert.equal(search.data.response[0].source_resolver_capability_id, "netease-source");
  await call("/api/actions/netease.auth.qr.start", {});
  await call("/api/actions/netease.auth.qr.status", { request: { key: "fixture-key" } });
  const plan = await plugin.invoke({ capabilityId: "netease-source", operation: "resolve", input: { song_id: 42, config: { sidecarBaseUrl: "http://ignored.invalid" } } });
  assert.equal(plan.source.url, "http://example.test/song.flac");
  assert.equal(selectedLevel, "lossless");
  assert.equal(lastCookie, "fixture-session");
  await call("/api/actions/playback.play_provider_track", { provider_id: "netease", provider_track_key: "42" });
  assert.deepEqual(commands.at(-1), { command: "playProviderTrack", track: { pluginId: context.pluginId, capabilityId: "netease-source", providerId: "netease", providerKey: "42" } });
  await call("/api/actions/playback.enqueue_track", { track_id: "9007199254740994" });
  assert.equal(commands.at(-1).trackIds[0], "9007199254740994");
  const events = await fetch(new URL("/api/events", first.url));
  const reader = events.body.getReader();
  assert.match(new TextDecoder().decode((await reader.read()).value), /"name":"snapshot"/);
  await reader.cancel();
  const invalid = await fetch(new URL("/api/config", first.url), { method: "PUT", body: JSON.stringify({ api_request_timeout_ms: -1 }) });
  assert.equal(invalid.status, 400);
  await plugin.shutdown();
  await assert.rejects(fetch(first.url));
  await plugin.initialize({ ...context, generation: 2 });
  const restarted = await plugin.openUi();
  const saved = await (await fetch(new URL("/api/config", restarted.url))).json();
  assert.equal(saved.config.default_level, "lossless");
  assert.equal(saved.config.cookie, "fixture-session");
  assert.equal(JSON.parse(await readFile(join(dataDir, "config.json"))).api_request_timeout_ms, 8000);
  await plugin.shutdown();
  await writeFile(join(dataDir, "config.json"), "invalid json");
  await assert.rejects(plugin.initialize(context));
});
