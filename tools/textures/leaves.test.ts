import assert from "node:assert/strict";
import { test } from "node:test";
import { Canvas, compose, drawLeaf, laidNormal, scanLeaves, scanStats, type Recipe, type Scan } from "./leaves.ts";
import { cellsOf, leavesOf } from "./textures.ts";

const close = (a: number[], b: number[], within = 1e-6) => a.every((v, i) => Math.abs(v - b[i]) < within);

/** A made-up scan: `count` upright leaves, each an ellipse with a stalk below it, green,
 * their normals tilted toward their own right side. */
function scan(count = 2): Scan {
  const [width, height] = [64 * count, 96];
  const colour = Buffer.alloc(width * height * 3);
  const normal = Buffer.alloc(width * height * 3);
  const alpha = Buffer.alloc(width * height);
  for (let k = 0; k < count; k++) {
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < 64; x++) {
        const i = y * width + k * 64 + x;
        const blade = ((x - 32) / 20) ** 2 + ((y - 40) / 30) ** 2 <= 1;
        const stalk = Math.abs(x - 32) <= 1 && y > 68 && y < 86;
        if (blade || stalk) alpha[i] = 255;
        colour.set([60, 120, 40], i * 3);
        normal.set([170, 128, 230], i * 3);
      }
    }
  }
  return { width, height, colour, normal, alpha };
}

test("a scan's leaves are found one by one, each stalk below its tip", () => {
  const { leaves, label } = scanLeaves(scan(3).alpha, 192, 96);
  assert.equal(leaves.length, 3);
  for (const leaf of leaves) {
    assert.ok(leaf.base[1] > 80 && leaf.tip[1] < 15, `base ${leaf.base}, tip ${leaf.tip}`);
    assert.ok(Math.abs(leaf.base[0] - leaf.tip[0]) < 2);
  }
  // Each leaf's pixels are its own.
  assert.notEqual(label[40 * 192 + 32], label[40 * 192 + 96]);
});

test("a leaf's normal turns with it on the card and tilts as it is turned about its midrib", () => {
  const up: [number, number] = [0, -1];
  const flat = { angle: -Math.PI / 2, turn: 0, tilt: 0, mirror: 1 };
  // Laid as it was scanned, its normals are as scanned.
  assert.ok(close(laidNormal([0.6, 0, 0.8], up, flat), [0.6, 0, 0.8]));
  // Laid pointing right, a quarter turn clockwise on the page: what faced its right faces down.
  assert.ok(close(laidNormal([0.6, 0, 0.8], up, { ...flat, angle: 0 }), [0, -0.6, 0.8]));
  // Mirrored, it faces its left.
  assert.ok(close(laidNormal([0.6, 0, 0.8], up, { ...flat, mirror: -1 }), [-0.6, 0, 0.8]));
  // A flat leaf turned about its midrib leans across it by as much; tilted, along it.
  const turned = laidNormal([0, 0, 1], up, { ...flat, turn: 0.5 });
  assert.ok(close(turned, [Math.sin(0.5), 0, Math.cos(0.5)]), `${turned}`);
  const tilted = laidNormal([0, 0, 1], up, { ...flat, tilt: 0.5 });
  assert.ok(close(tilted, [0, Math.sin(0.5), Math.cos(0.5)]), `${tilted}`);
});

test("a leaf laid on another shades it just past its edge, down and to the right", () => {
  const canvas = new Canvas(256);
  const patch = (w: number, h: number, grey: number) => ({
    cover: new Float32Array(w * h).fill(1),
    colour: new Float32Array(w * h * 3).fill(grey),
    normal: new Float32Array(w * h * 3).map((_, i) => (i % 3 === 2 ? 1 : 0)),
  });
  canvas.lay([0, 0, 256, 256], patch(256, 256, 0.5), 0.5);
  canvas.lay([64, 64, 128, 128], patch(64, 64, 0.5), 0.5);
  const at = (x: number, y: number) => canvas.colour[(y * 256 + x) * 3];
  assert.ok(at(129, 100) < 0.4, `just past its right edge ${at(129, 100)}`);
  assert.ok(Math.abs(at(240, 10) - 0.5) < 1e-6, `far off ${at(240, 10)}`);
  // Past its left edge, toward the light, less.
  assert.ok(at(62, 100) > at(129, 100), `${at(62, 100)} left, ${at(129, 100)} right`);
});

test("a composed card covers leaves and twig, its normal map flat where it covers nothing", () => {
  const recipe: Recipe = {
    scan: "made up", form: "spray", leaves: 10, length: [0.2, 0.3], shoots: 2, twig: [0.4, 0.3, 0.2], thickness: 0.01,
    tone: 0.2, young: 0.2, old: 0.2, underside: 0.1, saturation: 1, value: 1, seed: 4,
  };
  const card = compose(scan(2), recipe, 64);
  let covered = 0;
  for (let i = 0; i < 64 * 64; i++) {
    const a = card.rgba[i * 4 + 3];
    if (a > 128) covered++;
    const n = [...card.normal.subarray(i * 3, i * 3 + 3)].map((v) => v / 127.5 - 1);
    assert.ok(Math.abs(Math.hypot(...n) - 1) < 0.05, `normal ${n} not unit`);
    if (a === 0) assert.deepEqual([...card.normal.subarray(i * 3, i * 3 + 3)], [128, 128, 255]);
  }
  assert.ok(covered > 64 * 64 * 0.15, `covers ${covered}`);
  // The leaves lean every way, as they are turned and tilted, not all one way.
  let [left, right] = [0, 0];
  for (let i = 0; i < 64 * 64; i++) {
    if (card.rgba[i * 4 + 3] < 200) continue;
    if (card.normal[i * 3] < 110) left++;
    if (card.normal[i * 3] > 146) right++;
  }
  assert.ok(left > 20 && right > 20, `${left} lean left, ${right} right`);
  assert.deepEqual(compose(scan(2), recipe, 64).rgba, card.rgba, "the same recipe composes the same card");
});

const linear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);

/** The leaf pixels of a card (cover past half, green above red, so not the twig): their mean
 * colour (linear) and roughness. */
function leafPixels(card: ReturnType<typeof compose>) {
  const sum = [0, 0, 0];
  let [rough, n] = [0, 0];
  for (let i = 0; i < card.size * card.size; i++) {
    const [r, g] = [card.rgba[i * 4], card.rgba[i * 4 + 1]];
    if (card.rgba[i * 4 + 3] < 200 || g <= r) continue;
    for (let c = 0; c < 3; c++) sum[c] += linear(card.rgba[i * 4 + c] / 255);
    rough += card.rough[i] / 255;
    n++;
  }
  return { mean: sum.map((v) => v / n), rough: rough / n, n };
}

const graded: Recipe = {
  scan: "made up", form: "spray", leaves: 10, length: [0.2, 0.3], shoots: 2, twig: [0.4, 0.3, 0.2], thickness: 0.01,
  tone: 0, young: 0, old: 0, underside: 0, albedo: [0.3, 0.4, 0.15], roughness: 0.5, seed: 4,
};

test("a card graded to an albedo takes that colour's balance, not the scan's", () => {
  const { mean } = leafPixels(compose(scan(2), graded, 96));
  // The scan is sRGB (60, 120, 40), its red a quarter of its green in linear light; the
  // albedo's a little over a half. The leaves are darker than the albedo toward their stalks
  // and behind others, by the same in every channel.
  const want = linear(0.3) / linear(0.4);
  assert.ok(Math.abs(mean[0] / mean[1] - want) < 0.05, `red over green ${(mean[0] / mean[1]).toFixed(3)}, want ${want.toFixed(3)}`);
  assert.ok(mean[1] < linear(0.4) * 1.02 && mean[1] > linear(0.4) * 0.6, `green ${mean[1].toFixed(3)}`);
});

test("a card's roughness is its recipe's on the upper faces and matte on the undersides", () => {
  const upper = leafPixels(compose(scan(2), graded, 96));
  assert.ok(Math.abs(upper.rough - 0.5) < 0.04, `upper faces ${upper.rough.toFixed(3)}`);
  const under = leafPixels(compose(scan(2), { ...graded, underside: 1 }, 96));
  assert.ok(under.rough > 0.85, `undersides ${under.rough.toFixed(3)}`);
});

test("a folded leaf's halves lean toward each other across its midrib, and a hole is bitten out", () => {
  const made = scan(1);
  made.normal.fill(0).forEach((_, i, all) => (all[i] = [128, 128, 255][i % 3]));
  const { leaves, label } = scanLeaves(made.alpha, made.width, made.height);
  // Laid pointing up the card, so its right half is to the right.
  const laid = { leaf: leaves[0].id, at: [100, 180] as [number, number], angle: -Math.PI / 2, length: 160, turn: 0, tilt: 0, mirror: 1, underside: false, tint: [1, 1, 1] as [number, number, number] };
  const lean = (fold: number, holes?: [number, number, number][]) => {
    const { box, patch } = drawLeaf(made, label, leaves[0], { ...laid, fold, holes }, graded);
    let [left, right, cover, nl, nr] = [0, 0, 0, 0, 0];
    for (let y = 0; y < box[3] - box[1]; y++) {
      for (let x = 0; x < box[2] - box[0]; x++) {
        const j = y * (box[2] - box[0]) + x;
        cover += patch.cover[j];
        if (patch.cover[j] < 0.9) continue;
        if (box[0] + x < 90) [left, nl] = [left + patch.normal[j * 3], nl + 1];
        if (box[0] + x > 110) [right, nr] = [right + patch.normal[j * 3], nr + 1];
      }
    }
    return { left: left / nl, right: right / nr, cover };
  };
  const flat = lean(0);
  assert.ok(Math.abs(flat.left) < 0.02 && Math.abs(flat.right) < 0.02, `flat ${flat.left}, ${flat.right}`);
  const folded = lean(0.4);
  assert.ok(folded.left > 0.1 && folded.right < -0.1, `folded: the left half leans ${folded.left}, the right ${folded.right}`);
  const holed = lean(0, [[0.5, 0, 0.1]]);
  assert.ok(holed.cover < flat.cover * 0.97, `holed covers ${holed.cover.toFixed(0)} of ${flat.cover.toFixed(0)}`);
});

test("the card table's cells are read back by name", () => {
  const ron = `cards: {\n        "oak": (\n            cell: (0.0000, 0.2500),\n        ),\n        "pine": (\n            cell: (0.5000, 0.0000),`;
  assert.deepEqual([...cellsOf(ron)], [["oak", [0, 0.25]], ["pine", [0.5, 0]]]);
});

test("the card table's leaf lengths are read back by name, for the cards that give one", () => {
  const ron = `"oak": (\n            cell: (0.0000, 0.2500),\n            leaf: 0.1372,\n        ),\n        "pine": (\n            cell: (0.5000, 0.0000),\n        ),`;
  assert.deepEqual([...leavesOf(ron)], [["oak", 0.1372]]);
});

test("a card says how long its leaves are, shorter as the recipe asks for smaller leaves", () => {
  const recipe: Recipe = {
    scan: "made up", form: "spray", leaves: 10, length: [0.2, 0.3], shoots: 2, twig: [0.4, 0.3, 0.2], thickness: 0.01,
    tone: 0.2, young: 0.2, old: 0.2, underside: 0.1, saturation: 1, value: 1, seed: 4,
  };
  const big = compose(scan(2), recipe, 64).leaf;
  const small = compose(scan(2), { ...recipe, length: [0.1, 0.15] }, 64).leaf;
  // Measured on the card after it is fitted to its cell, so a little longer than asked.
  assert.ok(big > 0.15 && big < 0.4, `leaves ${big} of the card`);
  assert.ok(small < big * 0.75, `smaller leaves ${small} of the card, against ${big}`);
});

test("a conifer's card is drawn needle by needle, its needles as long as asked, its edge a fringe", () => {
  const recipe: Recipe = {
    form: "needles", leaves: 3, length: [0.04, 0.06], shoots: 10, twig: [0.3, 0.2, 0.13], thickness: 0.006, tone: 0.3,
    young: 0, old: 0, underside: 0, saturation: 1, value: 1, seed: 7,
    needle: { colour: [0.08, 0.14, 0.08], young: [0.15, 0.24, 0.11], width: 0.004, every: 0.006 },
  };
  const size = 128;
  const card = compose(undefined, recipe, size);
  assert.ok(card.leaf > 0.03 && card.leaf < 0.08, `needles ${card.leaf} of the card`);
  let covered = 0;
  let edge = 0;
  for (let y = 1; y < size - 1; y++) {
    for (let x = 1; x < size - 1; x++) {
      const on = (i: number) => card.rgba[i * 4 + 3] > 115;
      const i = y * size + x;
      if (!on(i)) continue;
      covered++;
      if (!on(i - 1) || !on(i + 1) || !on(i - size) || !on(i + size)) edge++;
    }
  }
  assert.ok(covered > size * size * 0.15, `covers ${covered}`);
  // Single needles at its edge: far more edge for what it covers than a solid round shape
  // as big, whose edge is its circumference.
  const solid = 2 * Math.sqrt(Math.PI * covered);
  assert.ok(edge > solid * 4, `${edge} of ${covered} covered pixels at an edge, against ${solid.toFixed(0)}`);
  assert.deepEqual(compose(undefined, recipe, size).rgba, card.rgba, "the same recipe draws the same card");
});

test("a spiny leaf's margin is cut back between its spines, its spines standing out", () => {
  const made = scan(1);
  const { leaves, label } = scanLeaves(made.alpha, made.width, made.height);
  const laid = { leaf: leaves[0].id, at: [100, 180] as [number, number], angle: -Math.PI / 2, length: 160, turn: 0, tilt: 0, mirror: 1, underside: false, tint: [1, 1, 1] as [number, number, number] };
  // Its outline's width at each row up the leaf.
  const widths = (recipe: Recipe) => {
    const { box, patch } = drawLeaf(made, label, leaves[0], laid, recipe, scanStats(made, leaves));
    const w = box[2] - box[0];
    const rows: number[] = [];
    for (let y = 0; y < box[3] - box[1]; y++) {
      let n = 0;
      for (let x = 0; x < w; x++) if (patch.cover[y * w + x] > 0.5) n++;
      rows.push(n);
    }
    return rows;
  };
  const plain = widths(graded);
  const spiny = widths({ ...graded, spines: { count: 4, depth: 0.4 } });
  const lost = plain.map((n, y) => n - spiny[y]);
  // Narrower in places, much less at others: scallops between spines that keep nearly the
  // full width.
  assert.ok(lost.reduce((a, b) => a + b, 0) > 0, "nothing cut");
  const blade = lost.slice(Math.round(plain.length * 0.15), Math.round(plain.length * 0.55));
  assert.ok(Math.min(...blade) < 0.35 * Math.max(...blade) && Math.max(...blade) >= 4, `cut back by ${blade.join(",")}`);
});

test("a frond's pinnae stand either side of one rachis corner to corner, longest low down", () => {
  const recipe: Recipe = {
    scan: "made up", form: "frond", leaves: 24, length: [0.18, 0.22], shoots: 0, twig: [0.3, 0.4, 0.2], thickness: 0.012,
    tone: 0, young: 0, old: 0, underside: 0, albedo: [0.25, 0.4, 0.12], seed: 3,
  };
  const card = compose(scan(2), recipe, 96);
  // Covered either side of the diagonal the rachis runs up, x + y = 96 at its middle, and
  // more round its lower third than round its top.
  const round = (cx: number, cy: number) => {
    let [above, below] = [0, 0];
    for (let y = 0; y < 96; y++)
      for (let x = 0; x < 96; x++) {
        if (Math.hypot(x - cx, y - cy) > 14 || card.rgba[(y * 96 + x) * 4 + 3] < 128) continue;
        const off = x - cx - (cy - y);
        if (off > 4) below++;
        if (off < -4) above++;
      }
    return { above, below };
  };
  const low = round(34, 62);
  const high = round(82, 14);
  assert.ok(low.above > 30 && low.below > 30, `low: ${low.above} above, ${low.below} below`);
  assert.ok(low.above + low.below > 1.5 * (high.above + high.below), `low ${low.above + low.below}, high ${high.above + high.below}`);
});
