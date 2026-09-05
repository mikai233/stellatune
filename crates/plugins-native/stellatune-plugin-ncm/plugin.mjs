import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { join } from 'node:path';

let packageRoot;
let starting;
let session;
let nextId = 0;

async function start() {
  if (session) return session;
  if (starting) return starting;
  starting = new Promise((resolve, reject) => {
    const executable = join(packageRoot, 'bin', process.platform === 'win32' ? 'stellatune-ncm-host.exe' : 'stellatune-ncm-host');
    const child = spawn(executable, [], { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
    const pending = new Map();
    const current = { child, pending, closed: new Promise(done => child.once('close', done)) };
    const lines = createInterface({ input: child.stdout });
    const timer = setTimeout(() => { child.kill(); reject(new Error('NCM host startup timed out')); }, 5000);
    function fail(error) {
      clearTimeout(timer);
      if (session === current) session = undefined;
      reject(error);
      for (const request of pending.values()) request.reject(error);
      pending.clear();
    }
    child.on('error', fail);
    child.stdin.on('error', fail);
    child.stderr.on('data', data => process.stderr.write(data));
    child.once('close', () => { lines.close(); fail(new Error('NCM host exited')); });
    lines.on('line', line => {
      try {
        const response = JSON.parse(line);
        if (response.baseUrl) {
          clearTimeout(timer);
          session = current;
          resolve(current);
          return;
        }
        const request = pending.get(response.id);
        if (!request) return;
        pending.delete(response.id);
        if (response.error) request.reject(new Error(response.error));
        else request.resolve(response.result);
      } catch (error) { fail(error); child.kill(); }
    });
  });
  try { return await starting; } finally { starting = undefined; }
}

export default {
  descriptor: { id: 'dev.stellatune.source.ncm', apiVersion: 2, capabilities: ['ncm-file'] },
  async initialize(context) {
    if (!context.packageRoot) throw new Error('NCM plugin requires packageRoot');
    packageRoot = context.packageRoot;
  },
  async invoke({ capabilityId, operation, input }) {
    if (capabilityId !== 'ncm-file' || !['resolve-file', 'inspect-file'].includes(operation)) throw new Error('unsupported NCM operation');
    if (typeof input?.path !== 'string') throw new Error('path must be a string');
    const current = await start();
    const id = ++nextId;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        current.pending.delete(id);
        reject(new Error('NCM request timed out'));
        current.child.kill();
      }, 8000);
      current.pending.set(id, {
        resolve(value) { clearTimeout(timer); resolve(value); },
        reject(error) { clearTimeout(timer); reject(error); },
      });
      current.child.stdin.write(JSON.stringify({ id, operation, path: input.path }) + '\n');
    });
  },
  async shutdown() {
    if (starting) await starting.catch(() => {});
    const current = session;
    session = undefined;
    if (!current) return;
    current.child.stdin.end();
    const timer = setTimeout(() => current.child.kill(), 250);
    try { await current.closed; } finally { clearTimeout(timer); }
  },
};
