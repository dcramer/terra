// skin.ts's depth on the real body: a distance to a surface changes no faster than the point
// moves, so a hand point stepped a centimetre outward can read at most a centimetre less deep.
// The female template's hip crease in Walk_Loop broke it (a thumb 3 cm farther out read
// -6.4 then +8.8 cm, item-attach round 4).
//   node --test tools/characters/skin.test.ts

import { test } from "node:test";
import assert from "node:assert/strict";
import { resolve } from "node:path";
import { Glb, type V3 } from "./glb.ts";
import { armPoints, shaped, skinOf } from "./skin.ts";

const ROOT = resolve(import.meta.dirname, "../..");

test("a hand point stepped outward by the hip changes depth by no more than the step", () => {
  const body = new Glb(resolve(ROOT, "assets/models/character/base.glb"));
  const clip = new Glb(resolve(ROOT, "assets/models/character/fight.glb")).clip("Walk_Loop")!;
  const { shapes, skinned } = skinOf(body, resolve(ROOT, "assets/models/character/templates.json"), ["female"]);
  const node = new Map(body.gltf.nodes.map((n, i) => [n.name, i]));
  const STEP = 0.01;
  const bad: string[] = [];
  for (let t = 0; t <= clip.end + 1e-6; t += 1 / 15) {
    const pose = shaped(body, shapes[0], clip, t);
    const inside = skinned(shapes[0], pose);
    for (const [side, s] of [[1, "l"], [-1, "r"]] as const)
      for (const [p, , what] of armPoints((n) => pose[node.get(n)!].t, s)) {
        if (!what.startsWith("hand")) continue;
        let last = inside(p).depth;
        for (let k = 1; k <= 6; k++) {
          const q: V3 = [p[0] + side * STEP * k, p[1], p[2]];
          const depth = inside(q).depth;
          if (Math.abs(depth - last) > STEP + 1e-6) bad.push(`${what} at ${t.toFixed(2)} s, ${k} cm out: ${(last * 100).toFixed(1)} -> ${(depth * 100).toFixed(1)} cm`);
          last = depth;
        }
      }
  }
  assert.deepEqual(bad.slice(0, 5), [], `${bad.length} steps jump`);
});
