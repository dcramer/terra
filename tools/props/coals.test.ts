import assert from "node:assert/strict";
import { test } from "node:test";
import { BED_BUDGET, bed, glowColour, maps, TILES, under, YELLOW } from "./coals.ts";
import { arms } from "./hearth.ts";

test("a dull coal glows deep orange-red, never near pure red, which the night's grade turns pink", () => {
  for (const h of [0.2, 0.4, 0.6, 0.8, 1]) {
    const [r, g, b] = glowColour(h);
    assert.ok(g / r > 0.3, `at heat ${h} its green ${g.toFixed(0)} against red ${r.toFixed(0)}`);
    assert.ok(b < g * 0.5, `at heat ${h} its blue stays low`);
  }
});

test("a coal bed keeps to its triangle budget", () => {
  assert.ok(bed(1, arms()).idx.length / 3 <= BED_BUDGET);
});

test("its coals are a packed heap: the bed under the logs covered, highest in the middle", () => {
  const { coals } = bed(1, arms());
  // Seen from above, three in four points within 0.2 m of the middle are under a coal (its
  // round size; its longer side covers more).
  let covered = 0, points = 0;
  for (let x = -0.2; x <= 0.2; x += 0.02) {
    for (let z = -0.2; z <= 0.2; z += 0.02) {
      if (Math.hypot(x, z) > 0.2) continue;
      points++;
      if (coals.some((c) => Math.hypot(c.at[0] - x, c.at[2] - z) < c.size)) covered++;
    }
  }
  assert.ok(covered / points > 0.75, `${covered} of ${points} covered`);
  // The heap's top, on average, within 0.1 m of the middle and past 0.25 m.
  const top = (from: number, to: number) => {
    const ring = coals.filter((c) => Math.hypot(c.at[0], c.at[2]) >= from && Math.hypot(c.at[0], c.at[2]) < to);
    return ring.reduce((sum, c) => sum + c.at[1] + c.size, 0) / ring.length;
  };
  assert.ok(top(0, 0.1) > top(0.25, 1) + 0.03, "no higher in the middle");
});

test("nothing of the bed lies under the prop's base, where the contract puts its origin", () => {
  const { pos } = bed(1, arms());
  for (let i = 1; i < pos.length; i += 3) assert.ok(pos[i] >= 0, `a point ${(-pos[i]).toFixed(3)} m under the base`);
});

test("every face of the bed faces out, so none is culled", () => {
  const { pos, idx, coals } = bed(1, arms());
  const p = (i: number) => [pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]];
  for (let t = 0; t < idx.length; t += 3) {
    const [a, b, c] = [p(idx[t]), p(idx[t + 1]), p(idx[t + 2])];
    const u = b.map((q, i) => q - a[i]), w = c.map((q, i) => q - a[i]);
    const n = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
    const mid = [0, 1, 2].map((i) => (a[i] + b[i] + c[i]) / 3);
    // The mound's faces look up; a coal's away from its middle.
    const coal = coals.findLast((k) => k.from <= t / 3);
    const out = coal ? mid.map((q, i) => q - coal.at[i]) : [0, 1, 0];
    assert.ok(n[0] * out[0] + n[1] * out[1] + n[2] * out[2] > 0, `triangle ${t / 3} faces in`);
  }
});

test("its glow is red to orange-yellow, never salmon, and hottest in the middle", () => {
  const emissive = maps(1, arms())[1];
  let lit = 0;
  for (let i = 0; i < emissive.length; i += 4) {
    const [r, g, b] = [emissive[i], emissive[i + 1], emissive[i + 2]];
    if (r < 8) continue;
    lit++;
    assert.ok(g <= r * (YELLOW[1] / YELLOW[0]) + 1 && b <= r * (YELLOW[2] / YELLOW[0]) + 1, `${r},${g},${b}`);
  }
  // The middle's tiles (the first row of four) glow yellower than the edge's (the last).
  const yellow = (tiles: number[]) => {
    let [g, r] = [0, 0];
    for (const [ox, oy] of tiles.map((t) => TILES[t])) for (let y = oy; y < oy + 128; y++) for (let x = ox; x < ox + 128; x++) [r, g] = [r + emissive[(y * 512 + x) * 4], g + emissive[(y * 512 + x) * 4 + 1]];
    return g / r;
  };
  // (1.3 before fire round 31, whose dull coals went orange-red from near pure red.)
  assert.ok(yellow([0, 1, 2, 3]) > yellow([8, 9, 10, 11]) * 1.15, "no hotter core");
  assert.ok(lit > emissive.length / 4 / 20, "it hardly glows");
});

test("its pit glows only under the logs, its ash round them dark and neutral grey, never tan", () => {
  const [base, emissive] = maps(1, arms());
  const logs = arms();
  let [ash, lit, warm, bright] = [0, 0, 0, 0];
  // The pit's disc: the bottom-left quarter of the atlas, 0.46 m (MOUND_R) to 0.49 of it.
  for (let y = 0; y < 256; y += 2) {
    for (let x = 0; x < 256; x += 2) {
      const [X, Z] = [((x / 256 - 0.5) / 0.49) * 0.46, ((y / 256 - 0.5) / 0.49) * 0.46];
      if (Math.hypot(X, Z) > 0.4 || under(X, Z, logs) > 0.02) continue;
      const o = ((256 + y) * 512 + x) * 4;
      ash++;
      if (emissive[o] > 40) lit++;
      const [r, g, b] = [base[o], base[o + 1], base[o + 2]];
      if (b < 0.94 * r) warm++;
      if (0.2126 * r + 0.7152 * g + 0.0722 * b > 60) bright++;
    }
  }
  assert.ok(ash > 2000, `only ${ash} texels of ash`);
  assert.ok(lit / ash < 0.03, `${lit} of ${ash} ash texels glow`);
  assert.ok(warm === 0 && bright === 0, `${warm} warm, ${bright} bright of ${ash}`);
  // Under the crossing, it glows.
  const mid = ((256 + 128) * 512 + 128) * 4;
  assert.ok(under(0, 0, logs) > 0.9 && emissive[mid] + emissive[mid + 4 * 6] + emissive[mid + 512 * 24] > 0, "no glow under the crossing");
});
