#!/usr/bin/env node
// Paints item icons in the art style (D130): each from the item's render, which fixes its shape,
// angle and framing, with the style reference and approved icons as references so icons keep to
// one style. Cuts the result out, fits it to the item's cells and records it in the manifest as
// painted, then saves a sheet of renders beside paintings to review. See README.md.
// Node 22.18 or later, which runs TypeScript directly; no dependencies beyond ImageMagick.

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const HERE = join(ROOT, "tools", "icons");
const ASSETS = join(ROOT, "assets");
const ICONS = join(ASSETS, "icons");
const MANIFEST = join(ICONS, "manifest.txt");
const TRIPO = join(ROOT, "tools", "tripo", "tripo.ts");
// As the sandbox's icons: pixels a cell, the empty slot and the gap between slots.
const PX_PER_CELL = 96;
const SLOT = "#1f2129";
const GAP = "#0d0f14";
// The flat background the image model paints on, cut out afterwards.
const KEY = "#FF00FF";
// Images asked for at once.
const AT_ONCE = 4;

const USAGE = `usage: node tools/icons/paint.ts [ITEM …] [--unpainted] [--renders DIR] [--sheet FILE]

  ITEM …          the items to paint, by id (stone_axe), each described in tools/icons/icons.txt
  --unpainted     every item described in icons.txt that has no painted icon yet
  --renders DIR   renders made already (cargo xtask icons --renders DIR); otherwise the icons
                  are baked first to make them
  --sheet FILE    where the review sheet goes (default shots/icons/review.png)
  --review        only draw the review sheet of the items named, painted already`;

/** FNV-1a over a file's bytes, as the sandbox's `icons::fingerprint`, in hex. */
export function fingerprint(bytes: Uint8Array): string {
  let hash = 0xcbf29ce484222325n;
  for (const byte of bytes) {
    hash ^= BigInt(byte);
    hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return hash.toString(16).padStart(16, "0");
}

/** A line of the manifest, as the sandbox's `icons::Baked`; "-" where there is nothing. */
export type Baked = { item: string; source: "rendered" | "painted"; look: string; fingerprint: string; style: string };

export function parseManifest(text: string): Baked[] {
  return text
    .split("\n")
    .map((line) => line.trim().split(/\s+/))
    .filter((words) => words.length === 5 && (words[1] === "rendered" || words[1] === "painted"))
    .map(([item, source, look, fingerprint, style]) => ({ item, source: source as Baked["source"], look, fingerprint, style }));
}

export function formatManifest(lines: Baked[]): string {
  return [...lines]
    .sort((a, b) => (a.item < b.item ? -1 : a.item > b.item ? 1 : 0))
    .map((line) => `${line.item} ${line.source} ${line.look} ${line.fingerprint} ${line.style}\n`)
    .join("");
}

/** icons.txt: `@style FILE`, `@anchors ITEM …`, and `ITEM: what it is` per item. */
export type Notes = { style: string; anchors: string[]; items: Map<string, string> };

export function parseNotes(text: string): Notes {
  const notes: Notes = { style: "", anchors: [], items: new Map() };
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    if (line.startsWith("@style ")) notes.style = line.slice(7).trim();
    else if (line.startsWith("@anchors ")) notes.anchors = line.slice(9).trim().split(/\s+/);
    else {
      const at = line.indexOf(":");
      if (at > 0) notes.items.set(line.slice(0, at).trim(), line.slice(at + 1).trim());
    }
  }
  return notes;
}

/** The image model's aspect nearest an item's cells. */
export function aspect(w: number, h: number): string {
  return w > h ? "16:9" : h > w ? "9:16" : "1:1";
}

export function prompt(what: string, rendered: boolean): string {
  const framing = rendered
    ? "Keep the exact shape, angle and framing of the first reference image, filling the frame, and add nothing that is not in it."
    : "Centred and filling the frame, seen from the front and a little above, framed like the icons in the other reference images.";
  return (
    `A game inventory icon of ${what}. ${framing} Painted in the art style of the other reference images: ` +
    "hand-painted surfaces with visible brush strokes and painted highlights, chunky readable forms, slightly " +
    "exaggerated proportions and a bold clear silhouette that reads at a small size, warm light from the upper left. " +
    `A single object only, no text, no border, no frame, no cast shadow, on a solid flat pure magenta (${KEY}) background.`
  );
}

function convert(args: string[]): string {
  const result = spawnSync("convert", args, { encoding: "utf8" });
  if (result.status !== 0) throw new Error(`convert ${args.join(" ")}: ${result.stderr}`);
  return result.stdout;
}

function size(file: string): [number, number] {
  const [w, h] = convert([file, "-format", "%w %h", "info:"]).trim().split(" ").map(Number);
  return [w, h];
}

/** An image flattened on grey, four times as big if it is an icon: a reference the model reads. */
function reference(file: string, out: string): string {
  convert([file, "-resize", "400%", "-background", "#9a9a9a", "-flatten", out]);
  return out;
}

function run(program: string, args: string[]): Promise<{ code: number; out: string }> {
  return new Promise((resolve) => {
    const child = spawn(program, args, { stdio: ["ignore", "pipe", "inherit"] });
    let out = "";
    child.stdout.on("data", (chunk) => (out += chunk));
    child.on("close", (code) => resolve({ code: code ?? 1, out }));
  });
}

/** Paints one item's icon into the assets; its manifest line, or an error. */
async function paint(item: string, what: string, notes: Notes, renders: string, work: string, before: Baked[]): Promise<Baked> {
  const render = join(renders, `${item}.png`);
  const rendered = existsSync(render);
  const [w, h] = rendered ? size(render).map((px) => Math.max(1, Math.round(px / PX_PER_CELL))) : [1, 1];
  const refs: string[] = [];
  if (rendered) refs.push(reference(render, join(work, `${item}-render.png`)));
  refs.push(join(HERE, notes.style));
  for (const anchor of notes.anchors) {
    const icon = join(ICONS, `${anchor}.png`);
    const painted = before.some((line) => line.item === anchor && line.source === "painted");
    if (anchor !== item && painted && existsSync(icon)) refs.push(reference(icon, join(work, `${item}-anchor-${anchor}.png`)));
  }
  const raw = join(work, `${item}-painted.png`);
  const args = [TRIPO, "image", prompt(what, rendered), "--aspect", aspect(w, h), "-o", raw];
  for (const ref of refs) args.push("--ref", ref);
  const { code } = await run("node", args);
  if (code !== 0 || !existsSync(raw)) throw new Error(`${item}: the image model failed`);
  const [cw, ch] = [w * PX_PER_CELL, h * PX_PER_CELL];
  const fit = `${Math.floor(cw * 0.87)}x${Math.floor(ch * 0.87)}`;
  convert([
    raw, "-fuzz", "30%", "-transparent", KEY,
    "-channel", "A", "-morphology", "Erode", "Disk:1.5", "-blur", "0x0.6", "+channel",
    "-trim", "+repage", "-resize", fit, "-background", "none", "-gravity", "center", "-extent", `${cw}x${ch}`,
    "-depth", "8", join(ICONS, `${item}.png`),
  ]);
  const look = before.find((line) => line.item === item)?.look ?? "-";
  const lookFile = join(ASSETS, look);
  return {
    item,
    source: "painted",
    look: rendered ? look : "-",
    fingerprint: rendered && existsSync(lookFile) ? fingerprint(readFileSync(lookFile)) : "-",
    style: "-",
  };
}

/** A column per item: its render above, its painting below, each on an empty slot; and at half size. */
function sheet(items: string[], renders: string, work: string, out: string) {
  const columns = items.map((item) => {
    const icon = join(ICONS, `${item}.png`);
    const [w, h] = size(icon);
    const render = join(renders, `${item}.png`);
    const slot = (file: string | null, name: string) => {
      const out = join(work, name);
      if (file && existsSync(file)) convert(["-size", `${w}x${h}`, `xc:${SLOT}`, file, "-gravity", "center", "-composite", out]);
      else convert(["-size", `${w}x${h}`, `xc:${SLOT}`, out]);
      return out;
    };
    const column = join(work, `${item}-column.png`);
    convert([slot(render, `${item}-slot-render.png`), slot(icon, `${item}-slot-icon.png`), "-background", GAP, "-splice", "0x6", "-append", column]);
    return column;
  });
  mkdirSync(dirname(out), { recursive: true });
  // Each column gets a gap on its left, then they are lined up along their tops.
  const spaced = columns.flatMap((column) => ["(", column, "-gravity", "west", "-splice", "6x0", ")"]);
  convert(["-background", GAP, ...spaced, "-gravity", "north", "+append", "-bordercolor", GAP, "-border", "6", "-depth", "8", out]);
  convert([out, "-resize", "50%", out.replace(/\.png$/, "-half.png")]);
}

async function main(argv: string[]) {
  const { values, positionals } = parseArgs({
    args: argv,
    allowPositionals: true,
    options: {
      unpainted: { type: "boolean" },
      renders: { type: "string" },
      sheet: { type: "string" },
      review: { type: "boolean" },
      help: { type: "boolean", short: "h" },
    },
  });
  if (values.help || (!positionals.length && !values.unpainted)) {
    console.log(USAGE);
    return;
  }
  if (spawnSync("convert", ["-version"]).status !== 0) throw new Error("needs ImageMagick: sudo apt install imagemagick");
  const notes = parseNotes(readFileSync(join(HERE, "icons.txt"), "utf8"));
  const work = mkdtempSync(join(tmpdir(), "terra-icons-"));
  let renders = values.renders as string | undefined;
  if (!renders) {
    renders = join(work, "renders");
    const baked = spawnSync("cargo", ["xtask", "icons", "--renders", renders], { cwd: ROOT, stdio: "inherit" });
    if (baked.status !== 0) throw new Error("baking the renders failed");
  }
  const out = (values.sheet as string | undefined) ?? join(ROOT, "shots", "icons", "review.png");
  if (values.review) {
    sheet(positionals, renders, work, out);
    console.log(`ICONS review ${out} and its -half.png`);
    return;
  }
  const before = parseManifest(existsSync(MANIFEST) ? readFileSync(MANIFEST, "utf8") : "");
  const items = values.unpainted
    ? [...notes.items.keys()].filter((item) => !before.some((line) => line.item === item && line.source === "painted"))
    : positionals;
  const unknown = items.filter((item) => !notes.items.has(item));
  if (unknown.length) throw new Error(`describe these in tools/icons/icons.txt first: ${unknown.join(", ")}`);
  const painted: Baked[] = [];
  for (let at = 0; at < items.length; at += AT_ONCE) {
    const batch = items.slice(at, at + AT_ONCE);
    const results = await Promise.allSettled(batch.map((item) => paint(item, notes.items.get(item)!, notes, renders!, work, before)));
    for (const result of results) {
      if (result.status === "fulfilled") painted.push(result.value);
      else console.error(`ICONS ${result.reason}`);
    }
  }
  const after = [...before.filter((line) => !painted.some((done) => done.item === line.item)), ...painted];
  writeFileSync(MANIFEST, formatManifest(after));
  if (!painted.length) throw new Error("nothing was painted");
  sheet(painted.map((line) => line.item), renders, work, out);
  console.log(`ICONS painted ${painted.map((line) => line.item).join(", ")}; review ${out} and its -half.png, then commit assets/icons`);
}

if (import.meta.main) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(`ICONS ${error instanceof Error ? error.message : error}`);
    process.exit(1);
  });
}
