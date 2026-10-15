#!/usr/bin/env node
// Makes the goods a market stall shows (a basket of apples, bread on a board, a row of jugs,
// bolts of cloth) from Poly Haven's CC0 photoscanned models, each cut to the style guide's
// budget with its own textures kept and downscaled, and composed in Blender into one prop by
// a recipe in art/goods.json: what each piece is, how many, and how they are laid (stood,
// heaped in a container, stacked). Every prop is made to the model contract (D303): its
// origin the middle of its base, its front +Z; its catalogue `dims` are what it measures.
//
//   node tools/props/goods.ts [<file>…]       every recipe in art/goods.json, or those named
//   node tools/props/goods.ts --list          what each recipe is made from
//
// The scans are fetched at 1k with their textures, and the fabrics' maps at 1k, into
// tools/props/out/goods/scans, which is not committed; tools/props/goods.py composes each
// prop and writes assets/models/props/<file>.glb, and tools/props/out/goods/<file>.json with
// its triangles and size. Credits are in THIRD_PARTY.md. Node 22.18 or later; Blender
// through tools/blender.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const OUT = join(ROOT, "tools", "props", "out", "goods");
const CACHE = join(OUT, "scans");
const LIST = join(ROOT, "art", "goods.json");
const BLENDER = join(ROOT, "tools", "blender", "run.sh");
const SCRIPT = join(ROOT, "tools", "props", "goods.py");

type Source = { from?: string; cloth?: string };

/** A recipe's sources: each a scanned model (`from`) or cloth made in a fabric (`cloth`),
 * both Poly Haven ids; the rest of a recipe is goods.py's. */
interface Recipe {
  sources: Record<string, Source>;
}

/** The Poly Haven ids a recipe uses, models and textures apart. */
export function sources(recipe: Recipe): { models: string[]; textures: string[] } {
  const all = Object.values(recipe.sources);
  const ids = (pick: (s: Source) => string | undefined) => [
    ...new Set(all.map(pick).filter((id): id is string => !!id)),
  ];
  return { models: ids((s) => s.from), textures: ids((s) => s.cloth) };
}

async function get(url: string, to: string) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: ${response.status}`);
  mkdirSync(dirname(to), { recursive: true });
  writeFileSync(to, Buffer.from(await response.arrayBuffer()));
}

/** Fetches a Poly Haven model's glTF at 1k with everything it includes: its geometry and
 * its colour, normal and roughness maps. */
async function fetchModel(id: string): Promise<string> {
  const dir = join(CACHE, id);
  const gltf = join(dir, `${id}.gltf`);
  if (existsSync(gltf)) return gltf;
  const files = await (await fetch(`https://api.polyhaven.com/files/${id}`)).json();
  const entry = files.gltf?.["1k"]?.gltf;
  if (!entry) throw new Error(`Poly Haven has no glTF of ${id}`);
  for (const [path, file] of Object.entries(entry.include ?? {}) as [string, { url: string }][]) {
    await get(file.url, join(dir, path));
  }
  await get(entry.url, gltf);
  return gltf;
}

/** Fetches a Poly Haven texture's colour, normal (OpenGL's, as glTF's) and roughness maps
 * at 1k, as JPEG, and how far across it is. */
async function fetchTexture(id: string): Promise<{ path: string; metres: number }> {
  const dir = join(CACHE, id);
  const info = join(dir, "info.json");
  if (existsSync(info)) return JSON.parse(readFileSync(info, "utf8"));
  const files = await (await fetch(`https://api.polyhaven.com/files/${id}`)).json();
  const pick = (map: string) => files[map]?.["1k"]?.jpg?.url;
  const maps: Record<string, string | undefined> = {
    "color.jpg": pick("Diffuse"),
    "normal.jpg": pick("nor_gl"),
    "rough.jpg": pick("Rough"),
  };
  for (const [name, url] of Object.entries(maps)) {
    if (!url) throw new Error(`Poly Haven's ${id} has no ${name} at 1k`);
    await get(url, join(dir, name));
  }
  // Poly Haven gives its size in millimetres.
  const { dimensions } = await (await fetch(`https://api.polyhaven.com/info/${id}`)).json();
  const found = { path: dir, metres: (dimensions?.[0] ?? 300) / 1000 };
  writeFileSync(info, JSON.stringify(found));
  return found;
}

async function main() {
  const list: Record<string, Recipe> = JSON.parse(readFileSync(LIST, "utf8"));
  const argv = process.argv.slice(2);
  if (argv.includes("--list")) {
    for (const [file, recipe] of Object.entries(list)) {
      const { models, textures } = sources(recipe);
      console.log(`${file}: ${[...models, ...textures].join(", ")}`);
    }
    return;
  }
  const wanted = argv.length ? argv : Object.keys(list);
  const found: Record<string, { path: string; metres?: number }> = {};
  for (const file of wanted) {
    const recipe = list[file];
    if (!recipe) throw new Error(`no recipe ${file} in art/goods.json`);
    const { models, textures } = sources(recipe);
    for (const id of models) found[id] = { path: await fetchModel(id) };
    for (const id of textures) found[id] = await fetchTexture(id);
  }
  mkdirSync(OUT, { recursive: true });
  const sourcesFile = join(OUT, "sources.json");
  writeFileSync(sourcesFile, JSON.stringify(found, null, 1));
  const result = spawnSync(BLENDER, [SCRIPT, LIST, sourcesFile, OUT, ...wanted], { cwd: ROOT, stdio: "inherit" });
  if (result.status !== 0) throw new Error("Blender failed");
}

if (import.meta.main) await main();
