// bones.ts's judging on made-up records: a layer turning a bone 20 degrees reads 20, on half
// the frames a mean of 10, as that layer's alone; a run off its script is not measured.
// node --test tools/controller/bones.test.ts

import assert from "node:assert/strict";
import { test } from "node:test";
import { type Line, MOTIONS, judge, layerWorst, offScript, summary, tally } from "./bones.ts";

/// A record of the whole script, each frame at its motion's speed on flat ground, the layers
/// `layers(frame)` gives.
function record(layers: (frame: number) => Line["layers"]): Line[] {
  const end = MOTIONS[MOTIONS.length - 1].to;
  return Array.from({ length: end }, (_, frame) => {
    const motion = MOTIONS.find((m) => frame >= m.from && frame < m.to);
    const speed = motion ? (motion.speed[0] + motion.speed[1]) / 2 : 0;
    return { frame, speed, up: 2000, layers: layers(frame) };
  });
}

const jog = MOTIONS.find((m) => m.name === "jog, hammer")!;
const inJog = (frame: number) => frame >= jog.from && frame < jog.to;

test("a layer turning a bone 20 degrees on every frame reads 20, worst and mean", () => {
  const lines = record((frame) => (inJog(frame) ? { feet: { calf_l: [20, 0] } } : {}));
  const found = layerWorst(tally(lines), "feet");
  assert.deepEqual(found.worst, ["calf_l", "jog, hammer", 20]);
  assert.ok(Math.abs(found.mean[2] - 20) < 1e-9, `${found.mean}`);
  const checks = judge(lines);
  const worst = checks.find((c) => c.name === "bones feet worst")!;
  assert.equal(worst.value, 20);
  assert.equal(worst.ok, false);
  // Only the foot lock's: the other layers read nothing and pass.
  assert.equal(checks.find((c) => c.name === "bones arms mean")!.value, 0);
  assert.ok(checks.find((c) => c.name === "bones arms mean")!.ok);
  assert.match(summary(tally(lines)).find((l) => l.startsWith("feet"))!, /worst calf_l 20°/);
  assert.equal(summary(tally(lines)).find((l) => l.startsWith("arms")), "arms: still");
});

test("on half the frames its mean halves and its worst holds", () => {
  const lines = record((frame) => (inJog(frame) && frame % 2 === 0 ? { feet: { foot_r: [20, 0] } } : {}));
  const found = layerWorst(tally(lines), "feet");
  assert.equal(found.worst[2], 20);
  assert.ok(Math.abs(found.mean[2] - 10) < 0.1, `${found.mean}`);
  assert.equal(judge(lines).find((c) => c.name === "bones feet mean")!.value, 10);
});

test("a bone moved, not turned, is its travel, not degrees", () => {
  const lines = record((frame) => (inJog(frame) ? { feet: { pelvis: [0, 3] } } : {}));
  const found = layerWorst(tally(lines), "feet");
  assert.equal(found.worst[2], 0);
  assert.deepEqual(found.cm, ["pelvis", "jog, hammer", 3]);
  assert.match(summary(tally(lines)).find((l) => l.startsWith("feet"))!, /moves pelvis 3 cm/);
});

test("a run off its script is not measured, saying why", () => {
  // Standing through the jog, and the ground rising under the sprint.
  const lines = record(() => ({})).map((l) => {
    if (inJog(l.frame)) return { ...l, speed: 0 };
    if (l.frame >= 870 && l.frame < 1140) return { ...l, up: 2000 + (l.frame - 870) * 0.001 };
    return l;
  });
  const whys = offScript(lines);
  assert.equal(whys.length, 2, whys.join("; "));
  assert.match(whys[0], /jog, hammer: 0.00 m\/s/);
  assert.match(whys[1], /sprint, hammer: the ground rose/);
  for (const c of judge(lines)) {
    assert.ok(Number.isNaN(c.value) && !c.ok);
    assert.match(c.line, /not measured/);
  }
  // A record cut short is off its script too.
  assert.match(offScript(lines.slice(0, 500)).join(";"), /frames recorded/);
});
