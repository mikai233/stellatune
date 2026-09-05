import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { test } from 'node:test';
import { cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

test('installed Rust NCM plugin streams seekable audio without disk caches', async t => {
  const root = await mkdtemp(join(tmpdir(), 'stellatune-ncm-stream-'));
  const packageRoot = join(root, 'package');
  await mkdir(packageRoot);
  const source = process.env.STELLATUNE_TEST_NCM_PACKAGE ?? fileURLToPath(new URL('../', import.meta.url));
  for (const name of ['manifest.json', 'plugin.mjs', 'bin']) await cp(join(source, name), join(packageRoot, name), { recursive: true });
  const plugin = (await import(pathToFileURL(join(packageRoot, 'plugin.mjs')))).default;
  t.after(async () => { await plugin.shutdown(); await rm(root, { recursive: true, force: true }); });
  const dataDir = join(root, 'data');
  await mkdir(dataDir);
  await plugin.initialize({ packageRoot, dataDir });
  const invoke = (operation, path) => plugin.invoke({ capabilityId: 'ncm-file', operation, input: { path } });
  for (const fixture of ['tone.ncm', 'tone-mp3.ncm']) {
    const path = join(root, fixture);
    const original = await readFile(new URL('fixtures/' + fixture, import.meta.url));
    await writeFile(path, original);
    const metadata = await invoke('inspect-file', path);
    assert.equal(metadata.title, 'Synthetic tone');
    assert.equal(metadata.durationMs, 2000);
    const [a, b] = await Promise.all([invoke('resolve-file', path), invoke('resolve-file', path)]);
    assert.deepEqual(a, b);
    assert.equal(a.source.kind, 'http');
    assert.equal(new URL(a.source.url).hostname, '127.0.0.1');
    const full = await fetch(a.source.url);
    const audio = Buffer.from(await full.arrayBuffer());
    if (fixture === 'tone.ncm') assert.equal(audio.subarray(0, 4).toString(), 'fLaC');
    const head = await fetch(a.source.url, { method: 'HEAD' });
    assert.equal(head.headers.get('content-length'), String(audio.length));
    assert.equal(head.headers.get('accept-ranges'), 'bytes');
    assert.equal((await head.arrayBuffer()).byteLength, 0);
    await Promise.all([[0, 17], [257, 4095], [audio.length - 53, audio.length - 1]].map(async ([start, end]) => {
      const response = await fetch(a.source.url, { headers: { Range: 'bytes=' + start + '-' + end } });
      assert.equal(response.status, 206);
      assert.equal(response.headers.get('content-range'), 'bytes ' + start + '-' + Math.min(end, audio.length - 1) + '/' + audio.length);
      assert.deepEqual(Buffer.from(await response.arrayBuffer()), audio.subarray(start, end + 1));
    }));
    const suffix = await fetch(a.source.url, { headers: { Range: 'bytes=-37' } });
    assert.deepEqual(Buffer.from(await suffix.arrayBuffer()), audio.subarray(-37));
    const invalid = await fetch(a.source.url, { headers: { Range: 'bytes=' + audio.length + '-' } });
    assert.equal(invalid.status, 416);
    await invalid.arrayBuffer();
    assert.deepEqual(await readFile(path), original);
    // A stale URL must not splice bytes from a replacement file into an active stream.
    await writeFile(path, original.subarray(0, 20));
    assert.equal((await fetch(a.source.url)).status, 409);
    await assert.rejects(invoke('resolve-file', path));
    await writeFile(path, original);
    const restored = await invoke('resolve-file', path);
    assert.notEqual(restored.source.url, a.source.url);
    await plugin.shutdown();
    await assert.rejects(fetch(restored.source.url));
    await plugin.initialize({ packageRoot, dataDir });
    const restarted = await invoke('resolve-file', path);
    const response = await fetch(restarted.source.url);
    assert.deepEqual(Buffer.from(await response.arrayBuffer()), audio);
  }
  assert.deepEqual(await readdir(dataDir), [], 'plugin never writes decrypted files');
});


test('APP pipe closure and Node crash both release the native HTTP server', async t => {
  const packageRoot = process.env.STELLATUNE_TEST_NCM_PACKAGE ?? fileURLToPath(new URL('../', import.meta.url));
  const runner = fileURLToPath(new URL('../../../../tools/typescript-plugin-runtime/runner.mjs', import.meta.url));
  const protocol = 'stellatune-capability-rpc/1';
  for (const crash of [false, true]) {
    const child = spawn(process.execPath, [runner, join(packageRoot, 'plugin.mjs'), 'dev.stellatune.source.ncm', protocol], { stdio: ['pipe','pipe','pipe'], windowsHide: true });
    t.after(() => child.kill());
    child.stderr.resume();
    let buffer = Buffer.alloc(0), nextId = 0;
    const pending = new Map();
    child.stdout.on('data', bytes => {
      buffer = Buffer.concat([buffer, bytes]);
      while (buffer.length >= 4 && buffer.length >= 4 + buffer.readUInt32BE(0)) {
        const length = buffer.readUInt32BE(0);
        const value = JSON.parse(buffer.subarray(4, length + 4));
        buffer = buffer.subarray(length + 4);
        const resolve = pending.get(value.id);
        pending.delete(value.id);
        resolve?.(value);
      }
    });
    const call = (method, params) => new Promise(resolve => {
      const id = ++nextId;
      pending.set(id, resolve);
      const payload = Buffer.from(JSON.stringify({ protocol, id, generation: 1, method, params }));
      const length = Buffer.alloc(4); length.writeUInt32BE(payload.length);
      child.stdin.write(Buffer.concat([length, payload]));
    });
    await call('plugin.initialize', { packageRoot });
    const response = await call('capability.invoke', { capabilityId: 'ncm-file', operation: 'resolve-file', input: { path: fileURLToPath(new URL('fixtures/tone.ncm', import.meta.url)) } });
    assert.ok(response.result, JSON.stringify(response));
    const url = response.result.source.url;
    await (await fetch(url)).arrayBuffer();
    const closed = once(child, 'close');
    if (crash) child.kill(); else child.stdin.end();
    await closed;
    // EOF propagation from Node to the Rust child is asynchronous.
    let alive = true;
    for (let retry = 0; retry < 50 && alive; retry++) {
      await new Promise(resolve => setTimeout(resolve, 10));
      alive = await fetch(url).then(async response => { await response.arrayBuffer(); return true; }, () => false);
    }
    assert.equal(alive, false, 'native child must exit when its owner disappears');
  }
});
