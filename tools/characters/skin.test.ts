// skin.ts's depth on the real body: a distance to a surface changes no faster than the point
// moves, so a hand point stepped a centimetre outward can read at most a centimetre less deep.
// The female template's hip crease in Walk_Loop broke it (a thumb 3 cm farther out read
// -6.4 then +8.8 cm, item-attach round 4). And surface.ts's fast search answers as its plain
// reference does, to the bit, on the body's poses, so the move check's numbers never move with it.
//   node --test tools/characters/skin.test.ts

import { test } from "node:test";
import assert from "node:assert/strict";
import { resolve } from "node:path";
import { Glb, type V3 } from "./glb.ts";
import { BODY, armPoints, shaped, skinOf } from "./skin.ts";
import { reference } from "./surface.ts";

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

test("the middle of a shin and of a foot are inside the body's skin, and outside the trunk's", () => {
  // Known another way: a bone runs inside its limb, so the point halfway down the shin and the
  // one halfway along the foot are under the skin, named for the part and side they are in. The
  // trunk (torso, head, thighs) has no skin there, which is how a tool's head in the shin passed.
  const body = new Glb(resolve(ROOT, "assets/models/character/base.glb"));
  const clip = new Glb(resolve(ROOT, "assets/models/character/fight.glb")).clip("Walk_Loop")!;
  const templates = resolve(ROOT, "assets/models/character/templates.json");
  const [whole, trunk] = [skinOf(body, templates, [], BODY), skinOf(body, templates)];
  const bad: string[] = [];
  whole.shapes.forEach((shape, i) => {
    for (const t of [0, clip.end / 3, (2 * clip.end) / 3]) {
      const pose = shaped(body, shape, clip, t);
      const at = (n: string) => pose[body.named(n)].t;
      const mid = (a: V3, b: V3): V3 => [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2];
      const [inBody, inTrunk] = [whole.skinned(shape, pose), trunk.skinned(trunk.shapes[i], pose)];
      for (const s of ["l", "r"])
        for (const [part, p] of [[`calf_${s}`, mid(at(`calf_${s}`), at(`foot_${s}`))], [`foot_${s}`, mid(at(`foot_${s}`), at(`ball_${s}`))]] as [string, V3][]) {
          const [b, k] = [inBody(p), inTrunk(p)];
          if (!(b.depth > 0.005) || b.part !== part) bad.push(`${part} at ${t.toFixed(2)} s, ${shape.name}: ${(b.depth * 100).toFixed(1)} cm in ${b.part} of the body`);
          if (k.depth > 0) bad.push(`${part} at ${t.toFixed(2)} s, ${shape.name}: ${(k.depth * 100).toFixed(1)} cm in ${k.part} of the trunk`);
        }
    }
  });
  assert.deepEqual(bad.slice(0, 5), [], `${bad.length} wrong`);
});

test("a template that is not one is an error naming it and the ones there are", () => {
  const body = new Glb(resolve(ROOT, "assets/models/character/base.glb"));
  const templates = resolve(ROOT, "assets/models/character/templates.json");
  assert.throws(() => skinOf(body, templates, ["female", "base"]), /no body template "base" in .*templates\.json.*male, female, heavy, thin, strong/);
  assert.equal(skinOf(body, templates, ["female"]).shapes.length, 1);
});

test("the fast surface answers as its reference does, to the bit, on the body's poses", () => {
  // Every template through a walk, a crouch, a roll, a blow and a block, at the arm's points and
  // the trunk's joints and points scattered within 30 cm of them (in the skin, on it, out of it,
  // where the hip's crease cuts through itself): the same depth, sign and part, every one.
  const body = new Glb(resolve(ROOT, "assets/models/character/base.glb"));
  const fight = new Glb(resolve(ROOT, "assets/models/character/fight.glb"));
  const { shapes, skinned } = skinOf(body, resolve(ROOT, "assets/models/character/templates.json"));
  const node = new Map(body.gltf.nodes.map((n, i) => [n.name, i]));
  let seed = 1;
  const random = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648) * 2 - 1;
  const differ: string[] = [];
  let [points, inside] = [0, 0];
  for (const name of ["Walk_Loop", "Crouch_Idle_Loop", "Fight_Roll", "Open_Heavy", "Clear_Block"]) {
    const clip = fight.clip(name)!;
    for (const shape of shapes)
      for (let t = 0; t <= clip.end + 1e-6; t += clip.end / 3) {
        const pose = shaped(body, shape, clip, t);
        const [fast, sure] = [skinned(shape, pose), skinned(shape, pose, reference)];
        const at = (n: string) => pose[node.get(n)!].t;
        // The trunk's own joints too, so the sample reaches into the skin however clear of it
        // the clips hold the arms (the D542 bodies' arms left 468 of 11,200 arm points inside).
        const trunk: [V3, number, string][] = ["pelvis", "spine_02", "thigh_l", "thigh_r"].map((n) => [at(n), 0, n]);
        for (const [p, , what] of [...armPoints(at, "l"), ...armPoints(at, "r"), ...trunk])
          for (let k = 0; k < 8; k++) {
            const reach = k === 0 ? 0 : k < 4 ? 0.03 : 0.3;
            const q: V3 = [p[0] + reach * random(), p[1] + reach * random(), p[2] + reach * random()];
            const [a, b] = [fast(q), sure(q)];
            points++;
            if (b.depth > 0) inside++;
            if (!Object.is(a.depth, b.depth) || a.part !== b.part) differ.push(`${what} +${k} at ${t.toFixed(2)} s of ${name}, ${shape.name}: ${a.depth} ${a.part}, not ${b.depth} ${b.part}`);
          }
      }
  }
  assert.deepEqual(differ.slice(0, 5), [], `${differ.length} of ${points} differ`);
  assert.ok(inside > points / 20, `only ${inside} of ${points} inside: the sample misses the skin`);
});
