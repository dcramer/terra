#!/usr/bin/env node
// Bakes fire flipbooks with free tools (docs/fire.md, "The baker"; D415): each set's recipe in
// sets/<set>.json is simulated in Blender's Mantaflow and rendered pass by pass in Cycles
// (bake.py, headless through tools/blender), then packed into the flipbook format (pack.ts)
// and written to assets/textures/fire/<set>.png, <set>.motion.png and <set>.json, with a
// contact sheet of its raw passes measured against its class's photographs (sheet.ts).
// A smoke set with `billow` (blaze-smoke) is not simulated either: billow.ts voxelises lumps of
// pyroclastic noise and lights them as bake.py would, in a minute or two.
// A set of kind "volume" (D493) is not simulated here: volume.py reads its sim's cache (the
// recipe's `volume.cache`) and volume.ts packs the box of cells it names into a looping 3D
// volume, <set>.dds and <set>.json, and checks the loop's seam.
//
//   node tools/fire/bake/bake.ts <set>... | --all [--draft] [--pack]
//
// --draft bakes a coarse, short version into shots/ only, to try a recipe in a minute; --pack
// packs what was baked last without running Blender. Blender renders on the CPU, so a bake
// never takes GPU memory another run is using. Raw passes, logs and sheets go to
// shots/fire/bake/<set>/. One line a set; Blender's own output is in its blender.log.
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { closeSync, existsSync, mkdirSync, openSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { type BillowRecipe, bakeBillow } from "./billow.ts";
import { pack } from "./pack.ts";
import { sheet } from "./sheet.ts";
import { packSmoke } from "./smoke.ts";
import { packVolume, type VolumeRecipe } from "./volume.ts";

const HERE = import.meta.dirname;
const ROOT = resolve(HERE, "../../..");
export const SETS = join(HERE, "sets");
export const ASSETS = join(ROOT, "assets/textures/fire");
export const SHOTS = join(ROOT, "shots/fire/bake");

export type Recipe = {
  name: string;
  about: string;
  class: string;
  kind: "loop" | "oneshot" | "volume";
  /// Lit six ways, not glowing: smoke (smoke.ts).
  six_way?: boolean;
  /// Frames across the sheet (8 by default); a smoke set's sheet is square.
  columns?: number;
  /// A smoke set whose every frame is moved and scaled so its puff sits in the middle with so
  /// much of the frame for its radius, resampled to `frame` texels (smoke.ts's `fitFrame`).
  fit?: { frame: number; radius: number };
  /// A smoke set baked without a sim, as billow.ts makes one (its domain and sources unread).
  billow?: BillowRecipe;
  /// Sim frames a rendered frame.
  step?: number;
  size: number;
  frames: number;
  crossfade?: number;
  warmup?: number;
  fps?: number;
  samples?: number;
  domain: Record<string, unknown> & { resolution?: number; noise?: number };
  sources: Record<string, unknown>[];
  camera: { center: [number, number]; extent: number };
  passes?: Record<string, number>;
};

export function recipe(set: string): Recipe {
  const r = JSON.parse(readFileSync(join(SETS, `${set}.json`), "utf8"));
  return { ...r, name: set };
}

/// A coarse, short version of a recipe, to try it quickly.
function draft(r: Recipe): Recipe {
  return {
    ...r,
    name: `${r.name}-draft`,
    size: 128,
    frames: 8,
    crossfade: 0,
    samples: 4,
    domain: { ...r.domain, resolution: Math.round((r.domain.resolution ?? 64) / 2) },
  };
}

function bake(r: Recipe, out: string, script = "bake.py") {
  mkdirSync(out, { recursive: true });
  const path = join(out, "recipe.json");
  writeFileSync(path, JSON.stringify(r, null, 1));
  // Blender's own output goes to a file, not through Node: a sim's progress is too long to hold.
  const logPath = join(out, "blender.log");
  const fd = openSync(logPath, "w");
  const run = spawnSync(join(ROOT, "tools/blender/run.sh"), [join(HERE, script), path, out], { stdio: ["ignore", fd, fd] });
  closeSync(fd);
  if (run.status !== 0) {
    const tail = readFileSync(logPath, "utf8").trim().split("\n").filter((l) => !/^Fra:/.test(l)).slice(-12).join("\n");
    throw new Error(`✗ ${r.name}: Blender exited ${run.status} (${logPath})\n${tail}`);
  }
}

function main() {
  const args = process.argv.slice(2);
  const flags = new Set(args.filter((a) => a.startsWith("--")));
  let sets = args.filter((a) => !a.startsWith("--"));
  if (flags.has("--all")) sets = readdirSync(SETS).filter((f) => f.endsWith(".json")).map((f) => f.slice(0, -5)).sort();
  if (!sets.length) {
    console.error("usage: node tools/fire/bake/bake.ts <set>... | --all [--draft] [--pack]");
    process.exit(2);
  }
  let failed = false;
  for (const set of sets) {
    let r = recipe(set);
    if (flags.has("--draft")) r = draft(r);
    const out = join(SHOTS, r.name);
    try {
      if (r.kind === "volume") {
        if (flags.has("--draft")) throw new Error(`✗ ${r.name}: a volume reads a sim already baked; it has no draft`);
        if (!flags.has("--pack") || !existsSync(join(out, "raw.bin"))) bake(r, out, "volume.py");
        const meta = JSON.parse(readFileSync(join(out, "meta.json"), "utf8"));
        const packed = packVolume(r as VolumeRecipe, out, ASSETS);
        console.log(`${r.name.padEnd(16)} read ${meta.frames} frames in ${meta.read_s} s (flame outside the box at most ${meta.outside_flame_max.toFixed(2)}); ${packed.line}`);
        if (!packed.seam.pass) failed = true;
        continue;
      }
      if (!flags.has("--pack") || !existsSync(join(out, "raw.bin"))) {
        const billow = r.billow;
        if (billow) bakeBillow({ ...r, billow }, out);
        else bake(r, out);
      }
      const meta = JSON.parse(readFileSync(join(out, "meta.json"), "utf8"));
      const dest = flags.has("--draft") ? out : ASSETS;
      const packed = r.six_way ? packSmoke(r, out, dest) : pack(r, out, dest);
      const judged = r.six_way ? `maps ${join(out, "maps.png")}` : sheet(r, out, packed);
      console.log(`${r.name.padEnd(16)} sim ${meta.sim_s} s, render ${meta.render_s} s; ${packed.line}; ${judged}`);
    } catch (e) {
      failed = true;
      console.log(e instanceof Error ? e.message : String(e));
    }
  }
  process.exit(failed ? 1 : 0);
}

if (import.meta.main) main();
