// The hands close on what they hold (D562), the twin of terra_walker::grip: whenever a hand
// socket holds a look, its fingers take a grip pose over whatever clip plays, a finger-only layer
// as Lyra and ALS lay a hand pose per weapon grip on the finger chains. The pose is solved, not
// keyed: each finger joint, knuckle first, closes from the rest pose about the handle's line
// until its bone, padded by a finger's half-thickness, touches the handle's surface, as
// GraspIt!'s auto-grasp closes each link until contact (Miller and Allen, "GraspIt!", IEEE
// Robotics & Automation Magazine 2004). The handle is read from the look: the section of its
// vertices across the grip (y = 0), its middle and its radius, tipped as the look is carried.
// A power grip's look is seated with its handle on the palm (`seatOf`); the fingers bend about
// the hand's own flexion axis however a tipped handle crosses them; the opposed thumb comes round
// the handle, its metacarpal swung no more than it must. Grip types by mount: in hand a power grip (fingers and thumb round the handle); hung a hook
// grip (the fingers alone, the thumb left to the clip); strapped none (the shield's arm is
// terra_walker::strap's). Shared by the move check (moves.ts). Node 22.18 or later; no
// dependencies.

import { type Glb, type Pose, type Q, type V3, Glb as GlbFile, apply, dot, len, place, qinv, qmul, rot, sub } from "./glb.ts";

/** A finger's half-thickness, metres: how far its bone runs off the handle when its pad is on it. */
export const FINGER_M = 0.008;
/** How far, degrees, a joint may open past its rest (negative) and close (positive): a finger's
 * knuckle, middle and end joints; the thumb's base, middle and end. */
const OPEN = -40;
// The knuckle's own reach and the ring's and little finger's palm bones cupping the fist (the
// skeleton has no palm bones; their 10 to 20 degrees go to the knuckle).
const FINGER_MOST = [115, 105, 80];
/** The thumb, opposed square to the handle, opens further first: it may rest pointing anywhere
 * round it. Its base is not turned. */
const THUMB_OPEN = -150;
const THUMB_MOST = [0, 150, 80];
/** The chains a grip closes, by the bones' stems; each runs to its `_04_leaf`. */
export const CHAINS = ["thumb", "index", "middle", "ring", "pinky"] as const;
/** The bars: a finger's bone this far off its place on the handle's surface, either way. */
export const FIST_CM = 1;

/** The handle a fist closes on, in the look's frame: a point on its line, the way it runs and
 * its radius, metres. */
export type Handle = { at: V3; way: V3; r: number };
export type Grip = "power" | "hook";

const unit = (v: V3): V3 => v.map((x) => x / len(v)) as V3;
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const axisAngle = (a: V3, deg: number): Q => {
  const h = (deg * Math.PI) / 360;
  const s = Math.sin(h);
  return [a[0] * s, a[1] * s, a[2] * s, Math.cos(h)];
};

/** The grip a look's file asks for, from its mount (its extras): none for a strapped look. */
export function gripOf(look: Glb): Grip | undefined {
  const node = look.gltf.nodes.findIndex((n) => n.mesh !== undefined);
  const extras = (look.gltf.nodes[node] as { extras?: { hold?: string; strap?: unknown } }).extras ?? {};
  if (extras.strap !== undefined) return undefined;
  return extras.hold === "hung" ? "hook" : "power";
}

/** How far from the handle's line a point of its section may be, metres: more is a head or a
 * basket's body. */
const NEAR_M = 0.05;

/** Where a mesh's triangles cross the plane y = 0 (the grip's): each edge that crosses it, cut. */
export function cut(points: V3[], tris: ArrayLike<number>): V3[] {
  const out: V3[] = [];
  for (let i = 0; i + 2 < tris.length; i += 3) {
    for (const [a, b] of [[0, 1], [1, 2], [2, 0]]) {
      const [p, q] = [points[tris[i + a]], points[tris[i + b]]];
      if (p[1] < 0 === q[1] < 0) continue;
      const k = p[1] / (p[1] - q[1]);
      out.push([p[0] + (q[0] - p[0]) * k, 0, p[2] + (q[2] - p[2]) * k]);
    }
  }
  return out;
}

/** The handle across a look's grip from its section at y = 0 (`cut`), the points within
 * `NEAR_M` of the +Y axis: their middle across (of their bounds) and their median distance from
 * it (terra_walker::grip::handle). */
export function handleIn(section: V3[]): Handle | undefined {
  const near = section.filter((q) => Math.hypot(q[0], q[2]) < NEAR_M);
  if (near.length < 3) return undefined;
  const lo = [Math.min(...near.map((q) => q[0])), Math.min(...near.map((q) => q[2]))];
  const hi = [Math.max(...near.map((q) => q[0])), Math.max(...near.map((q) => q[2]))];
  const [x, z] = [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2];
  const radii = near.map((q) => Math.hypot(q[0] - x, q[2] - z)).sort((a, b) => a - b);
  return { at: [x, 0, z], way: [0, 1, 0], r: radii[Math.floor(radii.length / 2)] };
}

/** A look's handle, from every primitive of its mesh. */
export function handleOf(look: Glb): Handle | undefined {
  const node = look.gltf.nodes.findIndex((n) => n.mesh !== undefined);
  if (node < 0) return undefined;
  const pose = look.rest(node);
  const section: V3[] = [];
  for (const prim of look.gltf.meshes![look.gltf.nodes[node].mesh!].primitives) {
    const p = look.read(prim.attributes.POSITION);
    const points: V3[] = [];
    for (let i = 0; i < p.length; i += 3) points.push(place(pose, [p[i], p[i + 1], p[i + 2]]));
    const tris = prim.indices !== undefined ? look.read(prim.indices) : points.map((_, i) => i);
    section.push(...cut(points, tris));
  }
  return handleIn(section);
}

/** A look's handle and grip, read from its file. */
export function lookGrip(file: string): { grip: Grip; handle: Handle } | undefined {
  const look = new GlbFile(file);
  const grip = gripOf(look);
  const handle = grip && handleOf(look);
  return grip && handle ? { grip, handle } : undefined;
}

/** Where the palm's skin lies over the handle's line, metres up the hand socket's +X (out of the
 * back of the hand), measured on the base body's skin at rest (terra_walker::grip::PALM_M). */
export const PALM_M = 0.021;

/** A power grip's look seated against the palm: the move, in the socket's frame before any tilt,
 * that brings its handle's line to lie on the palm (`PALM_M` less its radius up +X, on the
 * socket's Y). The line then runs through the socket's X axis, about which a carried look is
 * tipped, so the seat holds tipped (terra_walker::grip::seat). */
export function seatOf(h: Handle): V3 {
  return [PALM_M - h.r - h.at[0], -h.at[1], -h.at[2]];
}

/** The handle as its look is seated (`seatOf`), in the socket's frame. */
export function seated(h: Handle): Handle {
  return { at: [PALM_M - h.r, 0, 0], way: h.way, r: h.r };
}

/** The handle's line in a frame where the socket sits at `socket`. */
function inFrame(h: Handle, socket: Pose): { c: V3; d: V3; r: number } {
  return { c: place(socket, h.at), d: unit(rot(socket.r, h.way)), r: h.r * socket.s[0] };
}

/** How far a segment comes to a line (point `c`, unit way `d`), from `from` (a share of it) on. */
function toLine(a: V3, b: V3, c: V3, d: V3, from = 0): number {
  let most = Infinity;
  for (let i = Math.round(from * 8); i <= 8; i++) {
    const p = a.map((x, j) => x + ((b[j] - x) * i) / 8) as V3;
    const v = sub(p, c);
    const along = dot(v, d);
    most = Math.min(most, len(sub(v, d.map((x) => x * along) as V3)));
  }
  return most;
}

/** The bones a chain closes, the last a leaf it reaches to, for side "r" or "l". */
export const chainBones = (stem: string, side: string) => [1, 2, 3].map((k) => `${stem}_0${k}_${side}`).concat(`${stem}_04_leaf_${side}`);

/** The grip pose of a hand (terra_walker::grip::close): each chain's three joints' rotations
 * against their parents, closed from `locals` (every chain bone's rest pose against its parent,
 * by name, the hand's own frame for the first) round the handle on a socket at `socket` in the
 * hand's frame. A hook grip leaves the thumb. */
export function close(locals: Map<string, Pose>, socket: Pose, handle: Handle, grip: Grip, side: string): Map<string, Q> {
  const line = inFrame(handle, socket);
  const reach = line.r + FINGER_M * socket.s[0];
  const none: Pose = { t: [0, 0, 0], r: [0, 0, 0, 1], s: [1, 1, 1] };
  // Which way about the handle's line a finger closes: on round it the way the index's
  // knuckle bone points at rest. The thumb comes round the other way, to meet the fingers.
  const knuckle = apply(none, locals.get(`index_01_${side}`)!);
  const ahead = rot(knuckle.r, locals.get(`index_02_${side}`)!.t);
  const sense = Math.sign(dot(cross(line.d, sub(knuckle.t, line.c)), ahead)) || 1;
  // The fingers bend about the hand's own flexion axis, the way an untilted handle runs (the
  // socket's +Y), however a tilted one crosses them; the opposed thumb comes round the handle.
  const flex = unit(rot(socket.r, [0, 1, 0]));
  const out = new Map<string, Q>();
  for (const stem of CHAINS) {
    if (stem === "thumb" && grip === "hook") continue;
    const bones = chainBones(stem, side);
    if (stem !== "thumb") {
      for (const [b, q] of fold(locals, bones, none, 0, line, reach, flex, sense, FINGER_MOST).turns) out.set(b, q);
      continue;
    }
    // The thumb's base (its metacarpal, in the palm) turns no more than it must for the thumb
    // to come round the handle: as it rests, or, where the handle crosses under it (a look
    // tipped up the palm), swung about the hand's flexion axis to the turn that closes it best.
    const base = locals.get(bones[0])!;
    const m = unit(rot(base.r, locals.get(bones[1])!.t));
    const swing = unit(sub(flex, m.map((x) => x * dot(flex, m)) as V3));
    const across = unit(cross(m, swing));
    const turns: number[] = [];
    for (let i = 0; i <= (2 * THUMB_SWING) / 10; i++) turns.push((i % 2 ? 1 : -1) * Math.ceil(i / 2) * 10);
    let best: { turns: Map<string, Q>; worst: number; r: Q; cost: number } | undefined;
    for (const a of turns)
      for (const b of turns) {
        const r = qmul(axisAngle(across, b), qmul(axisAngle(swing, a), base.r));
        const tried = fold(locals, bones, { ...base, r }, 1, line, reach, line.d, -sense, THUMB_MOST);
        const cost = Math.abs(a) + Math.abs(b);
        if (!best || tried.worst < best.worst - 0.002 || (tried.worst < best.worst + 0.002 && cost < best.cost)) best = { ...tried, r, cost };
      }
    out.set(bones[0], best!.r);
    for (const [b, q] of best!.turns) out.set(b, q);
  }
  return out;
}

/** How far, degrees, the thumb's metacarpal may swing either way to come round a handle. */
const THUMB_SWING = 50;

/** One chain closed round the handle (`line`, `reach`) from joint `first` on: a finger's from its
 * knuckle (`parent` the hand), the thumb's from its second joint (`parent` its metacarpal, put).
 * Each joint closes about `axis` square to its bone, the way `sense` says. Its joints' turns
 * against their parents, and its bones' worst gap to the handle either way. */
function fold(locals: Map<string, Pose>, bones: string[], parent: Pose, first: number, line: { c: V3; d: V3 }, reach: number, axis: V3, sense: number, most: number[]): { turns: Map<string, Q>; worst: number } {
  const turns = new Map<string, Q>();
  let worst = 0;
  for (let k = first; k < 3; k++) {
    const rest = locals.get(bones[k])!;
    const child = locals.get(bones[k + 1])!.t;
    const atRest = apply(parent, rest);
    let from = atRest.r;
    let u = unit(rot(from, child));
    // The opposed thumb, turned square to the handle to come round it, may start pointing into
    // the handle: it swings clear first.
    const opposed = first === 1 && k === 1;
    if (opposed) {
      const flat = unit(sub(u, line.d.map((x) => x * dot(u, line.d)) as V3));
      from = qmul(between(u, flat), from);
      u = flat;
    }
    const hinge = unit(sub(axis, u.map((x) => x * dot(axis, u)) as V3)).map((x) => x * sense) as V3;
    const turned = (deg: number): Pose => ({ t: atRest.t, r: qmul(axisAngle(hinge, deg), from), s: atRest.s });
    // A joint past the first starts on the handle already: what touches is the rest of its bone.
    // So does a knuckle already in it (a handle thicker than the fist was made for).
    const on = k > first || toLine(atRest.t, atRest.t, line.c, line.d) <= reach;
    const gap = (deg: number) => {
      const p = turned(deg);
      return toLine(p.t, place(p, child), line.c, line.d, on ? 0.3 : 0) - reach;
    };
    const world = turned(closing(gap, opposed ? THUMB_OPEN : OPEN, most[k]));
    turns.set(bones[k], qmul(qinv(parent.r), world.r));
    worst = Math.max(worst, Math.abs(toLine(world.t, place(world, child), line.c, line.d) - reach));
    parent = world;
  }
  return { turns, worst };
}

/** The least turn from `lo` to `most` degrees, in steps of 2 and then halved, at which the bone
 * touches (`gap` at most 0); if it never does, the turn that brings it nearest. One already in
 * at `lo` starts from the turn where it is least in: there, if it is in at every turn
 * (terra_walker::grip::closing). */
function closing(gap: (deg: number) => number, lo: number, most: number): number {
  if (gap(lo) <= 0) {
    for (let d = lo; d <= most; d += 2) if (gap(d) > gap(lo)) lo = d;
    if (gap(lo) <= 0) return lo;
  }
  let near = lo;
  for (let d = lo + 2; d <= most; d += 2) {
    if (gap(d) < gap(near)) near = d;
    if (gap(d) > 0) continue;
    let [a, b] = [d - 2, d];
    for (let i = 0; i < 12; i++) {
      const m = (a + b) / 2;
      if (gap(m) <= 0) b = m;
      else a = m;
    }
    return b;
  }
  return near;
}

/** The shortest turn taking unit `a` to unit `b`. */
function between(a: V3, b: V3): Q {
  const c = cross(a, b);
  const q: Q = [c[0], c[1], c[2], 1 + dot(a, b)];
  const l = Math.hypot(...q);
  return q.map((x) => x / l) as Q;
}

/** Every chain bone's rest pose against its parent on `body`, with a template's joint offsets
 * (`moved`, by node), and the socket's against the hand. */
export function handRest(body: Glb, side: string, moved: Map<number, V3> = new Map()): { locals: Map<string, Pose>; socket: Pose } {
  const local = (name: string): Pose => {
    const i = body.named(name);
    const rest = body.rest(i);
    const by = moved.get(i);
    return by ? { ...rest, t: rest.t.map((x, j) => x + by[j]) as V3 } : rest;
  };
  const locals = new Map<string, Pose>();
  for (const stem of CHAINS) for (const b of chainBones(stem, side)) locals.set(b, local(b));
  return { locals, socket: local(side === "r" ? "socket.hand.R" : "socket.hand.L") };
}

/** How far round the handle's line a finger comes, degrees: the turn about the line from its
 * knuckle to its tip, through its joints (`at`, knuckle first, the tip last), each step the
 * least turn between them. A finger lying alongside the handle, or bent short of it, turns
 * little round it, however near its bones are; one closed round it turns past its far side. */
export function wrapOf(at: V3[], c: V3, d: V3): number {
  const across = (p: V3): V3 => {
    const v = sub(p, c);
    return sub(v, d.map((x) => x * dot(v, d)) as V3);
  };
  let turn = 0;
  for (let k = 1; k < at.length; k++) {
    const [a, b] = [across(at[k - 1]), across(at[k])];
    turn += Math.atan2(dot(cross(a, b), d), dot(a, b));
  }
  return Math.abs((turn * 180) / Math.PI);
}

/** How far a hand's fingers are from their place on the handle (each bone padded by a finger's
 * half-thickness against the handle's surface), cm, the worst either way and where: on `pose`
 * (the body's nodes in one space), the look's handle on the socket node `socket`. The thumb's
 * base (in the palm) is not counted; a hook grip counts no thumb. Near is not closed: a finger
 * lying along a handle run slantwise across the palm is as near it as one round it, so `wrap` is
 * the least a finger comes round the handle (`wrapOf`), the bar `WRAP_DEG`, and `short` that
 * finger. */
export function fistOff(body: Glb, pose: Pose[], side: string, socket: number, handle: Handle, grip: Grip): { cm: number; where: string; wrap: number; short: string } {
  const S = pose[socket];
  const line = inFrame(handle, S);
  const pad = FINGER_M * S.s[0];
  let worst = { cm: 0, where: "", wrap: Infinity, short: "" };
  for (const stem of CHAINS) {
    if (stem === "thumb" && grip === "hook") continue;
    const bones = chainBones(stem, side);
    for (let k = stem === "thumb" ? 1 : 0; k < 3; k++) {
      const off = (toLine(pose[body.named(bones[k])].t, pose[body.named(bones[k + 1])].t, line.c, line.d) - line.r - pad) * 100;
      if (Math.abs(off) > Math.abs(worst.cm)) worst = { ...worst, cm: off, where: bones[k] };
    }
    if (stem === "thumb") continue;
    const wrap = wrapOf(bones.map((b) => pose[body.named(b)].t), line.c, line.d);
    if (wrap < worst.wrap) worst = { ...worst, wrap, short: bones[0] };
  }
  return worst;
}

/** The least a finger closed round a handle comes round it from its knuckle, degrees: near a
 * quarter turn puts its tip past the handle's side, onto the side away from the knuckle. Every
 * look held square in the fist closes 98 to 165 degrees round its handle on the base body (the
 * sword 89 on the thin template); one slid 45 degrees slantwise across the palm (a carried tilt
 * about the palm's middle) left the index 25 round and the ring 46, the fingers bent beside the
 * handle, not round it (the developer's still, 2026-10-28). */
export const WRAP_DEG = 80;
