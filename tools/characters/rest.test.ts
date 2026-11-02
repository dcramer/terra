import assert from "node:assert/strict";
import { test } from "node:test";
import { Glb, type Clip, type Pose } from "./glb.ts";
import { keyClip } from "./key.ts";
import { judge, measure } from "./rest.ts";

// Arms posed by the keys on the game's body: what rest.ts measures is what they asked for.
const g = new Glb("assets/models/character/base.glb");
/// A clip of `samples` (a frame each 1/60 s), held by its nearest frame.
function clipOf(samples: Pose[][], name: string): Clip {
  const end = (samples.length - 1) / 60;
  const frame = (t: number) => samples[Math.max(0, Math.min(samples.length - 1, Math.round(t * 60)))];
  const tracks = new Map<string, Partial<Record<keyof Pose, (t: number) => number[]>>>();
  g.gltf.nodes.forEach((n: { name?: string }, i: number) => n.name && tracks.set(n.name, { r: (t) => frame(t)[i].r, t: (t) => frame(t)[i].t }));
  return { name, end, tracks };
}
const hang = (out: number, elbow: number) => {
  const spec = { name: "Test", guard: "Idle_Loop", end: 0.5, hang: true, rest_arm: { out, forward: 0, turn_in: 20, elbow, palm: 0 }, fingers: 90, keys: [{ t: 0, guard: true }, { t: 0.5 }] };
  return clipOf(keyClip(spec, g).samples, "Test");
};

test("rest.ts reads the arm's hang and elbow as keyed", () => {
  const m = measure(hang(7.5, 18), "male", 0);
  for (const s of ["r", "l"] as const) {
    assert.ok(Math.abs(m[s].upper - 7.5) < 1.5, `${s} upper ${m[s].upper.toFixed(1)}`);
    assert.ok(Math.abs(m[s].elbow - 18) < 1, `${s} elbow ${m[s].elbow.toFixed(1)}`);
  }
});

test("arms held out from the body or locked straight fail the person at ease", () => {
  for (const [out, elbow, what] of [[30, 18, "upper"], [7.5, 2, "elbow"]] as const) {
    const sides = judge(hang(out, elbow));
    assert.ok(sides.every((s) => !s.pass && s.parts.some((p) => p.startsWith(what) && p.endsWith("✗"))), `${what}: ${JSON.stringify(sides)}`);
  }
});
