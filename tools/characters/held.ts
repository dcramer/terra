// A held look hung on a body as `terra_walker::character` hangs it (D504, docs/attachment.md):
// from the hand socket it is held in, or, strapped (a shield on the forearm), from the bone its
// file's `strap` names. Shared by the move check (moves.ts) and the shield's board place
// (shield_place.ts). Node 22.18 or later; no dependencies.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { type Ball, type Board, board, meshOf, sliced } from "./clash.ts";
import { Glb, type Pose, type Q, type V3, dot, len, place, qinv, qmul, rot, sub } from "./glb.ts";
import { gripOf, handleOf, seatOf } from "./grip.ts";

// A few hundred of the look's vertices in the frame of the node it hangs from, and the blade's
// tip. The sword (in the right hand) also as balls, the shield (in the left) as its board, for
// the clash (clash.ts), in that frame. `front` is the way the look's painted front (its -Z)
// looks; `up` the way its -X looks (a heater's top: its point is +X, art/held/heater_shield.json);
// `grip` where its origin (the place it is held at) sits and `bar` the way its handle (+Y) runs,
// or a strapped look's hand strap (+X).
export type Held = { hand: string; socket: string; name: string; points: V3[]; tip: V3; thin: V3; front: V3; up: V3; centre: V3; grip: V3; bar: V3; balls?: Ball[]; board?: Board; strap?: Strap };
// `at`, in place of `fist`, is the strap of files before D554's round 2 (one place on the bone
// for every body, metres), read so shield_place.ts --at can read an older commit's assets.
export type Strap = { bone: string; fist?: string; at?: V3; face: V3; grip: V3 };

/** The least turn that takes +Y onto `to`: a bone's frame turned to run along its true limb. */
export function along(to: V3): Q {
  const u = to.map((x) => x / len(to)) as V3;
  const w = 1 + u[1];
  if (w < 1e-9) return [1, 0, 0, 0];
  const q: Q = [u[2], 0, -u[0], w];
  const n = Math.hypot(...q);
  return q.map((x) => x / n) as Q;
}

/** The frame a held look's points are in at `pose` (`named` a node's index by name): its hand
 * socket's, or, strapped (terra_walker::strap), its bone's turned to run along the true forearm
 * (elbow to the wrist, the fist's bone's joint) and moved to the fist, so the hand strap is in
 * this body's fist and the grip along its forearm, wherever the clip has the hand. */
export function frameOf(h: Held, pose: Pose[], named: (name: string) => number, parentOf: (i: number) => number | undefined): Pose {
  if (!h.strap?.fist) return pose[named(h.socket)];
  const bone = pose[named(h.strap.bone)];
  const fist = named(h.strap.fist);
  const wrist = pose[parentOf(fist)!].t;
  return { t: pose[fist].t, r: qmul(bone.r, along(rot(qinv(bone.r), sub(wrist, bone.t)))), s: bone.s };
}

// A strap's turn in the true forearm's frame, the look's front (-Z) along `face` and its grip
// (+Y) along `grip`, at the hand socket's scale against the bone's (`ratio`); its origin at the
// frame's, the fist (`frameOf`).
function strapped(strap: Strap, ratio: number): Pose {
  const unit = (v: V3): V3 => v.map((x) => x / len(v)) as V3;
  const y = unit(strap.grip);
  const back = strap.face.map((x) => -x) as V3;
  const z = unit(sub(back, y.map((x) => x * dot(back, y)) as V3));
  const x: V3 = [y[1] * z[2] - y[2] * z[1], y[2] * z[0] - y[0] * z[2], y[0] * z[1] - y[1] * z[0]];
  // The quaternion of the rotation whose columns are x, y and z.
  const [m00, m11, m22] = [x[0], y[1], z[2]];
  const trace = m00 + m11 + m22;
  let r: Q;
  if (trace > 0) {
    const k = 2 * Math.sqrt(1 + trace);
    r = [(y[2] - z[1]) / k, (z[0] - x[2]) / k, (x[1] - y[0]) / k, k / 4];
  } else if (m00 > m11 && m00 > m22) {
    const k = 2 * Math.sqrt(1 + m00 - m11 - m22);
    r = [k / 4, (x[1] + y[0]) / k, (z[0] + x[2]) / k, (y[2] - z[1]) / k];
  } else if (m11 > m22) {
    const k = 2 * Math.sqrt(1 + m11 - m00 - m22);
    r = [(x[1] + y[0]) / k, k / 4, (y[2] + z[1]) / k, (z[0] - x[2]) / k];
  } else {
    const k = 2 * Math.sqrt(1 + m22 - m00 - m11);
    r = [(z[0] + x[2]) / k, (y[2] + z[1]) / k, k / 4, (x[1] - y[0]) / k];
  }
  return { t: strap.at ? (strap.at.map((v) => v * ratio) as V3) : [0, 0, 0], r, s: [ratio, ratio, ratio] };
}

/** The look `file` held in `hand` (a socket's name) on `body` at its rest pose `rest`. */
export function hang(body: Glb, rest: Pose[], hand: string, file: string): Held {
  const g = new Glb(file);
  const node = g.gltf.nodes.findIndex((n) => n.mesh !== undefined);
  const strap = (g.gltf.nodes[node] as { extras?: { strap?: Strap } }).extras?.strap;
  const socket = strap?.bone ?? hand;
  const hung = strap ? strapped(strap, rest[body.named(hand)].s[0] / rest[body.named(strap.bone)].s[0]) : undefined;
  // In hand a power grip's look is seated with its handle on the palm (grip.ts seatOf,
  // terra_walker::grip::seat); the seat lies on the socket's X, so it holds when carried tipped.
  const handle = !strap && gripOf(g) === "power" ? handleOf(g) : undefined;
  const seat: V3 = handle ? seatOf(handle) : [0, 0, 0];
  const put = (p: V3): V3 => (hung ? place(hung, p) : [p[0] + seat[0], p[1] + seat[1], p[2] + seat[2]]);
  const pose = g.rest(node);
  // Every primitive: an axe's head and a spear's are primitives of their own, past the first.
  const all: V3[] = [];
  for (const prim of g.gltf.meshes![g.gltf.nodes[node].mesh!].primitives) {
    const p = g.read(prim.attributes.POSITION);
    for (let i = 0; i < p.length; i += 3) all.push(place(pose, [p[i], p[i + 1], p[i + 2]]));
  }
  const step = Math.max(1, Math.floor(all.length / 300));
  const points = all.filter((_, i) => i % step === 0);
  const tip = all.reduce((a, b) => (len(b) > len(a) ? b : a));
  const lo: V3 = [Infinity, Infinity, Infinity];
  const hi: V3 = [-Infinity, -Infinity, -Infinity];
  for (const q of all) for (let j = 0; j < 3; j++) (lo[j] = Math.min(lo[j], q[j])), (hi[j] = Math.max(hi[j], q[j]));
  const size = hi.map((x, j) => x - lo[j]);
  const axis = size.indexOf(Math.min(...size));
  const thin: V3 = [0, 0, 0];
  thin[axis] = 1;
  const centre = lo.map((x, j) => (x + hi[j]) / 2) as V3;
  const name = file.split("/").at(-1)!.replace(".glb", "");
  const k = hung?.s[0] ?? 1;
  const turned = (v: V3): V3 => (hung ? rot(hung.r, v) : v);
  const mesh = hand === "socket.hand.R" || hand === "socket.hand.L" ? meshOf(g) : undefined;
  const balls = hand === "socket.hand.R" ? sliced(mesh!, tip).map((b) => ({ ...b, p: put(b.p), r: b.r * k })) : undefined;
  // A shield is the off hand's strapped look; another (a torch, carried) has no board.
  const disc = hand === "socket.hand.L" && strap ? board(mesh!) : undefined;
  const plate = disc && { centre: put(disc.centre), normal: turned(disc.normal), radius: disc.radius * k, half: disc.half * k };
  const front = turned(rot(pose.r, [0, 0, -1]));
  const up = turned(rot(pose.r, [-1, 0, 0]));
  // A held grip's bar runs along its handle (+Y); a strapped look's hand strap runs across its
  // board, square to the forearm (+X), where the fist closes on it (D459, D504).
  const bar = turned(rot(pose.r, strap ? [1, 0, 0] : [0, 1, 0]));
  return { hand, socket, name, points: points.map(put), tip: put(tip), thin: turned(thin), front, up, centre: put(centre), grip: put(pose.t), bar, balls, board: plate, strap };
}

type CarryExtras = { carry_tilt?: number; carry_hold?: string; carry_upright?: boolean };
const extrasOf = (g: Glb, node: number): CarryExtras => (g.gltf.nodes[node] as { extras?: CarryExtras }).extras ?? {};

/** The carry pose a look names (its extras' `carry_hold`, D560): the clip whose first pose the
 * arm holding it takes while it is carried, as terra_walker::character's `held_ready` reads it. */
export function carryHold(file: string): string | undefined {
  const g = new Glb(file);
  const node = g.gltf.nodes.findIndex((n) => n.mesh !== undefined);
  const hold = node < 0 ? undefined : extrasOf(g, node).carry_hold;
  return typeof hold === "string" ? hold : undefined;
}

/** Whether a look's carry pose is held upright (its extras' `carry_upright`, from the pose's
 * `upright` in its set's data): its arm then keeps its turn in the body's frame (carry_arm.ts);
 * any other pose's arm leans with the chest, as terra_walker::carry_arm reads it. */
export function carryUpright(file: string): boolean {
  const g = new Glb(file);
  const node = g.gltf.nodes.findIndex((n) => n.mesh !== undefined);
  return node >= 0 && extrasOf(g, node).carry_upright === true;
}

/** How far a look turns about its X when carried, degrees, as terra_walker::character's
 * `held_ready` finds it: its extras' `carry_tilt` where given, else none. */
export function carryTilt(file: string): number {
  const g = new Glb(file);
  const node = g.gltf.nodes.findIndex((n) => n.mesh !== undefined);
  const tilt = node < 0 ? undefined : extrasOf(g, node).carry_tilt;
  return typeof tilt === "number" ? tilt : 0;
}

/** The clips in which the game carries what is in hand rather than uses it, read from its
 * source (terra_walker::character::CARRIED_IN), so the two cannot differ. */
export function carriedClips(): string[] {
  const rs = readFileSync(join(import.meta.dirname, "..", "..", "crates", "terra_walker", "src", "character", "held.rs"), "utf8");
  const list = /pub const CARRIED_IN: \[&str; \d+\] = \[([^\]]*)\]/.exec(rs);
  if (!list) throw new Error("no CARRIED_IN in terra_walker's character/held.rs");
  return [...list[1].matchAll(/"(\w+)"/g)].map((m) => m[1]);
}
