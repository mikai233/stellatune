import { pathToFileURL } from "node:url";
import { Console } from "node:console";

// stdout is reserved for framed RPC, including while plugins serve HTTP requests.
globalThis.console = new Console(process.stderr, process.stderr);

const MAX_FRAME_BYTES = 1024 * 1024;
const [entryPath, expectedPluginId, expectedProtocol] = process.argv.slice(2);

if (!entryPath || !expectedPluginId || !expectedProtocol) {
  process.stderr.write("runner requires entry path, plugin id, and protocol\n");
  process.exit(2);
}

const loaded = await import(pathToFileURL(entryPath).href);
const plugin = loaded.default ?? loaded.plugin;
if (!plugin || typeof plugin !== "object") {
  throw new Error("plugin bundle must export a default plugin object");
}

let inputBuffer = Buffer.alloc(0);
let chain = Promise.resolve();
let shutdownPromise;
function shutdownPlugin() {
  shutdownPromise ??= Promise.resolve().then(() => plugin.shutdown?.());
  return shutdownPromise;
}

// A resident plugin must also exit if the APP disappears without sending RPC.
// Closing Node's owned pipes then terminates plugin sidecars such as NCM.
process.stdin.once("end", () => {
  setTimeout(() => process.exit(0), 500);
  shutdownPlugin().catch(error => process.stderr.write(`${error}\n`))
    .finally(() => process.exit(0));
});

process.stdin.on("data", (chunk) => {
  inputBuffer = Buffer.concat([inputBuffer, chunk]);
  while (inputBuffer.length >= 4) {
    const length = inputBuffer.readUInt32BE(0);
    if (length === 0 || length > MAX_FRAME_BYTES) {
      process.stderr.write(`invalid frame length ${length}\n`);
      process.exit(3);
    }
    if (inputBuffer.length < length + 4) break;
    const payload = inputBuffer.subarray(4, length + 4);
    inputBuffer = inputBuffer.subarray(length + 4);
    chain = chain.then(() => handleFrame(payload)).catch((error) => {
      process.stderr.write(`${error?.stack ?? error}\n`);
    });
  }
});

async function handleFrame(payload) {
  let request;
  try {
    request = JSON.parse(payload.toString("utf8"));
  } catch (error) {
    process.stderr.write(`invalid JSON frame: ${error}\n`);
    return;
  }

  const response = {
    jsonrpc: "2.0",
    id: request.id,
    generation: request.generation,
  };
  try {
    response.result = await dispatch(request);
  } catch (error) {
    response.error = normalizeError(error);
  }
  writeFrame(response);
}

async function dispatch(request) {
  if (request.protocol !== expectedProtocol) {
    throw pluginError("protocol_mismatch", `unsupported protocol ${request.protocol}`, false);
  }
  switch (request.method) {
    case "plugin.handshake": {
      const descriptor = plugin.descriptor ?? {};
      if (descriptor.id && descriptor.id !== expectedPluginId) {
        throw pluginError("plugin_id_mismatch", `bundle id ${descriptor.id} does not match ${expectedPluginId}`, false);
      }
      return {
        pluginId: expectedPluginId,
        protocol: expectedProtocol,
        apiVersion: descriptor.apiVersion ?? 2,
        capabilities: descriptor.capabilities ?? [],
      };
    }
    case "plugin.initialize":
      return (await plugin.initialize?.(request.params ?? {})) ?? null;
    case "plugin.open_ui":
      if (typeof plugin.openUi !== "function") throw pluginError("method_not_found", "plugin does not export openUi", false);
      return await plugin.openUi();
    case "capability.create":
      return (await plugin.create?.(request.params ?? {})) ?? null;
    case "capability.invoke": {
      if (typeof plugin.invoke !== "function") {
        throw pluginError("method_not_found", "plugin does not export invoke", false);
      }
      return await plugin.invoke(request.params ?? {});
    }
    case "capability.drop":
      return (await plugin.drop?.(request.params ?? {})) ?? null;
    case "plugin.shutdown":
      await shutdownPlugin();
      setImmediate(() => process.exit(0));
      return null;
    default:
      throw pluginError("method_not_found", `unsupported method ${request.method}`, false);
  }
}

function pluginError(code, message, retryable, details) {
  return Object.assign(new Error(message), { code, retryable, details });
}

function normalizeError(error) {
  return {
    code: typeof error?.code === "string" ? error.code : "plugin_error",
    message: typeof error?.message === "string" ? error.message : String(error),
    retryable: error?.retryable === true,
    ...(error?.details === undefined ? {} : { details: error.details }),
  };
}

function writeFrame(value) {
  const payload = Buffer.from(JSON.stringify(value), "utf8");
  if (payload.length > MAX_FRAME_BYTES) {
    throw new Error(`response frame exceeds ${MAX_FRAME_BYTES} bytes`);
  }
  const prefix = Buffer.allocUnsafe(4);
  prefix.writeUInt32BE(payload.length, 0);
  process.stdout.write(Buffer.concat([prefix, payload]));
}
