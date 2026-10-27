// Colour and see-through by height in flame crops, image only (no matte), so our stills and another
// game's (RDR2's) read alike; the fire rounds' measure of whether a flame's top parts into tongues.
// Usage: node tools/fire/tips.ts <label>=<file>@<WxH+X+Y>[@...] ...  (a strip: <label>=<file>@strip:<wxh+x+y>
// takes that crop in each of its 8 frames, 360x300 in a 4x2 grid at the top).
// Per label, the mean over its crops of, by third of the flame's height (top, mid, low; from its top
// row, tipstop.ts, to the crop's foot): pale (white + pale) %, white %, yellow %, orange+red %;
// fill (flame pixels over each row's span, leftmost to rightmost flame pixel: 1 a solid body, under 1
// separate tongues with dark between); runs (separate flame runs a row, gaps of 2 px or more); dim %
// (flame pixels under half the crop's flame p90 luma: thin, see-through flame). And measure.ts's
// whole-crop white, pale and yellow % (the sheet's numbers, here without a matte).
import { colourBand, isFlame, load, luma, measure } from "./measure.ts";
import { BLOB, topRow } from "./tipstop.ts";

// The top row is the first with 3 pixels of a blob of at least TIPS_BLOB (default BLOB) of the
// crop's flame (tipstop.ts); TIPS_BLOB=0 is the rule before fire round g61 (any 3 bright pixels).
// RDR2's two campfire stills by it: top third fill 0.57, runs 3.56 (0.57, 3.46 by the old rule).
const SHARE = process.env.TIPS_BLOB === undefined ? BLOB : Number(process.env.TIPS_BLOB);

// Flame here is also bright (its largest channel at least BRIGHT): lit ground by a fire passes isFlame.
const BRIGHT = 215;
type Crop = { w: number; h: number; x: number; y: number };
const box = (s: string): Crop => {
  const m = /^(\d+)x(\d+)\+(\d+)\+(\d+)$/.exec(s);
  if (!m) throw new Error(`bad crop ${s}`);
  return { w: +m[1], h: +m[2], x: +m[3], y: +m[4] };
};
const f1 = (v: number) => v.toFixed(1);
const f2 = (v: number) => v.toFixed(2);
console.log("label            third  pale  white yellow o+r   fill  runs  dim | whole: white pale yellow (n crops)");
for (const arg of process.argv.slice(2)) {
  const [label, rest] = arg.split("=");
  const [file, ...specs] = rest.split("@");
  const pic = load(file);
  const crops: Crop[] = [];
  for (const s of specs) {
    if (s.startsWith("strip:")) {
      const c = box(s.slice(6));
      for (let k = 0; k < 8; k++) crops.push({ ...c, x: c.x + (k % 4) * 360, y: c.y + Math.floor(k / 4) * 300 });
    } else crops.push(box(s));
  }
  const acc = [0, 1, 2].map(() => ({ pale: 0, white: 0, yellow: 0, or: 0, fill: 0, runs: 0, dim: 0 }));
  const whole = { white: 0, pale: 0, yellow: 0 };
  for (const c of crops) {
    const at = (x: number, y: number) => {
      const i = ((c.y + y) * pic.width + c.x + x) * 4;
      return [pic.rgba[i], pic.rgba[i + 1], pic.rgba[i + 2]];
    };
    const flame: boolean[][] = [];
    const lumas: number[] = [];
    for (let y = 0; y < c.h; y++) {
      const row: boolean[] = [];
      for (let x = 0; x < c.w; x++) {
        const [r, g, b] = at(x, y);
        const f = isFlame(r, g, b) && Math.max(r, g, b) >= BRIGHT;
        row.push(f);
        if (f) lumas.push(luma(r, g, b));
      }
      flame.push(row);
    }
    const top = topRow(flame, SHARE);
    lumas.sort((a, b) => a - b);
    const p90 = lumas[Math.floor(lumas.length * 0.9)] ?? 255;
    const H = c.h - Math.max(top, 0);
    for (let t = 0; t < 3; t++) {
      const y0 = top + Math.floor((t * H) / 3);
      const y1 = top + Math.floor(((t + 1) * H) / 3);
      const n = { all: 0, pale: 0, white: 0, yellow: 0, or: 0, span: 0, runs: 0, rows: 0, dim: 0 };
      for (let y = y0; y < y1; y++) {
        let left = -1;
        let right = -1;
        let runs = 0;
        let gap = 99;
        for (let x = 0; x < c.w; x++) {
          if (!flame[y][x]) {
            gap++;
            continue;
          }
          if (gap >= 2) runs++;
          gap = 0;
          if (left < 0) left = x;
          right = x;
          const [r, g, b] = at(x, y);
          const band = colourBand(r, g, b);
          n.all++;
          if (band === "white" || band === "pale") n.pale++;
          if (band === "white") n.white++;
          if (band === "yellow") n.yellow++;
          if (band === "orange" || band === "red") n.or++;
          if (luma(r, g, b) < 0.5 * p90) n.dim++;
        }
        if (left >= 0) {
          n.span += right - left + 1;
          n.runs += runs;
          n.rows++;
        }
      }
      const a = acc[t];
      const all = Math.max(n.all, 1);
      a.pale += (100 * n.pale) / all;
      a.white += (100 * n.white) / all;
      a.yellow += (100 * n.yellow) / all;
      a.or += (100 * n.or) / all;
      a.fill += n.all / Math.max(n.span, 1);
      a.runs += n.runs / Math.max(n.rows, 1);
      a.dim += (100 * n.dim) / all;
    }
    const m = measure(pic, c);
    whole.white += m.white;
    whole.pale += m.pale;
    whole.yellow += m.yellow;
  }
  const k = crops.length;
  ["top", "mid", "low"].forEach((name, t) => {
    const a = acc[t];
    const tail = t === 0 ? ` | ${f1(whole.white / k)} ${f1(whole.pale / k)} ${f1(whole.yellow / k)} (${k})` : "";
    console.log(
      `${label.padEnd(16)} ${name.padEnd(5)} ${f1(a.pale / k).padStart(5)} ${f1(a.white / k).padStart(5)} ${f1(a.yellow / k).padStart(5)} ${f1(a.or / k).padStart(5)} ${f2(a.fill / k).padStart(5)} ${f2(a.runs / k).padStart(5)} ${f1(a.dim / k).padStart(5)}${tail}`,
    );
  });
}
