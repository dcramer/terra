// Leaf cards composed from scanned leaves, as studios build their foliage atlases for
// SpeedTree and Unreal (Megascans' and SpeedTree's own leaf atlases): a twig, and single
// leaves cut from a scan laid along it, each at an angle, a size and a turn of its own, some
// young and yellower, some old and darker, a few turned over to their paler underside,
// overlapping with a soft shadow where one lies on another; and a normal map beside the
// colour, each leaf's scanned relief turned with it and tilted as the leaf is, so in the game
// every leaf catches the light at its own angle. A conifer's card is drawn needle by needle
// instead, as SpeedTree's needle atlases are rendered from modelled needles: a twig and its
// shoots clothed all round in short needles, so its edge is a fringe of single needles, not
// the cut outline of a picture of a whole bough (D276).
//
// A card's recipe is a JSON file (art/textures/cards/<card>.leaves.json); `textures.ts cards`
// composes it when a card is given one instead of a picture. Pure functions, no I/O, so the
// tests can check them on made-up scans.

/** A scan of leaves laid flat, its stalks toward the bottom, as ambientCG's leaf sets are:
 * colour and normal (OpenGL's convention: green up the picture) three bytes a pixel, its
 * opacity one, and its roughness one, where the set has it. */
export type Scan = { width: number; height: number; colour: Buffer; normal: Buffer; alpha: Buffer; roughness?: Buffer };

/** One leaf found in a scan: where its stalk ends and its tip is, its box, and which
 * pixels are its own (`label`, shared by every leaf of the scan). */
export type ScanLeaf = { id: number; base: [number, number]; tip: [number, number]; box: [number, number, number, number]; pixels: number };

/** How a card is composed. Shares are of the card's width; colours sRGB from 0 to 1. */
export type Recipe = {
  /** ambientCG's id of the scan the leaves come from, such as LeafSet024; none for needles. */
  scan?: string;
  /** `spray`: leaves one by one along a twig and its shoots, as hazel and oak bear them;
   * `compound`: leaf stalks along a cane, each ending in three or five leaflets, as bramble;
   * `litter`: fallen leaves lying flat in a loose heap, with fallen needles, twigs (`shoots`
   * of them) and cones among them, for the forest floor; `needles`: a conifer's branch end,
   * every twig clothed all round in needles (`needle`), drawn rather than cut from a scan. */
  form: "spray" | "compound" | "litter" | "needles";
  /** How many leaves, or for `compound`, leaf stalks; for `needles`, the shoots each side
   * shoot has of its own, at most. */
  leaves: number;
  /** A leaf's length, stalk to tip, as a share of the card: least and most; a needle's. */
  length: [number, number];
  /** For `needles`: an old needle's colour and a young one's at the shoots' tips (sRGB), a
   * needle's width at its base as a share of the card, and how far apart needles are along
   * a twig, as a share of the card; how far, in radians, a needle leans forward off its twig,
   * least and most (0.5 to 1.0 by default, a spruce's), and how many leave each place along it
   * (3: a Scots pine's stand in pairs, so a brush of them is thicker). */
  needle?: { colour: [number, number, number]; young: [number, number, number]; width: number; every: number; lean?: [number, number]; per?: number };
  /** For `needles`: how far the side shoots turn off the main twig, in radians, least and
   * most (0.8 to 1.15 by default, a spruce's flat spray): a Scots pine's run forward and up,
   * a tuft. Its `shoot` says where along the main twig they leave it (0.08 to 0.88) and how
   * long they are (0.42 to 0.58 of the card). */
  spread?: [number, number];
  /** For `needles`: how much of the card the main twig runs up (0.92): shorter, with its
   * shoots fanned from its top, a tuft is as wide as it is long. */
  stalk?: number;
  /** Side shoots off the main twig. */
  shoots: number;
  /** Where along the main twig the shoots leave it, as shares of its length, and how long
   * they are, as shares of the card: by default a quarter to two thirds along, a third to a
   * half of the card long. Small
   * leaves need the shoots spread wider to fill the card. */
  shoot?: { from: [number, number]; length: [number, number] };
  twig: [number, number, number];
  /** The twig's thickness where it enters the card. */
  thickness: number;
  /** How far leaves differ in lightness, one from the next. */
  tone: number;
  /** Shares of leaves young (lighter, yellower), old (darker, olive) and turned over. */
  young: number;
  old: number;
  underside: number;
  /** The scan's colour graded, where no `albedo` is given: its saturation and lightness, as multiples. */
  saturation?: number;
  value?: number;
  /** The scan's colour multiplied, channel by channel, to shift its hue: a spring scan's yellow
   * toward summer's deeper green. */
  hue?: [number, number, number];
  /** The average colour of the leaves' upper faces, sRGB, as a summer leaf of the kind is
   * measured (D292). The scan is scaled to it channel by channel in linear light, so each
   * scanned leaf keeps its own veins, blemishes and shading; `saturation`, `value` and `hue`
   * grade it instead where this is not given. */
  albedo?: [number, number, number];
  /** How far a leaf's own light and dark is deepened about its average: 1 as scanned. A scan
   * is lit flat and evenly, so its veins and mottling read fainter than on a living leaf. */
  contrast?: number;
  /** The upper faces' roughness on average (perceptual, 0 to 1): the scan's own roughness
   * map, its veins rougher than the blade, scaled to average this. A waxy leaf is glossy above,
   * every underside matte (`UNDERSIDE_ROUGH`). Without it, a card is as rough as `ROUGH`. */
  roughness?: number;
  /** How far, at most, a leaf folds up along its midrib, its halves facing each other a
   * little (radians), and how far its tip curls over: each leaf then shades from one half to
   * the other and from base to tip, as a real leaf does, not as one flat plane. */
  fold?: number;
  curl?: number;
  /** Shares of leaves yellowing in patches in from their margin and tip, as leaves do when
   * old or short of water, and bitten or holed by insects. */
  yellowed?: number;
  holes?: number;
  /** How far a leaf's hue wanders from the next, greener or yellower. */
  hues?: number;
  /** Berries in bunches along the twig, if it bears them. */
  berries?: { count: number; colour: [number, number, number]; radius: number };
  /** For `litter`: fallen needles, how many and their colour, and pine cones. */
  needles?: { count: number; colour: [number, number, number]; length: [number, number] };
  cones?: { count: number; colour: [number, number, number]; length: number };
  seed: number;
};

/** A composed card: its colour and coverage, four bytes a pixel, and its normal map, three;
 * and, for a twig of leaves or needles, how long they are on average, as a share of the card's
 * width, so a plant can be grown with its leaves at their real size (D276). */
export type Composed = { size: number; rgba: Buffer; normal: Buffer; rough: Buffer; leaf?: number };

/** How rough a card is where its recipe or picture says nothing: nearly matte, since a
 * picture's own painted light already has its sheen, and at the leaf material's 0.8 a fern
 * by the camera went chalky in the sun. */
export const ROUGH = 0.9;
/** A leaf's underside, hairy or waxless and greyer: matte whatever its upper face. */
const UNDERSIDE_ROUGH = 0.88;
/** Bark of a twig, a needle's waxy face, a berry's bloom. */
const TWIG_ROUGH = 0.75;
const BERRY_ROUGH = 0.45;
// A needle is too thin for a card's one normal to glint true: glossier, a spruce's cards went
// white against the sun.
const NEEDLE_ROUGH = 0.85;

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
  /** Folded up along its midrib and its tip curled over (radians), as `Recipe.fold` and
   * `curl`; yellowing in from its margin, 0 to 1, by a noise of its own (`seed`); holes
   * bitten in it, each where along its midrib and across it (in midrib lengths) and how
   * wide; and young (-1), grown (0) or old (1), which a leaf's gloss follows. */
  fold?: number;
  curl?: number;
  yellow?: number;
  holes?: [number, number, number][];
  age?: number;
  seed?: number;
};

/** What a scan's leaves are on average, measured once: their colour (linear), their
 * roughness, and how far in from its margin each pixel is (`inside`, 0 at the edge to 1 a
 * fifteenth of a leaf in). */
export type ScanStats = { mean: [number, number, number]; rough: number; inside: Float32Array };

export function scanStats(scan: Scan, leaves: ScanLeaf[]): ScanStats {
  const mean = [0, 0, 0];
  let [rough, n] = [0, 0];
  for (let i = 0; i < scan.width * scan.height; i++) {
    if (scan.alpha[i] < 128) continue;
    for (let c = 0; c < 3; c++) mean[c] += linear(scan.colour[i * 3 + c] / 255);
    rough += scan.roughness ? scan.roughness[i] / 255 : ROUGH;
    n++;
  }
  const lengths = leaves.map((l) => Math.hypot(l.tip[0] - l.base[0], l.tip[1] - l.base[1])).sort((a, b) => a - b);
  const r = Math.max(1, Math.round((lengths[Math.floor(lengths.length / 2)] ?? 30) / 30));
  let inside = new Float32Array(scan.width * scan.height);
  for (let i = 0; i < inside.length; i++) inside[i] = scan.alpha[i] >= 128 ? 1 : 0;
  for (let pass = 0; pass < 3; pass++) inside = boxBlur(inside, scan.width, scan.height, r);
  // Half covered at the margin itself; past twice that, all the way in.
  for (let i = 0; i < inside.length; i++) inside[i] = Math.min(1, Math.max(0, (inside[i] - 0.5) * 2));
  return { mean: mean.map((v) => v / Math.max(1, n)) as [number, number, number], rough: rough / Math.max(1, n), inside };
}

/** Smooth value noise in the plane, 0 to 1, the same for the same seed. */
function noise2(x: number, y: number, seed: number): number {
  const hash = (i: number, j: number) => {
    let h = (i * 374761393 + j * 668265263 + seed * 2246822519) >>> 0;
    h = Math.imul(h ^ (h >>> 13), 1274126177) >>> 0;
    return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
  };
  const [i, j] = [Math.floor(x), Math.floor(y)];
  const [fx, fy] = [x - i, y - j];
  const [u, v] = [fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)];
  const top = hash(i, j) + (hash(i + 1, j) - hash(i, j)) * u;
  const bottom = hash(i, j + 1) + (hash(i + 1, j + 1) - hash(i, j + 1)) * u;
  return top + (bottom - top) * v;
}

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

/** What is drawn onto a canvas: cover, colour (linear), normal and, where it says, roughness,
 * at each pixel of a box. */
export type Patch = { cover: Float32Array; colour: Float32Array; normal: Float32Array; rough?: Float32Array };

/** A canvas a card is composed on: colour (linear, premultiplied by cover), cover, the
 * normal and the roughness, all floats. */
export class Canvas {
  colour: Float32Array;
  cover: Float32Array;
  normal: Float32Array;
  rough: Float32Array;
  size: number;
  constructor(size: number) {
    this.size = size;
    this.colour = new Float32Array(size * size * 3);
    this.cover = new Float32Array(size * size);
    this.normal = new Float32Array(size * size * 3);
    this.rough = new Float32Array(size * size).fill(ROUGH);
    for (let i = 0; i < size * size; i++) this.normal[i * 3 + 2] = 1;
  }

  /** Lays a patch over what is there: its cover, colour (linear), normal and roughness at
   * each pixel of a box. First darkens what lies under it near its edge, a soft shadow a
   * little down and to the right, as leaves lying on each other shade the one below. */
  lay(box: [number, number, number, number], patch: Patch, shadow: number) {
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
        // What lay here before shows through where the patch is part covered; under nothing,
        // the patch's own.
        const under = this.cover[i] > 0 ? this.rough[i] : (patch.rough?.[j] ?? ROUGH);
        this.rough[i] = (patch.rough?.[j] ?? ROUGH) * a + under * (1 - a);
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
 * the scan, bilinearly, from that leaf's own pixels only. Its colour graded to the recipe's
 * (to its `albedo`, or by its saturation and lightness), its normal turned with it and, if it
 * is folded or curled, bent across and along it, and its roughness the scan's. */
export function drawLeaf(scan: Scan, label: Int32Array, leaf: ScanLeaf, laid: Laid, recipe: Recipe, stats?: ScanStats) {
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
  const patch = { cover: new Float32Array(w * h), colour: new Float32Array(w * h * 3), normal: new Float32Array(w * h * 3), rough: new Float32Array(w * h).fill(ROUGH) };
  const grey = (r: number, g: number, b: number) => 0.2126 * r + 0.7152 * g + 0.0722 * b;
  // Graded to the measured albedo: each channel scaled so the scan's average is the kind's.
  const target = recipe.albedo && stats ? recipe.albedo.map((c, k) => linear(c) / Math.max(1e-4, stats.mean[k])) : undefined;
  const contrast = recipe.contrast ?? 1;
  const roughScale = recipe.roughness !== undefined && stats ? recipe.roughness / Math.max(0.05, stats.rough) : undefined;
  const width = 0.5 * Math.max(leaf.box[2] - leaf.box[0], leaf.box[3] - leaf.box[1]) / axisLength;
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
      const n = [0, 0, 0];
      let [rough, inside] = [0, 0];
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
          n[ch] += (scan.normal[q * 3 + ch] / 127.5 - 1) * k;
        }
        rough += (scan.roughness ? scan.roughness[q] / 255 : ROUGH) * k;
        inside += (stats?.inside[q] ?? 1) * k;
      }
      // A hole an insect bit: nothing, and a thin dead rim round it.
      let rim = 0;
      for (const [ha, hc, hr] of laid.holes ?? []) {
        // Ragged, as a bite is, not a punched circle.
        const d = Math.hypot(a - ha, (c - hc) * 1.15) / (hr * (0.75 + 0.5 * noise2(a / hr * 1.5, c / hr * 1.5, laid.seed ?? 0)));
        if (d < 1) cover = 0;
        else rim = Math.max(rim, 1 - Math.min(1, (d - 1) * 4));
      }
      if (cover <= 0.002) continue;
      const j = y * w + x;
      patch.cover[j] = Math.min(1, cover);
      let [r, g, b] = colour.map((v) => v / cover);
      inside /= cover;
      if (target) {
        // Its own light and dark about the scan's average deepened, then scaled to the albedo.
        [r, g, b] = [r, g, b].map((v, k) => {
          const m = stats!.mean[k];
          return Math.max(0, m * (Math.max(1e-5, v) / m) ** contrast * target[k]);
        });
      } else {
        // Graded: saturation about its own grey, then lightness.
        const l = grey(r, g, b);
        [r, g, b] = [r, g, b].map((v, k) => Math.max(0, (l + (v - l) * (recipe.saturation ?? 1)) * (recipe.value ?? 1) * (recipe.hue?.[k] ?? 1)));
      }
      let edge = 0;
      if (laid.yellow) {
        // Yellowing in patches between the veins, more toward the tip and the margin, with
        // ragged edges; where it is furthest on, a fleck of brown.
        const wander = noise2(a * 4, c * 4, laid.seed ?? 0) * 0.6 + noise2(a * 13, c * 13, (laid.seed ?? 0) + 1) * 0.4;
        const from = (1 - inside) * 0.25 + Math.max(0, a - 0.5) * 0.7 + wander;
        const yellow = Math.min(1, Math.max(0, (from - 1.15 + laid.yellow) * 2.5));
        const l = grey(r, g, b);
        const gold = [l * 1.55, l * 1.4, l * 0.3];
        [r, g, b] = [r, g, b].map((v, k) => v + (gold[k] - v) * yellow);
        edge = Math.max(0, yellow - 0.85) * 2;
      }
      edge = Math.max(edge, rim * 0.8);
      if (edge > 0) {
        const l = grey(r, g, b);
        const brown = [l * 1.4, l * 0.85, l * 0.35];
        [r, g, b] = [r, g, b].map((v, k) => (v + (brown[k] - v) * edge) * (1 - 0.35 * edge));
      }
      if (laid.underside) {
        // A leaf's underside: paler and greyer, a little blue, its veins standing out less.
        // Graded to an albedo, less grey and blue: against living leaves' greens the old
        // underside read chalky and slate.
        const u = grey(r, g, b);
        const [keep, blue] = recipe.albedo ? [0.75, 1.15] : [0.6, 1.3];
        [r, g, b] = [(r * keep + u * (1 - keep)) * 1.2, (g * keep + u * (1 - keep)) * 1.18, (b * keep + u * (1 - keep)) * blue];
      }
      // A little darker toward the stalk, where a leaf's base lies in its neighbours' shade.
      const base = 0.82 + 0.18 * Math.min(1, Math.max(0, a * 1.6));
      patch.colour[j * 3] = r * laid.tint[0] * base;
      patch.colour[j * 3 + 1] = g * laid.tint[1] * base;
      patch.colour[j * 3 + 2] = b * laid.tint[2] * base;
      // The normal, bent where the leaf folds along its midrib (each half facing in toward
      // the other) and curls over toward its tip.
      const fold = (laid.fold ?? 0) * Math.sign(c) * Math.min(1, Math.abs(c) / Math.max(0.05, width * 0.35));
      const curl = (laid.curl ?? 0) * Math.max(0, a) ** 2;
      const scanned: [number, number, number] = [n[0] / cover, n[1] / cover, n[2] / cover];
      const out = laidNormal(laid.underside ? [-scanned[0], scanned[1], scanned[2]] : scanned, axis, {
        ...laid,
        turn: laid.turn - fold,
        tilt: laid.tilt + curl,
      });
      patch.normal.set(out, j * 3);
      // Its roughness: the scan's, scaled to the kind's gloss; a young leaf's a little
      // glossier, an old or yellowing one's duller; an underside matte.
      let rr = roughScale !== undefined ? (rough / cover) * roughScale : ROUGH;
      rr += (laid.age ?? 0) * 0.07 + edge * 0.2 + (laid.yellow ? 0.08 : 0);
      patch.rough[j] = laid.underside ? UNDERSIDE_ROUGH : Math.min(1, Math.max(0.2, rr));
    }
  }
  return { box: [x0, y0, x1, y1] as [number, number, number, number], patch };
}

/** A twig drawn along points: round, darker at its sides, its normal turned across it. */
function drawTwig(canvas: Canvas, points: [number, number][], radius: [number, number], colour: [number, number, number], rough = TWIG_ROUGH) {
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
  const patch = { cover: new Float32Array(w * h), colour: new Float32Array(w * h * 3), normal: new Float32Array(w * h * 3), rough: new Float32Array(w * h).fill(rough) };
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
  const patch = { cover: new Float32Array(w * h), colour: new Float32Array(w * h * 3), normal: new Float32Array(w * h * 3), rough: new Float32Array(w * h).fill(BERRY_ROUGH) };
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

/** A leaf as it has lived (D292): folded and curled, its hue its own, some yellowing, some
 * bitten, as the recipe asks, each by a roll of its own so the leaves' places and turns stay
 * as they were. */
function lived(leaf: Laid, recipe: Recipe, k: number): Laid {
  const roll = dice(recipe.seed * 7919 + k + 1);
  const hue = roll.range(-(recipe.hues ?? 0), recipe.hues ?? 0);
  const out: Laid = {
    ...leaf,
    tint: [leaf.tint[0] * (1 + hue), leaf.tint[1], leaf.tint[2] * (1 - 0.6 * hue)],
    fold: (recipe.fold ?? 0) * roll.range(0.3, 1),
    curl: (recipe.curl ?? 0) * roll.sign() * roll.range(0.2, 1),
    seed: recipe.seed * 31 + k,
  };
  if (roll.unit() < (recipe.yellowed ?? 0)) out.yellow = roll.range(0.3, 0.8);
  if (roll.unit() < (recipe.holes ?? 0)) {
    const holes: [number, number, number][] = [];
    for (let h = Math.floor(roll.range(1, 4)); h > 0; h--) holes.push([roll.range(0.3, 0.85), roll.range(-0.22, 0.22), roll.range(0.02, 0.05)]);
    // A bite out of its margin.
    if (roll.unit() < 0.5) holes.push([roll.range(0.35, 0.8), roll.sign() * roll.range(0.32, 0.4), roll.range(0.07, 0.13)]);
    out.holes = holes;
  }
  return out;
}

/** Composes a card `size` pixels square from a scan's leaves by its recipe. Drawn at twice
 * the size and halved, for smooth edges. */
export function compose(scan: Scan | undefined, recipe: Recipe, size: number): Composed {
  if (recipe.form === "needles") return needles(recipe, size);
  if (!scan) throw new Error(`a ${recipe.form} card needs a scan`);
  const { leaves, label } = scanLeaves(scan.alpha, scan.width, scan.height);
  if (!leaves.length) throw new Error(`${recipe.scan} has no leaves in its opacity`);
  const big = size * 2;
  const room = Math.round(big * 0.2);
  const canvas = new Canvas(big + 2 * room);
  const roll = dice(recipe.seed);
  if (recipe.form === "litter") {
    floor(canvas, scan, label, leaves, recipe, roll, big, room);
    return halve(fit(canvas, big).canvas, size);
  }
  // The twig enters at one side low down and runs up across the card, its shoots off it.
  const start: [number, number] = [room + big * 0.06, room + big * roll.range(0.6, 0.78)];
  const heading = -roll.range(0.2, 0.45);
  const main = curve(start, heading, big * 0.82, roll.range(-0.5, 0.2));
  const twigs: [number, number][][] = [main];
  const shoot = recipe.shoot ?? { from: [0.25, 0.65], length: [0.32, 0.5] };
  for (let k = 0; k < recipe.shoots; k++) {
    const from = along(main, roll.range(shoot.from[0], shoot.from[1]));
    const out = from.heading + roll.sign() * roll.range(0.45, 0.85);
    twigs.push(curve(from.at, out, big * roll.range(shoot.length[0], shoot.length[1]), roll.range(-0.4, 0.4), 8));
  }
  const thick = recipe.thickness * big;
  twigs.forEach((points, k) => drawTwig(canvas, points, k ? [thick * 0.6, thick * 0.3] : [thick, thick * 0.35], recipe.twig));

  // Every leaf as laid: where along which twig, its side, and its look.
  const laid: Laid[] = [];
  const look = (t: number): Pick<Laid, "turn" | "tilt" | "mirror" | "underside" | "tint" | "leaf" | "age"> => {
    const roll2 = roll.unit();
    // Young leaves toward the tips, old toward the base.
    const young = roll2 < recipe.young * (0.4 + 1.2 * t);
    const old = !young && roll.unit() < recipe.old * (1.6 - 1.2 * t);
    const v = 1 + roll.range(-recipe.tone, recipe.tone * 0.6);
    const tint: [number, number, number] = young ? [v * 1.08, v * 1.06, v * 0.8] : old ? [v * 0.72, v * 0.74, v * 0.55] : [v, v, v * roll.range(0.9, 1.05)];
    // Mostly a little turned, some steeply, so they foreshorten and catch the light apart.
    const turn = roll.sign() * (roll.unit() < 0.25 ? roll.range(0.6, 1.15) : roll.range(0.0, 0.5));
    const tilt = roll.sign() * roll.range(0.0, 0.7);
    return { leaf: leaves[Math.floor(roll.unit() * leaves.length)].id, turn, tilt, mirror: roll.sign(), underside: roll.unit() < recipe.underside, tint, age: young ? -1 : old ? 1 : 0 };
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
  const stats = scanStats(scan, leaves);
  laid.forEach((leaf, k) => {
    const back = 0.8 + (0.2 * k) / Math.max(1, laid.length - 1);
    const life = lived(leaf, recipe, k);
    const drawn = drawLeaf(scan, label, leaves.find((l) => l.id === leaf.leaf)!, { ...life, tint: life.tint.map((v) => v * back) as [number, number, number] }, recipe, stats);
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
  const fitted = fit(canvas, big);
  const mean = laid.reduce((sum, leaf) => sum + leaf.length, 0) / Math.max(1, laid.length);
  return { ...halve(fitted.canvas, size), leaf: (mean * fitted.scale) / big };
}

/** A conifer's branch end drawn needle by needle (`needles`), its stalk at the foot of the
 * card and its tip at the top, as the game lays a conifer's cards along its branches: a main
 * twig with side shoots alternating off it, shorter toward its tip, each with a few of its
 * own, and every twig clothed all round in short stiff needles pointing forward. A needle on
 * the far side of its twig is drawn first and darker, then the twig, then those on the near
 * side over it; one pointing toward or away from the eye is foreshortened. Needles at the
 * shoots' tips are the young growth's lighter green. */
function needles(recipe: Recipe, size: number): Composed {
  const needle = recipe.needle;
  if (!needle) throw new Error("a needles card needs its `needle`");
  const big = size * 2;
  const room = Math.round(big * 0.2);
  const canvas = new Canvas(big + 2 * room);
  const roll = dice(recipe.seed);
  const main = curve([room + big * 0.5, room + big * 0.98], -Math.PI / 2 + roll.range(-0.1, 0.1), big * (recipe.stalk ?? 0.92), roll.range(-0.25, 0.25), 16);
  const twigs: { points: [number, number][]; rank: number }[] = [{ points: main, rank: 0 }];
  for (let k = 0; k < recipe.shoots; k++) {
    const [first, last] = recipe.shoot?.from ?? [0.08, 0.88];
    const t = first + ((last - first) * (k + roll.range(0.1, 0.9))) / recipe.shoots;
    const from = along(main, t);
    const side = k % 2 ? 1 : -1;
    const [far, farthest] = recipe.shoot?.length ?? [0.42, 0.58];
    const [least, most] = recipe.spread ?? [0.8, 1.15];
    const reach = big * roll.range(far, farthest) * (1.15 - 0.85 * t);
    const shoot = curve(from.at, from.heading + side * roll.range(least, most), reach, -side * roll.range(0.15, 0.45), 10);
    twigs.push({ points: shoot, rank: 1 });
    const own = Math.floor(roll.unit() * (recipe.leaves + 1));
    for (let j = 0; j < own; j++) {
      const off = along(shoot, roll.range(0.25, 0.7));
      twigs.push({ points: curve(off.at, off.heading + roll.sign() * roll.range(0.6, 1.0), reach * roll.range(0.25, 0.45), 0, 5), rank: 2 });
    }
  }
  type Needle = { points: [number, number][]; width: number; colour: [number, number, number]; front: boolean; length: number };
  const laid: Needle[] = [];
  const [old, young] = [needle.colour, needle.young];
  for (const twig of twigs) {
    let total = 0;
    for (let i = 1; i < twig.points.length; i++) total += Math.hypot(twig.points[i][0] - twig.points[i - 1][0], twig.points[i][1] - twig.points[i - 1][1]);
    const steps = Math.max(3, Math.round(total / (needle.every * big)));
    for (let i = 0; i < steps; i++) {
      const t = Math.min(1, (i + roll.unit()) / steps);
      const { at, heading } = along(twig.points, t);
      const [forward0, forward1] = needle.lean ?? [0.5, 1.0];
      for (let n = 0; n < (needle.per ?? 3); n++) {
        // Round the twig at `phi`: across it in the picture by its cosine, toward the eye by
        // its sine; leaning forward from the twig by `lean`.
        const phi = roll.range(0, Math.PI * 2);
        const lean = roll.range(forward0, forward1);
        const length = big * roll.range(recipe.length[0], recipe.length[1]) * (1 - 0.25 * t * t);
        const [hx, hy] = [Math.cos(heading), Math.sin(heading)];
        const [px, py] = [-hy, hx];
        const forward = Math.cos(lean);
        const across = Math.sin(lean) * Math.cos(phi);
        const tip: [number, number] = [at[0] + (hx * forward + px * across) * length, at[1] + (hy * forward + py * across) * length];
        // A needle curves a little toward the tip of its twig.
        const middle: [number, number] = [(at[0] + tip[0]) / 2 + hx * length * 0.06, (at[1] + tip[1]) / 2 + hy * length * 0.06];
        const fresh = twig.rank > 0 ? Math.min(1, Math.max(0, (t - 0.7) / 0.25)) : 0;
        const tone = 1 + roll.range(-recipe.tone, recipe.tone * 0.5);
        const front = Math.sin(phi) > 0;
        const shade = tone * (front ? 1 : 0.62);
        const colour = old.map((c, k) => (c + (young[k] - c) * fresh) * shade) as [number, number, number];
        laid.push({ points: [at, middle, tip], width: needle.width * big * roll.range(0.8, 1.15), colour, front, length });
      }
    }
  }
  const draw = (front: boolean) => {
    // Needles as matte as `NEEDLE_ROUGH` says.
    for (const n of laid) if (n.front === front) drawTwig(canvas, n.points, [n.width, n.width * 0.3], n.colour, NEEDLE_ROUGH);
  };
  draw(false);
  const thick = recipe.thickness * big;
  for (const twig of twigs) drawTwig(canvas, twig.points, [thick * [1, 0.6, 0.4][twig.rank], thick * 0.3], recipe.twig);
  draw(true);
  const fitted = fit(canvas, big);
  const mean = laid.reduce((sum, n) => sum + n.length, 0) / Math.max(1, laid.length);
  return { ...halve(fitted.canvas, size), leaf: (mean * fitted.scale) / big };
}

/** What a canvas covers, scaled and moved to fill a canvas `to` pixels square, short of its
 * edge by a hair, keeping its shape. */
function fit(canvas: Canvas, to: number): { canvas: Canvas; scale: number } {
  const n = canvas.size;
  let [x0, y0, x1, y1] = [n, n, 0, 0];
  for (let y = 0; y < n; y++) {
    for (let x = 0; x < n; x++) {
      if (canvas.cover[y * n + x] < 0.01) continue;
      [x0, y0, x1, y1] = [Math.min(x0, x), Math.min(y0, y), Math.max(x1, x + 1), Math.max(y1, y + 1)];
    }
  }
  const out = new Canvas(to);
  if (x1 <= x0) return { canvas: out, scale: 1 };
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
      out.rough[i] = 0;
      for (const [ox, oy, w] of [[0, 0, (1 - fx) * (1 - fy)], [1, 0, fx * (1 - fy)], [0, 1, (1 - fx) * fy], [1, 1, fx * fy]]) {
        const [qx, qy] = [ix + ox, iy + oy];
        if (qx < 0 || qy < 0 || qx >= n || qy >= n) {
          out.normal[i * 3 + 2] += w;
          out.rough[i] += ROUGH * w;
          continue;
        }
        const q = qy * n + qx;
        out.cover[i] += canvas.cover[q] * w;
        out.rough[i] += canvas.rough[q] * w;
        for (let c = 0; c < 3; c++) {
          out.colour[i * 3 + c] += canvas.colour[q * 3 + c] * w;
          out.normal[i * 3 + c] += canvas.normal[q * 3 + c] * w;
        }
      }
    }
  }
  return { canvas: out, scale };
}

/** The canvas halved, each pixel the four under it averaged by their cover, in sRGB bytes. */
function halve(canvas: Canvas, size: number): Omit<Composed, "leaf"> {
  const rgba = Buffer.alloc(size * size * 4);
  const normal = Buffer.alloc(size * size * 3);
  const rough = Buffer.alloc(size * size);
  const big = canvas.size;
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      let cover = 0;
      const colour = [0, 0, 0];
      const n = [0, 0, 0];
      let r = 0;
      for (const [dx, dy] of [[0, 0], [1, 0], [0, 1], [1, 1]]) {
        const i = (y * 2 + dy) * big + x * 2 + dx;
        const a = canvas.cover[i];
        cover += a / 4;
        r += (canvas.rough[i] * a + ROUGH * (1 - a)) / 4;
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
      rough[j] = Math.round(Math.min(1, Math.max(0, r)) * 255);
    }
  }
  return { size, rgba, normal, rough };
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
  const patch = { cover: new Float32Array(w * h), colour: new Float32Array(w * h * 3), normal: new Float32Array(w * h * 3), rough: new Float32Array(w * h).fill(0.85) };
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
