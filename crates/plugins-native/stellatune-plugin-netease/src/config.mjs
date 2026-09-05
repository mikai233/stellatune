import { mkdir, readFile, writeFile, rename, rm } from "node:fs/promises";
import { join } from "node:path";
import { randomUUID } from "node:crypto";

export const defaults = Object.freeze({ sidecar_base_url: "http://127.0.0.1:46321", sidecar_path: null,
  sidecar_args: [], api_request_timeout_ms: 8000, default_level: "standard", cookie: "" });

function validate(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("plugin config must be an object");
  const config = { ...defaults, ...value };
  const url = new URL(config.sidecar_base_url);
  if (!["http:", "https:"].includes(url.protocol)) throw new Error("sidecar_base_url must use HTTP");
  if (config.sidecar_path !== null && typeof config.sidecar_path !== "string") throw new Error("sidecar_path must be a string or null");
  if (!Array.isArray(config.sidecar_args) || config.sidecar_args.some(v => typeof v !== "string")) throw new Error("sidecar_args must be strings");
  if (!Number.isSafeInteger(config.api_request_timeout_ms) || config.api_request_timeout_ms < 500) throw new Error("API timeout must be at least 500 ms");
  if (typeof config.default_level !== "string" || !config.default_level.trim()) throw new Error("default_level is required");
  if (typeof config.cookie !== "string") throw new Error("cookie must be a string");
  return config;
}

export async function openConfig(dataDir) {
  await mkdir(dataDir, { recursive: true });
  const file = join(dataDir, "config.json");
  let current;
  try { current = validate(JSON.parse(await readFile(file, "utf8"))); }
  catch (error) { if (error.code !== "ENOENT") throw error; current = validate({}); }
  let pending = Promise.resolve();
  return {
    get: () => structuredClone(current),
    save(patch) {
      const operation = pending.then(async () => {
        if (!patch || typeof patch !== "object" || Array.isArray(patch)) throw new Error("plugin config must be an object");
        const next = validate({ ...current, ...patch });
        const temp = `${file}.${randomUUID()}.tmp`;
        try { await writeFile(temp, JSON.stringify(next, null, 2)); await rename(temp, file); }
        finally { await rm(temp, { force: true }); }
        current = next;
        return structuredClone(current);
      });
      pending = operation.catch(() => {});
      return operation;
    },
  };
}
