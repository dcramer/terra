#!/usr/bin/env node
// The marks a character bears, as shown on the arm (D590): each a glyph drawn here by rule,
// white on nothing, which the game tints and lights (`terra_game::play::marks::shown`), so a
// new mark is a row of data and a glyph here, not a painted picture. The troll's is the
// spiral the gate's stones are carved with (art/props/realm_gate), with a tail and three
// dots beside its stem. Written to assets/models/character/marks/<key>.png.
//
//   node tools/characters/marks.ts
//
// Node 22.18 or later; needs ImageMagick's convert.

import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "../..");
const OUT = resolve(ROOT, "assets/models/character/marks");
const SIZE = 256;

type Glyph = { key: string; strokes: [number, number][][]; dots: [number, number, number][] };

/// A spiral about a middle, out from `from` to `to` px over so many turns, as points.
function spiral(middle: [number, number], from: number, to: number, turns: number, start: number): [number, number][] {
  const steps = Math.ceil(turns * 48);
  return Array.from({ length: steps + 1 }, (_, step) => {
    const along = step / steps;
    const angle = start + along * turns * 2 * Math.PI;
    const radius = from + (to - from) * along;
    return [middle[0] + Math.cos(angle) * radius, middle[1] + Math.sin(angle) * radius];
  });
}

/// The troll's: a fiddlehead, the spiral wound out from its middle and let down in a stem,
/// with three dots beside the stem.
const troll: Glyph = {
  key: "troll",
  strokes: [
    // Ending at its rightmost point, heading down, so the stem runs on from it unbroken.
    [...spiral([118, 108], 6, 74, 2.25, -Math.PI * 0.5), [192, 150], [192, 238]],
  ],
  dots: [
    [36, 188, 6],
    [62, 214, 8],
    [94, 232, 6.5],
  ],
};

mkdirSync(OUT, { recursive: true });
let failed = false;
for (const glyph of [troll]) {
  const draw = [
    ...glyph.strokes.map((points) => `polyline ${points.map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`).join(" ")}`),
    ...glyph.dots.map(([x, y, r]) => `circle ${x},${y} ${x + r},${y}`),
  ];
  const file = resolve(OUT, `${glyph.key}.png`);
  const made = spawnSync(
    "convert",
    ["-size", `${SIZE}x${SIZE}`, "xc:none", "-stroke", "white", "-strokewidth", "11", "-fill", "none", "-draw", `stroke-linecap round stroke-linejoin round ${draw.slice(0, glyph.strokes.length).join(" ")}`, "-fill", "white", "-stroke", "none", "-draw", draw.slice(glyph.strokes.length).join(" "), "-blur", "0x0.8", "-strip", file],
    { encoding: "utf8" },
  );
  if (made.status !== 0) {
    failed = true;
    console.error(`✗ marks: ${glyph.key} was not drawn: ${made.stderr.trim() || "convert is not installed (ImageMagick)"}`);
  } else console.log(`✓ marks: ${glyph.key} drawn to assets/models/character/marks/${glyph.key}.png`);
}
process.exit(failed ? 1 : 0);
