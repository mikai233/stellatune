import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { cp, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { test } from 'node:test';

test('relocated bundle starts plugins without Node on PATH', { timeout: 15000 }, async t => {
  assert.ok(process.env.STELLATUNE_TEST_RUNTIME_DIR, 'set STELLATUNE_TEST_RUNTIME_DIR');
  const root = await mkdtemp(join(tmpdir(), 'stellatune runtime '));
  const runtime = join(root, 'plugin-runtime');
  await cp(resolve(process.env.STELLATUNE_TEST_RUNTIME_DIR), runtime, { recursive: true });
  const name = process.platform === 'win32' ? 'node.exe'
    : process.platform === 'darwin' ? `node-${process.arch}` : 'node';
  const executable = join(runtime, name);
  const plugin = join(root, 'plugin.mjs');
  await writeFile(plugin, `
import { startUiServer } from './plugin-runtime/ui-server.mjs';
import { createHostClient } from './plugin-runtime/host-client.mjs';
let server;
export default {
  descriptor: { id: 'bundle-fixture' },
  async initialize() {
    server = await startUiServer({ root: '.', handleApi: async (req, res) => {
      res.end('bundle ready'); return true;
    }});
  },
  invoke() { return { executable: process.execPath, version: process.version,
    url: server.url, client: typeof createHostClient }; },
  shutdown() { return server.close(); },
};
`);
  const child = spawn(executable, [join(runtime, 'runner.mjs'), plugin,
    'bundle-fixture', 'stellatune-capability-rpc/1'], {
    cwd: root, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'],
    env: { ...process.env, PATH: '', NODE_PATH: '', NODE_OPTIONS: '' },
  });
  const closed = once(child, 'close');
  t.after(async () => { if (child.exitCode === null) child.kill(); await closed;
    await rm(root, { recursive: true, force: true }); });
  let stderr = '';
  child.stderr.on('data', bytes => { stderr += bytes; });
  let buffer = Buffer.alloc(0), nextId = 0;
  const pending = new Map();
  child.stdout.on('data', bytes => {
    buffer = Buffer.concat([buffer, bytes]);
    while (buffer.length >= 4 && buffer.length >= 4 + buffer.readUInt32BE(0)) {
      const length = buffer.readUInt32BE(0);
      const response = JSON.parse(buffer.subarray(4, length + 4));
      buffer = buffer.subarray(length + 4);
      pending.get(response.id)?.(response); pending.delete(response.id);
    }
  });
  const call = (method, params = {}) => new Promise(resolve => {
    const id = ++nextId;
    pending.set(id, resolve);
    const bytes = Buffer.from(JSON.stringify({ protocol: 'stellatune-capability-rpc/1',
      generation: 1, id, method, params }));
    const prefix = Buffer.alloc(4); prefix.writeUInt32BE(bytes.length);
    child.stdin.write(Buffer.concat([prefix, bytes]));
  });
  assert.equal((await call('plugin.handshake')).result.pluginId, 'bundle-fixture');
  assert.equal((await call('plugin.initialize')).error, undefined);
  const response = await call('capability.invoke');
  assert.equal(response.error, undefined, JSON.stringify(response));
  const { result } = response;
  assert.equal(result.executable.toLowerCase(), executable.toLowerCase());
  assert.equal(result.version, 'v' + (await readFile(join(runtime, 'NODE-VERSION'), 'utf8')).trim());
  assert.equal(result.client, 'function');
  assert.equal(await (await fetch(result.url)).text(), 'bundle ready');
  await call('plugin.shutdown');
  assert.equal((await closed)[0], 0, stderr);
  await assert.rejects(fetch(result.url));
});
