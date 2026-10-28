// Cuts a kind's concept sheet into its parts (D565 step 4; research/2026-10-28-assembled-parts-art.md
// section 1): each part drawn apart on the sheet's plain background is found as one region unlike
// the background, cropped with a margin and padded square on the background's own colour, the
// cut-out a part's model is made from. Parts in reading order: rows top to bottom, then left to right.
//
//   node tools/items/cutout.ts <sheet.png> <name>... [--margin 0.08]
//
// Writes the sheet's folder's <name>/cutout.png for each name, as many names as parts found.
// ImageMagick does the pixels (magick, as the props and icon tools use it).

import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";

type Box = { x: number; y: number; w: number; h: number };

// ImageMagick 6's `convert`, as WSL's Ubuntu ships it (7's `magick` takes the same arguments).
const magick = (args: string[]) => execFileSync("convert", args, { encoding: "utf8", maxBuffer: 1 << 26 });

/// The regions unlike the background's colour (its top-left pixel), larger than `area` pixels.
export function regions(sheet: string, area: number): Box[] {
  const bg = magick([sheet, "-format", "%[pixel:p{4,4}]", "info:"]).trim();
  const out = magick([
    sheet, "-fuzz", "7%", "-fill", "black", "-opaque", bg, "-fill", "white", "+opaque", "black",
    "-define", "connected-components:verbose=true", "-define", `connected-components:area-threshold=${area}`,
    "-connected-components", "8", "null:",
  ]);
  const boxes: Box[] = [];
  for (const line of out.split("\n")) {
    const m = line.match(/^\s*\d+: (\d+)x(\d+)\+(\d+)\+(\d+) [\d.]+,[\d.]+ (\d+) (?:srgb|gray)\((\d+)/);
    if (m && Number(m[6]) > 128) boxes.push({ w: +m[1], h: +m[2], x: +m[3], y: +m[4] });
  }
  return merged(boxes);
}

/// Boxes that overlap are one part: a highlight as pale as the background splits a part's region.
export function merged(boxes: Box[]): Box[] {
  const out = boxes.map((b) => ({ ...b }));
  for (let again = true; again; ) {
    again = false;
    for (let i = 0; i < out.length && !again; i++)
      for (let j = i + 1; j < out.length && !again; j++) {
        const [a, b] = [out[i], out[j]];
        if (a.x < b.x + b.w + 2 && b.x < a.x + a.w + 2 && a.y < b.y + b.h + 2 && b.y < a.y + a.h + 2) {
          const [x, y] = [Math.min(a.x, b.x), Math.min(a.y, b.y)];
          out[i] = { x, y, w: Math.max(a.x + a.w, b.x + b.w) - x, h: Math.max(a.y + a.h, b.y + b.h) - y };
          out.splice(j, 1);
          again = true;
        }
      }
  }
  return out;
}

/// Reading order: a part whose top is above another's middle comes first, then left to right.
export function reading(boxes: Box[]): Box[] {
  const rows: Box[][] = [];
  for (const b of [...boxes].sort((a, c) => a.y - c.y)) {
    const row = rows.find((r) => r.some((o) => b.y < o.y + o.h && o.y < b.y + b.h));
    if (row) row.push(b);
    else rows.push([b]);
  }
  return rows.flatMap((r) => r.sort((a, c) => a.x - c.x));
}

function main(argv: string[]) {
  const i = argv.indexOf("--margin");
  const margin = i >= 0 ? Number(argv.splice(i, 2)[1]) : 0.08;
  const [sheet, ...names] = argv;
  if (!sheet || names.length === 0) throw new Error("usage: node tools/items/cutout.ts <sheet.png> <name>... [--margin 0.08]");
  const bg = magick([sheet, "-format", "%[pixel:p{4,4}]", "info:"]).trim();
  const boxes = reading(regions(sheet, 2000));
  if (boxes.length !== names.length)
    throw new Error(`${sheet}: ${boxes.length} parts found, ${names.length} names given; name one a part in reading order, or redraw a sheet whose parts touch or run off its edge`);
  const [W, H] = magick([sheet, "-format", "%w %h", "info:"]).trim().split(" ").map(Number);
  boxes.forEach((b, k) => {
    const side = Math.round(Math.max(b.w, b.h) * (1 + 2 * margin));
    // The crop takes the margin from the sheet itself, so an edge's highlight as pale as the
    // background, left out of the region, is kept; then it is padded square.
    const m = Math.round(Math.max(b.w, b.h) * margin);
    const [x0, y0] = [Math.max(0, b.x - m), Math.max(0, b.y - m)];
    const [x1, y1] = [Math.min(W, b.x + b.w + m), Math.min(H, b.y + b.h + m)];
    const file = join(dirname(sheet), names[k], "cutout.png");
    mkdirSync(dirname(file), { recursive: true });
    magick([sheet, "-crop", `${x1 - x0}x${y1 - y0}+${x0}+${y0}`, "+repage", "-background", bg, "-gravity", "center", "-extent", `${side}x${side}`, file]);
    console.log(`✓ cutout: ${file}, ${b.w}x${b.h} at ${b.x},${b.y}`);
  });
}

if (import.meta.main) main(process.argv.slice(2));
