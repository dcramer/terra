#!/usr/bin/env node
// The real-fire references (docs/fire.md, "The automatic check"; D415): photographs of real
// fire by class, measured by measure.ts into the bands every fire is held to.
//
//   node tools/fire/references.ts [--fetch] [--dir <photos>] [--games <frames>]
//
// photos.json lists each photograph: its file, class, crop round its flame, light (day, dusk,
// night), what it shows and its credit (Wikimedia Commons title, author, page, licence). The
// photographs stay out of the repository (their licences ask for attribution and share-alike
// terms a public repository of code should not take on); they are kept in --dir, by default
// ~/.cache/terra-review/refs/fire-real. --fetch downloads any missing from Commons, as the same
// rendition at most 2000 px wide its crop was chosen on. Writes references.json: per class its
// photos' count by light and each number's median, least and most; and every photo's numbers.
// Class "excluded" is measured and listed but in no class's bands. Then the AAA game frames of
// games.json (D423), in --games (games.ts's GAMES_DIR by default), measured through games.ts's
// ground-cut proxy: a class's colour bands are taken out to its frames' where they lie outside
// the photographs' (`widened`), and the frames' numbers are listed too. Node 22.18 or later,
// ffmpeg; no dependencies.

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { GAMES_DIR, gameFrames, measureFrame, widened } from "./games.ts";
import { type Band, load, METRICS, type Metrics, measure, parseBox, REFERENCES_FILE, type References } from "./measure.ts";

export type Photo = {
  file: string;
  class: string;
  crop: string;
  light: string;
  note: string;
  title: string;
  author: string;
  source: string;
  licence: string;
};

export const PHOTOS_FILE = resolve(import.meta.dirname, "photos.json");
export const PHOTOS_DIR = join(homedir(), ".cache/terra-review/refs/fire-real");

const median = (a: number[]) => {
  const s = [...a].sort((x, y) => x - y);
  const n = s.length;
  return n % 2 ? s[(n - 1) / 2] : (s[n / 2 - 1] + s[n / 2]) / 2;
};

/// Each class's bands over its photos' numbers.
export function bandsOf(rows: { class: string; light: string; metrics: Metrics }[]): References["classes"] {
  const classes: References["classes"] = {};
  for (const cls of [...new Set(rows.map((r) => r.class))].filter((c) => c !== "excluded").sort()) {
    const of = rows.filter((r) => r.class === cls);
    const light: Record<string, number> = {};
    for (const r of of) light[r.light] = (light[r.light] ?? 0) + 1;
    const bands: Record<string, Band> = {};
    for (const { key } of METRICS) {
      const v = of.map((r) => r.metrics[key]);
      bands[key] = { median: median(v), min: Math.min(...v), max: Math.max(...v) };
    }
    classes[cls] = { photos: of.length, light, bands };
  }
  return classes;
}

const UA = { "User-Agent": "TerraFireReferences/1.0 (photographs of real fire for a game's fire check)" };
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function get(url: string): Promise<Response> {
  for (let i = 0; i < 6; i++) {
    const r = await fetch(url, { headers: UA });
    if (r.ok) return r;
    await sleep(8000 * (i + 1));
  }
  throw new Error(`could not fetch ${url}`);
}

/// Downloads a Commons file as the rendition at most 2000 px wide.
async function fetchPhoto(photo: Photo, dir: string) {
  const api = `https://commons.wikimedia.org/w/api.php?action=query&format=json&prop=imageinfo&iiprop=size|url|mime&iiurlwidth=2000&titles=${encodeURIComponent(`File:${photo.title}`)}`;
  const page = Object.values(((await (await get(api)).json()) as { query: { pages: Record<string, { imageinfo?: { width: number; mime: string; url: string; thumburl: string }[] }> } }).query.pages)[0];
  const info = page.imageinfo?.[0];
  if (!info) throw new Error(`${photo.title}: not on Commons`);
  const src = info.width > 2000 || !/jpeg|png/.test(info.mime) ? info.thumburl : info.url;
  await sleep(2000);
  writeFileSync(join(dir, photo.file), Buffer.from(await (await get(src)).arrayBuffer()));
  await sleep(2000);
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const at = argv.indexOf("--dir");
  const dir = at >= 0 ? argv[at + 1] : PHOTOS_DIR;
  const photos = JSON.parse(readFileSync(PHOTOS_FILE, "utf8")) as Photo[];
  const missing = photos.filter((p) => !existsSync(join(dir, p.file)));
  if (missing.length && argv.includes("--fetch")) {
    for (const p of missing) {
      await fetchPhoto(p, dir);
      console.log(`fetched ${p.file}`);
    }
  } else if (missing.length) {
    console.error(`✗ ${missing.length} photos missing from ${dir} (${missing.map((p) => p.file).join(", ")}); --fetch downloads them`);
    process.exit(1);
  }
  const rows = photos.map((p) => ({ file: p.file, class: p.class, light: p.light, metrics: measure(load(join(dir, p.file)), parseBox(p.crop)) }));
  for (const r of rows) for (const k of Object.keys(r.metrics) as (keyof Metrics)[]) r.metrics[k] = Math.round(r.metrics[k] * 1000) / 1000;
  const classes = bandsOf(rows);
  const gat = argv.indexOf("--games");
  const gdir = gat >= 0 ? argv[gat + 1] : GAMES_DIR;
  const frames = gameFrames();
  const lacking = frames.filter((g) => !existsSync(join(gdir, g.file)));
  if (lacking.length) {
    console.error(`✗ ${lacking.length} game frames missing from ${gdir} (${lacking.map((g) => g.file).join(", ")})`);
    process.exit(1);
  }
  const games = frames.map((g) => ({ file: g.file, game: g.game, class: g.class, light: g.light, metrics: measureFrame(load(join(gdir, g.file)), parseBox(g.crop)) }));
  for (const r of games) for (const k of Object.keys(r.metrics) as (keyof Metrics)[]) r.metrics[k] = Math.round(r.metrics[k] * 1000) / 1000;
  for (const [cls, c] of Object.entries(classes)) {
    const of = games.filter((g) => g.class === cls).map((g) => g.metrics);
    if (!of.length) continue;
    c.games = of.length;
    c.bands = widened(c.bands, of);
  }
  const out: References = {
    about: "Real fire measured by tools/fire/measure.ts from the photographs in photos.json, and AAA game frames from games.json through games.ts's ground-cut proxy matte (node tools/fire/references.ts): per class, each number's median, least and most over its photographs, its colour bands taken out to its game frames' where those lie outside (D423); a fire is held to min-max widened by each number's tolerance (METRICS).",
    classes,
    photos: rows,
    games,
  };
  writeFileSync(REFERENCES_FILE, `${JSON.stringify(out, null, 1)}\n`);
  console.log(`✓ ${rows.length} photos and ${games.length} game frames measured: ${Object.entries(classes).map(([c, v]) => `${c} ${v.photos}${v.games ? `+${v.games}` : ""}`).join(", ")}; ${REFERENCES_FILE}`);
}
