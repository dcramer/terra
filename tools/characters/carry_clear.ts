// A carried look kept clear of the ground and the body by a constraint solved from the look
// itself: the twin of terra_walker::carry_clear and carry_radii (held-constraint j3), function
// for function, so holds.ts and solve.ts measure what the game runs. Its margins, passes and
// wrist limits are read from the game's source. NOT YET held to the game by fixtures, and not
// yet laid by carry_arm.ts or moves.ts: held-constraint j4 (the handoff lists both).
//
// All in the body's frame, +Y up. `clear` finds how far the hand (the wrist) must move for the
// look's points to stand a margin over the ground and off the legs' and trunk's capsules;
// `reach` lays the arm so (the analytic two-bone IK); `back` turns the wrist back so the look
// keeps its direction; `ease` brings a move in over time. The method and its sources are named
// in the game's file.
//
//   node tools/characters/carry_clear.ts --radii [BODY.glb]   the capsules' radii of a body, cm
//
// Node 22.18 or later; no dependencies.

import { type Glb, type Pose, type Q, type V3, dot, len, qinv, qmul, rot, sub } from "./glb.ts";
import { rustConst } from "./rust_const.ts";
import { type HandAxes, WRIST, bendOf } from "./wrist.ts";

const CLEAR_RS = "crates/terra_walker/src/carry_clear.rs";
/** The switch and the margins, read from the game's source so the tools measure what it runs:
 * whether a hanging look is kept clear at all (carry_arm.rs `CLEAR_ON`, 1 or 0), and the least
 * its points stand over the ground and off a capsule, metres. tune.ts sets them for one run
 * (`clear.on`, `clear.ground`, `clear.body`). */
export const CLEAR = { on: rustConst("crates/terra_walker/src/carry_arm.rs", "CLEAR_ON"), ground: rustConst(CLEAR_RS, "CLEAR_GROUND_M"), body: rustConst(CLEAR_RS, "CLEAR_BODY_M") };
export const PASSES = rustConst(CLEAR_RS, "PASSES");
const NEAR_M = rustConst(CLEAR_RS, "NEAR_M");
/** The share of a part's vertices its capsule's radius holds within it (carry_radii.rs). */
export const WITHIN = rustConst("crates/terra_walker/src/carry_radii.rs", "WITHIN");

const ID: Q = [0, 0, 0, 1];
const add = (a: V3, b: V3): V3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const mul = (a: V3, k: number): V3 => [a[0] * k, a[1] * k, a[2] * k];
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
/** glam's `try_normalize`: none for a vector with no length. */
const unit = (v: V3): V3 | undefined => {
  const l = len(v);
  return l > 0 && Number.isFinite(1 / l) ? mul(v, 1 / l) : undefined;
};
const isZero = (v: V3) => v[0] === 0 && v[1] === 0 && v[2] === 0;
const isId = (q: Q) => q[0] === 0 && q[1] === 0 && q[2] === 0 && q[3] === 1;
const axisAngle = (axis: V3, angle: number): Q => {
  const s = Math.sin(angle / 2);
  return [axis[0] * s, axis[1] * s, axis[2] * s, Math.cos(angle / 2)];
};
const normal = (q: Q): Q => {
  const l = Math.hypot(...q);
  return [q[0] / l, q[1] / l, q[2] / l, q[3] / l];
};
/** glam's `Quat::from_rotation_arc`: the least turn of one unit vector onto another. */
function arc(from: V3, to: V3): Q {
  const d = dot(from, to);
  const one = 1 - 2 * 1.1920929e-7;
  if (d > one) return ID;
  if (d < -one) {
    // Any vector square to `from` (glam's `any_orthonormal_vector`), turned half round.
    const sign = from[2] >= 0 ? 1 : -1;
    const a = -1 / (sign + from[2]);
    return axisAngle([1 + sign * from[0] * from[0] * a, sign * from[0] * from[1] * a, -sign * from[0]], Math.PI);
  }
  const c = cross(from, to);
  return normal([c[0], c[1], c[2], 1 + d]);
}
/** glam's `Quat::slerp` from no turn to `end`, the short way. */
function slerpFromId(end: Q, s: number): Q {
  let d = end[3];
  let e = end;
  if (d < 0) [e, d] = [end.map((x) => -x) as Q, -d];
  if (d > 0.9995) return normal([e[0] * s, e[1] * s, e[2] * s, 1 + (e[3] - 1) * s]);
  const theta = Math.acos(d);
  const [a, b, k] = [Math.sin(theta * (1 - s)), Math.sin(theta * s), 1 / Math.sin(theta)];
  return [e[0] * b * k, e[1] * b * k, e[2] * b * k, (a + e[3] * b) * k];
}

/** An arm's shoulder, elbow and hand (its wrist joint) in the body. */
export type Arm = { shoulder: V3; elbow: V3; hand: V3 };
/** A bone of the body as a capsule: its two ends and its radius. */
export type Capsule = { a: V3; b: V3; radius: number };
/** The hand a look is held in: its bone's rotation in the body as the carry pose lays it, its
 * own axes in the bone's frame (wrist.ts `handAxes`) and its bend at the body's rest. */
export type Hand = { turn: Q; axes: HandAxes; rest: { flex: number; dev: number } };
/** An arm laid again: the swing at the shoulder and the bend at the elbow, turns in the body. */
export type Laid = [Q, Q];

/** The arm laid so its hand is `by` from where it was: the swing at the shoulder and the bend
 * at the elbow. The hand goes as far toward the target as the arm's length lets it. */
export function reach(arm: Arm, by: V3): Laid {
  if (isZero(by)) return [ID, ID];
  const [upper, lower] = [sub(arm.elbow, arm.shoulder), sub(arm.hand, arm.elbow)];
  const [a, b] = [len(upper), len(lower)];
  const target = sub(add(arm.hand, by), arm.shoulder);
  if (a < NEAR_M || b < NEAR_M || len(target) < NEAR_M) return [ID, ID];
  // The elbow's inner angle for a shoulder to hand distance (the law of cosines).
  const inner = (c: number) => Math.acos(Math.max(-1, Math.min(1, (a * a + b * b - c * c) / (2 * a * b))));
  const far = Math.max(Math.abs(a - b) + NEAR_M, Math.min(a + b - NEAR_M, len(target)));
  // A straight arm bends as an arm hanging at the side does, the forearm coming forward.
  const axis = unit(cross(upper, lower)) ?? [-1, 0, 0];
  const elbow = axisAngle(axis, inner(len(sub(arm.hand, arm.shoulder))) - inner(far));
  const [from, to] = [unit(add(upper, rot(elbow, lower))), unit(target)];
  return [from && to ? arc(from, to) : ID, elbow];
}

/** Where a point carried with the forearm (the hand, the look in it) stands after `reach`. */
export function moved(arm: Arm, [swing, elbow]: Laid, point: V3): V3 {
  return add(arm.shoulder, rot(swing, add(sub(arm.elbow, arm.shoulder), rot(elbow, sub(point, arm.elbow)))));
}

/** `reach` as the turns carry_arm writes: the upper arm's in the body, then the forearm's in
 * the body under its parent as it then stands. */
export function turns(arm: Arm, by: V3): Laid {
  const [swing, elbow] = reach(arm, by);
  return [swing, qmul(qmul(swing, elbow), qinv(swing))];
}

/** The turn in the body that takes the hand back from where the arm laid by `laid` turned it:
 * the whole of the forearm's turn undone where the wrist's limits (wrist.ts `WRIST`) allow it,
 * else the most of it, by halving, that keeps the wrist inside them; none for no hand, and none
 * where the pose is past them already. */
export function back(arm: Arm, [swing, elbow]: Laid, hand: Hand | undefined): Q {
  if (!hand) return ID;
  const fore = qmul(swing, elbow);
  if (isId(fore)) return ID;
  const along = rot(fore, sub(arm.hand, arm.elbow));
  const within = (turn: Q) => {
    const b = bendOf([0, 0, 0], along, qmul(qmul(turn, fore), hand.turn), hand.axes);
    const [flex, dev] = [b.flex - hand.rest.flex, b.dev - hand.rest.dev];
    return flex >= -WRIST.ext && flex <= WRIST.flex && dev >= -WRIST.ulnar && dev <= WRIST.radial;
  };
  const whole = qinv(fore);
  if (within(whole)) return whole;
  if (!within(ID)) return ID;
  let [least, most] = [0, 1];
  for (let i = 0; i < PASSES; i++) {
    const mid = (least + most) * 0.5;
    if (within(slerpFromId(whole, mid))) least = mid;
    else most = mid;
  }
  return slerpFromId(whole, least);
}

/** Where a point of the look stands after `reach` with the hand turned `back` at the wrist. */
export function held(arm: Arm, laid: Laid, turn: Q, point: V3): V3 {
  const wrist = moved(arm, laid, arm.hand);
  return add(wrist, rot(turn, sub(moved(arm, laid, point), wrist)));
}

/** The nearest point to `p` on a capsule's axis. */
function onAxis(c: Capsule, p: V3): V3 {
  const along = sub(c.b, c.a);
  const l2 = dot(along, along);
  return l2 > 0 ? add(c.a, mul(along, Math.max(0, Math.min(1, dot(sub(p, c.a), along) / l2)))) : c.a;
}

/** How far the hand must move for the look's `points` (in the body, as the carry pose holds
 * them) to stand `CLEAR.ground` over `ground` and `CLEAR.body` off every capsule, the hand
 * turned back at the wrist as `back` allows: zero, exactly, when they do already. A point in a
 * capsule goes out along its normal when that is toward the side of the capsule the hand is
 * on, else out through that side; `side` is the way out when the hand is on the axis. */
export function clear(arm: Arm, hand: Hand | undefined, points: V3[], ground: number, capsules: Capsule[], side: V3): V3 {
  const most = len(sub(arm.elbow, arm.shoulder)) + len(sub(arm.hand, arm.elbow));
  // The move that mends the deepest breaches with the hand moved `by`; none if there is none.
  const mend = (by: V3): V3 | undefined => {
    const laid = reach(arm, by);
    const turn = back(arm, laid, hand);
    const wrist = moved(arm, laid, arm.hand);
    let [under, deep, out]: [number, number, V3] = [0, 0, [0, 0, 0]];
    for (const point of points) {
      const p = held(arm, laid, turn, point);
      under = Math.max(under, ground + CLEAR.ground - p[1]);
      for (const c of capsules) {
        const off = sub(p, onAxis(c, p));
        const far = c.radius + CLEAR.body;
        if (len(off) >= far) continue;
        const way = unit(sub(wrist, onAxis(c, wrist))) ?? side;
        const toward = dot(off, way);
        // Along `way` from `off` to the capsule's margin: a ray out of a circle.
        const through = -toward + Math.sqrt(Math.max(0, toward * toward - dot(off, off) + far * far));
        const n = unit(off);
        const [over, to]: [number, V3] = n && toward >= 0 ? [far - len(off), n] : [through, way];
        if (over > deep) [deep, out] = [over, mul(to, over)];
      }
    }
    return under > NEAR_M || deep > NEAR_M ? add([0, Math.max(0, under), 0], out) : undefined;
  };
  let [short, by]: [V3, V3] = [[0, 0, 0], [0, 0, 0]];
  for (let pass = 0; pass < PASSES; pass++) {
    const more = mend(by);
    if (!more) {
      if (pass === 0) return [0, 0, 0];
      // A hand lifted by its elbow tips the look up too: the least move that clears lies
      // between the last that did not and this one, found by halving.
      for (let i = 0; i < PASSES; i++) {
        const mid = mul(add(short, by), 0.5);
        if (mend(mid)) short = mid;
        else by = mid;
      }
      return by;
    }
    short = by;
    by = add(by, more);
    if (len(by) > most) by = mul(by, most / len(by));
  }
  return by;
}

/** A hand's move eased in time: where it is and how fast it goes, metres. */
export type Eased = { at: V3; speed: V3 };

/** `eased` after `dt` seconds toward `to`: carry_arm.ts `ease`'s exact critically damped spring,
 * on a vector. */
export function ease(eased: Eased, to: V3, rate: number, dt: number): Eased {
  const away = sub(eased.at, to);
  const j = add(eased.speed, mul(away, rate));
  const decay = Math.exp(-rate * dt);
  return { at: add(to, mul(add(away, mul(j, dt)), decay)), speed: mul(sub(eased.speed, mul(j, rate * dt)), decay) };
}

// The capsules' radii (terra_walker::carry_radii).

/** The capsules, in this order: the left thigh and calf, the right thigh and calf, the trunk;
 * each the joints its axis runs between. */
export const AXES: readonly (readonly [string, string])[] = [["thigh_l", "calf_l"], ["calf_l", "foot_l"], ["thigh_r", "calf_r"], ["calf_r", "foot_r"], ["pelvis", "neck_01"]];

/** Which capsule a bone's skin is part of, by the bone's name; none for any other bone. */
export function part(bone: string): number | undefined {
  const m = bone.match(/^(?:(thigh|calf)(?:_twist_01)?_([lr])|(pelvis)|(spine_0).*)$/);
  if (!m) return undefined;
  return m[1] ? (m[2] === "l" ? 0 : 2) + (m[1] === "thigh" ? 0 : 1) : 4;
}

/** `sorted`'s value `share` of the way up it, as the game indexes it (in f32). */
const shareOf = (sorted: number[], share: number) => sorted[Math.floor(Math.fround((sorted.length - 1) * Math.fround(share)))];

/** A capsule's radius from where its part's vertices stand off its axis (`offAxis`): the
 * distance from their middle (their mean) that `WITHIN` of them are inside; 0 for none. From
 * their middle, not the axis: a limb's joints do not run down its middle (the game's `radius`
 * says by how much). */
export function radius(off: V3[]): number {
  if (!off.length) return 0;
  const middle = mul(off.reduce(add, [0, 0, 0]), 1 / off.length);
  return shareOf(off.map((o) => len(sub(o, middle))).sort((a, b) => a - b), WITHIN);
}

/** Where `p` stands off the line through `a` and `b`, square to it; none past either end and
 * for an axis with no length. */
export function offAxis(p: V3, a: V3, b: V3): V3 | undefined {
  const along = sub(b, a);
  const l2 = dot(along, along);
  if (!(l2 > 0)) return undefined;
  const t = dot(sub(p, a), along) / l2;
  return t >= 0 && t <= 1 ? sub(sub(p, a), mul(along, t)) : undefined;
}

/** One part's vertices as `--radii` explains them: how many are its bones', how many of those
 * lie along its axis, their middle off the axis, and their distances from the axis and from
 * that middle (the half, `WITHIN` and all of them), metres. */
export type Spread = { part: string; skin: number; along: number; middle: V3; axis: [number, number, number]; own: [number, number, number] };

/** The radii of a body's capsules, metres, from every skinned mesh of it at its bind pose: each
 * vertex given to its heaviest joint, a joint's place the inverse of its inverse bind matrix;
 * 0 for a part with no skin or a joint the skin has not. As the game, it reads no morph. */
export function radii(body: Glb, spread?: Spread[]): number[] {
  const off: V3[][] = AXES.map(() => []);
  const counted = AXES.map(() => 0);
  for (const node of body.gltf.nodes as { mesh?: number; skin?: number }[]) {
    if (node.mesh === undefined || node.skin === undefined) continue;
    const skin = body.gltf.skins![node.skin];
    const ibm = body.read(skin.inverseBindMatrices);
    // The translation of a column-major affine matrix's inverse: -(L^-1) t.
    const joint = (name: string): V3 | undefined => {
      const k = skin.joints.findIndex((j: number) => body.gltf.nodes[j].name === name);
      if (k < 0) return undefined;
      const m = ibm.subarray(k * 16, k * 16 + 16);
      const [c0, c1, c2, t]: V3[] = [0, 4, 8, 12].map((i) => [m[i], m[i + 1], m[i + 2]]);
      const det = dot(c0, cross(c1, c2));
      return [-dot(cross(c1, c2), t) / det, -dot(cross(c2, c0), t) / det, -dot(cross(c0, c1), t) / det];
    };
    const ends = AXES.map(([a, b]) => [joint(a), joint(b)] as const);
    const parts = skin.joints.map((j: number) => part(body.gltf.nodes[j].name ?? ""));
    for (const prim of body.gltf.meshes![node.mesh].primitives as { attributes: Record<string, number> }[]) {
      const at = body.read(prim.attributes.POSITION);
      // Each vertex's heaviest joint, over both sets of four where the mesh has eight (D601).
      const sets = [0, 1].filter((n) => prim.attributes[`JOINTS_${n}`] !== undefined).map((n) => [body.read(prim.attributes[`JOINTS_${n}`]), body.read(prim.attributes[`WEIGHTS_${n}`])] as const);
      for (let v = 0; v < at.length / 3; v++) {
        let [weight, j] = [0, 0];
        for (const [joints, weights] of sets) for (let k = 0; k < 4; k++) if (weights[v * 4 + k] > weight) [weight, j] = [weights[v * 4 + k], joints[v * 4 + k]];
        const k = weight > 0 ? parts[j] : undefined;
        if (k === undefined) continue;
        const [a, b] = ends[k];
        const o = a && b ? offAxis([at[v * 3], at[v * 3 + 1], at[v * 3 + 2]], a, b) : undefined;
        counted[k]++;
        if (o) off[k].push(o);
      }
    }
  }
  spread?.push(
    ...off.map((o, k): Spread => {
      const middle: V3 = o.length ? mul(o.reduce(add, [0, 0, 0]), 1 / o.length) : [0, 0, 0];
      const three = (d: number[]): [number, number, number] => (d.sort((x, y) => x - y).length ? [shareOf(d, 0.5), shareOf(d, WITHIN), d[d.length - 1]] : [0, 0, 0]);
      return { part: AXES[k][0], skin: counted[k], along: o.length, middle, axis: three(o.map((p) => len(p))), own: three(o.map((p) => len(sub(p, middle)))) };
    }),
  );
  return off.map(radius);
}

/** The capsules of a body at `pose`: each axis between its joints as posed, with its radius;
 * none for a part with no radius. */
export function capsules(body: Glb, pose: Pose[], radii: number[]): Capsule[] {
  return AXES.flatMap(([a, b], k) => (radii[k] > 0 ? [{ a: pose[body.named(a)].t, b: pose[body.named(b)].t, radius: radii[k] }] : []));
}

if (import.meta.main) {
  const { Glb: Load } = await import("./glb.ts");
  if (process.argv.includes("--radii")) {
    const file = process.argv[process.argv.indexOf("--radii") + 1] ?? "assets/models/character/fight.glb";
    const spread: Spread[] = [];
    const r = radii(new Load(file), spread);
    const cm = (x: number) => (x * 100).toFixed(1);
    console.log(`${file}: ${["thigh_l", "calf_l", "thigh_r", "calf_r", "trunk"].map((n, k) => `${n} ${cm(r[k])} cm`).join(", ")} (the distance from their middle ${WITHIN} of a part's vertices lie within; 0 is no capsule)`);
    // Why each is what it is: a middle far off the axis is a limb whose joints run down one side.
    for (const s of spread)
      console.log(`  ${s.part}: ${s.along} of ${s.skin} vertices along the axis; their middle ${s.middle.map(cm).join(", ")} cm off it; from the middle half ${cm(s.own[0])}, ${WITHIN} ${cm(s.own[1])}, all ${cm(s.own[2])} cm; from the axis ${cm(s.axis[0])}, ${cm(s.axis[1])}, ${cm(s.axis[2])} cm`);
  } else {
    console.error("✗ carry_clear: say --radii [BODY.glb] for the capsules' radii of a body");
    process.exit(2);
  }
}
