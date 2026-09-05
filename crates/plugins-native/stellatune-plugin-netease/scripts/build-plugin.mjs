import { build } from "../ui-web/node_modules/esbuild/lib/main.js";
import { fileURLToPath } from "node:url";
await build({
  entryPoints: [fileURLToPath(new URL("../src/plugin.mjs", import.meta.url))],
  outfile: fileURLToPath(new URL("../plugin.mjs", import.meta.url)),
  bundle: true, platform: "node", format: "esm", target: "node22",
});
