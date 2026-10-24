import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { bed } from "./coals.ts";
import { arms, BUDGET, BURN_R, HALF, hearth, layout, STONE_TONES, TALL, woodMaps } from "./hearth.ts";

const { wood, stone, logs, stones } = hearth();
const tris = (m: { idx: number[] }) => m.idx.length / 3;

test("a hero campfire keeps to its budget, its coal bed's included, and spends most of it", () => {
  const all = tris(wood) + tris(stone) + bed(1, arms()).idx.length / 3;
  assert.ok(all <= BUDGET && all >= BUDGET * 0.6, `${all} triangles`);
});

test("it stands inside its catalogue footprint and height, on the ground", () => {
  for (const m of [wood, stone]) {
    for (let i = 0; i < m.pos.length; i += 3) {
      assert.ok(Math.hypot(m.pos[i], m.pos[i + 2]) <= HALF, `${m.pos[i]}, ${m.pos[i + 2]} past the footprint`);
      assert.ok(m.pos[i + 1] <= TALL && m.pos[i + 1] >= 0,`height ${m.pos[i + 1]}`);
    }
  }
});

test("its few round logs lie low across the ring, their butts out past the stones", () => {
  // RDR2's 2-4 round logs at 10-30 deg, the pile 0.15-0.3 of the ring tall, their butts out over
  // the stones (campfire low-lay round), not an upright teepee standing inside them.
  assert.ok(logs.length >= 2 && logs.length <= 4, `${logs.length} logs`);
  const ring = Math.min(...stones.map((s) => Math.hypot(s.at[0], s.at[2])));
  const outer = Math.max(...stones.map((s) => Math.hypot(s.at[0], s.at[2]) + Math.max(s.size[0], s.size[2])));
  for (const l of logs) {
    const end = l.foot.map((v, i) => v + l.axis[i] * l.L);
    const slope = (Math.asin(l.axis[1]) * 180) / Math.PI;
    assert.ok(Math.hypot(l.foot[0], l.foot[2]) > outer, "butt past the stones");
    assert.ok(Math.hypot(end[0], end[2]) < 0.12, "its end over the middle");
    assert.ok(slope >= 8 && slope <= 30, `lying at ${slope.toFixed(1)} deg`);
  }
  let top = 0;
  for (let i = 1; i < wood.pos.length; i += 3) top = Math.max(top, wood.pos[i]);
  assert.ok(top <= 0.3 * 2 * ring, `the pile ${top.toFixed(3)} m, ${(top / (2 * ring)).toFixed(2)} of the ring`);
  // Each passes out over the ring through a gap: no stone stands in a log.
  for (const s of stones) {
    const a = Math.atan2(s.at[2], s.at[0]);
    for (const l of logs) {
      const d = Math.atan2(Math.sin(a - Math.atan2(l.out[2], l.out[0])), Math.cos(a - Math.atan2(l.out[2], l.out[0])));
      assert.ok(Math.abs(d) * ring > l.R + Math.max(s.size[0], s.size[2]), "a stone in a log's way");
    }
  }
});

test("every face of it faces out: its logs from their axes, its stones from their middles", () => {
  const p = (m: { pos: number[] }, k: number) => [m.pos[k * 3], m.pos[k * 3 + 1], m.pos[k * 3 + 2]];
  const sub = (a: number[], b: number[]) => a.map((v, i) => v - b[i]);
  const cross = (a: number[], b: number[]) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  const dot = (a: number[], b: number[]) => a.reduce((s, v, i) => s + v * b[i], 0);
  let inward = 0;
  for (let i = 0; i < stone.idx.length; i += 3) {
    const [a, b, c] = [0, 1, 2].map((k) => p(stone, stone.idx[i + k]));
    const near = stones.findLast((s) => s.from! <= i / 3)!;
    if (dot(cross(sub(b, a), sub(c, a)), sub(a, near.at)) < 0) inward++;
  }
  assert.equal(inward, 0, `${inward} stone faces turned in`);
  // And their vertex normals, which light them: turned in, a stone is black under the sun.
  for (let i = 0; i < stone.idx.length; i++) {
    const k = stone.idx[i], near = stones.findLast((s) => s.from! <= Math.floor(i / 3))!;
    assert.ok(dot(stone.nrm.slice(k * 3, k * 3 + 3), sub(p(stone, k), near.at)) > 0, "a stone's normal turned in");
  }
  // A log's vertex normals point away from its axis or along it at its ends.
  for (let k = 0; k < wood.pos.length / 3; k++) {
    const at = p(wood, k), n = wood.nrm.slice(k * 3, k * 3 + 3);
    const l = logs.findLast((x) => x.from! <= k)!;
    const r = sub(at, l.foot), t = dot(r, l.axis);
    const off = t <= 0.001 ? l.axis.map((v) => -v) : t >= l.L - 0.001 ? l.axis : sub(r, l.axis.map((v) => v * t));
    assert.ok(dot(n, off) > -1e-6, "a log's normal turned in");
  }
});

test("each stone is one of the field stone's tones, lighter or darker, never tinted", () => {
  for (const s of stones) {
    const near = STONE_TONES.some((t) => {
      const k = s.tone[0] / t[0];
      return k > 0.89 && k < 1.11 && t.every((c, i) => Math.abs(s.tone[i] - c * k) < 1e-6);
    });
    assert.ok(near, `stone tone ${s.tone.map(Math.round)} is a palette tone scaled`);
  }
});

test("its logs' cracks glow orange-red at their dullest and orange at their brightest, never pure red nor pale white", () => {
  const glow = woodMaps(layout().logs)[1];
  const lit: [number, number, number][] = [];
  for (let i = 0; i < glow.length; i += 4) if (glow[i] >= 12) lit.push([glow[i], glow[i + 1], glow[i + 2]]);
  lit.sort((a, b) => a[0] - b[0]);
  const tenth = Math.floor(lit.length / 10);
  const greenToRed = (px: [number, number, number][]) => px.reduce((s, p) => s + p[1], 0) / px.reduce((s, p) => s + p[0], 0);
  assert.ok(greenToRed(lit.slice(0, tenth)) > 0.25, "its dullest cracks near pure red");
  // Deep orange (#FF5A10) to orange-yellow, not the pale yellow-white whose even stripes read as
  // a barcode at the feet (fire round 32), nor the yellow that read tan by firelight (campfire
  // glow round): its green between 0.28 and 0.6 of its red and its blue low.
  const bright = lit.slice(-tenth);
  assert.ok(greenToRed(bright) > 0.28 && greenToRed(bright) < 0.6, `its brightest cracks not orange: ${greenToRed(bright).toFixed(2)}`);
  const blueToRed = bright.reduce((s, p) => s + p[2], 0) / bright.reduce((s, p) => s + p[0], 0);
  assert.ok(blueToRed < 0.25, "its brightest cracks pale");
});

test("its char is matte and near-black, and the atlas's gutter round each island takes the island's colour", () => {
  const [colour, , mr] = woodMaps(layout().logs);
  // Charcoal (fire round 37): where the colour is near-black the roughness is past 0.8, as
  // charcoal's is; at 0.6 the firelight drew a gloss on it.
  let dark = 0, glossy = 0;
  for (let i = 0; i < colour.length; i += 4) {
    if (colour[i] + colour[i + 1] + colour[i + 2] > 3 * 30) continue;
    dark++;
    if (mr[i + 1] < 0.8 * 255) glossy++;
  }
  assert.ok(dark > 0 && glossy / dark < 0.02, `${glossy} of ${dark} dark texels glossy`);
  // Edge padding: the gutter takes its islands' colours, no white left in it, so the mips never
  // average white into an island's edge.
  let bled = 0;
  for (let i = 0; i < colour.length; i += 4) if (colour[i] === 255 && colour[i + 1] === 255 && colour[i + 2] === 255) bled++;
  assert.equal(bled, 0, `${bled} white gutter texels`);
});

test("the fire's flames are told where the campfire's own logs lie (terra_world fire/hearth.rs)", () => {
  // The flames lick up the logs where the shader is told they lie: the game's numbers are read
  // here against the model's layout, so a new layout can't leave its flames standing beside it.
  const rust = readFileSync(new URL("../../crates/terra_world/src/fire/hearth.rs", import.meta.url), "utf8");
  const block = rust.slice(rust.indexOf("pub const CAMPFIRE: Logs"), rust.indexOf("pub const NONE"));
  const told = (name: string) => Number(new RegExp(`\\b${name}: ([-\\d.]+)`).exec(block)![1]);
  const { logs } = layout(1);
  const tops = logs.map((l) => l.foot.map((x, i) => x + l.axis[i] * (l.L - 0.05)));
  // Each log's burning length, from where it enters the fire (`BURN_R` out) to its end: its butt
  // out past the stones lies cold on the ground.
  const feet = logs.map((l) => {
    const [a, b] = [l.foot[0], l.foot[2]], [u, v] = [l.axis[0], l.axis[2]];
    const k = u * u + v * v, m = a * u + b * v, t = (-m - Math.sqrt(m * m - k * (a * a + b * b - BURN_R * BURN_R))) / k;
    return l.foot.map((x, i) => x + l.axis[i] * t);
  });
  const mean = (f: (i: number) => number) => logs.reduce((a, _, i) => a + f(i), 0) / logs.length;
  const bearing = (v: number[]) => Math.atan2(v[2], v[0]);
  const turn = (a: number) => Math.atan2(Math.sin(a), Math.cos(a));
  const near = (name: string, want: number, within: number) =>
    assert.ok(Math.abs(told(name) - want) < within, `${name} ${told(name)}, the model's ${want.toFixed(3)}`);
  assert.equal(told("count"), logs.length);
  // Evenly round from `turn`, the best fit: each log within 0.15 rad of where the shader lays it.
  near("turn", mean((i) => turn(bearing(feet[i]) - (i * 2 * Math.PI) / logs.length)), 0.01);
  logs.forEach((_, i) => assert.ok(Math.abs(turn(bearing(feet[i]) - told("turn") - (i * 2 * Math.PI) / logs.length)) < 0.15, `log ${i}'s bearing`));
  near("twist", mean((i) => turn(bearing(tops[i]) - bearing(feet[i]))), 0.03);
  near("radius", mean((i) => logs[i].R), 0.005);
  near("foot", mean((i) => Math.hypot(feet[i][0], feet[i][2])), 0.005);
  near("foot_up", mean((i) => feet[i][1]), 0.005);
  near("top", mean((i) => Math.hypot(tops[i][0], tops[i][2])), 0.005);
  near("top_up", mean((i) => tops[i][1]), 0.005);
});
