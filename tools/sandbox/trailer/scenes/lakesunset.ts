// The dusk lake from its west corner, facing the set sun's side across the water (D446; Greg,
// desk q-lake-evening: "Face the sunset only"), the trailer's lake at dusk. The troll sat on a mossy
// boulder by its campfire on the shore, the camera drifting slowly past it, the far water and
// the low skyline 30-50 degrees left of the set sun (the lowest skyline from any shore, ~8-10
// degrees; toward the sun itself the range stands 19-22 degrees). Places in the start's metres.
import type { Script } from "../../qa/script.ts";

const ROLLS = 360;
const SECONDS = 18;
/// The way the camera looks, from the start's east toward its south (degrees).
const WAY = Number(process.env.LS_WAY ?? -35);
const TROLL: [number, number] = [Number(process.env.LS_E ?? -374), Number(process.env.LS_S ?? -76)];
const SEAT_BACK = 0.95;
const SEAT_SCALE = 0.9;
const FIRE_OFF = 1.2;
const FIRE_TURN = -0.35;
const FIRE_SCALE = 1.3;
const FIRE_BURNS = [1.95, 0.9, 0.9];
const SIT_BEND = { upperarm_l: [-16, 0, 30], upperarm_r: [-16, 0, -30], lowerarm_l: [30, 0, 14], lowerarm_r: [30, 0, -14] };

function script(s: Script) {
  const t = (WAY * Math.PI) / 180;
  const d: [number, number] = [Math.cos(t), Math.sin(t)];
  const r: [number, number] = [-d[1], d[0]];
  const at = (back: number, side: number): [number, number] => [TROLL[0] - d[0] * back + r[0] * side, TROLL[1] - d[1] * back + r[1] * side];
  // The troll faces the camera, its back to the water, turned a little toward the sun.
  const FACES = at(6, 2);
  const f = [FACES[0] - TROLL[0], FACES[1] - TROLL[1]];
  const fl = Math.hypot(f[0], f[1]);
  const [fe, fs] = [f[0] / fl, f[1] / fl];
  s.frame = 30;
  s.call("staged.spawn", {
    name: "seat",
    model: "models/nature/forest/mossy_rock_b_6.glb",
    at: [TROLL[0] - fe * SEAT_BACK, 0, TROLL[1] - fs * SEAT_BACK],
    facing: [TROLL[0] - fe * SEAT_BACK + fs, 0, TROLL[1] - fs * SEAT_BACK - fe],
    scale: SEAT_SCALE,
    base: true,
  });
  const [c, n] = [Math.cos(FIRE_TURN), Math.sin(FIRE_TURN)];
  const fire: [number, number] = [TROLL[0] + (fe * c - fs * n) * FIRE_OFF, TROLL[1] + (fs * c + fe * n) * FIRE_OFF];
  s.call("staged.spawn", { name: "fire", model: "models/props/Campfire.glb", at: [fire[0], 0, fire[1]], facing: [fire[0] + 1, 0, fire[1]], scale: FIRE_SCALE, burns: FIRE_BURNS, base: true });
  s.call("staged.spawn", { name: "troll", model: "models/characters/troll.glb", bend: SIT_BEND, at: [TROLL[0], 0, TROLL[1]], facing: [FACES[0], 0, FACES[1]], turn_deg_s: 20, base: true });
  s.frame = 40;
  s.call("staged.play", { name: "troll", clip: "Sitting_Idle_Loop", speed: 0.55, blend_ms: 0 });
  // The camera: inland of the troll, a little left, drifting right and in over the film,
  // looking past him over the water to the far skyline, tilted a little up for the sky.
  const far = (e: [number, number]): [number, number, number] => [e[0] + d[0] * 900, 40, e[1] + d[1] * 900];
  const e0 = at(18, -4);
  const e1 = at(15.5, -2.5);
  s.frame = 60;
  s.cut([e0[0], 3.9, e0[1]], far(e0));
  s.frame = ROLLS;
  s.glide([e1[0], 3.6, e1[1]], far(e1), SECONDS * 60 + 30);
  // Its head turns slowly once over the film, toward the water and back.
  for (let frame = 60; frame <= ROLLS + SECONDS * 60; frame += 30) {
    const turn = 25 * Math.sin(((frame - ROLLS) / (SECONDS * 60)) * Math.PI);
    s.frame = frame;
    s.call("staged.bend", { name: "troll", bend: { ...SIT_BEND, spine_03: [0, turn * 0.2, 0], neck_01: [-4, turn * 0.45, 0], Head: [0, turn * 0.35, 0] }, blend_ms: 800 });
  }
}

export default {
  about: "the dusk lake from its west corner, the troll by its campfire on the shore, facing the set sun's side across the water",
  sandbox: [],
  rolls: ROLLS,
  script,
};
