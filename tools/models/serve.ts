#!/usr/bin/env node
// Serves the model viewer: a web page that shows any model in assets/models or art, in three.js,
// beside a body for scale, over a metre grid, with its size, triangles and materials, so a model
// can be looked at in a browser without Blender or the sandbox (D71). See README.md.
// Node 22.18 or later, which runs TypeScript directly; no dependencies.

import { createReadStream, readdirSync, statSync } from "node:fs";
import { createServer } from "node:http";
import { dirname, extname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const PAGE = join(ROOT, "tools", "models", "index.html");
// What the page may read: the models the game loads, and the source art they are made from.
export const SERVED = ["assets/models", "art"];
const TYPES: Record<string, string> = {
  ".glb": "model/gltf-binary",
  ".gltf": "model/gltf+json",
  ".bin": "application/octet-stream",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".html": "text/html; charset=utf-8",
};

/** A requested path as a file under one of the served folders, or nothing if it is outside them. */
export function servedFile(root: string, wanted: string): string | undefined {
  const file = resolve(root, decodeURIComponent(wanted).replace(/^\/+/, ""));
  const inside = SERVED.some((folder) => file.startsWith(resolve(root, folder) + sep));
  return inside ? file : undefined;
}

export type Listed = { path: string; bytes: number; changed: number };

/** Every model under the served folders, by path from the root, with its size and when it changed. */
export function models(root: string): Listed[] {
  const found: Listed[] = [];
  const walk = (folder: string) => {
    for (const entry of readdirSync(folder, { withFileTypes: true })) {
      const path = join(folder, entry.name);
      if (entry.isDirectory()) walk(path);
      else if ([".glb", ".gltf"].includes(extname(entry.name))) {
        const stat = statSync(path);
        found.push({ path: relative(root, path).split(sep).join("/"), bytes: stat.size, changed: stat.mtimeMs });
      }
    }
  };
  // The game's models first, then the source art, each in order.
  for (const folder of SERVED) {
    const from = found.length;
    walk(join(root, folder));
    found.splice(from, found.length - from, ...found.slice(from).sort((a, b) => a.path.localeCompare(b.path)));
  }
  return found;
}

function main() {
  const { values } = parseArgs({ options: { port: { type: "string", default: "8765" }, help: { type: "boolean", short: "h" } } });
  if (values.help) {
    console.log("usage: node tools/models/serve.ts [--port 8765]\n\nServes the model viewer on http://localhost:PORT/");
    return;
  }
  const server = createServer((request, response) => {
    const url = new URL(request.url ?? "/", "http://localhost");
    const send = (status: number, type: string, body: string) => {
      response.writeHead(status, { "content-type": type, "cache-control": "no-store" });
      response.end(body);
    };
    if (url.pathname === "/") {
      response.writeHead(200, { "content-type": TYPES[".html"], "cache-control": "no-store" });
      createReadStream(PAGE).pipe(response);
    } else if (url.pathname === "/api/models") {
      send(200, "application/json", JSON.stringify(models(ROOT)));
    } else if (url.pathname.startsWith("/files/")) {
      const file = servedFile(ROOT, url.pathname.slice("/files/".length));
      let stat;
      try {
        stat = file && statSync(file);
      } catch {}
      if (!file || !stat || !stat.isFile()) return send(404, "text/plain", "not found");
      response.writeHead(200, {
        "content-type": TYPES[extname(file).toLowerCase()] ?? "application/octet-stream",
        "content-length": stat.size,
        "cache-control": "no-store",
      });
      createReadStream(file).pipe(response);
    } else {
      send(404, "text/plain", "not found");
    }
  });
  const port = Number(values.port);
  server.listen(port, "127.0.0.1", () => console.log(`Model viewer on http://localhost:${port}/`));
}

if (import.meta.main) main();
