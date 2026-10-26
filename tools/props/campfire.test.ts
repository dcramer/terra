import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { BUDGET, BURN_R, HALF, INNER_R, layout, STACK, TALL } from "./campfire.ts";

const { logs, stones } = layout(1);

/** The committed model's JSON and its accessors' counts and bounds. */
function model() {
  const glb = readFileSync(new URL("../../assets/models/props/Campfire.glb", import.meta.url));
  return JSON.parse(glb.subarray(20, 20 + glb.readUInt32LE(12)).toString("utf8"));
}

test("its logs, 9-11 cm thick and not all alike, lean 20-55 deg into a stack over the middle, their butts inside the stones", () => {
  assert.equal(logs.length, 5);
  const girth = logs.map((l) => l.R);
  for (const R of girth) assert.ok(2 * R >= 0.09 && 2 * R <= 0.11, `a log ${(200 * R).toFixed(1)} cm thick`);
  assert.ok(Math.max(...girth) - Math.min(...girth) >= 0.004, "of mixed girth");
  let top = 0;
  for (const l of logs) {
    const end = l.foot.map((v, i) => v + l.axis[i] * l.L);
    const slope = (Math.asin(l.axis[1]) * 180) / Math.PI;
    assert.ok(Math.hypot(end[0], end[2]) < 0.12, "its end over the middle");
    assert.ok(slope >= 20 && slope <= 55, `leaning at ${slope.toFixed(1)} deg`);
    assert.ok(Math.hypot(l.foot[0], l.foot[2]) < INNER_R, "its butt inside the stones");
    top = Math.max(top, end[1]);
  }
  assert.ok(top > 0.8 * STACK && top <= STACK, `the stack ${top.toFixed(3)} m`);
});

test("its fieldstones are 12-14 of mixed size, from more than one scan, their inner faces clear of the fire", () => {
  assert.ok(stones.length >= 12 && stones.length <= 14, `${stones.length} stones`);
  const sizes = stones.map((s) => s.size);
  // Thirteen nearly touching a 0.55 m ring keep within 0.19-0.35 m (round 3), under twice.
  assert.ok(Math.max(...sizes) / Math.min(...sizes) >= 1.75, "of mixed size");
  assert.ok(new Set(stones.map((s) => s.source)).size >= 3, "from several scans");
  // A scan used twice shows another face or another size.
  const seen = new Set(stones.map((s) => `${s.source} ${s.roll} ${s.size.toFixed(2)}`));
  assert.equal(seen.size, stones.length, "a stone repeated as it was");
  for (const s of stones) assert.ok(s.inner >= INNER_R && s.inner < INNER_R + 0.1, `a stone's inner face ${s.inner.toFixed(3)} m out`);
});

test("the model keeps to its budget and stands inside its footprint and height, on the ground", () => {
  const json = model();
  let tris = 0, lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  for (const node of json.nodes.filter((n: { mesh?: number }) => n.mesh !== undefined)) {
    assert.ok(!node.rotation && !node.scale && !node.matrix, `node ${node.name} is turned or scaled`);
    const t = node.translation ?? [0, 0, 0];
    for (const p of json.meshes[node.mesh].primitives) {
      tris += json.accessors[p.indices].count / 3;
      const { min, max } = json.accessors[p.attributes.POSITION];
      lo = lo.map((v, k) => Math.min(v, min[k] + t[k]));
      hi = hi.map((v, k) => Math.max(v, max[k] + t[k]));
    }
  }
  assert.ok(tris <= BUDGET && tris >= 0.6 * BUDGET, `${tris} triangles of ${BUDGET}`);
  for (const k of [0, 2]) assert.ok(lo[k] >= -HALF * 1.02 && hi[k] <= HALF * 1.02, `axis ${k}: ${lo[k].toFixed(3)} to ${hi[k].toFixed(3)}`);
  assert.ok(lo[1] >= -0.01 && hi[1] <= TALL, `${lo[1].toFixed(3)} to ${hi[1].toFixed(3)} m tall`);
});

test("its wood glows by its own map, its stones never, and its worn ground is cut out by its alpha (D513)", () => {
  const materials = new Map<string, any>(model().materials.map((m: { name: string }) => [m.name, m]));
  for (const name of ["Wood", "Stone", "Ground", "Coals"]) assert.ok(materials.has(name), `no ${name} material`);
  // props.rs `smoulder`: a material with an emissive map glows with the fire.
  assert.ok(materials.get("Wood").emissiveTexture, "its wood has no glow map");
  assert.ok(!materials.get("Stone").emissiveTexture, "its stones glow");
  assert.equal(materials.get("Ground").alphaMode, "MASK");
  assert.ok(!materials.has("Fire"), "a Fire material without a map, hidden by the game");
});

test("the fire's flames are told where the campfire's own logs lie (terra_world fire/hearth.rs)", () => {
  // The flames lick up the logs where the shader is told they lie: the game's numbers are read
  // here against the model's layout, so a new layout can't leave its flames standing beside it.
  const rust = readFileSync(new URL("../../crates/terra_world/src/fire/hearth.rs", import.meta.url), "utf8");
  const block = rust.slice(rust.indexOf("pub const CAMPFIRE: Logs"), rust.indexOf("pub const NONE"));
  const told = (name: string) => Number(new RegExp(`\\b${name}: ([-\\d.]+)`).exec(block)![1]);
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
