// The game's swim clips are made in the water (D584): the strokes lie level, legs kicking from
// the hip, afloat at the man's float line, and the tread stands upright, so the game shows them
// as made and pitches a stroke only along its way (terra_walker::swim). A stroke keyed standing
// and laid down by the game (the Kimodo mimes: trunk upright, knees to 168 deg) fails here,
// on the clips the game plays, with no Blender.
// node --test tools/characters/swim_clips.test.ts

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { Glb, posed, sub, dot, len, type V3 } from "./glb.ts";

const body = new Glb("assets/models/character/base.glb");
const clips = new Glb(process.env.SWIM_CLIPS ?? "assets/models/character/fight.glb");
const deg = (r: number) => (r * 180) / Math.PI;
const angle = (a: V3, b: V3) => deg(Math.acos(Math.max(-1, Math.min(1, dot(a, b) / (len(a) * len(b))))));
const node = (name: string) => body.named(name);

/** Where a floating man's surface is over his feet, m: movers.ron's height times float depth. */
function floatLine(): number {
  const ron = readFileSync("assets/data/movers.ron", "utf8");
  const height = Number(ron.match(/^\s*height:\s*([\d.]+)/m)?.[1]);
  const depth = Number(ron.match(/^\s*float_depth:\s*([\d.]+)/m)?.[1]);
  return height * depth;
}

/** Through a clip at 30 frames a second: the trunk's angle from level (pelvis to neck, deg),
 * the knees' bend (deg) and the hips' height (m), each frame. */
function through(clip: string) {
  const motion = clips.clip(clip);
  assert.ok(motion, `${clip} is not in fight.glb: rebuild the bodies (build_bodies.sh)`);
  const out = { trunk: [] as number[], knee: [] as number[], hips: [] as number[] };
  for (let t = 0; t <= motion.end + 1e-6; t += 1 / 30) {
    const p = posed(body, motion, t);
    const at = (name: string) => p[node(name)].t;
    out.trunk.push(90 - angle(sub(at("neck_01"), at("pelvis")), [0, 1, 0]));
    for (const s of ["l", "r"]) out.knee.push(angle(sub(at(`calf_${s}`), at(`thigh_${s}`)), sub(at(`foot_${s}`), at(`calf_${s}`))));
    out.hips.push(at("pelvis")[1]);
  }
  return out;
}

const most = (xs: number[]) => Math.max(...xs.map(Math.abs));
const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / xs.length;

for (const clip of ["Swim_Fwd_Loop", "Swim_Dive_Loop"]) {
  test(`${clip} lies level in the water, kicking from the hip, afloat`, () => {
    const m = through(clip);
    // A crawl's trunk rolls and lifts to breathe within a few tens of degrees of level.
    assert.ok(most(m.trunk) <= 30, `${clip}: the trunk is ${most(m.trunk).toFixed(0)} deg off level at worst: it is not made lying`);
    // A flutter kick bends the knees a little; a standing mime's high knees fold past 90.
    assert.ok(most(m.knee) <= 60, `${clip}: a knee bends ${most(m.knee).toFixed(0)} deg: a kick from the hip keeps them near straight`);
    const line = floatLine();
    assert.ok(Math.abs(mean(m.hips) - line) <= 0.15, `${clip}: the hips ride ${mean(m.hips).toFixed(2)} m over the ground on average, the float line is ${line.toFixed(2)} m (made.json afloat)`);
  });
}

test("Swim_Tread_Loop treads upright", () => {
  const m = through("Swim_Tread_Loop");
  assert.ok(mean(m.trunk) >= 70, `the tread's trunk is ${mean(m.trunk).toFixed(0)} deg over level on average`);
});
