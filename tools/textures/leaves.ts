// Leaf cards composed from scanned leaves, as studios build their foliage atlases for
// SpeedTree and Unreal (Megascans' and SpeedTree's own leaf atlases): a twig, and single
// leaves cut from a scan laid along it, each at an angle, a size and a turn of its own, some
// young and yellower, some old and darker, a few turned over to their paler underside,
// overlapping with a soft shadow where one lies on another; and a normal map beside the
// colour, each leaf's scanned relief turned with it and tilted as the leaf is, so in the game
// every leaf catches the light at its own angle.
//
// A card's recipe is a JSON file (art/textures/cards/<card>.leaves.json); `textures.ts cards`
// composes it when a card is given one instead of a picture. Pure functions, no I/O, so the
// tests can check them on made-up scans.

/** A scan of leaves laid flat, its stalks toward the bottom, as ambientCG's leaf sets are:
 * colour and normal (OpenGL's convention: green up the picture) three bytes a pixel, its
 * opacity one. */
export type Scan = { width: number; height: number; colour: Buffer; normal: Buffer; alpha: Buffer };

/** One leaf found in a scan: where its stalk ends and its tip is, its box, and which
 * pixels are its own (`label`, shared by every leaf of the scan). */
export type ScanLeaf = { id: number; base: [number, number]; tip: [number, number]; box: [number, number, number, number]; pixels: number };

/** How a card is composed. Shares are of the card's width; colours sRGB from 0 to 1. */
export type Recipe = {
  /** ambientCG's id of the scan the leaves come from, such as LeafSet024. */
  scan: string;
  /** `spray`: leaves one by one along a twig and its shoots, as hazel and oak bear them;
   * `compound`: leaf stalks along a cane, each ending in three or five leaflets, as bramble;
   * `litter`: fallen leaves lying flat in a loose heap, with fallen needles, twigs (`shoots`
   * of them) and cones among them, for the forest floor. */
  form: "spray" | "compound" | "litter";
  /** How many leaves, or for `compound`, leaf stalks. */
  leaves: number;
  /** A leaf's length, stalk to tip, as a share of the card: least and most. */
  length: [number, number];
  /** Side shoots off the main twig. */
  shoots: number;
  twig: [number, number, number];
  /** The twig's thickness where it enters the card. */
  thickness: number;
  /** How far leaves differ in lightness, one from the next. */
  tone: number;
  /** Shares of leaves young (lighter, yellower), old (darker, olive) and turned over. */
  young: number;
  old: number;
  underside: number;
  /** The scan's colour graded: its saturation and lightness, as multiples. */
  saturation: number;
  value: number;
  /** The scan's colour multiplied, channel by channel, to shift its hue: a spring scan's yellow
   * toward summer's deeper green. */
  hue?: [number, number, number];
  /** Berries in bunches along the twig, if it bears them. */
  berries?: { count: number; colour: [number, number, number]; radius: number };
  /** For `litter`: fallen needles, how many and their colour, and pine cones. */
  needles?: { count: number; colour: [number, number, number]; length: [number, number] };
  cones?: { count: number; colour: [number, number, number]; length: number };
  seed: number;
};

/** A composed card: its colour and coverage, four bytes a pixel, and its normal map, three. */
export type Composed = { size: number; rgba: Buffer; normal: Buffer };

/** The leaves of a scan, each a connected patch of its opacity past half, the smallest left
 * out (bits broken off, specks of dust), with the label map that says which pixel is whose. */
export function scanLeaves(alpha: Buffer, width: number, height: number): { leaves: ScanLeaf[]; label: Int32Array } {
  const label = new Int32Array(width * height).fill(-1);
  const leaves: ScanLeaf[] = [];
  const stack = new Int32Array(width * height);
  for (let start = 0; start < width * height; start++) {
    if (alpha[start] < 128 || label[start] !== -1) continue;
    const id = leaves.length;
    let top = 0;
    stack[top++] = start;
    label[start] = id;
    let [x0, y0, x1, y1, count] = [width, height, 0, 0, 0];
    while (top) {
      const i = stack[--top];
      const [x, y] = [i % width, Math.floor(i / width)];
      count++;
      x0 = Math.min(x0, x);
      x1 = Math.max(x1, x);
      y0 = Math.min(y0, y);
      y1 = Math.max(y1, y);
      for (const j of [x > 0 ? i - 1 : -1, x < width - 1 ? i + 1 : -1, y > 0 ? i - width : -1, y < height - 1 ? i + width : -1]) {
        if (j >= 0 && label[j] === -1 && alpha[j] >= 128) {
          label[j] = id;
          stack[top++] = j;
        }
      }
    }
    // The stalk's end is the lowest of it, the tip the highest, each the middle of its row.
    const middle = (row: number) => {
      let [sum, n] = [0, 0];
      for (let x = x0; x <= x1; x++) if (label[row * width + x] === id) [sum, n] = [sum + x, n + 1];
      return n ? sum / n : (x0 + x1) / 2;
    };
    leaves.push({ id, base: [middle(y1), y1], tip: [middle(y0), y0], box: [x0, y0, x1 + 1, y1 + 1], pixels: count });
  }
  const largest = leaves.reduce((most, leaf) => Math.max(most, leaf.pixels), 0);
  return { leaves: leaves.filter((leaf) => leaf.pixels > largest * 0.2), label };
}

/** A small seeded random source, the same numbers for the same seed on every machine. */
export function dice(seed: number) {
  let state = (seed * 2654435761) >>> 0 || 1;
  const unit = () => {
    state ^= state << 13;
    state >>>= 0;
    state ^= state >>> 17;
    state ^= state << 5;
    state >>>= 0;
    return state / 4294967296;
  };
  return { unit, range: (a: number, b: number) => a + (b - a) * unit(), sign: () => (unit() < 0.5 ? -1 : 1) };
}

/** A leaf as laid on the card: where its stalk meets the twig, which way it points (radians,
 * the picture's y down), its length in pixels, how far it is turned about its own midrib
 * and tilted tip toward or away from the eye (radians; each foreshortens it), whether it is
 * mirrored or shows its underside, and its colour as a multiple. */
export type Laid = {
  leaf: number;
  at: [number, number];
  angle: number;
  length: number;
  turn: number;
  tilt: number;
  mirror: number;
  underside: boolean;
  tint: [number, number, number];
};

/** A normal from a scan's leaf (OpenGL's convention, green up its picture) as the leaf lies
 * on the card: its scan's midrib `axis` (a unit vector in the picture, y down, stalk to
 * tip), turned about its midrib by `turn`, tilted by `tilt`, mirrored, and pointed `angle`
 * on the card. Returned in the card's OpenGL convention, unit length. */
export function laidNormal(n: [number, number, number], axis: [number, number], laid: Pick<Laid, "angle" | "turn" | "tilt" | "mirror">): [number, number, number] {
  // In the scan's picture, y down: across its midrib and along it, toward the tip.
  const [ix, iy] = [n[0], -n[1]];
  const perp: [number, number] = [-axis[1], axis[0]];
  let across = (ix * perp[0] + iy * perp[1]) * laid.mirror;
  let along = ix * axis[0] + iy * axis[1];
  let z = n[2];
  // Turned about the midrib, then tilted about the line across it.
  [across, z] = [across * Math.cos(laid.turn) + z * Math.sin(laid.turn), -across * Math.sin(laid.turn) + z * Math.cos(laid.turn)];
  [along, z] = [along * Math.cos(laid.tilt) + z * Math.sin(laid.tilt), -along * Math.sin(laid.tilt) + z * Math.cos(laid.tilt)];
  const dir = [Math.cos(laid.angle), Math.sin(laid.angle)];
  const side = [-dir[1], dir[0]];
  const x = side[0] * across + dir[0] * along;
  const y = side[1] * across + dir[1] * along;
  const out: [number, number, number] = [x, -y, Math.max(z, 0.05)];
  const length = Math.hypot(...out);
  return out.map((v) => v / length) as [number, number, number];
}

/** A canvas a card is composed on: colour (linear, premultiplied by cover), cover, and the
 * normal, all floats. */
export class Canvas {
  colour: Float32Array;
  cover: Float32Array;
  normal: Float32Array;
  size: number;
  constructor(size: number) {
    this.size = size;
    this.colour = new Float32Array(size * size * 3);
    this.cover = new Float32Array(size * size);
    this.normal = new Float32Array(size * size * 3);
    for (let i = 0; i < size * size; i++) this.normal[i * 3 + 2] = 1;
  }

  /** Lays a patch over what is there: its cover, colour (linear) and normal at each pixel of
   * a box. First darkens what lies under it near its edge, a soft shadow a little down and
   * to the right, as leaves lying on each other shade the one below. */
  lay(box: [number, number, number, number], patch: { cover: Float32Array; colour: Float32Array; normal: Float32Array }, shadow: number) {
    const [x0, y0, x1, y1] = box;
    const [w, h] = [x1 - x0, y1 - y0];
    if (w <= 0 || h <= 0) return;
    if (shadow > 0) {
      const r = Math.max(2, Math.round(this.size * 0.008));
      const [dx, dy] = [Math.round(this.size * 0.004), Math.round(this.size * 0.007)];
      const pad = 3 * r + Math.max(dx, dy);
      const [bw, bh] = [w + 2 * pad, h + 2 * pad];
      let mask = new Float32Array(bw * bh);
      for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) mask[(y + pad) * bw + x + pad] = patch.cover[y * w + x];
      for (let pass = 0; pass < 3; pass++) mask = boxBlur(mask, bw, bh, r);
      for (let y = 0; y < bh; y++) {
        for (let x = 0; x < bw; x++) {
          const [cx, cy] = [x0 - pad + x + dx, y0 - pad + y + dy];
          if (cx < 0 || cy < 0 || cx >= this.size || cy >= this.size) continue;
          const dark = 1 - shadow * Math.min(1, mask[y * bw + x] * 1.4);
          const i = cy * this.size + cx;
          for (let c = 0; c < 3; c++) this.colour[i * 3 + c] *= dark;
        }
      }
    }
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const [cx, cy] = [x0 + x, y0 + y];
        if (cx < 0 || cy < 0 || cx >= this.size || cy >= this.size) continue;
        const a = patch.cover[y * w + x];
        if (a <= 0) continue;
        const i = cy * this.size + cx;
        const j = y * w + x;
        for (let c = 0; c < 3; c++) {
          this.colour[i * 3 + c] = patch.colour[j * 3 + c] * a + this.colour[i * 3 + c] * (1 - a);
          this.normal[i * 3 + c] = patch.normal[j * 3 + c] * a + this.normal[i * 3 + c] * (1 - a);
        }
        this.cover[i] = a + this.cover[i] * (1 - a);
      }
    }
  }
}

function boxBlur(data: Float32Array, w: number, h: number, r: number): Float32Array {
  const tmp = new Float32Array(w * h);
  const out = new Float32Array(w * h);
  for (let y = 0; y < h; y++) {
    let sum = 0;
    for (let x = -r; x <= r; x++) sum += data[y * w + Math.min(w - 1, Math.max(0, x))];
    for (let x = 0; x < w; x++) {
      tmp[y * w + x] = sum / (2 * r + 1);
      sum += data[y * w + Math.min(w - 1, x + r + 1)] - data[y * w + Math.max(0, x - r)];
    }
  }
  for (let x = 0; x < w; x++) {
    let sum = 0;
    for (let y = -r; y <= r; y++) sum += tmp[Math.min(h - 1, Math.max(0, y)) * w + x];
    for (let y = 0; y < h; y++) {
      out[y * w + x] = sum / (2 * r + 1);
      sum += tmp[Math.min(h - 1, y + r + 1) * w + x] - tmp[Math.max(0, y - r) * w + x];
    }
  }
  return out;
}

const linear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const encoded = (v: number) => (v <= 0.0031308 ? v * 12.92 : 1.055 * Math.max(0, v) ** (1 / 2.4) - 0.055);

/** A scan's leaf drawn as `laid` lays it: each pixel of the box it covers sampled back from
 * the scan, bilinearly, from that leaf's own pixels only. */
function drawLeaf(scan: Scan, label: Int32Array, leaf: ScanLeaf, laid: Laid, recipe: Recipe) {
  const [bx, by] = leaf.base;
  const axisLength = Math.hypot(leaf.tip[0] - bx, leaf.tip[1] - by);
  const axis: [number, number] = [(leaf.tip[0] - bx) / axisLength, (leaf.tip[1] - by) / axisLength];
  const perp = [-axis[1], axis[0]];
  const dir = [Math.cos(laid.angle), Math.sin(laid.angle)];
  const side = [-dir[1], dir[0]];
  const along = laid.length * Math.cos(laid.tilt);
  const across = laid.length * Math.cos(laid.turn);
  // Scan to card: a point's share along the midrib and across it, in midrib lengths.
  const toCard = (sx: number, sy: number): [number, number] => {
    const a = ((sx - bx) * axis[0] + (sy - by) * axis[1]) / axisLength;
    const c = (((sx - bx) * perp[0] + (sy - by) * perp[1]) / axisLength) * laid.mirror;
    return [laid.at[0] + dir[0] * a * along + side[0] * c * across, laid.at[1] + dir[1] * a * along + side[1] * c * across];
  };
  const [sx0, sy0, sx1, sy1] = leaf.box;
  const corners = [toCard(sx0, sy0), toCard(sx1, sy0), toCard(sx0, sy1), toCard(sx1, sy1)];
  const x0 = Math.floor(Math.min(...corners.map((p) => p[0]))) - 1;
  const y0 = Math.floor(Math.min(...corners.map((p) => p[1]))) - 1;
  const x1 = Math.ceil(Math.max(...corners.map((p) => p[0]))) + 1;
  const y1 = Math.ceil(Math.max(...corners.map((p) => p[1]))) + 1;
  const [w, h] = [x1 - x0, y1 - y0];
  const patch = { cover: new Float32Array(w * h), colour: new Float32Array(w * h * 3), normal: new Float32Array(w * h * 3) };
  const grey = (r: number, g: number, b: number) => 0.2126 * r + 0.7152 * g + 0.0722 * b;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const [px, py] = [x0 + x + 0.5 - laid.at[0], y0 + y + 0.5 - laid.at[1]];
      const a = (px * dir[0] + py * dir[1]) / along;
      const c = ((px * side[0] + py * side[1]) / across) * laid.mirror;
      const sx = bx + axis[0] * a * axisLength + perp[0] * c * axisLength - 0.5;
      const sy = by + axis[1] * a * axisLength + perp[1] * c * axisLength - 0.5;
      const ix = Math.floor(sx);
      const iy = Math.floor(sy);
      const [fx, fy] = [sx - ix, sy - iy];
      let cover = 0;
      const colour = [0, 0, 0];
      for (const [ox, oy, wt] of [[0, 0, (1 - fx) * (1 - fy)], [1, 0, fx * (1 - fy)], [0, 1, (1 - fx) * fy], [1, 1, fx * fy]]) {
        const [qx, qy] = [ix + ox, iy + oy];
        if (qx < 0 || qy < 0 || qx >= scan.width || qy >= scan.height) continue;
        const q = qy * scan.width + qx;
        if (label[q] !== leaf.id && scan.alpha[q] >= 128) continue;
        // A neighbour's pixels never; the edge's own soft pixels, unlabelled, do count.
        const k = (scan.alpha[q] / 255) * wt;
        cover += k;
        for (let ch = 0; ch < 3; ch++) {
          colour[ch] += linear(scan.colour[q * 3 + ch] / 255) * k;
        }
      }
      if (cover <= 0.002) continue;
      const j = y * w + x;
      patch.cover[j] = Math.min(1, cover);
      let [r, g, b] = colour.map((v) => v / cover);
      // Graded: saturation about its own grey, then lightness.
      const l = grey(r, g, b);
      [r, g, b] = [r, g, b].map((v, c) => Math.max(0, (l + (v - l) * recipe.saturation) * recipe.value * (recipe.hue?.[c] ?? 1)));
      if (laid.underside) {
        // A leaf's underside: paler and greyer, a little blue, its veins standing out less.
        const u = grey(r, g, b);
        [r, g, b] = [(r * 0.6 + u * 0.4) * 1.2, (g * 0.6 + u * 0.4) * 1.18, (b * 0.6 + u * 0.4) * 1.3];
      }
      // A little darker toward the stalk, where a leaf's base lies in its neighbours' shade.
      const base = 0.82 + 0.18 * Math.min(1, Math.max(0, a * 1.6));
      patch.colour[j * 3] = r * laid.tint[0] * base;
      patch.colour[j * 3 + 1] = g * laid.tint[1] * base;
      patch.colour[j * 3 + 2] = b * laid.tint[2] * base;
      // The normal, from the nearest of the leaf's own pixels.
      const q = Math.min(scan.height - 1, Math.max(0, Math.round(sy))) * scan.width + Math.min(scan.width - 1, Math.max(0, Math.round(sx)));
      const n: [number, number, number] = [scan.normal[q * 3] / 127.5 - 1, scan.normal[q * 3 + 1] / 127.5 - 1, scan.normal[q * 3 + 2] / 127.5 - 1];
      const out = laidNormal(laid.underside ? [-n[0], n[1], n[2]] : n, axis, laid);
      patch.normal.set(out, j * 3);
    }
  }
  return { box: [x0, y0, x1, y1] as [number, number, number, number], patch };
}

/** A twig drawn along points: round, darker at its sides, its normal turned across it. */
function drawTwig(canvas: Canvas, points: [number, number][], radius: [number, number], colour: [number, number, number]) {
  const lengths = [0];
  for (let i = 1; i < points.length; i++) lengths.push(lengths[i - 1] + Math.hypot(points[i][0] - points[i - 1][0], points[i][1] - points[i - 1][1]));
  const total = lengths[lengths.length - 1];
  const xs = points.map((p) => p[0]);
  const ys = points.map((p) => p[1]);
  const pad = radius[0] + 2;
  const box: [number, number, number, number] = [
    Math.floor(Math.min(...xs) - pad), Math.floor(Math.min(...ys) - pad), Math.ceil(Math.max(...xs) + pad), Math.ceil(Math.max(...ys) + pad),
  ];
  const [w, h] = [box[2] - box[0], box[3] - box[1]];
  const patch = { cover: new Float32Array(w * h), colour: new Float32Array(w * h * 3), normal: new Float32Array(w * h * 3) };
  const tone = colour.map(linear);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const p = [box[0] + x + 0.5, box[1] + y + 0.5];
      // The nearest point of the twig, and how far along it that is.
      let best = Infinity;
      let signed = 0;
      let t = 0;
      for (let i = 1; i < points.length; i++) {
        const [a, b] = [points[i - 1], points[i]];
        const [ex, ey] = [b[0] - a[0], b[1] - a[1]];
        const len2 = ex * ex + ey * ey || 1;
        const s = Math.min(1, Math.max(0, ((p[0] - a[0]) * ex + (p[1] - a[1]) * ey) / len2));
        const [qx, qy] = [a[0] + ex * s - p[0], a[1] + ey * s - p[1]];
        const d = Math.hypot(qx, qy);
        if (d < best) {
          best = d;
          signed = (ex * (p[1] - a[1]) - ey * (p[0] - a[0])) / Math.sqrt(len2);
          t = (lengths[i - 1] + s * Math.sqrt(len2)) / total;
        }
      }
      const r = radius[0] + (radius[1] - radius[0]) * t;
      const cover = Math.min(1, Math.max(0, r - best + 0.5));
      if (cover <= 0) continue;
      const j = y * w + x;
      const s = Math.max(-1, Math.min(1, signed / r));
      const round = Math.sqrt(1 - s * s);
      patch.cover[j] = cover;
      for (let c = 0; c < 3; c++) patch.colour[j * 3 + c] = tone[c] * (0.55 + 0.45 * round);
      // Across the twig, in the picture's y down; the twig's own direction at this point is
      // close enough to its nearest segment's for a stem a few pixels wide.
      const k = Math.min(points.length - 1, Math.max(1, Math.round(t * (points.length - 1))));
      const [ex, ey] = [points[k][0] - points[k - 1][0], points[k][1] - points[k - 1][1]];
      const len = Math.hypot(ex, ey) || 1;
      const [nx, ny] = [-ey / len, ex / len];
      patch.normal[j * 3] = nx * s;
      patch.normal[j * 3 + 1] = -ny * s;
      patch.normal[j * 3 + 2] = round;
    }
  }
  canvas.lay(box, patch, 0);
}

/** A berry: a little sphere, its own shading gentle, its normal round. */
function drawBerry(canvas: Canvas, at: [number, number], radius: number, colour: [number, number, number]) {
  const box: [number, number, number, number] = [Math.floor(at[0] - radius - 1), Math.floor(at[1] - radius - 1), Math.ceil(at[0] + radius + 1), Math.ceil(at[1] + radius + 1)];
  const [w, h] = [box[2] - box[0], box[3] - box[1]];
  const patch = { cover: new Float32Array(w * h), colour: new Float32Array(w * h * 3), normal: new Float32Array(w * h * 3) };
  const tone = colour.map(linear);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const [dx, dy] = [(box[0] + x + 0.5 - at[0]) / radius, (box[1] + y + 0.5 - at[1]) / radius];
      const d = Math.hypot(dx, dy);
      const cover = Math.min(1, Math.max(0, (1 - d) * radius + 0.5));
      if (cover <= 0) continue;
      const z = Math.sqrt(Math.max(0, 1 - Math.min(1, d) ** 2));
      const j = y * w + x;
      patch.cover[j] = cover;
      // A bilberry's bloom: a dusty sheen, palest where it faces out.
      for (let c = 0; c < 3; c++) patch.colour[j * 3 + c] = tone[c] * (0.6 + 0.4 * z) + 0.02 * z * z;
      patch.normal.set([dx, -dy, z].map((v) => v / Math.max(1e-6, Math.hypot(dx, dy, z))), j * 3);
    }
  }
  canvas.lay(box, patch, 0.35);
}

/** A curve from `from` heading `angle`, bending by `bend` over its length, as points. */
function curve(from: [number, number], angle: number, length: number, bend: number, steps = 12): [number, number][] {
  const points: [number, number][] = [from];
  let [x, y, a] = [from[0], from[1], angle];
  for (let i = 0; i < steps; i++) {
    a += bend / steps;
    x += (Math.cos(a) * length) / steps;
    y += (Math.sin(a) * length) / steps;
    points.push([x, y]);
  }
  return points;
}

/** A point and the heading of a curve, a share of the way along it. */
function along(points: [number, number][], t: number): { at: [number, number]; heading: number } {
  const f = Math.min(points.length - 1.001, Math.max(0, t * (points.length - 1)));
  const i = Math.floor(f);
  const [a, b] = [points[i], points[i + 1]];
  const s = f - i;
  return { at: [a[0] + (b[0] - a[0]) * s, a[1] + (b[1] - a[1]) * s], heading: Math.atan2(b[1] - a[1], b[0] - a[0]) };
}

/** Composes a card `size` pixels square from a scan's leaves by its recipe. Drawn at twice
 * the size and halved, for smooth edges. */
export function compose(scan: Scan, recipe: Recipe, size: number): Composed {
  const { leaves, label } = scanLeaves(scan.alpha, scan.width, scan.height);
  if (!leaves.length) throw new Error(`${recipe.scan} has no leaves in its opacity`);
  const big = size * 2;
  const room = Math.round(big * 0.2);
  const canvas = new Canvas(big + 2 * room);
  const roll = dice(recipe.seed);
  if (recipe.form === "litter") {
    floor(canvas, scan, label, leaves, recipe, roll, big, room);
    return halve(fit(canvas, big), size);
  }
  // The twig enters at one side low down and runs up across the card, its shoots off it.
  const start: [number, number] = [room + big * 0.06, room + big * roll.range(0.6, 0.78)];
  const heading = -roll.range(0.2, 0.45);
  const main = curve(start, heading, big * 0.82, roll.range(-0.5, 0.2));
  const twigs: [number, number][][] = [main];
  for (let k = 0; k < recipe.shoots; k++) {
    const from = along(main, roll.range(0.25, 0.65));
    const out = from.heading + roll.sign() * roll.range(0.45, 0.85);
    twigs.push(curve(from.at, out, big * roll.range(0.32, 0.5), roll.range(-0.4, 0.4), 8));
  }
  const thick = recipe.thickness * big;
  twigs.forEach((points, k) => drawTwig(canvas, points, k ? [thick * 0.6, thick * 0.3] : [thick, thick * 0.35], recipe.twig));

  // Every leaf as laid: where along which twig, its side, and its look.
  const laid: Laid[] = [];
  const look = (t: number): Pick<Laid, "turn" | "tilt" | "mirror" | "underside" | "tint" | "leaf"> => {
    const roll2 = roll.unit();
    // Young leaves toward the tips, old toward the base.
    const young = roll2 < recipe.young * (0.4 + 1.2 * t);
    const old = !young && roll.unit() < recipe.old * (1.6 - 1.2 * t);
    const v = 1 + roll.range(-recipe.tone, recipe.tone * 0.6);
    const tint: [number, number, number] = young ? [v * 1.08, v * 1.06, v * 0.8] : old ? [v * 0.72, v * 0.74, v * 0.55] : [v, v, v * roll.range(0.9, 1.05)];
    // Mostly a little turned, some steeply, so they foreshorten and catch the light apart.
    const turn = roll.sign() * (roll.unit() < 0.25 ? roll.range(0.6, 1.15) : roll.range(0.0, 0.5));
    const tilt = roll.sign() * roll.range(0.0, 0.7);
    return { leaf: leaves[Math.floor(roll.unit() * leaves.length)].id, turn, tilt, mirror: roll.sign(), underside: roll.unit() < recipe.underside, tint };
  };
  const length = (t: number) => big * roll.range(recipe.length[0], recipe.length[1]) * (1 - 0.35 * t);
  if (recipe.form === "spray") {
    const weights = twigs.map((_, k) => (k ? 0.6 : 1));
    const total = weights.reduce((a, b) => a + b, 0);
    twigs.forEach((points, k) => {
      const n = Math.max(2, Math.round((recipe.leaves * weights[k]) / total));
      for (let i = 0; i < n; i++) {
        const last = i === n - 1;
        const t = last ? 1 : 0.15 + (0.8 * (i + roll.range(0, 0.6))) / n;
        const { at, heading } = along(points, t);
        const side = i % 2 ? 1 : -1;
        const angle = last ? heading + roll.range(-0.2, 0.2) : heading + side * roll.range(0.55, 1.15);
        laid.push({ at, angle, length: length(t), ...look(t) });
      }
    });
  } else {
    // Leaf stalks along the cane, alternate, each with its leaflets fanned from its end:
    // the terminal one largest, then pairs, the lowest pair smallest.
    for (let i = 0; i < recipe.leaves; i++) {
      const t = 0.12 + (0.82 * (i + roll.range(0.2, 0.8))) / recipe.leaves;
      const { at, heading } = along(main, t);
      const side = i % 2 ? 1 : -1;
      const out = heading + side * roll.range(0.5, 0.9);
      const stalk = curve(at, out, big * roll.range(0.07, 0.11), roll.range(-0.3, 0.3), 4);
      drawTwig(canvas, stalk, [thick * 0.45, thick * 0.3], recipe.twig);
      const tip = stalk[stalk.length - 1];
      const end = Math.atan2(tip[1] - stalk[stalk.length - 2][1], tip[0] - stalk[stalk.length - 2][0]);
      const five = roll.unit() < 0.5;
      const fan: [number, number][] = five ? [[0, 1], [0.95, 0.85], [-0.95, 0.85], [1.9, 0.6], [-1.9, 0.6]] : [[0, 1], [1.05, 0.82], [-1.05, 0.82]];
      const leaflet = length(t);
      for (const [spread, scale] of fan) {
        laid.push({ at: tip, angle: end + spread * roll.range(0.8, 1.1), length: leaflet * scale, ...look(t) });
      }
    }
  }
  // Drawn from the back: the farther leaves first, a little darker, so those in front shade them.
  laid.sort(() => roll.unit() - 0.5);
  laid.forEach((leaf, k) => {
    const back = 0.8 + (0.2 * k) / Math.max(1, laid.length - 1);
    const drawn = drawLeaf(scan, label, leaves.find((l) => l.id === leaf.leaf)!, { ...leaf, tint: leaf.tint.map((v) => v * back) as [number, number, number] }, recipe);
    canvas.lay(drawn.box, drawn.patch, 0.5);
  });
  if (recipe.berries) {
    const { count, colour, radius } = recipe.berries;
    for (let k = 0; k < count; k++) {
      const { at } = along(twigs[k % twigs.length], roll.range(0.3, 0.95));
      for (let b = 0; b < 2 + Math.floor(roll.unit() * 2); b++) {
        const r = radius * big * roll.range(0.8, 1.15);
        drawBerry(canvas, [at[0] + roll.range(-1.5, 1.5) * r, at[1] + roll.range(0.3, 2.2) * r], r, colour.map((c) => c * roll.range(0.8, 1.15)) as [number, number, number]);
      }
    }
  }
  return halve(fit(canvas, big), size);
}

/** What a canvas covers, scaled and moved to fill a canvas `to` pixels square, short of its
 * edge by a hair, keeping its shape. */
function fit(canvas: Canvas, to: number): Canvas {
  const n = canvas.size;
  let [x0, y0, x1, y1] = [n, n, 0, 0];
  for (let y = 0; y < n; y++) {
    for (let x = 0; x < n; x++) {
      if (canvas.cover[y * n + x] < 0.01) continue;
      [x0, y0, x1, y1] = [Math.min(x0, x), Math.min(y0, y), Math.max(x1, x + 1), Math.max(y1, y + 1)];
    }
  }
  const out = new Canvas(to);
  if (x1 <= x0) return out;
  const scale = (to * 0.98) / Math.max(x1 - x0, y1 - y0);
  const [cx, cy] = [(x0 + x1) / 2, (y0 + y1) / 2];
  for (let y = 0; y < to; y++) {
    for (let x = 0; x < to; x++) {
      const sx = cx + (x + 0.5 - to / 2) / scale - 0.5;
      const sy = cy + (y + 0.5 - to / 2) / scale - 0.5;
      const [ix, iy] = [Math.floor(sx), Math.floor(sy)];
      const [fx, fy] = [sx - ix, sy - iy];
      const i = y * to + x;
      out.normal[i * 3 + 2] = 0;
      for (const [ox, oy, w] of [[0, 0, (1 - fx) * (1 - fy)], [1, 0, fx * (1 - fy)], [0, 1, (1 - fx) * fy], [1, 1, fx * fy]]) {
        const [qx, qy] = [ix + ox, iy + oy];
        if (qx < 0 || qy < 0 || qx >= n || qy >= n) {
          out.normal[i * 3 + 2] += w;
          continue;
        }
        const q = qy * n + qx;
        out.cover[i] += canvas.cover[q] * w;
        for (let c = 0; c < 3; c++) {
          out.colour[i * 3 + c] += canvas.colour[q * 3 + c] * w;
          out.normal[i * 3 + c] += canvas.normal[q * 3 + c] * w;
        }
      }
    }
  }
  return out;
}

/** The canvas halved, each pixel the four under it averaged by their cover, in sRGB bytes. */
function halve(canvas: Canvas, size: number): Composed {
  const rgba = Buffer.alloc(size * size * 4);
  const normal = Buffer.alloc(size * size * 3);
  const big = canvas.size;
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      let cover = 0;
      const colour = [0, 0, 0];
      const n = [0, 0, 0];
      for (const [dx, dy] of [[0, 0], [1, 0], [0, 1], [1, 1]]) {
        const i = (y * 2 + dy) * big + x * 2 + dx;
        const a = canvas.cover[i];
        cover += a / 4;
        for (let c = 0; c < 3; c++) {
          colour[c] += canvas.colour[i * 3 + c];
          n[c] += canvas.normal[i * 3 + c] * a + (c === 2 ? 1 - a : 0);
        }
      }
      const j = y * size + x;
      rgba[j * 4 + 3] = Math.round(cover * 255);
      for (let c = 0; c < 3; c++) rgba[j * 4 + c] = Math.round(255 * Math.min(1, encoded(cover > 0 ? colour[c] / 4 / cover : 0)));
      if (rgba[j * 4 + 3] === 0) [n[0], n[1], n[2]] = [0, 0, 1];
      const len = Math.hypot(n[0], n[1], n[2]) || 1;
      for (let c = 0; c < 3; c++) normal[j * 3 + c] = Math.round((n[c] / len) * 127.5 + 127.5);
    }
  }
  return { size, rgba, normal };
}

/** A forest floor's litter, `litter` recipes: fallen leaves lying flat in a loose heap, each
 * at a turn of its own, a little curled (turned about its midrib) and tilted, browned from the
 * scan's green by the recipe's grading, some paler and yellower, some dark and wet; fallen
 * needles strewn under and over them; small twigs; and pine cones. All within the middle of
 * the card, thinning out toward its edge, so a card laid on the ground has no edge. */
function floor(canvas: Canvas, scan: Scan, label: Int32Array, leaves: ScanLeaf[], recipe: Recipe, roll: ReturnType<typeof dice>, big: number, room: number) {
  const middle: [number, number] = [room + big / 2, room + big / 2];
  // A place in the heap: Gaussian round its middle, kept within the card.
  const place = (spread: number): [number, number] => {
    for (;;) {
      const r = Math.sqrt(-2 * Math.log(Math.max(1e-4, roll.unit()))) * spread * big;
      const a = roll.range(0, Math.PI * 2);
      if (r < big * 0.42) return [middle[0] + Math.cos(a) * r, middle[1] + Math.sin(a) * r];
    }
  };
  const strew = (count: number) => {
    const needles = recipe.needles;
    if (!needles) return;
    for (let k = 0; k < count; k++) {
      const at = place(0.2);
      const angle = roll.range(0, Math.PI * 2);
      const length = big * roll.range(needles.length[0], needles.length[1]);
      const tone = roll.range(0.7, 1.2);
      const colour = needles.colour.map((c) => Math.min(1, c * tone)) as [number, number, number];
      drawTwig(canvas, curve(at, angle, length, roll.range(-0.25, 0.25), 3), [big * 0.0028, big * 0.0018], colour);
    }
  };
  strew(Math.round((recipe.needles?.count ?? 0) * 0.6));
  for (let k = 0; k < recipe.shoots; k++) {
    const at = place(0.16);
    const angle = roll.range(0, Math.PI * 2);
    const length = big * roll.range(0.3, 0.6);
    const thick = recipe.thickness * big * roll.range(0.6, 1.2);
    const stick = curve([at[0] - (Math.cos(angle) * length) / 2, at[1] - (Math.sin(angle) * length) / 2], angle, length, roll.range(-0.5, 0.5), 8);
    drawTwig(canvas, stick, [thick, thick * 0.6], recipe.twig.map((c) => c * roll.range(0.8, 1.15)) as [number, number, number]);
    // A side twig or two off it.
    for (let s = 0; s < 2; s++) {
      if (roll.unit() < 0.4) continue;
      const from = along(stick, roll.range(0.25, 0.75));
      drawTwig(canvas, curve(from.at, from.heading + roll.sign() * roll.range(0.4, 0.9), length * roll.range(0.15, 0.3), roll.range(-0.3, 0.3), 4), [thick * 0.6, thick * 0.4], recipe.twig);
    }
  }
  const laid: Laid[] = [];
  for (let k = 0; k < recipe.leaves; k++) {
    const roll2 = roll.unit();
    const young = roll2 < recipe.young;
    const old = !young && roll.unit() < recipe.old;
    const v = 1 + roll.range(-recipe.tone, recipe.tone * 0.6);
    const tint: [number, number, number] = young ? [v * 1.2, v * 1.08, v * 0.7] : old ? [v * 0.55, v * 0.5, v * 0.45] : [v, v * roll.range(0.92, 1.04), v * roll.range(0.85, 1.05)];
    laid.push({
      leaf: leaves[Math.floor(roll.unit() * leaves.length)].id,
      at: place(0.17),
      angle: roll.range(0, Math.PI * 2),
      length: big * roll.range(recipe.length[0], recipe.length[1]),
      // Fallen leaves curl as they dry: turned about their midribs, a little tilted.
      turn: roll.sign() * roll.range(0.1, 0.75),
      tilt: roll.sign() * roll.range(0.0, 0.35),
      mirror: roll.sign(),
      underside: roll.unit() < recipe.underside,
      tint,
    });
  }
  laid.forEach((leaf) => {
    const drawn = drawLeaf(scan, label, leaves.find((l) => l.id === leaf.leaf)!, leaf, recipe);
    canvas.lay(drawn.box, drawn.patch, 0.45);
  });
  if (recipe.cones) {
    for (let k = 0; k < recipe.cones.count; k++) {
      drawCone(canvas, place(0.12), roll.range(0, Math.PI * 2), recipe.cones.length * big * roll.range(0.85, 1.15), recipe.cones.colour.map((c) => c * roll.range(0.85, 1.1)) as [number, number, number]);
    }
  }
  strew(Math.round((recipe.needles?.count ?? 0) * 0.4));
}

/** A pine cone lying on its side, seen from above: an egg pointed at its tip, in rows of
 * woody scales, each lighter at its rim and dark in the gap under the next, shaded round. */
function drawCone(canvas: Canvas, at: [number, number], angle: number, length: number, colour: [number, number, number]) {
  const reach = length * 0.6;
  const box: [number, number, number, number] = [Math.floor(at[0] - reach), Math.floor(at[1] - reach), Math.ceil(at[0] + reach), Math.ceil(at[1] + reach)];
  const [w, h] = [box[2] - box[0], box[3] - box[1]];
  const patch = { cover: new Float32Array(w * h), colour: new Float32Array(w * h * 3), normal: new Float32Array(w * h * 3) };
  const tone = colour.map(linear);
  const [dx, dy] = [Math.cos(angle), Math.sin(angle)];
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const [px, py] = [box[0] + x + 0.5 - at[0], box[1] + y + 0.5 - at[1]];
      // Along the cone from its foot (-0.5) to its tip (0.5), and across it.
      const u = (px * dx + py * dy) / length;
      const v = (-px * dy + py * dx) / length;
      // Widest a third of the way from its foot, narrowing to a point.
      const t = u + 0.5;
      const half = 0.22 * Math.sin(Math.PI * Math.min(1, Math.max(0, t)) ** 0.8);
      if (t <= 0 || t >= 1 || half <= 0) continue;
      const s = v / half;
      const edge = (1 - Math.abs(s)) * half * length;
      const cover = Math.min(1, Math.max(0, edge + 0.5));
      if (cover <= 0) continue;
      const round = Math.sqrt(Math.max(0, 1 - s * s));
      // Woody scales in staggered rows, each a rounded plate whose rim, toward the foot,
      // stands out pale over the dark gap under the next row.
      const around = Math.asin(Math.max(-1, Math.min(1, s))) / Math.PI + 0.5;
      const row = t * 8;
      const across0 = around * 3.5 + 0.5 * Math.floor(row);
      const [fr, fc] = [row - Math.floor(row), across0 - Math.floor(across0)];
      const plate = Math.hypot((fc - 0.5) * 1.6, fr - 0.35);
      const scale = Math.min(1, plate);
      const lip = (0.4 + 0.6 * (1 - Math.min(1, plate * 1.4)) ** 0.6) * (fr < 0.12 ? 0.55 + fr * 3.5 : 1) * (0.85 + 0.3 * (1 - fr));
      const j = y * w + x;
      patch.cover[j] = cover;
      for (let c = 0; c < 3; c++) patch.colour[j * 3 + c] = tone[c] * (0.35 + 0.65 * round) * lip;
      // Round across it, and each scale tipped toward the cone's foot.
      const across = s * 0.8;
      const tip = (0.35 - fr) * 0.8 * (1 - scale * 0.5);
      const nx = -dy * across + dx * tip;
      const ny = dx * across + dy * tip;
      const len = Math.hypot(nx, ny, round) || 1;
      patch.normal.set([nx / len, -ny / len, round / len], j * 3);
    }
  }
  canvas.lay(box, patch, 0.6);
}
