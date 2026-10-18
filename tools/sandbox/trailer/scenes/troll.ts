// A troll fights a man on the mountain lake's south shore (docs/troll-fight.md): a troll
// about 4 m tall comes out of the trees at a man with a sword and a round shield on the
// gravel between the forest's edge and the water, roars and comes on; it smashes at him and
// he rolls aside, the blow throwing up dust and stones, and while it is still bent over the
// blow he cuts at its leg twice; it sweeps him back on his shield; it presses him to the
// water's edge and smashes again into the shallows as he sidesteps; his combination of three
// drives it reeling back into the water, where it roars over him. Told over the remote
// (`play::staged`), the camera close and cut on the beats, never looking into the low morning
// sun in the east-north-east.
//
// The fight is timed as shipped melee is (research/melee-feel.md): every blow from
// its clip's own moments, the top of its wind-up held a beat so it reads (Elden Ring's giants),
// its strike fast, a hit-stop on contact freezing attacker and target for a few frames with a
// short shake (Sakurai; Monster Hunter's heavy weapons), and its follow-through played out;
// the next move starts in the last one's recovery, as a combo cancels, never from an idle.
//
// Places are in the start's own metres, east, up and south of it (the base frame of a scene
// run, which `--at` is measured from), written here from the arena: east and south of its
// middle on the gravel, `ARENA`. Frames are a sixtieth of a second each.

import type { Script, Vec3 } from "../../qa/script.ts";

/// The frame the film rolls on: before it the fighters are stood, and the camera goes round
/// every shot's place so the land is streamed in round them.
const ROLLS = 900;
const at = (seconds: number) => ROLLS + Math.round(seconds * 60);
const S = (seconds: number) => Math.round(seconds * 60);

/// The arena, east and south of the start: the gravel strip along the water, the lake to the
/// north-west. `WATERLINE` is a point on the water's edge and `SHALLOWS` one a third of a
/// metre deep, both east and south of the arena's middle. Set from a probe of the shore:
/// tiny figures stood on a 5 m grid, their spawn logs saying each spot's height and water.
const ARENA: [number, number, number] = [244, 0, 10];
const WATERLINE: [number, number] = [-3.5, -1];
const SHALLOWS: [number, number] = [-5, -3];

/// The probe's heights over the shore, metres up of the start, every 5 m from 225 east and
/// -5 south; over water, the lake's surface (-1.45). Read between points as a bilinear patch,
/// so the fighters' places and the camera stand on the land rather than on the start's level.
const GROUND = [
  [-1.4, -1.4, -1.4, -1.5, -1.5, -1.5, -1.3, -0.5], // -5
  [-1.4, -1.4, -1.5, -1.5, -1.5, -1.2, -0.6, 0.1], // 0
  [-1.4, -1.5, -1.5, -1.5, -1.3, -0.6, 0.0, 0.7], // 5
  [-1.5, -1.5, -1.5, -1.5, -0.9, -0.1, 0.4, 1.0], // 10
  [-0.9, -1.0, -0.8, -0.7, -0.2, 0.4, 0.8, 1.4], // 15
  [0.0, 0.0, 0.5, 0.7, 0.7, 1.0, 1.3, 2.2], // 20
  [1.2, 1.4, 2.0, 2.1, 1.8, 1.8, 2.0, 2.9], // 25
  [2.4, 2.6, 3.3, 3.7, 3.5, 3.3, 3.3, 3.7], // 30
];
function ground(east: number, south: number): number {
  const u = Math.min(6.999, Math.max(0, (east - 225) / 5));
  const v = Math.min(6.999, Math.max(0, (south + 5) / 5));
  const [i, j] = [Math.floor(u), Math.floor(v)];
  const [fu, fv] = [u - i, v - j];
  const row = (r: number[]) => r[i] + (r[i + 1] - r[i]) * fu;
  return row(GROUND[j]) + (row(GROUND[j + 1]) - row(GROUND[j])) * fv;
}

/// A clip as a fighter plays it: its name in the file and its moments, seconds into the clip
/// at its own speed, measured from its take (made_clips.py's ONCE line, the hands' heights
/// and speeds): where its action starts after a slow start (`from`), the top of its wind-up
/// (`top`), when the blow lands (`hit`), and when it is back on its feet (`done`).
type Clip = { name: string; from?: number; top?: number; hit?: number; done?: number };
const TROLL_CLIPS: Record<string, Clip> = {
  idle: { name: "Troll_Idle" },
  walk: { name: "Troll_Walk" },
  // Its hands gather low at 1.3 s, are highest at 1.8, fastest at 2.1 and in the ground at
  // 2.3; it is bent over the blow until 3.5.
  smash: { name: "Troll_Smash", from: 1.0, top: 1.78, hit: 2.3, done: 3.5 },
  // Its right arm drawn back at 1.0 s, fastest at 1.33 across its front, through by 1.5.
  sweep: { name: "Troll_Sweep", from: 0.6, top: 1.0, hit: 1.3, done: 2.2 },
  hit: { name: "Troll_Hit", done: 1.6 },
  roar: { name: "Troll_Roar" },
};
const MAN_CLIPS: Record<string, Clip> = {
  idle: { name: "Idle_Loop" },
  guard: { name: "Fight_Guard" },
  left: { name: "Fight_Strafe_L" },
  right: { name: "Fight_Strafe_R" },
  // The sword arm highest at 0.37, 0.43 and 0.5 s, fastest at 0.82, 0.82 and 0.98, landing (it
  // first slows below 30% of that) at 1.02, 0.98 and about 1.1: the chop's measured 1.62 is
  // late, its lunge carrying the hand on after the cut.
  slash: { name: "Fight_Slash", from: 0.15, top: 0.37, hit: 1.02, done: 1.55 },
  backhand: { name: "Fight_Backhand", from: 0.2, top: 0.43, hit: 0.98, done: 1.5 },
  chop: { name: "Fight_Chop", from: 0.25, top: 0.5, hit: 1.1, done: 1.7 },
  // The shield up and braced at 1.2 s, pushed back from 1.6.
  block: { name: "Fight_Block", from: 0.7, hit: 1.25, done: 2.0 },
  dodge: { name: "Fight_Dodge" },
  roll: { name: "Roll" },
  step: { name: "Walk_Loop" },
};

/// The troll: its model and how fast it plays its clips, slowed for its size by Froude
/// scaling (twice our height moves about 1.5 times slower), which is what makes it read as
/// heavy; its strikes are sped up again within the blow (`perform`).
const TROLL = { model: "models/characters/troll.glb", rate: 0.85 };
/// Its clips were made on a person: their hands hang and meet where a person's would, inside
/// its belly. Its upper arms are turned out from its sides and forward over every clip
/// (`staged.bend`), as a retarget pose fits a clip to a heavier body.
const TROLL_BEND = { upperarm_l: [-14, 0, 22], upperarm_r: [-14, 0, -22] };
/// The man: the base body as the player's template, dressed, a sword in his right hand and a
/// round shield in his left; his clips from the fight file, on the base's skeleton. His left
/// upper arm is brought forward and out so the shield he carries on it stands clear in front
/// of his chest rather than through it.
const MAN = {
  model: "models/character/base.glb",
  template: "male",
  clips: "models/character/fight.glb",
  wears: ["shirt", "trousers", "boots", "leather_coat"],
  hold: { "socket.hand.R": "models/held/arming_sword.glb", "socket.hand.L": "models/held/round_shield.glb" },
  bend: { upperarm_l: [-30, 0, 14], lowerarm_l: [0, 0, 0] },
};
/// How far ahead of the troll its blows land, metres: its arm and fist at its size.
const REACH = 3.1;
/// A hit-stop, frames at 60 a second: how long attacker and target are held on contact, as
/// fighting games hold a light hit about 6 frames and a heavy one 10 to 14.
const STOP = { light: 6, heavy: 10, smash: 12 };
/// How slow a figure's time runs in a hit-stop: not quite still, so it does not read as a
/// stalled frame (Sakurai keeps the attacker moving a little).
const STOPPED = 0.04;

type V2 = [number, number];
const add = (a: Vec3, b: Vec3): Vec3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const scale = (a: Vec3, k: number): Vec3 => [a[0] * k, a[1] * k, a[2] * k];
const mix = (a: Vec3, b: Vec3, t: number): Vec3 => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
const level = (a: Vec3, b: Vec3) => Math.hypot(b[0] - a[0], b[2] - a[2]);
/// A level way from a to b, a unit long.
const way = (a: Vec3, b: Vec3): Vec3 => {
  const d = level(a, b) || 1;
  return [(b[0] - a[0]) / d, 0, (b[2] - a[2]) / d];
};
/// A level way turned a quarter to the right of another (seen from above, east of north).
const right = (w: Vec3): Vec3 => [-w[2], 0, w[0]];
const smooth = (t: number) => {
  const c = Math.min(1, Math.max(0, t));
  return c * c * (3 - 2 * c);
};
/// A place in the arena, east and south of its middle and so high over its gravel.
const p = ([x, z]: V2, y = 0): Vec3 => [ARENA[0] + x, ARENA[1] + ground(ARENA[0] + x, ARENA[2] + z) + y, ARENA[2] + z];
/// A point so high over the ground (or the water) under it: where the camera may stand.
const over = (v: Vec3, high: number): Vec3 => [v[0], Math.max(ground(v[0], v[2]), -1.45) + high, v[2]];

/// A fighter, as the remote is told it, and where it is on each frame, worked out as
/// `play::staged` moves it (straight on the level at its speed), so the camera can frame it;
/// and how fast its own time runs from each frame (`staged.rate`), so its blows are timed.
class Fighter {
  private legs: { frame: number; from: Vec3; to: Vec3; frames: number }[] = [];
  private rates: { frame: number; rate: number }[] = [];
  readonly name: string;
  readonly clips: Record<string, Clip>;
  readonly rate: number;
  private start: Vec3;
  constructor(name: string, start: Vec3, clips: Record<string, Clip>, rate: number) {
    this.name = name;
    this.start = start;
    this.clips = clips;
    this.rate = rate;
  }

  at(frame: number): Vec3 {
    let here = this.start;
    for (const leg of this.legs) {
      if (leg.frame > frame) break;
      here = mix(leg.from, leg.to, Math.min(1, (frame - leg.frame) / leg.frames));
    }
    return here;
  }

  /// Plays one of its clips from a frame: looping, or once and held on its last pose.
  play(s: Script, frame: number, clip: string, { once = false, blend = 250, speed = 1, from = 0 } = {}) {
    s.frame = frame;
    s.call("staged.play", { name: this.name, clip: this.clips[clip].name, speed: speed * this.rate, once, blend_ms: blend, ...(from > 0 ? { from_s: from } : {}) });
  }

  /// How fast its own time runs on a frame.
  rateAt(frame: number): number {
    let rate = 1;
    for (const r of this.rates) if (r.frame <= frame) rate = r.rate;
    return rate;
  }

  /// Its own time runs at a rate from a frame on.
  setRate(s: Script, frame: number, rate: number) {
    this.rates.push({ frame, rate });
    this.rates.sort((a, b) => a.frame - b.frame);
    s.frame = frame;
    s.call("staged.rate", { name: this.name, rate });
  }

  /// A hit-stop: its time all but stopped for so many frames from one.
  stop(s: Script, frame: number, frames: number) {
    this.setRate(s, frame, STOPPED);
    this.setRate(s, frame + frames, 1);
  }

  /// A blow (or a block, a stagger) played once from a frame, from its clip's `from`, its
  /// wind-up's top held `hold` seconds at a crawl and its strike played `strike` times as
  /// fast; a hit-stop of `stop` frames on contact. Returns the frame any moment of the clip
  /// is reached on, the hit-stops told later counted.
  perform(s: Script, frame: number, clip: string, { speed = 1, blend = 160, hold = 0, strike = 1, stop = 0 } = {}): (moment: number) => number {
    const c = this.clips[clip];
    const from = c.from ?? 0;
    this.play(s, frame, clip, { once: true, blend, speed, from });
    const when = (moment: number) => {
      let t = from;
      let f = frame;
      while (t < moment - 1e-6 && f < frame + 60 * 20) {
        t += (speed * this.rate * this.rateAt(f)) / 60;
        f++;
      }
      return f;
    };
    if (c.top !== undefined && hold > 0) {
      // Slowed into the top so the hold does not jolt, held, then the strike.
      const top = when(c.top - 0.06);
      this.setRate(s, top, 0.12);
      this.setRate(s, top + S(hold), strike);
    } else if (strike !== 1 && c.top !== undefined) {
      this.setRate(s, when(c.top), strike);
    }
    if (c.hit !== undefined) {
      const hit = when(c.hit);
      if (stop > 0) this.stop(s, hit, stop);
      else if (strike !== 1) this.setRate(s, hit, 1);
    }
    return when;
  }

  /// Moves it from a frame to a point at a speed, facing a point or another fighter.
  move(s: Script, frame: number, to: Vec3, speed: number, face?: Vec3 | Fighter) {
    const from = this.at(frame);
    this.legs.push({ frame, from, to, frames: Math.max(1, Math.round((level(from, to) / speed) * 60)) });
    this.legs.sort((a, b) => a.frame - b.frame);
    s.frame = frame;
    s.call("staged.move", { name: this.name, to, speed, ...facing("face", face), base: true });
  }

  /// The frame it gets to where it was last sent.
  arrives(): number {
    const leg = this.legs[this.legs.length - 1];
    return leg.frame + leg.frames;
  }

  face(s: Script, frame: number, face: Vec3 | Fighter) {
    s.frame = frame;
    s.call("staged.face", { name: this.name, ...facing("at", face), base: true });
  }
}

/// What a fighter is told to face, as the call takes it: another fighter by name, or a point.
const facing = (key: string, face?: Vec3 | Fighter) => (face === undefined ? {} : face instanceof Fighter ? { toward: face.name } : { [key]: face });

/// The heavy landings, each frame and how hard: the camera shakes on them.
const landings: { frame: number; amount: number }[] = [];
function impact(s: Script, frame: number, point: Vec3, size: number, shake: number) {
  s.frame = frame;
  s.call("staged.impact", { at: point, size, base: true });
  landings.push({ frame, amount: shake });
}

/// The camera on a frame, shaken by the landings before it: a hard jolt down and to the side
/// on the frame of contact, then a fast shake along the view's right and up dying away over
/// about 0.4 s, as a heavy blow near the camera jolts it; the look shaken a little less than
/// the eye, so the frame rocks rather than slides.
function shaken(frame: number, eye: Vec3, look: Vec3): [Vec3, Vec3] {
  let off: Vec3 = [0, 0, 0];
  const ahead = way(eye, look);
  const side = right(ahead);
  for (const { frame: f, amount } of landings) {
    const t = (frame - f) / 60;
    if (t < 0 || t > 0.5) continue;
    const fade = amount * Math.exp(-t * 9);
    off = add(off, add(scale(side, fade * Math.sin(t * 60 + 0.4)), [0, -fade * Math.cos(t * 75), 0]));
  }
  return [add(eye, off), add(look, scale(off, 0.5))];
}

/// A shot: from one frame to another the eye and what it looks at, each worked out per frame
/// (`t` from 0 to 1 through the shot), so it can follow the fighters as they move.
type Shot = { from: number; to: number; eye: (frame: number, t: number) => Vec3; look: (frame: number, t: number) => Vec3 };
function film(s: Script, shots: Shot[]) {
  for (const shot of shots) {
    for (let frame = shot.from; frame < shot.to; frame++) {
      const t = (frame - shot.from) / Math.max(1, shot.to - shot.from);
      const [eye, look] = shaken(frame, shot.eye(frame, t), shot.look(frame, t));
      s.frame = frame;
      s.cut(eye, look);
    }
  }
}

/// The eye over a fighter's shoulder: so far behind it from what it faces, to one side
/// (right if positive), and so high over the ground there, as third-person games frame a big
/// enemy.
const shoulder = (who: Vec3, toward: Vec3, back: number, side: number, high: number): Vec3 => {
  const w = way(toward, who);
  return over(add(who, add(scale(w, back), scale(right(way(who, toward)), side))), high);
};
/// The eye side on to a line from a to b: off its middle (`along` from a to b) by `off` to
/// the right of a-to-b (negative to the left), so high over the ground.
const side = (a: Vec3, b: Vec3, along: number, off: number, high: number): Vec3 => over(add(mix(a, b, along), scale(right(way(a, b)), off)), high);

function script(s: Script) {
  // The places of the fight, east and south of the arena's middle (the water to the north).
  const M0 = p([0, 0]);
  const troll0 = p([-12.5, 11.5]);
  const T1 = p([-6.4, 2.6]);
  const man = new Fighter("man", M0, MAN_CLIPS, 1);
  const troll = new Fighter("troll", troll0, TROLL_CLIPS, TROLL.rate);
  // Where the second smash lands, in the shallows, and where the man stands at the water's
  // edge in front of it to take it.
  const splash = p(SHALLOWS);
  const edge = p(WATERLINE);

  // Setting up, not filmed: the fighters stood, the camera round the arena while the land
  // streams in round it.
  s.frame = 30;
  s.call("staged.spawn", { name: "man", model: MAN.model, template: MAN.template, clips: MAN.clips, wears: MAN.wears, hold: MAN.hold, bend: MAN.bend, at: M0, facing: T1, turn_deg_s: 260, base: true });
  s.call("staged.spawn", { name: "troll", model: TROLL.model, bend: TROLL_BEND, at: troll0, facing: M0, turn_deg_s: 90, base: true });
  man.play(s, 40, "idle");
  troll.play(s, 40, "idle");
  s.frame = 60;
  s.orbit(p([-3, 2]), { radius: 14, height: 4, from: 0, to: 360, frames: 600, look: 1.5 });
  s.frame = 700;
  s.cut(p([4, -1.5], 1.5), p([-8, 6], 2));

  // 1. Out of the trees: the troll lumbers out at the man and roars; he raises his shield.
  const walkIn = at(-3.4);
  troll.play(s, walkIn, "walk");
  troll.move(s, walkIn, T1, 2.2, man);
  const arrived = troll.arrives();
  troll.play(s, arrived - 8, "roar", { once: true, speed: 1.15, blend: 260 });
  man.face(s, at(0), troll);
  man.play(s, at(1.0), "guard", { blend: 350 });
  s.mark("out", "The troll comes out of the trees and roars; the man raises his shield", at(0), arrived + S(2.6));

  // 2. It comes on before the roar has died; he strafes to his left to meet it, it turns to
  // keep him in front.
  const comes = arrived + S(2.5);
  const M1 = p([-0.8, 3.0]);
  const T2 = p([-4.3, 3.4]);
  troll.play(s, comes, "walk", { blend: 350 });
  troll.move(s, comes, T2, 1.2, man);
  man.play(s, comes - 10, "left", { blend: 300 });
  man.move(s, comes - 10, M1, 1.4, troll);
  man.play(s, man.arrives(), "guard", { blend: 250 });

  // 3. The overhead smash from its stride: the wind-up held at the top, the strike fast, a
  // hit-stop in the gravel where he stood in a burst of dust and stones; he rolls aside.
  const smash = troll.arrives() - 12;
  troll.face(s, smash, M1);
  const smashes = troll.perform(s, smash, "smash", { blend: 220, hold: 0.42, strike: 1.45, stop: STOP.smash });
  const smashLands = smashes(TROLL_CLIPS.smash.hit!);
  // Bent over the blow, it is slow to come up: the window he cuts in.
  troll.setRate(s, smashLands + STOP.smash, 0.55);
  const rolled = p([-1.0, 0.4]);
  man.play(s, smashLands - 26, "roll", { once: true, blend: 100, speed: 1.15 });
  man.move(s, smashLands - 26, rolled, 3.0, M1);
  const blowAt = add(T2, scale(way(T2, M1), REACH));
  impact(s, smashLands, blowAt, 2.4, 0.32);
  s.mark("smash", "The smash: he rolls aside and it lands in the gravel", comes, smashLands + 20);

  // 4. Out of the roll he steps straight back in and cuts twice at its leg while it is still
  // bent over the blow; the second staggers it.
  const stepIn = smashLands + 38;
  const M3 = add(T2, scale(way(T2, rolled), 1.75));
  man.play(s, stepIn, "step", { blend: 180 });
  man.move(s, stepIn, M3, 3.2, troll);
  const cut1 = man.arrives() - 6;
  const first = man.perform(s, cut1, "slash", { speed: 1.2, blend: 120, strike: 1.4, stop: STOP.light });
  const hit1 = first(MAN_CLIPS.slash.hit!);
  troll.stop(s, hit1, STOP.light);
  troll.setRate(s, hit1 + STOP.light, 0.55);
  landings.push({ frame: hit1, amount: 0.05 });
  const cut2 = first(MAN_CLIPS.slash.hit! + 0.18);
  const second = man.perform(s, cut2, "backhand", { speed: 1.2, blend: 140, strike: 1.4, stop: STOP.light });
  const hit2 = second(MAN_CLIPS.backhand.hit!);
  troll.setRate(s, hit2, 1);
  troll.perform(s, hit2, "hit", { speed: 1.1, blend: 90 });
  troll.stop(s, hit2, STOP.light);
  landings.push({ frame: hit2, amount: 0.06 });
  const T3 = add(T2, scale(way(M3, T2), 0.8));
  troll.move(s, hit2 + STOP.light, T3, 1.3, man);
  man.play(s, second(MAN_CLIPS.backhand.done!), "guard", { blend: 250 });
  s.mark("cuts", "He cuts twice at its leg while it is bent over; it staggers", stepIn, hit2 + 30);

  // 5. Out of its stagger it sweeps at him: he takes it on his shield, both held on the blow,
  // and is driven back a stride toward the water.
  const sweep = hit2 + S(0.75);
  troll.face(s, sweep, man);
  const sweeps = troll.perform(s, sweep, "sweep", { blend: 200, speed: 1.05, hold: 0.32, strike: 1.5, stop: STOP.heavy });
  const sweepLands = sweeps(TROLL_CLIPS.sweep.hit!);
  const braces = sweepLands - Math.round(((MAN_CLIPS.block.hit! - MAN_CLIPS.block.from!) / 1.0) * 60);
  man.perform(s, braces, "block", { blend: 120 });
  man.stop(s, sweepLands, STOP.heavy);
  const driven = add(M3, scale(way(T3, M3), 1.6));
  man.move(s, sweepLands + STOP.heavy, driven, 3.6, troll);
  impact(s, sweepLands + STOP.heavy + 4, add(M3, [0, 0.1, 0]), 0.6, 0.16);
  s.mark("sweep", "The sweep drives him back on his shield", sweep, sweepLands + 30);

  // 6. It presses him to the water's edge, and smashes again from its stride: he sidesteps
  // along the shore and it lands in the shallows where he stood, in a splash.
  const intoWater = way(edge, splash);
  const T4 = add(edge, scale(intoWater, -1.6));
  const presses = sweeps(TROLL_CLIPS.sweep.done!) - 10;
  troll.play(s, presses, "walk", { blend: 300 });
  troll.move(s, presses, T4, 2.0, man);
  man.play(s, sweepLands + STOP.heavy + 30, "right", { blend: 260 });
  man.move(s, sweepLands + STOP.heavy + 30, edge, 1.9, troll);
  man.play(s, man.arrives(), "guard", { blend: 220 });
  const smash2 = troll.arrives() - 14;
  troll.face(s, smash2, splash);
  const smashes2 = troll.perform(s, smash2, "smash", { blend: 220, hold: 0.36, strike: 1.5, stop: STOP.smash });
  const smash2Lands = smashes2(TROLL_CLIPS.smash.hit!);
  troll.setRate(s, smash2Lands + STOP.smash, 0.5);
  const aside = add(edge, scale(right(way(T4, edge)), -2.4));
  man.play(s, smash2Lands - 30, "dodge", { once: true, blend: 100, speed: 1.2 });
  man.move(s, smash2Lands - 30, aside, 3.4, T4);
  impact(s, smash2Lands, splash, 2.6, 0.3);
  s.mark("splash", "A second smash lands in the shallows", presses, smash2Lands + 20);

  // 7. His combination of three while it is bent over the water, each blow starting as the
  // last follows through: it reels back into the shallows.
  const close = add(T4, scale(intoWater, -1.75));
  const combo = smash2Lands + 30;
  man.face(s, combo, troll);
  man.play(s, combo, "step", { blend: 160 });
  man.move(s, combo, close, 3.4, troll);
  let swing = man.arrives() - 6;
  let back = T4;
  let lands = swing;
  let done = swing;
  for (const [k, blow] of (["slash", "backhand", "chop"] as const).entries()) {
    const last = k === 2;
    const plays = man.perform(s, swing, blow, { speed: 1.2, blend: 120, strike: 1.4, stop: last ? STOP.heavy : STOP.light });
    lands = plays(MAN_CLIPS[blow].hit!);
    troll.setRate(s, lands, 1);
    troll.perform(s, lands, "hit", { speed: last ? 0.95 : 1.15, blend: 90 });
    troll.stop(s, lands, last ? STOP.heavy : STOP.light);
    back = add(back, scale(intoWater, last ? 1.3 : 0.8));
    troll.move(s, lands + (last ? STOP.heavy : STOP.light), back, last ? 1.4 : 1.1, man);
    landings.push({ frame: lands, amount: last ? 0.12 : 0.05 });
    if (last) impact(s, lands + 20, add(back, scale(intoWater, -0.4)), 0.9, 0);
    swing = plays(MAN_CLIPS[blow].hit! + 0.2);
    done = plays(MAN_CLIPS[blow].done!);
  }
  man.play(s, done, "guard", { blend: 300 });
  man.move(s, done, add(close, scale(intoWater, 0.5)), 0.8, troll);
  s.mark("reels", "His combination of three; it reels back into the shallows", combo, lands + 40);

  // 8. It roars over him in the shallows, and he stands to it, shield up.
  const roar = lands + S(1.0);
  troll.face(s, roar, man);
  troll.play(s, roar, "roar", { once: true, speed: 0.95, blend: 280 });
  const end = roar + S(4.4);
  s.mark("roar", "It roars over him in the shallows", roar, end);

  // The camera, beat by beat: close, low, moving, cut on the action; looking west along the
  // shore, south to the trees, or north-west over the water, never east into the sun, and
  // never from behind the troll.
  const mid = (f: number, k = 0.5) => mix(man.at(f), troll.at(f), k);
  const head = (f: number, up = 3.3) => add(troll.at(f), [0, up, 0]);
  const roars = arrived - 8;
  film(s, [
    // Low over his right shoulder, the troll lumbering out of the trees beyond him.
    {
      from: at(0),
      to: roars + S(0.5),
      eye: (f, t) => shoulder(man.at(f), troll.at(f), 3.0 - 0.8 * smooth(t), 0.95, 1.45),
      look: (f) => add(mix(man.at(f), troll.at(f), 0.75), [0, 2.0, 0]),
    },
    // Its roar, close and low in front of it, looking up at its face.
    {
      from: roars + S(0.5),
      to: comes,
      eye: (f, t) => over(add(T1, add(scale(way(T1, M0), 5.0 - 0.6 * t), scale(right(way(T1, M0)), -1.6))), 1.1),
      look: () => head(arrived, 2.9),
    },
    // Side on from the water, the two closing, tracked.
    {
      from: comes,
      to: smash + S(0.6),
      eye: (f, t) => side(man.at(f), troll.at(f), 0.5, -(7.2 - 0.8 * t), 1.5),
      look: (f) => add(mid(f), [0, 1.7, 0]),
    },
    // Low over his left shoulder, looking up at it: the wind-up held over him.
    {
      from: smash + S(0.6),
      to: smashLands - 8,
      eye: (f, t) => shoulder(M1, T2, 2.6 + 0.4 * t, -1.0, 0.95),
      look: (f, t) => mix(head(f, 3.2), add(blowAt, [0, 1.4, 0]), smooth(t) * 0.4),
    },
    // Cut on the strike: side on from the water, low, the blow landing and the burst, his roll
    // and his cuts.
    {
      from: smashLands - 8,
      to: hit1 + 6,
      eye: (f, t) => side(blowAt, T2, 0.35, 6.6 + 0.6 * t, 1.0),
      look: (f, t) => add(mix(blowAt, man.at(f), 0.3 * smooth(t)), [0, 1.0 + 0.4 * t, 0]),
    },
    // Close past its leg from the trees' side, looking north-west over the water: the cuts.
    {
      from: hit1 + 6,
      to: sweep + S(0.5),
      eye: (f, t) => side(man.at(f), troll.at(f), 0.45, 4.0 - 0.4 * t, 1.1),
      look: (f) => add(mid(f, 0.5), [0, 1.5, 0]),
    },
    // Over his left shoulder, the sweep coming at him and his shield taking it.
    {
      from: sweep + S(0.5),
      to: sweepLands + S(1.0),
      eye: (f, t) => shoulder(man.at(f), troll.at(f), 2.4 + 0.6 * t, -1.1, 1.3),
      look: (f) => add(mid(f, 0.6), [0, 1.8, 0]),
    },
    // From the water, low over it, side on: it presses him to the edge, the smash into the
    // shallows and the splash, his sidestep.
    {
      from: sweepLands + S(1.0),
      to: combo + 20,
      eye: (f, t) => side(T4, splash, 0.6, 7.0 - 0.8 * t, 1.1),
      look: (f, t) => add(mix(troll.at(f), splash, 0.45), [0, 1.6, 0]),
    },
    // Over his right shoulder, pushing in on the combination, the water behind it.
    {
      from: combo + 20,
      to: lands - 14,
      eye: (f, t) => shoulder(man.at(f), troll.at(f), 2.6 - 0.5 * smooth(t), 0.9, 1.5),
      look: (f) => add(mid(f, 0.7), [0, 1.9, 0]),
    },
    // The chop, side on from the shore, low: it reels back into the water.
    {
      from: lands - 14,
      to: roar + S(0.4),
      eye: (f, t) => side(close, back, 0.5, -(5.2 + 0.5 * t), 1.0),
      look: (f) => add(mid(f, 0.6), [0, 1.7, 0]),
    },
    // Its roar from low behind his shield, looking up at it in the shallows: the hero shot.
    {
      from: roar + S(0.4),
      to: end,
      eye: (f, t) => shoulder(man.at(f), troll.at(f), 1.9 + 0.5 * smooth(t), -0.75, 0.8 + 0.15 * t),
      look: (f, t) => add(mix(man.at(f), troll.at(f), 0.8), [0, 2.6 + 0.3 * t, 0]),
    },
  ]);
  s.frame = end;
  console.error(`troll: the film runs ${((end - ROLLS) / 60).toFixed(1)} s; beats at ${[smashLands, hit1, hit2, sweepLands, smash2Lands, lands, roar].map((f) => ((f - ROLLS) / 60).toFixed(1)).join(", ")} s`);
}

export default {
  about: "a troll comes out of the trees on the mountain lake's south shore at a man with a sword and shield; they fight: a smash into the gravel, his cuts, a sweep on his shield, a smash into the shallows, his combination and its roar",
  sandbox: [],
  rolls: ROLLS,
  script,
};
