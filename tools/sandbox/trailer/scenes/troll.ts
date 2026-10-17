// A troll fights a man on the mountain lake's south shore (docs/troll-fight.md): a troll
// about 4 m tall comes out of the trees at a man with a sword and a round shield on the
// gravel between the forest's edge and the water; they circle; it smashes at him and he rolls
// aside, the blow throwing up dust and stones; he cuts at its leg twice; it sweeps him back
// a stride on his shield; a second smash at the water's edge lands in the shallows in a
// splash as he sidesteps; his combination of three drives it reeling back into the shallows,
// where it roars. Told over the remote (`play::staged`), the camera close and moving on each
// beat, a small shake on each heavy landing, and every shot looking away from the low
// morning sun in the east-north-east: west along the shore, south to the trees, or north-west
// over the water to the peaks.
//
// Places are in the start's own metres, east, up and south of it (the base frame of a scene
// run, which `--at` is measured from), written here from the arena: east and south of its
// middle on the gravel, `ARENA`. Frames are a sixtieth of a second each.

import type { Script, Vec3 } from "../../qa/script.ts";

/// The frame the film rolls on: before it the fighters are stood, and the camera goes round
/// every shot's place so the land is streamed in round them.
const ROLLS = 900;
const at = (seconds: number) => ROLLS + Math.round(seconds * 60);

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

/// What is in yet: the troll (assets/models/characters/troll.glb, which carries the base's
/// clips too), the held looks (held/arming_sword.glb and round_shield.glb), and the fight
/// clips (the troll's Troll_* in its own file, the man's in character/fight.glb). Until the
/// clips are, both play the nearest of the base's.
const TROLL_MODEL = true;
const HELD = true;
const REAL = true;

/// A clip as a fighter plays it: its name in the file, and, for a blow, the moment it lands,
/// seconds into the clip at its own speed. These moments are to be tuned to the real clips.
type Clip = { name: string; hit?: number };
const TROLL_CLIPS: Record<string, Clip> = REAL
  ? {
      idle: { name: "Troll_Idle" },
      walk: { name: "Troll_Walk" },
      smash: { name: "Troll_Smash", hit: 2.4 },
      sweep: { name: "Troll_Sweep", hit: 1.44 },
      hit: { name: "Troll_Hit" },
      roar: { name: "Troll_Roar" },
    }
  : {
      idle: { name: "Idle_Loop" },
      walk: { name: "Walk_Loop" },
      smash: { name: "Sword_Attack", hit: 0.62 },
      sweep: { name: "Sword_Attack", hit: 0.62 },
      hit: { name: "Hit_Chest" },
      roar: { name: "Hit_Head" },
    };
const MAN_CLIPS: Record<string, Clip> = REAL
  ? {
      idle: { name: "Idle_Loop" },
      guard: { name: "Fight_Guard" },
      left: { name: "Fight_Strafe_L" },
      right: { name: "Fight_Strafe_R" },
      slash: { name: "Fight_Slash", hit: 0.4 },
      backhand: { name: "Fight_Backhand", hit: 1.19 },
      chop: { name: "Fight_Chop", hit: 1.4 },
      block: { name: "Fight_Block", hit: 1.1 },
      dodge: { name: "Fight_Dodge" },
      roll: { name: "Roll" },
      step: { name: "Walk_Loop" },
    }
  : {
      idle: { name: "Idle_Loop" },
      guard: { name: "Sword_Idle" },
      left: { name: "Walk_Loop" },
      right: { name: "Walk_Loop" },
      slash: { name: "Sword_Attack", hit: 0.62 },
      backhand: { name: "Sword_Attack", hit: 0.62 },
      chop: { name: "Sword_Attack", hit: 0.62 },
      block: { name: "Hit_Chest", hit: 0.1 },
      dodge: { name: "Roll" },
      roll: { name: "Roll" },
      step: { name: "Walk_Loop" },
    };

/// The troll: its model, how big against it, and how fast it plays a person's clips, slowed
/// for its size by Froude scaling (twice our height moves about 1.5 times slower), which is
/// what makes it read as heavy.
const TROLL = TROLL_MODEL
  ? { model: "models/characters/troll.glb", clips: undefined, scale: 1, rate: REAL ? 0.85 : 0.68 }
  : { model: "models/character/base.glb", clips: undefined, scale: 2.2, rate: 0.68 };
/// The man: the base body as the player's template, dressed, a sword in his right hand and a
/// round shield in his left; his clips from the fight file, on the base's skeleton.
const MAN = {
  model: "models/character/base.glb",
  template: "male",
  clips: REAL ? "models/character/fight.glb" : undefined,
  wears: ["shirt", "trousers", "boots", "leather_coat"],
  hold: HELD
    ?{ "socket.hand.R": "models/held/arming_sword.glb", "socket.hand.L": "models/held/round_shield.glb" }
    : { "socket.hand.R": "models/held/spear.glb" },
};
/// How fast the troll plays its hit reaction, before its rate: the made clip staggers for
/// three seconds, cut short by the next blow.
const HIT = REAL ? 0.9 : 0.6;
/// How far ahead of the troll its blows land, metres: its arm and fist at its size.
const REACH = 3.1;

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

/// A fighter, as the remote is told it, and where it is on each frame, worked out as
/// `play::staged` moves it (straight on the level at its speed), so the camera can frame it.
class Fighter {
  private legs: { frame: number; from: Vec3; to: Vec3; frames: number }[] = [];
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
  play(s: Script, frame: number, clip: string, { once = false, blend = 250, speed = 1 } = {}) {
    s.frame = frame;
    s.call("staged.play", { name: this.name, clip: this.clips[clip].name, speed: speed * this.rate, once, blend_ms: blend });
  }

  /// The frame a blow played from a frame lands on.
  lands(frame: number, clip: string, speed = 1): number {
    return frame + Math.round(((this.clips[clip].hit ?? 0) / (speed * this.rate)) * 60);
  }

  /// Moves it from a frame to a point at a speed, facing a point or another fighter.
  move(s: Script, frame: number, to: Vec3, speed: number, face?: Vec3 | Fighter) {
    const from = this.at(frame);
    this.legs.push({ frame, from, to, frames: Math.max(1, Math.round((level(from, to) / speed) * 60)) });
    s.frame = frame;
    s.call("staged.move", { name: this.name, to, speed, ...facing("face", face), base: true });
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

/// The camera on a frame, shaken by the landings before it: a quick shake along the view's
/// right and up, dying away over half a second, as a heavy blow near the camera jolts it.
function shaken(frame: number, eye: Vec3, look: Vec3): [Vec3, Vec3] {
  let off: Vec3 = [0, 0, 0];
  const ahead = way(eye, look);
  const side = right(ahead);
  for (const { frame: f, amount } of landings) {
    const t = (frame - f) / 60;
    if (t < 0 || t > 0.6) continue;
    const fade = amount * Math.exp(-t * 7);
    off = add(off, add(scale(side, fade * Math.sin(t * 53)), [0, fade * Math.sin(t * 71 + 1.3), 0]));
  }
  return [add(eye, off), add(look, scale(off, 0.6))];
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
/// (right if positive), and so high, as third-person games frame a big enemy.
const shoulder = (who: Vec3, toward: Vec3, back: number, side: number, high: number): Vec3 => {
  const w = way(toward, who);
  return add(add(who, add(scale(w, back), scale(right(way(who, toward)), side))), [0, high, 0]);
};

function script(s: Script) {
  // The places of the fight, east and south of the arena's middle (the water to the north).
  const M0 = p([0, 0]);
  const troll0 = p([-11, 10]);
  const T1 = p([-6.2, 1.8]);
  const man = new Fighter("man", M0, MAN_CLIPS, 1);
  const troll = new Fighter("troll", troll0, TROLL_CLIPS, TROLL.rate);
  // Where the second smash lands, in the shallows, and where the man stands at the water's
  // edge in front of it to take it.
  const splash = p(SHALLOWS);
  const edge = p(WATERLINE);

  // Setting up, not filmed: the fighters stood, the camera round the arena while the land
  // streams in round it.
  s.frame = 30;
  s.call("staged.spawn", { name: "man", model: MAN.model, template: MAN.template, ...(MAN.clips ? { clips: MAN.clips } : {}), wears: MAN.wears, hold: MAN.hold, at: M0, facing: T1, turn_deg_s: 200, base: true });
  s.call("staged.spawn", { name: "troll", model: TROLL.model, ...(TROLL.clips ? { clips: TROLL.clips } : {}), at: troll0, facing: M0, scale: TROLL.scale, turn_deg_s: 80, base: true });
  man.play(s, 40, "idle");
  troll.play(s, 40, "idle");
  s.frame = 60;
  s.orbit(p([-3, 2]), { radius: 14, height: 4, from: 0, to: 360, frames: 600, look: 1.5 });
  s.frame = 700;
  s.cut(p([4, -1.5], 1.5), p([-8, 6], 2));

  // 1. Out of the trees: the troll lumbers out at the man and roars; he raises his shield.
  const walkIn = at(REAL ? -2.2 : -1.2);
  troll.play(s, walkIn, "walk");
  troll.move(s, walkIn, T1, 2.1, man);
  const arrived = walkIn + Math.round((level(troll0, T1) / 2.1) * 60);
  troll.play(s, arrived, "roar", { once: true, speed: REAL ? 1.2 : 0.45, blend: 300 });
  man.face(s, at(0), troll);
  man.play(s, at(1.6), "guard", { blend: 400 });
  troll.play(s, arrived + (REAL ? 190 : 110), "idle", { blend: 500 });
  s.mark("out", "The troll comes out of the trees and roars; the man raises his shield", at(0), at(5.2));

  // 2. They circle: he strafes to his left, it turns to keep him in front, edging in.
  const M1 = p([-0.6, 3.2]);
  const T2 = p([-3.7, 3.6]);
  man.play(s, at(5.2), "left", { blend: 300 });
  man.move(s, at(5.2), M1, 1.3, troll);
  troll.play(s, at(5.5), "walk", { blend: 400 });
  troll.move(s, at(5.5), T2, 0.9, man);
  const circled = at(5.2) + Math.round((level(M0, M1) / 1.3) * 60);
  man.play(s, circled, "guard", { blend: 300 });
  troll.play(s, at(5.5) + Math.round((level(T1, T2) / 0.9) * 60), "idle", { blend: 400 });
  s.mark("circle", "They circle", at(5.2), at(8));

  // 3. The overhead smash: a long wind-up, he rolls aside to his right, it lands in the gravel
  // where he stood in a burst of dust and stones.
  const smash = at(8.0);
  const smashLands = troll.lands(smash, "smash");
  troll.face(s, smash, M1);
  troll.play(s, smash, "smash", { once: true, blend: 200 });
  const rolled = p([-1.2, 0.6]);
  man.play(s, smashLands - 30, "roll", { once: true, blend: 120 });
  man.move(s, smashLands - 30, rolled, 2.4, M1);
  impact(s, smashLands, add(T2, scale(way(T2, M1), REACH)), 1.0, 0.09);
  man.face(s, smashLands + 40, troll);
  man.play(s, smashLands + 55, "guard", { blend: 350 });
  troll.play(s, smashLands + 70, "idle", { blend: 500 });
  troll.face(s, smashLands + 70, man);
  s.mark("smash", "The smash: he rolls aside and it lands in the gravel", at(8), at(11));

  // 4. He steps in and cuts twice at its leg; it staggers.
  // Each beat from here starts off the last one's blow, so the clips' own timing holds.
  const stepIn = smashLands + 20;
  const T3 = add(T2, scale(way(rolled, T2), 0.7));
  const M3 = add(T2, scale(way(T2, rolled), 1.9));
  man.play(s, stepIn, "step", { blend: 200 });
  man.move(s, stepIn, M3, 2.6, troll);
  const cut1 = stepIn + Math.round((level(rolled, M3) / 2.6) * 60);
  man.play(s, cut1, "slash", { once: true, blend: 150, speed: 1.15 });
  const hit1 = man.lands(cut1, "slash", 1.15);
  troll.play(s, hit1, "hit", { once: true, blend: 100, speed: HIT });
  const cut2 = hit1 + 12;
  man.play(s, cut2, "backhand", { once: true, blend: 150, speed: 1.15 });
  const hit2 = man.lands(cut2, "backhand", 1.15);
  troll.play(s, hit2, "hit", { once: true, blend: 100, speed: HIT });
  troll.move(s, hit2, T3, 0.9, man);
  man.play(s, hit2 + 40, "guard", { blend: 300 });
  troll.play(s, hit2 + 60, "idle", { blend: 400 });
  s.mark("cuts", "He cuts twice at its leg; it staggers", stepIn, hit2 + 40);

  // 5. The sweep: he takes it on his shield and is driven back a stride, toward the water.
  const sweep = hit2 + 40;
  const sweepLands = troll.lands(sweep, "sweep");
  troll.play(s, sweep, "sweep", { once: true, blend: 200 });
  man.play(s, sweepLands - 20, "block", { once: true, blend: 120 });
  const driven = add(M3, scale(way(T3, M3), 1.4));
  man.move(s, sweepLands, driven, 3.2, troll);
  landings.push({ frame: sweepLands, amount: 0.04 });
  man.play(s, sweepLands + 45, "guard", { blend: 350 });
  // Each gives ground to the water's edge: he backs to it, it follows.
  // It stops a little landward of him, on the line from him into the lake, so its smash
  // reaches past him into the shallows and his combination drives it back into the water.
  const intoWater = way(edge, splash);
  const T4 = add(edge, scale(intoWater, -1.0));
  const backs = sweepLands + 50;
  man.play(s, backs, "right", { blend: 300 });
  man.move(s, backs, edge, 1.6, troll);
  troll.play(s, sweepLands + 30, "walk", { blend: 300 });
  troll.move(s, sweepLands + 30, T4, 2.2, man);
  const followed = sweepLands + 30 + Math.round((level(T3, T4) / 2.2) * 60);
  man.play(s, backs + Math.round((level(driven, edge) / 1.6) * 60), "guard", { blend: 250 });
  s.mark("sweep", "The sweep drives him back on his shield", sweep, followed);

  // 6. A second smash at the water's edge: he sidesteps along the shore and it lands in the
  // shallows behind where he stood, in a splash.
  const smash2 = Math.max(followed, backs + Math.round((level(driven, edge) / 1.6) * 60)) + 6;
  const smash2Lands = troll.lands(smash2, "smash");
  troll.face(s, smash2, splash);
  troll.play(s, smash2, "smash", { once: true, blend: 200 });
  const aside = add(edge, scale(right(way(T4, edge)), -2.4));
  man.play(s, smash2Lands - 26, "dodge", { once: true, blend: 120 });
  man.move(s, smash2Lands - 26, aside, 2.6, T4);
  impact(s, smash2Lands, splash, 1.1, 0.08);
  man.face(s, smash2Lands + 35, troll);
  man.play(s, smash2Lands + 50, "guard", { blend: 300 });
  troll.face(s, smash2Lands + 60, man);
  troll.play(s, smash2Lands + 60, "idle", { blend: 400 });
  const combo = smash2Lands + 40;
  s.mark("splash", "A second smash lands in the shallows", smash2, combo);

  // 7. His combination of three: it reels back into the shallows and roars.
  // He comes round to its landward side, so each blow drives it a metre into the lake.
  const close = add(T4, scale(intoWater, -1.9));
  man.play(s, combo, "step", { blend: 200 });
  man.move(s, combo, close, 2.4, troll);
  let swing = combo + Math.round((level(aside, close) / 2.4) * 60);
  let back = T4;
  const deeper = intoWater;
  let lands = swing;
  for (const [k, blow] of ["slash", "backhand", "chop"].entries()) {
    man.play(s, swing, blow, { once: true, blend: 140, speed: 1.15 });
    lands = man.lands(swing, blow, 1.15);
    troll.play(s, lands, "hit", { once: true, blend: 100, speed: HIT });
    back = add(back, scale(deeper, 1.0));
    troll.move(s, lands, back, 1.1, man);
    if (k === 2) landings.push({ frame: lands, amount: 0.03 });
    swing = lands + 12;
  }
  man.move(s, lands + 30, add(close, scale(deeper, 0.6)), 1.0, troll);
  man.play(s, lands + 30, "guard", { blend: 300 });
  const roar = lands + 50;
  troll.play(s, roar, "roar", { once: true, speed: REAL ? 0.9 : 0.4, blend: 300 });
  const end = roar + 150;
  s.mark("reels", "His combination of three; it reels back into the shallows and roars", combo, end);

  // The camera, beat by beat: close, moving, readable; looking west along the shore, south to
  // the trees, or north-west over the water, never east into the sun.
  film(s, [
    // Over his right shoulder, the troll coming out of the trees beyond him; pushing in.
    {
      from: at(0),
      to: at(5.2),
      eye: (f, t) => shoulder(man.at(f), T1, 3.6 - 1.2 * smooth(t), 1.0, 1.75 - 0.2 * t),
      look: (f, t) => mix(add(troll.at(f), [0, 2.4, 0]), add(mix(man.at(f), troll.at(f), 0.6), [0, 1.8, 0]), smooth(t) * 0.5),
    },
    // Side on from the water, the two circling, tracked.
    {
      from: at(5.2),
      to: smash,
      eye: (f, t) => add(mix(man.at(f), troll.at(f), 0.5), [2.2 - 1.6 * t, 1.5, -6.8]),
      look: (f) => add(mix(man.at(f), troll.at(f), 0.5), [0, 1.6, 0]),
    },
    // Low over his left shoulder, looking up at it: the wind-up, the roll, the blow.
    {
      from: smash,
      to: stepIn,
      eye: (f, t) => shoulder(M1, T2, 2.6 + 0.6 * t, -0.9, 1.05),
      look: (f, t) => mix(add(T2, [0, 2.6, 0]), add(add(T2, scale(way(T2, M1), REACH)), [0, 0.6, 0]), smooth((f - (smashLands - 25)) / 30) * 0.6),
    },
    // Close and side on to his cuts, from the east.
    {
      from: stepIn,
      to: sweep,
      eye: (f, t) => add(mix(man.at(f), troll.at(f), 0.4), add(scale(right(way(man.at(f), troll.at(f))), -4.2), [1.2 - 0.8 * t, 1.25, 0])),
      look: (f) => add(mix(man.at(f), troll.at(f), 0.45), [0, 1.3, 0]),
    },
    // Behind him as the sweep drives him back toward the camera.
    {
      from: sweep,
      to: smash2,
      eye: (f, t) => shoulder(driven, T3, 2.4 + 0.5 * t, 0.9, 1.6),
      look: (f) => add(mix(man.at(f), troll.at(f), 0.65), [0, 1.9, 0]),
    },
    // From the shore, low, over the water's edge to the peaks: the smash into the shallows.
    {
      from: smash2,
      to: combo,
      eye: (f, t) => add(splash, [4.6 - 0.8 * t, 1.25, 5.2 - 0.6 * t]),
      look: (f, t) => add(mix(splash, troll.at(f), 0.4), [0, 1.3 + 0.2 * t, 0]),
    },
    // Over his shoulder, pushing in on the combination, the water behind it.
    {
      from: combo,
      to: roar,
      eye: (f, t) => shoulder(man.at(f), troll.at(f), 2.9 - 0.7 * smooth(t), 0.85, 1.65),
      look: (f) => add(mix(man.at(f), troll.at(f), 0.7), [0, 2.0, 0]),
    },
    // Wide and low from the shore: the troll roaring in the shallows, the man before it.
    {
      from: roar,
      to: end,
      eye: (f, t) => add(man.at(roar), add(scale(way(troll.at(roar), man.at(roar)), 4.2 - 0.8 * t), [1.6, 1.1 + 0.3 * t, 0])),
      look: (f) => add(mix(man.at(f), troll.at(f), 0.75), [0, 2.4, 0]),
    },
  ]);
  s.frame = end;
  console.error(`troll: the film runs ${((end - ROLLS) / 60).toFixed(1)} s`);
}

export default {
  about: "a troll comes out of the trees on the mountain lake's south shore at a man with a sword and shield; they fight: a smash into the gravel, his cuts, a sweep on his shield, a smash into the shallows, his combination and its roar",
  sandbox: [],
  rolls: ROLLS,
  script,
};
