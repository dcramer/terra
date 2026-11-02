import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { Glb, place, type V3 } from "./glb.ts";

const ROOT = join(import.meta.dirname, "..", "..");

test("a held look's row says which of its points went deepest, into what, when and on which body", () => {
  const dir = mkdtempSync(join(tmpdir(), "moves-where-"));
  const json = join(dir, "rows.json");
  const look = "assets/models/held/stone_axe.glb";
  const run = spawnSync(process.execPath, [join(ROOT, "tools", "characters", "moves.ts"), "--set", "none", "--also", "Walk_Loop", "--template", "male", "--hold", `socket.hand.R=${look}`, "--carry", "--arms", "--json", json], { cwd: ROOT, encoding: "utf8" });
  const rows = JSON.parse(readFileSync(json, "utf8")) as { clip: string; measured: Record<string, number>; where?: Record<string, string> }[];
  rmSync(dir, { recursive: true, force: true });
  const walk = rows.find((r) => r.clip === "Walk_Loop");
  assert.ok(walk, run.stderr);
  const said = walk.where?.["held_cm.stone_axe"] ?? "";
  const m = /^its point \((-?\d+), (-?\d+), (-?\d+)\) cm in (\w+) in Walk_Loop at ([\d.]+) s, male$/.exec(said);
  assert.ok(m, said);
  // The point is one of the look's own vertices (its frame as made, the grip at the origin), to a
  // centimetre's rounding, not one hung in the hand.
  const g = new Glb(join(ROOT, look));
  const node = g.gltf.nodes.findIndex((n) => n.mesh !== undefined);
  const pose = g.rest(node);
  const at = [1, 2, 3].map((i) => Number(m[i]) / 100);
  let near = Infinity;
  for (const prim of g.gltf.meshes![g.gltf.nodes[node].mesh!].primitives) {
    const p = g.read(prim.attributes.POSITION);
    for (let i = 0; i < p.length; i += 3) near = Math.min(near, Math.hypot(...place(pose, [p[i], p[i + 1], p[i + 2]] as V3).map((v, j) => v - at[j])));
  }
  assert.ok(near < 0.01, `${said}: ${(near * 100).toFixed(1)} cm from its nearest vertex`);
});
