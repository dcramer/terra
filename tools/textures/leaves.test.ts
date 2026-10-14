import assert from "node:assert/strict";
import { test } from "node:test";
import { Canvas, compose, laidNormal, scanLeaves, type Recipe, type Scan } from "./leaves.ts";
import { cellsOf } from "./textures.ts";

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

test("the card table's cells are read back by name", () => {
  const ron = `cards: {\n        "oak": (\n            cell: (0.0000, 0.2500),\n        ),\n        "pine": (\n            cell: (0.5000, 0.0000),`;
  assert.deepEqual([...cellsOf(ron)], [["oak", [0, 0.25]], ["pine", [0.5, 0]]]);
});
