#!/usr/bin/env node
// Makes game models from scanned ones in a free library: rocks and the like, whose real
// shape a scan has and a generator does not. Each is a Poly Haven model (CC0), fetched
// without its textures and cut by tools/blender/scan.py into its loose pieces, largest
// first, each reduced to a triangle budget, shaded smooth across curves and split at
// creases, its foot at the origin; the piece wanted is written to
// assets/models/<into>/<file>.glb. The game draws it in its kind's real material
// (flora.ron), projected onto it, so one rock's shape serves several kinds (D222).
// tools/blender/scan.py keeps a scan's textures where it is given them, as the lush
// forest's floor does; these come without, so it has none to keep.
//
//   node tools/props/scans.ts [<file>…]     every scan in art/scans.json, or those named
//
// art/scans.json lists them: file → { from: Poly Haven id, piece: which of its pieces,
// largest first (0 if not given), budget: triangles (500 if not given), into: the folder of
// assets/models (nature if not given) }. The fetched scans are kept in
// tools/props/out/scans, which is not committed. Node 22.18 or later; Blender through
// tools/blender.

import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const CACHE = join(ROOT, "tools", "props", "out", "scans");
const LIST = join(ROOT, "art", "scans.json");
const BLENDER = join(ROOT, "tools", "blender", "run.sh");
const SCAN = join(ROOT, "tools", "blender", "scan.py");

export interface Scan {
  from: string;
  piece?: number;
  budget?: number;
  into?: string;
}

/** What tools/blender/scan.py is told for one scan, the file of the piece wanted among
 * what it writes, and where that piece goes. */
export function scanArgs(file: string, scan: Scan, source: string): { args: string[]; piece: string; out: string } {
  const prefix = join(CACHE, "pieces", file);
  return {
    args: [source, prefix, String(scan.budget ?? 500), "--loose", "--crease", "50"],
    piece: `${prefix}_${(scan.piece ?? 0) + 1}.glb`,
    out: join(ROOT, "assets", "models", scan.into ?? "nature", `${file}.glb`),
  };
}

/** Fetches a Poly Haven model's glTF and its geometry, without its textures; or, `textures`
 * given, with them at that resolution ("1k", "2k"), as the campfire's parts keep theirs. */
export async function fetchScan(id: string, textures?: string): Promise<string> {
  const dir = join(CACHE, textures ? `${id}-${textures}` : id);
  const gltf = join(dir, `${id}.gltf`);
  if (existsSync(gltf)) return gltf;
  mkdirSync(join(dir, "textures"), { recursive: true });
  const files = await (await fetch(`https://api.polyhaven.com/files/${id}`)).json();
  const entry = files.gltf?.[textures ?? "1k"]?.gltf;
  if (!entry) throw new Error(`Poly Haven has no glTF of ${id}`);
  const get = async (url: string, to: string) => {
    const response = await fetch(url);
    if (!response.ok) throw new Error(`${url}: ${response.status}`);
    writeFileSync(to, Buffer.from(await response.arrayBuffer()));
  };
  for (const [path, file] of Object.entries(entry.include ?? {}) as [string, { url: string }][]) {
    if (textures || path.endsWith(".bin")) await get(file.url, join(dir, path));
  }
  const json = await (await fetch(entry.url)).json();
  if (!textures) {
    // The glTF without its images, so Blender looks for none and keeps none.
    delete json.images;
    delete json.textures;
    delete json.samplers;
    for (const material of json.materials ?? []) {
      material.pbrMetallicRoughness = { baseColorFactor: [0.5, 0.5, 0.5, 1] };
      delete material.normalTexture;
      delete material.occlusionTexture;
      delete material.emissiveTexture;
    }
  }
  writeFileSync(gltf, JSON.stringify(json));
  return gltf;
}

async function main() {
  const list: Record<string, Scan> = JSON.parse(readFileSync(LIST, "utf8"));
  const wanted = process.argv.slice(2);
  for (const file of wanted.length ? wanted : Object.keys(list)) {
    const scan = list[file];
    if (!scan) throw new Error(`no scan ${file} in art/scans.json`);
    const { args, piece, out } = scanArgs(file, scan, await fetchScan(scan.from));
    const result = spawnSync(BLENDER, [SCAN, ...args], { cwd: ROOT, stdio: "inherit" });
    if (result.status !== 0) throw new Error(`Blender failed on ${file}`);
    if (!existsSync(piece)) throw new Error(`${scan.from} has no piece ${(scan.piece ?? 0) + 1}`);
    mkdirSync(dirname(out), { recursive: true });
    copyFileSync(piece, out);
    console.log(`${file}: ${out}`);
  }
}

if (import.meta.main) await main();
