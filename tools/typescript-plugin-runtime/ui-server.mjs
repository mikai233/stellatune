import { createServer } from "node:http";
import { readFile, realpath } from "node:fs/promises";
import { resolve, relative, isAbsolute, extname } from "node:path";

export function sendJson(response, value, status = 200) {
  response.writeHead(status, { "content-type": "application/json; charset=utf-8" });
  response.end(JSON.stringify(value));
}
export async function readJson(request) {
  const chunks = []; let size = 0;
  for await (const chunk of request) {
    size += chunk.length;
    if (size > 1024 * 1024) throw new Error("request exceeds 1 MiB");
    chunks.push(chunk);
  }
  return JSON.parse(Buffer.concat(chunks).toString("utf8") || "{}");
}

/** handleApi returns true when it handled the request. No plugin routing policy. */
export async function startUiServer({ root, handleApi }) {
  const filesRoot = await realpath(root);
  const sockets = new Set();
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url, "http://localhost");
      if (await handleApi(request, response, url)) return;
      if (request.method !== "GET" && request.method !== "HEAD") return sendJson(response, { message: "not found" }, 404);
      let filename;
      try { filename = await realpath(resolve(filesRoot, `.${decodeURIComponent(url.pathname === "/" ? "/index.html" : url.pathname)}`)); }
      catch { return sendJson(response, { message: "not found" }, 404); }
      const rel = relative(filesRoot, filename);
      if (rel.startsWith("..") || isAbsolute(rel)) return sendJson(response, { message: "not found" }, 404);
      const bytes = await readFile(filename);
      const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".json": "application/json", ".svg": "image/svg+xml", ".png": "image/png", ".ico": "image/x-icon" };
      response.writeHead(200, { "content-type": types[extname(filename)] ?? "application/octet-stream", "cache-control": "no-cache" });
      response.end(request.method === "HEAD" ? undefined : bytes);
    } catch (error) {
      if (!response.headersSent) sendJson(response, { message: error.message, code: error.code ?? "invalidRequest" }, 400);
      else response.destroy();
    }
  });
  server.on("connection", socket => { sockets.add(socket); socket.on("close", () => sockets.delete(socket)); });
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
  return {
    url: `http://127.0.0.1:${server.address().port}/`,
    close: () => new Promise(resolve => { server.close(resolve); for (const socket of sockets) socket.destroy(); }),
  };
}
