#!/usr/bin/env node
// The AAA game frames a fire class's colour answers to (docs/fire.md, "The automatic check";
// D423, D433): fire is judged against AAA game frames, and where a class's bands from the
// photographs and from the game frames differ, the frames win.
//
//   node tools/fire/games.ts [--dir <frames>]
//
// games.json lists each frame: its file, game, class (or "excluded", with why), light, the box
// round its flame, what it shows and where it came from (Steam Community screenshots). The
// frames stay out of the repository, in --dir, by default ~/.cache/terra-review/fire/refs-games.
// A frame has no matte, and its lit ground has the flame's colour, so it is measured through a
// proxy matte cut at its own ground's light (`groundCut`, fire round g8): the flame-coloured
// pixels in the box brighter than the 90th percentile of the luma of a band 60 px wide round it.
// On our sheet's frames the proxy reads within 2 points of the renderer's true matte (D433).
// references.ts folds the frames into references.json; this prints each frame's numbers.
// Node 22.18 or later, ffmpeg; no dependencies.

import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import type { Picture } from "../sandbox/png.ts";
import { type Band, type Box, isFlame, load, type Metrics, measure, parseBox } from "./measure.ts";

export type GameFrame = { file: string; game: string; class: string; light: string; crop: string; note: string; source: string };

export const GAMES_FILE = resolve(import.meta.dirname, "games.json");
export const GAMES_DIR = join(homedir(), ".cache/terra-review/fire/refs-games");
/// The band round the flame's box the ground's light is read from, and the share of it the
/// flame must be brighter than.
export const GROUND_PAD = 60;
export const GROUND_Q = 0.9;
/// The colour bands a game frame sets; its shape and scale (soot, pockets, octaves) depend on
/// the box drawn round it and a proxy's hard edge, so they stay the photographs'.
export const COLOURS: (keyof Metrics)[] = ["white", "pale", "yellow", "orange", "red", "redOrange"];

const luma = (p: Picture, i: number) => 0.2126 * p.rgba[4 * i] + 0.7152 * p.rgba[4 * i + 1] + 0.0722 * p.rgba[4 * i + 2];

/// A proxy flame matte for a frame with none: white where a pixel passes the colour rule and is
/// brighter than the `GROUND_Q` share of the band `GROUND_PAD` px wide round `box`.
export function groundCut(pic: Picture, box: Box): Picture {
  const ls: number[] = [];
  const [x0, y0, x1, y1] = [box.x, box.y, box.x + box.w - 1, box.y + box.h - 1];
  for (let y = Math.max(0, y0 - GROUND_PAD); y <= Math.min(pic.height - 1, y1 + GROUND_PAD); y++)
    for (let x = Math.max(0, x0 - GROUND_PAD); x <= Math.min(pic.width - 1, x1 + GROUND_PAD); x++)
      if (x < x0 || x > x1 || y < y0 || y > y1) ls.push(luma(pic, y * pic.width + x));
  ls.sort((a, b) => a - b);
  const cut = ls.length ? ls[Math.floor(GROUND_Q * (ls.length - 1))] : 0;
  const n = pic.width * pic.height;
  const rgba = new Uint8Array(4 * n);
  for (let p = 0; p < n; p++) {
    const v = isFlame(pic.rgba[4 * p], pic.rgba[4 * p + 1], pic.rgba[4 * p + 2]) && luma(pic, p) > cut ? 255 : 0;
    rgba.set([v, v, v, 255], 4 * p);
  }
  return { width: pic.width, height: pic.height, rgba };
}

/// A game frame's numbers, by the sheet's own measure through the ground-cut proxy.
export function measureFrame(pic: Picture, box: Box): Metrics {
  return measure(pic, box, groundCut(pic, box));
}

/// A class's bands with its colour bands taken out to its game frames' least and most where
/// those lie outside the photographs' (D423: the frames win where they differ); where the
/// frames lie inside, the photographs' limit stands.
export function widened(bands: Record<string, Band>, frames: Metrics[]): Record<string, Band> {
  if (!frames.length) return bands;
  const out = { ...bands };
  for (const key of COLOURS) {
    const v = frames.map((m) => m[key]);
    out[key] = { ...bands[key], min: Math.min(bands[key].min, ...v), max: Math.max(bands[key].max, ...v) };
  }
  return out;
}

export const gameFrames = () => JSON.parse(readFileSync(GAMES_FILE, "utf8")) as GameFrame[];

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const at = argv.indexOf("--dir");
  const dir = at >= 0 ? argv[at + 1] : GAMES_DIR;
  for (const g of gameFrames()) {
    const m = measureFrame(load(join(dir, g.file)), parseBox(g.crop));
    console.log(`${g.file.padEnd(18)} ${g.class.padEnd(9)} ${COLOURS.map((k) => `${k} ${m[k].toFixed(1)}`).join(", ")} (${m.flamePx} px)`);
  }
}
