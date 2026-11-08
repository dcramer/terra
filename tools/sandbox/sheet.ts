#!/usr/bin/env node
// Several stills as one picture, to be looked at once (D71, D627): a series from a scripted
// run (a fight a quarter second apart, a walk into a cave) laid out in rows, each still cut
// to the part that matters, made small, and lifted if it is dark. One read of the sheet
// costs what one still does; reading each costs that many times over.
//
//   node tools/sandbox/sheet.ts STILL.png... --out SHEET.png [--crop W,H,X,Y] [--wide 320] [--across N] [--lift STOPS]
//
// --crop takes that part of each still (its width, height, and left and top edge); --wide is
// each tile's width in pixels; --across how many to a row (as near square as may be if not
// given); --lift brightens by so many stops (1 doubles), for stills taken in the dark.
// Node 22.18 or later; no dependencies.

import { readPng, writePng, type Picture } from "./png.ts";

/// A part of a picture: so wide and high, from so far in from its left and top.
export function crop(from: Picture, [width, height, left, top]: number[]): Picture {
  const [w, h] = [Math.min(width, from.width - left), Math.min(height, from.height - top)];
  const rgba = new Uint8Array(w * h * 4);
  for (let y = 0; y < h; y++) {
    const row = ((top + y) * from.width + left) * 4;
    rgba.set(from.rgba.subarray(row, row + w * 4), y * w * 4);
  }
  return { width: w, height: h, rgba };
}

/// A picture made so wide, its height in keeping, each pixel the mean of those it covers,
/// lifted by so many stops.
export function small(from: Picture, wide: number, lift = 0): Picture {
  const width = Math.min(wide, from.width);
  const height = Math.max(1, Math.round((from.height * width) / from.width));
  const gain = 2 ** lift;
  const rgba = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) {
    const [y0, y1] = [Math.floor((y * from.height) / height), Math.max(Math.floor(((y + 1) * from.height) / height), Math.floor((y * from.height) / height) + 1)];
    for (let x = 0; x < width; x++) {
      const [x0, x1] = [Math.floor((x * from.width) / width), Math.max(Math.floor(((x + 1) * from.width) / width), Math.floor((x * from.width) / width) + 1)];
      const sum = [0, 0, 0];
      for (let v = y0; v < y1; v++) for (let u = x0; u < x1; u++) for (let c = 0; c < 3; c++) sum[c] += from.rgba[(v * from.width + u) * 4 + c];
      const count = (y1 - y0) * (x1 - x0);
      const at = (y * width + x) * 4;
      for (let c = 0; c < 3; c++) rgba[at + c] = Math.min(255, Math.round((sum[c] / count) * gain));
      rgba[at + 3] = 255;
    }
  }
  return { width, height, rgba };
}

/// Pictures of one size laid out so many to a row, in order, a dark line between them.
export function sheet(tiles: Picture[], across: number): Picture {
  const [w, h, gap] = [tiles[0].width, tiles[0].height, 2];
  const rows = Math.ceil(tiles.length / across);
  const [width, height] = [across * (w + gap) - gap, rows * (h + gap) - gap];
  const rgba = new Uint8Array(width * height * 4);
  for (let at = 3; at < rgba.length; at += 4) rgba[at] = 255;
  tiles.forEach((tile, index) => {
    const [left, top] = [(index % across) * (w + gap), Math.floor(index / across) * (h + gap)];
    for (let y = 0; y < Math.min(h, tile.height); y++) rgba.set(tile.rgba.subarray(y * tile.width * 4, (y * tile.width + Math.min(w, tile.width)) * 4), ((top + y) * width + left) * 4);
  });
  return { width, height, rgba };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const option = (flag: string) => (args.includes(flag) ? args[args.indexOf(flag) + 1] : undefined);
  const flags = ["--out", "--crop", "--wide", "--across", "--lift"];
  const stills = args.filter((arg, at) => !flags.includes(arg) && !flags.includes(args[at - 1] ?? ""));
  const out = option("--out");
  if (!out || stills.length === 0) {
    console.error("usage: sheet.ts STILL.png... --out SHEET.png [--crop W,H,X,Y] [--wide 320] [--across N] [--lift STOPS]");
    process.exit(2);
  }
  const part = option("--crop")?.split(",").map(Number);
  if (part && (part.length !== 4 || part.some((n) => !Number.isFinite(n)))) {
    console.error(`--crop wants a width, height, left and top, as 900,760,510,160, not ${option("--crop")}`);
    process.exit(2);
  }
  const tiles = stills.map((path) => {
    const still = readPng(path);
    return small(part ? crop(still, part) : still, Number(option("--wide") ?? 320), Number(option("--lift") ?? 0));
  });
  const across = Number(option("--across") ?? Math.ceil(Math.sqrt(tiles.length)));
  const laid = sheet(tiles, across);
  writePng(out, laid);
  console.log(`sheet: ${tiles.length} stills, ${across} to a row, ${laid.width}x${laid.height}, in ${out}`);
}
