import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { test } from "node:test";
import { cleared, raises, twoBone } from "./clear.ts";
import { Glb, len, posed, sub, type V3 } from "./glb.ts";
import { bake } from "./markers.ts";

const FIGHT = resolve(import.meta.dirname, "../../assets/models/character/fight.glb");

test("a gliding swing is raised along half a sine until its highest is the bar; planted frames and a clear swing are not", () => {
  // A clip played once: planted, a swing (frames 3-7) gliding a centimetre up, planted again.
  const lock = [1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 1];
  const glide = [0, 0, 0, 0.01, 0.012, 0.01, 0.012, 0.01, 0, 0, 0];
  const up = raises(glide, lock, false, () => 0.08);
  const highest = Math.max(...glide.map((s, i) => s + up[i]));
  assert.ok(Math.abs(highest - 0.08) < 1e-9, `highest ${highest}`);
  lock.forEach((l, i) => l > 0 && assert.equal(up[i], 0, `planted frame ${i} raised`));
  assert.ok(up[5] > up[3] && up[5] > up[7], "the middle raised most");
  // A swing already over its bar (the walk's) keeps its shape.
  const step = [0, 0, 0, 0.12, 0.1, 0.09, 0.1, 0.12, 0, 0, 0];
  assert.deepEqual(raises(step, lock, false, () => 0.08), step.map(() => 0));
});

test("two-bone IK brings the ankle to its goal with the thigh and shin their lengths", () => {
  const [hip, knee, ankle]: V3[] = [
    [0, 1, 0],
    [0, 0.55, 0.08],
    [0, 0.1, 0],
  ];
  const goal: V3 = [0, 0.18, 0];
  const to = twoBone(hip, knee, ankle, goal);
  assert.ok(len(sub(to.ankle, goal)) < 1e-6);
  assert.ok(Math.abs(len(sub(to.knee, hip)) - len(sub(knee, hip))) < 1e-6);
  assert.ok(Math.abs(len(sub(to.ankle, to.knee)) - len(sub(ankle, knee))) < 1e-6);
  assert.ok(to.knee[2] > 0.08, "the knee bends further the way it bent");
});

test("the game's fight clips are already clear: clearing them again changes nothing", () => {
  const { out, done } = cleared(readFileSync(FIGHT), { Fight_Strafe_R: {}, Fight_Strafe_L: {}, Walk_Loop: {} });
  assert.equal(out, undefined, done.map((d) => `${d.clip} ${d.side} raised ${(d.most * 100).toFixed(1)} cm`).join(", "));
});

test("Fight_Strafe_R cleared to a 12 cm bar: each swing's foot comes to it, every planted frame where it was", () => {
  const name = "Fight_Strafe_R";
  const bytes = readFileSync(FIGHT);
  const { out } = cleared(bytes, { [name]: { clear: 0.12 } });
  assert.ok(out, "a bar over its highest raises it");
  const [was, now] = [new Glb(bytes), new Glb(out)];
  const { markers } = bake(was)[name];
  for (const side of ["l", "r"]) {
    const lock = side === "l" ? markers.lock_l : markers.lock_r;
    const [heel, ball] = [`foot_${side}`, `ball_${side}`].map((n) => was.named(n));
    const at = (body: Glb, k: number) => posed(body, body.clip(name)!, k / markers.rate);
    const frames = lock.map((_, k) => [at(was, k), at(now, k)]);
    const low = [heel, ball].map((p) => Math.min(...frames.map(([w]) => w[p].t[1])));
    let highest = 0;
    let moved = 0;
    frames.forEach(([w, n], k) => {
      if (lock[k] > 0) for (const p of [heel, ball]) moved = Math.max(moved, len(sub(w[p].t, n[p].t)));
      else highest = Math.max(highest, Math.min(n[heel].t[1] - low[0], n[ball].t[1] - low[1]));
    });
    assert.ok(moved < 0.001, `${side}: a planted frame moved ${(moved * 100).toFixed(2)} cm`);
    assert.ok(highest > 0.115, `${side}: highest ${(highest * 100).toFixed(1)} cm`);
  }
});
