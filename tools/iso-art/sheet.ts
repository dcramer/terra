// The asset sheet and the clip sheet (iso-art, judging an asset): every asset the asset view
// shot (crates/terra_iso's `cargo iso --asset NAME --shot DIR/NAME.png`, which writes NAME.png, NAME-<clip>-<k>.jpg
// and NAME.json), so a judge reads them all at once at the game camera.
//
// - sheet.png: each asset's still cropped to what is not the view's grey ground, all at one
//   scale (the view's is the 1080p game frame's, 106 px a metre), labelled with its name, its
//   source (this pipeline, with a ledger in art/iso, or the repository's), its kind and its
//   triangles against its budget (kinds.ts, D630); above them crops of the refs' creatures at
//   their store frames' own 1080p scale, from refs/creatures.json.
// - clips.png: each animated asset a row of its clips, each clip its frames left to right,
//   cropped to the box every frame of the asset fits and scaled to one row height.
//
// Both stay under LONG pixels on their long side. Node and ffmpeg (drawtext with Liberation
// Sans), no dependencies: images are decoded and resized by stats.ts, laid out here, and ffmpeg
// draws the labels and writes the PNG.
//
//   node tools/iso-art/iso-art.ts sheet [--dir DIR]   (DIR holds views/ and refs/; the default
//   ~/.cache/terra-review/iso-art; writes DIR/sheet.png and DIR/clips.png)

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { homedir } from "node:os";
import { basename, join } from "node:path";
import { measure } from "./check.ts";
import { readGlb } from "./gltf.ts";
import { type Kind, budget } from "./kinds.ts";
import { type Rgb, decodeAny, resizeLanczos } from "./stats.ts";

export const DIR = join(homedir(), ".cache", "terra-review", "iso-art");
const FONT = "/usr/share/fonts/liberation/LiberationSans-Regular.ttf";
const BOLD = "/usr/share/fonts/liberation/LiberationSans-Bold.ttf";
/** The longest side a sheet may have: a judge reads it whole. */
export const LONG = 3900;
const BG: [number, number, number] = [28, 28, 30];

export type ViewClip = { name: string; seconds: number; loops: boolean; frames: { at: number; file: string }[] };
export type View = { asset: string; still: string; clips: ViewClip[]; crop: [number, number]; px_per_m: number };

/** What the sheet says of an asset: where it came from and its triangles against its budget. */
export type Facts = { name: string; source: "Tripo pipeline" | "repository"; kind: Kind; triangles: number; budget: number; height: number; texture: number };

export function readViews(dir: string): View[] {
  return readdirSync(dir)
    .filter((f) => f.endsWith(".json"))
    .map((f) => JSON.parse(readFileSync(join(dir, f), "utf8")))
    .filter((v) => typeof v.asset === "string" && typeof v.still === "string")
    .sort((a, b) => a.asset.localeCompare(b.asset));
}

export function facts(root: string, v: View): Facts {
  const name = basename(v.asset, ".glb");
  const ledger = join(root, "art", "iso", name, "asset.json");
  let kind: Kind;
  let source: Facts["source"];
  if (existsSync(ledger)) [kind, source] = [JSON.parse(readFileSync(ledger, "utf8")).kind, "Tripo pipeline"];
  else if (v.asset.startsWith("models/creatures/")) [kind, source] = ["creature", "repository"];
  else throw new Error(`${v.asset} has neither a ledger art/iso/${name}/asset.json nor a place in models/creatures, so its kind and budget are unknown (kinds.ts): adopt it (\`iso-art.ts adopt\`) or leave it out of the view's folder`);
  const m = measure(readGlb(readFileSync(join(root, "assets", v.asset))));
  return { name, source, kind, triangles: m.triangles, budget: budget(kind, m), height: m.height, texture: Math.max(0, ...m.textures) };
}

export type Box = [number, number, number, number]; // x0, y0, x1, y1, end exclusive

/** The part of a view's frame that is not its plain ground: pixels further than `threshold`
 * from the border's median colour, in rows and columns with at least three of them (so a
 * JPEG's ringing is not content), widened by `margin`. The whole frame if nothing stands out. */
export function contentBox(im: Rgb, threshold = 14, margin = 8): Box {
  const { width: w, height: h, data } = im;
  const edge: number[][] = [[], [], []];
  const take = (x: number, y: number) => { for (let c = 0; c < 3; c++) edge[c].push(data[(y * w + x) * 3 + c]); };
  for (let x = 0; x < w; x++) { take(x, 0); take(x, h - 1); }
  for (let y = 0; y < h; y++) { take(0, y); take(w - 1, y); }
  const bg = edge.map((e) => e.sort((a, b) => a - b)[e.length >> 1]);
  const cols = new Uint32Array(w), rows = new Uint32Array(h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const i = (y * w + x) * 3;
    if (Math.max(Math.abs(data[i] - bg[0]), Math.abs(data[i + 1] - bg[1]), Math.abs(data[i + 2] - bg[2])) > threshold) { cols[x]++; rows[y]++; }
  }
  const first = (a: Uint32Array) => a.findIndex((n) => n >= 3);
  const last = (a: Uint32Array) => a.length - 1 - [...a].reverse().findIndex((n) => n >= 3);
  if (first(cols) < 0) return [0, 0, w, h];
  return [Math.max(0, first(cols) - margin), Math.max(0, first(rows) - margin), Math.min(w, last(cols) + 1 + margin), Math.min(h, last(rows) + 1 + margin)];
}

export const union = (bs: Box[]): Box => [Math.min(...bs.map((b) => b[0])), Math.min(...bs.map((b) => b[1])), Math.max(...bs.map((b) => b[2])), Math.max(...bs.map((b) => b[3]))];

export function crop(im: Rgb, [x0, y0, x1, y1]: Box): Rgb {
  const width = x1 - x0, height = y1 - y0;
  const data = new Uint8Array(width * height * 3);
  for (let y = 0; y < height; y++) data.set(im.data.subarray(((y0 + y) * im.width + x0) * 3, ((y0 + y) * im.width + x1) * 3), y * width * 3);
  return { width, height, data };
}

const canvas = (width: number, height: number): Rgb => {
  const data = new Uint8Array(width * height * 3);
  for (let i = 0; i < data.length; i += 3) data.set(BG, i);
  return { width, height, data };
};

function blit(dst: Rgb, src: Rgb, x: number, y: number): void {
  for (let r = 0; r < src.height; r++) dst.data.set(src.data.subarray(r * src.width * 3, (r + 1) * src.width * 3), ((y + r) * dst.width + x) * 3);
}

/** Rows left to right, a new row when the next would pass `maxWidth`: each item's place, and the
 * width and height they take. */
export function shelf(sizes: { w: number; h: number }[], maxWidth: number, gap: number): { at: [number, number][]; width: number; height: number } {
  const at: [number, number][] = [];
  let x = 0, y = 0, row = 0, width = 0;
  for (const s of sizes) {
    if (x > 0 && x + s.w > maxWidth) { y += row + gap; x = 0; row = 0; }
    at.push([x, y]);
    x += s.w + gap;
    row = Math.max(row, s.h);
    width = Math.max(width, x - gap);
  }
  return { at, width, height: sizes.length ? y + row : 0 };
}

type Text = { x: number; y: number; text: string; size: number; color: string; bold?: boolean };

/** The canvas with its labels drawn, scaled by `scale`, written as a PNG by ffmpeg. */
function encode(im: Rgb, texts: Text[], out: string, scale = 1): void {
  // drawtext's own escaping has two levels; labels keep to characters that need none.
  const plain = (s: string) => s.replace(/[^A-Za-z0-9 ._()/+-]/g, " ");
  const vf = texts.map((t) => `drawtext=fontfile=${t.bold ? BOLD : FONT}:expansion=none:text='${plain(t.text)}':x=${Math.round(t.x)}:y=${Math.round(t.y)}:fontsize=${Math.round(t.size)}:fontcolor=${t.color}`);
  if (scale < 1) vf.push(`scale=${Math.round(im.width * scale)}:${Math.round(im.height * scale)}:flags=lanczos`);
  execFileSync("ffmpeg", ["-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24", "-video_size", `${im.width}x${im.height}`, "-i", "-", "-vf", vf.join(",") || "null", "-frames:v", "1", out], { input: Buffer.from(im.data.buffer, im.data.byteOffset, im.data.length), maxBuffer: 1 << 26 });
}

const k = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n));

type Tile = { img: Rgb; lines: { text: string; color: string; bold?: boolean }[] };

/** sheet.png: the refs' crops in a row, then every asset's still, tallest first, at one scale. */
export function assetSheet(root: string, dir: string, out: string): string {
  const views = join(dir, "views");
  const refTiles: Tile[] = [];
  const refsFile = join(dir, "refs", "creatures.json");
  if (existsSync(refsFile)) for (const r of JSON.parse(readFileSync(refsFile, "utf8")) as { file: string; crop: number[]; note: string }[]) {
    const [x, y, w, h] = r.crop;
    refTiles.push({ img: crop(decodeAny(join(dir, "refs", r.file)), [x, y, x + w, y + h]), lines: [{ text: `REF ${basename(r.file).replace(/\.\w+$/, "")}`, color: "0xffd860", bold: true }, { text: r.note, color: "0xc8c8c8" }, { text: "1 to 1 with its 1080p store frame", color: "0xc8c8c8" }] });
  }
  const all = readViews(views);
  const assetTiles: Tile[] = all.map((v) => {
    const f = facts(root, v);
    const still = decodeAny(join(views, v.still));
    const over = f.triangles > f.budget;
    return {
      img: crop(still, contentBox(still)),
      lines: [
        { text: f.name, color: "white", bold: true },
        { text: `${f.source}  ${f.kind}`, color: f.source === "repository" ? "0x9cc8ff" : "0xc8c8c8" },
        { text: `${k(f.triangles)} tris of ${k(f.budget)}${over ? "  OVER" : ""}`, color: over ? "0xff6a5a" : "0x8ee08e", bold: over },
        { text: `${f.height.toFixed(2)} m  ${v.clips.length} clips  tex ${f.texture}`, color: "0xc8c8c8" },
      ],
    };
  });
  assetTiles.sort((a, b) => b.img.height - a.img.height);
  // The largest scale at which the sheet fits LONG, labels at one size on the final image.
  for (const s of [1, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3]) {
    const line = 22 / s, gap = 18 / s, title = 44 / s, font = 17 / s;
    const size = (t: Tile) => ({ w: Math.max(t.img.width, Math.max(...t.lines.map((l) => l.text.length)) * font * 0.56), h: t.img.height + t.lines.length * line + 6 / s });
    const refs = shelf(refTiles.map(size), LONG / s, gap);
    const assets = shelf(assetTiles.map(size), LONG / s, gap);
    const top = title + (refTiles.length ? refs.height + gap * 2 : 0);
    const width = Math.ceil(Math.max(refs.width, assets.width, 1600 / s) + 2 * gap), height = Math.ceil(top + assets.height + 2 * gap);
    if (Math.max(width, height) * s > LONG && s > 0.3) continue;
    const im = canvas(width, height);
    const texts: Text[] = [{ x: gap, y: 10 / s, size: 24 / s, color: "white", bold: true, text: `iso-art assets at the game camera  ${all[0]?.px_per_m ?? 106} px a metre  shown ${s === 1 ? "1 to 1" : `at ${s}`}  refs above at the same scale  ${new Date().toISOString().slice(0, 10)}` }];
    const place = (tiles: Tile[], at: [number, number][], y0: number) => tiles.forEach((t, i) => {
      const [x, y] = [Math.round(gap + at[i][0]), Math.round(y0 + at[i][1])];
      blit(im, t.img, x, y);
      t.lines.forEach((l, j) => texts.push({ x, y: y + t.img.height + 4 / s + j * line, text: l.text, size: font, color: l.color, bold: l.bold }));
    });
    place(refTiles, refs.at, title);
    place(assetTiles, assets.at, top);
    encode(im, texts, out, s);
    return `sheet ${out}: ${assetTiles.length} assets, ${refTiles.length} refs, ${Math.round(width * s)}x${Math.round(height * s)} at ${s}`;
  }
  throw new Error("unreachable");
}

/** clips.png: each animated asset's clips, a row of frames a clip, wrapped at LONG. */
export function clipSheet(root: string, dir: string, out: string): string {
  const views = join(dir, "views");
  const animated = readViews(views).filter((v) => v.clips.length > 0);
  const assets = animated.map((v) => {
    const frames = v.clips.map((c) => c.frames.map((f) => decodeAny(join(views, f.file))));
    return { v, f: facts(root, v), frames, box: union(frames.flat().map((im) => contentBox(im, 16))) };
  });
  const LABEL = 230, GAP = 10, CLIP = 24, PAD = 2;
  for (const cell of [150, 130, 110, 90, 70]) {
    const placed: { a: (typeof assets)[number]; scale: number; cw: number; groups: [number, number][] }[] = [];
    let y = 54;
    for (const a of assets) {
      const [bw, bh] = [a.box[2] - a.box[0], a.box[3] - a.box[1]];
      const scale = Math.min(cell / bh, 3);
      const cw = Math.max(1, Math.round(bw * scale));
      const groups: [number, number][] = [];
      let x = LABEL;
      for (const c of a.v.clips) {
        const gw = c.frames.length * (cw + PAD);
        if (x > LABEL && x + gw > LONG) { x = LABEL; y += CLIP + cell + GAP; }
        groups.push([x, y]);
        x += gw + GAP * 2;
      }
      placed.push({ a, scale, cw, groups });
      y += CLIP + cell + GAP * 3;
    }
    if (y > LONG && cell > 70) continue;
    const width = Math.min(LONG, Math.max(...placed.flatMap((p) => p.groups.map(([x], i) => x + p.a.v.clips[i].frames.length * (p.cw + PAD)))) + GAP);
    const im = canvas(width, y);
    const texts: Text[] = [{ x: GAP, y: 14, size: 24, color: "white", bold: true, text: `iso-art clips from the asset view  each asset scaled to a ${cell} px row (its scale against the game frame under its name)  frames evenly through each clip` }];
    for (const { a, scale, cw, groups } of placed) {
      const ch = Math.round((a.box[3] - a.box[1]) * scale);
      const [, y0] = groups[0];
      texts.push({ x: GAP, y: y0 + CLIP, text: a.f.name, size: 20, color: "white", bold: true }, { x: GAP, y: y0 + CLIP + 26, text: a.f.source, size: 16, color: a.f.source === "repository" ? "0x9cc8ff" : "0xc8c8c8" }, { x: GAP, y: y0 + CLIP + 48, text: `x${scale.toFixed(2)} of the game frame`, size: 16, color: "0xc8c8c8" });
      a.v.clips.forEach((c, i) => {
        const [gx, gy] = groups[i];
        texts.push({ x: gx, y: gy + 2, text: `${c.name}  ${c.seconds.toFixed(2)} s${c.loops ? "  loops" : ""}`, size: 17, color: "0xffd860" });
        a.frames[i].forEach((fr, j) => blit(im, resizeLanczos(crop(fr, a.box), cw, ch), gx + j * (cw + PAD), gy + CLIP + Math.round((cell - ch) / 2)));
      });
    }
    encode(im, texts, out);
    return `clips ${out}: ${placed.length} animated assets, ${assets.reduce((n, a) => n + a.frames.flat().length, 0)} frames, ${width}x${y}, ${cell} px a row`;
  }
  throw new Error("unreachable");
}
