import { strict as assert } from "node:assert";
import { writeFileSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "node:test";
import { Glb, posed } from "./glb.ts";
import { mirrored, partner } from "./mirror.ts";

const FIGHT = resolve(import.meta.dirname, "../../assets/models/character/fight.glb");

test("a bone's partner is across the middle, a middle bone its own", () => {
  const names = new Set(["thigh_l", "thigh_r", "pelvis", "index_04_leaf_l", "index_04_leaf_r"]);
  assert.equal(partner("thigh_l", names), "thigh_r");
  assert.equal(partner("index_04_leaf_r", names), "index_04_leaf_l");
  assert.equal(partner("pelvis", names), "pelvis");
});

test("a mirrored clip puts each joint where its partner was, reflected; mirrored again, as it was", () => {
  const dir = mkdtempSync(join(tmpdir(), "mirror-"));
  const once = join(dir, "once.glb");
  writeFileSync(once, mirrored(FIGHT, { Mirrored: "Jog_Stop" })!);
  const twice = join(dir, "twice.glb");
  writeFileSync(twice, mirrored(once, { Back: "Mirrored" })!);
  const [a, b, c] = [new Glb(FIGHT), new Glb(once), new Glb(twice)];
  const joints = ["ball_l", "hand_r", "Head", "foot_r"];
  const rest = posed(a, undefined, 0);
  const middle = (rest[a.named("thigh_l")].t[0] + rest[a.named("thigh_r")].t[0]) / 2;
  for (const t of [0.4, 1.8, 2.1]) {
    const [pa, pb, pc] = [posed(a, a.clip("Jog_Stop"), t), posed(b, b.clip("Mirrored"), t), posed(c, c.clip("Back"), t)];
    for (const name of joints) {
      const other = partner(name, new Set(a.gltf.nodes.map((n) => n.name ?? "")));
      const was = pa[a.named(other)].t;
      const now = pb[b.named(name)].t;
      // Across the model's X (its thighs lie apart along it), midway between the thighs at
      // rest, a rest pose itself within 7 mm of symmetric.
      assert.ok(Math.hypot(now[0] - (2 * middle - was[0]), now[1] - was[1], now[2] - was[2]) < 0.007, `${name} at ${t}: ${now} vs ${was}`);
      const back = pc[c.named(name)].t;
      const orig = pa[a.named(name)].t;
      assert.ok(Math.hypot(back[0] - orig[0], back[1] - orig[1], back[2] - orig[2]) < 1e-3, `${name} back at ${t}`);
    }
  }
});
