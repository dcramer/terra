// The character render audit: each link between a clip as authored and the pixels a person
// sees, measured against a reference that does not share its code, a line a link (D388).
//
//   rest    each limb segment's turn off its bone's +Y at rest, ours against the skeleton the
//           clips were authored on (mannequin.glb, whose every child sits on its bone's +Y).
//           A clip turns bones; where a segment does not run along its bone, the limb points
//           off where the clip points it by that angle, in every pose (the retarget).
//   clips   per clip, how far the hands, feet and head land from where the clip puts them:
//           our segments' lengths laid along the directions the authoring skeleton's segments
//           take in the same clip (the mannequin posed by its own clip of that name, else, for a
//           clip copied bone by bone, the bone's +Y), from the same shoulder, hip or pelvis.
//           Unreal's IK Retargeter and Unity's Humanoid retarget to make these zero.
//   morph   per body template, each moved joint against the skin's crease round it (vertices
//           weighted between it and its parent): the joint should move as the morphed skin does.
//   gear    each worn look's skeleton at rest against the body's, joint by joint: a skinned
//           look is drawn with the body's joints and its own bind poses (character::dress), so
//           a joint standing elsewhere in its file puts its skin off the body by as much.
//   strap   a strapped look (a shield) against the forearm it rides, per template: its grip
//           line off the elbow-to-wrist line (degrees) and the wrist off the board's plane past
//           the elbow's (cm).
//   skin    our weight-blended dual quaternions (assets/shaders/skinning.wesl, ported) against
//           the plain linear blend and against themselves turned by a known angle.
//
// node tools/characters/audit.ts [--body FILE] [--source FILE] [--clips FILE,...] [--clip NAME]
//   [--at SECONDS] [--json OUT]. Node 22.18 or later; no dependencies.

import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { type Clip, Glb, type Pose, type Q, type V3, dot, len, matrix, posed, qinv, qmul, rot, sub } from "./glb.ts";
import { along } from "./held.ts";
import { type Shape, shaped, skinOf } from "./skin.ts";

const ROOT = resolve(import.meta.dirname, "../..");
const CHAR = `${ROOT}/assets/models/character`;
const DEG = 180 / Math.PI;

/** The chains whose ends a person watches: each from its root joint down; a foot's to the
 * ankle, its turn to the ball apart (aim.py holds the ball's pitch to be the foot's shape). */
export const CHAINS: Record<string, string[]> = {
  hand_l: ["upperarm_l", "lowerarm_l", "hand_l"],
  hand_r: ["upperarm_r", "lowerarm_r", "hand_r"],
  foot_l: ["thigh_l", "calf_l", "foot_l"],
  foot_r: ["thigh_r", "calf_r", "foot_r"],
  toe_l: ["foot_l", "ball_l"],
  toe_r: ["foot_r", "ball_r"],
  head: ["pelvis", "spine_01", "spine_02", "spine_03", "neck_01", "Head"],
};

/** What a skeleton tool needs of a file: Glb, or one made up for a test. */
export type Skeleton = Pick<Glb, "gltf" | "parentOf" | "rest" | "named">;

const unit = (v: V3): V3 => {
  const l = len(v) || 1;
  return [v[0] / l, v[1] / l, v[2] / l];
};
const add = (a: V3, b: V3): V3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const scale = (a: V3, k: number): V3 => [a[0] * k, a[1] * k, a[2] * k];
const angle = (a: V3, b: V3) => Math.acos(Math.max(-1, Math.min(1, dot(unit(a), unit(b))))) * DEG;

/** Each segment of the chains (bone, child): its child's rest translation off the bone's +Y,
 * degrees. A skeleton whose children sit on +Y reads 0 everywhere. */
export function offAxis(body: Skeleton): { bone: string; child: string; deg: number }[] {
  const out: { bone: string; child: string; deg: number }[] = [];
  for (const chain of Object.values(CHAINS))
    for (let k = 1; k < chain.length; k++) {
      const child = body.named(chain[k]);
      if (child < 0 || body.named(chain[k - 1]) < 0) continue;
      out.push({ bone: chain[k - 1], child: chain[k], deg: angle(body.rest(child).t as V3, [0, 1, 0]) });
    }
  return out;
}

/** Where each chain's end lands at a pose against where the authoring skeleton's pose sends
 * it: from our chain's root, each of our segments laid along `aims` (the authoring segment's
 * direction in the model's space), cm. */
export function chainOff(body: Skeleton, pose: Pose[], aims: (chain: string[], k: number, name?: string) => V3 | undefined): Record<string, number> {
  const out: Record<string, number> = {};
  for (const [name, chain] of Object.entries(CHAINS)) {
    const at = chain.map((b) => body.named(b));
    if (at.some((i) => i < 0)) continue;
    let meant = pose[at[0]].t as V3;
    let ok = true;
    for (let k = 1; k < chain.length; k++) {
      const ours = sub(pose[at[k]].t, pose[at[k - 1]].t);
      const aim = aims(chain, k, name);
      if (!aim) {
        ok = false;
        break;
      }
      meant = add(meant, scale(unit(aim), len(ours)));
    }
    if (ok) out[name] = len(sub(pose[at[chain.length - 1]].t, meant)) * 100;
  }
  return out;
}

/** The authoring segment's direction for a clip copied bone by bone onto a skeleton whose
 * children sat on +Y: the bone's own +Y in our pose. */
export const alongBone = (body: Skeleton, pose: Pose[]) => (chain: string[], k: number): V3 => rot(pose[body.named(chain[k - 1])].r, [0, 1, 0]);

/** The authoring segment's direction from the source skeleton posed by its own clip. */
export const alongSource = (source: Skeleton, pose: Pose[]) => (chain: string[], k: number): V3 | undefined => {
  const [a, b] = [source.named(chain[k - 1]), source.named(chain[k])];
  return a < 0 || b < 0 ? undefined : sub(pose[b].t, pose[a].t);
};

/** The chains whose lines are the body's own shape, not a posture (retarget.py, D542): the
 * torso's joints inside an upright body, and how far a foot pitches from ankle to ball. */
export const SHAPED = new Set(["head", "toe_l", "toe_r"]);

/** For a shaped chain, the direction the clip means for our segment: the source bone's turn in
 * the model from its rest, laid on our rest segment (a foot's first turned about the up axis
 * onto the source's, its pitch kept). Elsewhere the source's own segment. */
export const meantBy = (body: Skeleton, source: Skeleton, pose: Pose[]) => {
  const ours = posed(body as Glb, undefined, 0);
  const theirs = posed(source as Glb, undefined, 0);
  const along = alongSource(source, pose);
  return (chain: string[], k: number, name?: string): V3 | undefined => {
    if (!name || !SHAPED.has(name)) return along(chain, k);
    const [a, b] = [source.named(chain[k - 1]), source.named(chain[k])];
    const [i, j] = [body.named(chain[k - 1]), body.named(chain[k])];
    if (a < 0 || b < 0 || i < 0 || j < 0) return undefined;
    let line = sub(ours[j].t, ours[i].t);
    if (name.startsWith("toe")) {
      const src = sub(theirs[b].t, theirs[a].t);
      const yaw = Math.atan2(src[0], src[2]) - Math.atan2(line[0], line[2]);
      line = rot([0, Math.sin(yaw / 2), 0, Math.cos(yaw / 2)], line);
    }
    return rot(qmul(pose[a].r, qinv(theirs[a].r)), line);
  };
};

// ---- skinning: the shader's blend, ported line for line from assets/shaders/skinning.wesl.

type M4 = number[];
type Joint = { real: Q; dual: Q; scale: number };

function quatFrom(m: M4, s: number): Q {
  const c = (col: number, row: number) => m[col * 4 + row] / s;
  const trace = c(0, 0) + c(1, 1) + c(2, 2);
  if (trace > 0) {
    const k = Math.sqrt(trace + 1) * 2;
    return [(c(1, 2) - c(2, 1)) / k, (c(2, 0) - c(0, 2)) / k, (c(0, 1) - c(1, 0)) / k, 0.25 * k];
  }
  if (c(0, 0) > c(1, 1) && c(0, 0) > c(2, 2)) {
    const k = Math.sqrt(1 + c(0, 0) - c(1, 1) - c(2, 2)) * 2;
    return [0.25 * k, (c(1, 0) + c(0, 1)) / k, (c(2, 0) + c(0, 2)) / k, (c(1, 2) - c(2, 1)) / k];
  }
  if (c(1, 1) > c(2, 2)) {
    const k = Math.sqrt(1 + c(1, 1) - c(0, 0) - c(2, 2)) * 2;
    return [(c(1, 0) + c(0, 1)) / k, 0.25 * k, (c(2, 1) + c(1, 2)) / k, (c(2, 0) - c(0, 2)) / k];
  }
  const k = Math.sqrt(1 + c(2, 2) - c(0, 0) - c(1, 1)) * 2;
  return [(c(2, 0) + c(0, 2)) / k, (c(2, 1) + c(1, 2)) / k, 0.25 * k, (c(0, 1) - c(1, 0)) / k];
}

function jointOf(m: M4): Joint {
  const s = Math.hypot(m[0], m[1], m[2]);
  const q = quatFrom(m, s);
  const t: V3 = [m[12], m[13], m[14]];
  const v = add(scale(t, q[3]), [t[1] * q[2] - t[2] * q[1], t[2] * q[0] - t[0] * q[2], t[0] * q[1] - t[1] * q[0]]);
  return { real: q, dual: [0.5 * v[0], 0.5 * v[1], 0.5 * v[2], -0.5 * dot(t, [q[0], q[1], q[2]])], scale: s };
}

/** The skin matrix of up to four joints by weights, as the shader makes it: the linear blend
 * mixed with the dual quaternion blend by the share the weights carry (sum less one). */
export function skinMatrix(ms: M4[], weights: number[]): M4 {
  const total = weights.reduce((a, b) => a + b, 0);
  const w = weights.map((x) => x / total);
  const linear = new Array(16).fill(0).map((_, i) => ms.reduce((s, m, k) => s + w[k] * m[i], 0));
  const share = Math.min(1, Math.max(0, total - 1));
  if (share < 0.001) return linear;
  const js = ms.map(jointOf);
  const real = [0, 0, 0, 0];
  const dual = [0, 0, 0, 0];
  let sc = 0;
  js.forEach((j, k) => {
    const sign = k > 0 && dot4(js[0].real, j.real) < 0 ? -1 : 1;
    for (let i = 0; i < 4; i++) (real[i] += sign * w[k] * j.real[i]), (dual[i] += sign * w[k] * j.dual[i]);
    sc += w[k] * j.scale;
  });
  const n = Math.hypot(...real);
  const [x, y, z, ww] = real.map((v) => v / n);
  const d = dual.map((v) => v / n);
  const cr: V3 = [y * d[2] - z * d[1], z * d[0] - x * d[2], x * d[1] - y * d[0]];
  const t: V3 = [2 * (ww * d[0] - d[3] * x + cr[0]), 2 * (ww * d[1] - d[3] * y + cr[1]), 2 * (ww * d[2] - d[3] * z + cr[2])];
  const dq = [
    (1 - 2 * (y * y + z * z)) * sc, 2 * (x * y + ww * z) * sc, 2 * (x * z - ww * y) * sc, 0,
    2 * (x * y - ww * z) * sc, (1 - 2 * (x * x + z * z)) * sc, 2 * (y * z + ww * x) * sc, 0,
    2 * (x * z + ww * y) * sc, 2 * (y * z - ww * x) * sc, (1 - 2 * (x * x + y * y)) * sc, 0,
    t[0], t[1], t[2], 1,
  ];
  return linear.map((l, i) => l * (1 - share) + dq[i] * share);
}
const dot4 = (a: number[], b: number[]) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
const apply = (m: M4, p: V3): V3 => [0, 1, 2].map((j) => m[j] * p[0] + m[4 + j] * p[1] + m[8 + j] * p[2] + m[12 + j]) as V3;

/** The share of dual quaternions a vertex takes (skinning.rs `Blend::share`). */
function shareOf(names: string[], weights: number[], blend: { linear_joints: string[]; linear_gain: number }): number {
  const total = weights.reduce((a, b) => a + b, 0);
  if (total <= 0) return 0;
  const on = weights.reduce((s, w, k) => s + (blend.linear_joints.includes(names[k]) ? w : 0), 0) / total;
  return Math.min(1, Math.max(0, 1 - blend.linear_gain * on));
}

// ---- the stages

type Line = { stage: string; ok: boolean | null; text: string; data: unknown };

function stageRest(body: Glb, source: Glb): Line {
  const ours = offAxis(body);
  const theirs = new Map(offAxis(source).map((s) => [s.bone, s.deg]));
  const off = ours.filter((s) => s.deg > 2).sort((a, b) => b.deg - a.deg);
  const text = off.map((s) => `${s.bone} ${s.deg.toFixed(1)}° (source ${(theirs.get(s.bone) ?? NaN).toFixed(1)})`).join(", ");
  return { stage: "rest", ok: off.length === 0, text: off.length ? `${off.length} segments off their bone: ${text}` : "every segment on its bone", data: ours };
}

function stageClips(body: Glb, source: Glb, files: string[], only?: string): Line {
  const rows: { clip: string; file: string; by: string; worst: Record<string, number> }[] = [];
  for (const file of files) {
    const g = file === body.file ? body : new Glb(file);
    for (const name of g.clipNames()) {
      if (only && name !== only) continue;
      const clip = g.clip(name)!;
      const own = source.clip(name);
      const worst: Record<string, number> = {};
      for (let i = 0; i <= 20; i++) {
        const t = (clip.end * i) / 20;
        const pose = posed(body, clip, t);
        const aims = own ? meantBy(body, source, posed(source, own, Math.min(t, own.end))) : alongBone(body, pose);
        for (const [k, cm] of Object.entries(chainOff(body, pose, aims))) worst[k] = Math.max(worst[k] ?? 0, cm);
      }
      rows.push({ clip: name, file: file.split("/").pop()!, by: own ? "source clip" : "bone +Y", worst });
    }
  }
  const ends = Object.keys(CHAINS);
  const median = (xs: number[]) => xs.sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? 0;
  const per = ends.map((e) => `${e} ${median(rows.map((r) => r.worst[e] ?? 0)).toFixed(1)}`).join(", ");
  const top = rows.map((r) => ({ r, m: Math.max(...Object.values(r.worst)) })).sort((a, b) => b.m - a.m)[0];
  const end = top && Object.entries(top.r.worst).sort((a, b) => b[1] - a[1])[0];
  const at = end ? `; worst ${top.r.clip} (${top.r.file}) ${end[0]} ${end[1].toFixed(1)} cm` : "";
  const bad = rows.filter((r) => Math.max(...Object.values(r.worst)) > 2).length;
  return { stage: "clips", ok: bad === 0, text: `${bad} of ${rows.length} clips put an end over 2 cm off the clip's; median worst cm: ${per}${at}`, data: rows };
}

const LIMB = /^(upperarm|lowerarm|hand|calf|foot|neck_01|Head)/;

function stageMorph(body: Glb, shapes: Shape[]): Line {
  const skin = body.gltf.skins![0];
  const prim = body.gltf.meshes![0].primitives[0];
  const base = body.read(prim.attributes.POSITION);
  const joints = body.read(prim.attributes.JOINTS_0);
  const weights = body.read(prim.attributes.WEIGHTS_0);
  const rows: { template: string; joint: string; cm: number }[] = [];
  for (const shape of shapes) {
    for (const [node, by] of shape.shift) {
      const parent = body.parentOf[node];
      // Limbs only: their creases are round, so the crease's middle is the joint's; the
      // torso's are not (a chest or a belly moves one side of them).
      if (parent === undefined || len(by) === 0 || !LIMB.test(body.gltf.nodes[node].name ?? "")) continue;
      const jk = skin.joints.indexOf(node);
      const pk = skin.joints.indexOf(parent);
      if (jk < 0 || pk < 0) continue;
      // The crease: vertices weighted between this joint and its parent, each 0.3 or more.
      const moved: V3 = [0, 0, 0];
      let n = 0;
      for (let v = 0; v < base.length / 3; v++) {
        let wj = 0;
        let wp = 0;
        for (let k = 0; k < 4; k++) {
          if (joints[v * 4 + k] === jk) wj += weights[v * 4 + k];
          if (joints[v * 4 + k] === pk) wp += weights[v * 4 + k];
        }
        if (wj < 0.3 || wp < 0.3) continue;
        for (let j = 0; j < 3; j++) moved[j] += shape.positions[v * 3 + j] - base[v * 3 + j];
        n++;
      }
      if (n < 8) continue;
      rows.push({ template: shape.name, joint: body.gltf.nodes[node].name ?? "", cm: len(sub(scale(moved, 1 / n), by)) * 100 });
    }
  }
  rows.sort((a, b) => b.cm - a.cm);
  const top = rows.slice(0, 4).map((r) => `${r.template} ${r.joint} ${r.cm.toFixed(1)}`).join(", ");
  return { stage: "morph", ok: (rows[0]?.cm ?? 0) <= 2.5, text: `${rows.length} moved limb joints against their creases; worst cm: ${top}`, data: rows };
}

function stageGear(body: Glb): Line {
  const rest = posed(body, undefined, 0);
  const rows: { look: string; joint: string; cm: number; deg: number }[] = [];
  const dir = `${ROOT}/assets/models/gear`;
  for (const f of readdirSync(dir).filter((f) => f.endsWith(".glb"))) {
    const look = new Glb(`${dir}/${f}`);
    if (!look.gltf.skins?.length) continue;
    const theirs = posed(look, undefined, 0);
    let worst = { look: f, joint: "", cm: 0, deg: 0 };
    for (const j of new Set(look.gltf.skins.flatMap((s) => s.joints))) {
      const name = look.gltf.nodes[j].name;
      const b = name ? body.named(name) : -1;
      if (b < 0) continue;
      const cm = len(sub(theirs[j].t, rest[b].t)) * 100;
      const q = qmul(qinv(rest[b].r), theirs[j].r);
      const deg = 2 * Math.acos(Math.min(1, Math.abs(q[3]))) * DEG;
      if (cm + deg / 10 > worst.cm + worst.deg / 10) worst = { look: f, joint: name!, cm, deg };
    }
    rows.push(worst);
  }
  rows.sort((a, b) => b.cm - a.cm);
  const bad = rows.filter((r) => r.cm > 0.5 || r.deg > 1);
  const top = rows[0] ? `; worst ${rows[0].look} ${rows[0].joint} ${rows[0].cm.toFixed(2)} cm ${rows[0].deg.toFixed(2)}°` : "";
  return { stage: "gear", ok: bad.length === 0, text: `${rows.length} skinned looks, ${bad.length} with a joint off the body's${top}`, data: rows };
}

/** A strapped look's grip line against the forearm at a pose: degrees between them, and how
 * far the wrist stands off the board's plane beyond the elbow (cm). */
export function strapOff(pose: Pose[], bone: number, child: number, strap: { face: V3; grip: V3 }): { deg: number; cm: number } {
  const b = pose[bone];
  const s = b.s[0];
  const at = (p: V3): V3 => rot(qinv(b.r), sub(p, b.t)).map((x) => x / s) as V3;
  const wrist = at(pose[child].t);
  const elbow: V3 = [0, 0, 0];
  // The strap's face and grip are in the true forearm's frame (held.ts `frameOf`).
  const forearm = along(wrist);
  const plane = (p: V3) => dot(p, unit(rot(forearm, strap.face)));
  return { deg: angle(rot(forearm, strap.grip), sub(wrist, elbow)), cm: Math.abs(plane(wrist) - plane(elbow)) * s * 100 };
}

function stageStrap(body: Glb, shapes: Shape[]): Line {
  const dir = `${ROOT}/assets/models/held`;
  const rows: { look: string; template: string; deg: number; cm: number }[] = [];
  const still: Clip = { name: "rest", end: 0, tracks: new Map() };
  for (const f of readdirSync(dir).filter((f) => f.endsWith(".glb"))) {
    const look = new Glb(`${dir}/${f}`);
    const strap = look.gltf.nodes.map((n) => (n as { extras?: { strap?: { bone: string; face: V3; grip: V3 } } }).extras?.strap).find(Boolean);
    if (!strap) continue;
    const bone = body.named(strap.bone);
    const child = body.named(strap.bone.replace("lowerarm", "hand").replace("_twist_01", ""));
    for (const shape of shapes) rows.push({ look: f, template: shape.name, ...strapOff(shaped(body, shape, still, 0), bone, child, strap) });
  }
  const worst = rows.sort((a, b) => b.deg - a.deg)[0];
  const text = worst ? `${rows.length / Math.max(1, new Set(rows.map((r) => r.template)).size)} strapped looks; worst ${worst.look} on ${worst.template}: grip ${worst.deg.toFixed(1)}° off the forearm, wrist ${worst.cm.toFixed(1)} cm off the board's plane past the elbow` : "no strapped look";
  return { stage: "strap", ok: !worst || worst.deg <= 5, text, data: rows };
}

function stageSkin(body: Glb, clip: Clip | undefined, t: number): Line {
  const blend = JSON.parse(readFileSync(`${CHAR}/skinning.json`, "utf8"));
  const skin = body.gltf.skins![0];
  const prim = body.gltf.meshes![0].primitives[0];
  const pos = body.read(prim.attributes.POSITION);
  const joints = body.read(prim.attributes.JOINTS_0);
  const weights = body.read(prim.attributes.WEIGHTS_0);
  const ibm = body.read(skin.inverseBindMatrices);
  const pose = posed(body, clip, t);
  const mats = skin.joints.map((j, k) => mmul4(matrix(pose[j]), Array.from(ibm.subarray(k * 16, k * 16 + 16))));
  const names = skin.joints.map((j) => body.gltf.nodes[j].name ?? "");
  const diffs: { cm: number; joint: string }[] = [];
  for (let v = 0; v < pos.length / 3; v++) {
    const idx = [0, 1, 2, 3].map((k) => joints[v * 4 + k]);
    const w = [0, 1, 2, 3].map((k) => weights[v * 4 + k]);
    const total = w.reduce((a, b) => a + b, 0);
    if (total <= 0) continue;
    const share = shareOf(idx.map((i) => names[i]), w, blend);
    const carried = w.map((x) => (x * (1 + share)) / total);
    const p: V3 = [pos[v * 3], pos[v * 3 + 1], pos[v * 3 + 2]];
    const ours = apply(skinMatrix(idx.map((i) => mats[i]), carried), p);
    const lin = apply(skinMatrix(idx.map((i) => mats[i]), w.map((x) => x / total)), p);
    const most = w.indexOf(Math.max(...w));
    diffs.push({ cm: len(sub(ours, lin)) * 100, joint: names[idx[most]] });
  }
  diffs.sort((a, b) => b.cm - a.cm);
  const p95 = diffs[Math.floor(diffs.length * 0.05)]?.cm ?? 0;
  return { stage: "skin", ok: null, text: `${clip?.name ?? "rest"} at ${t.toFixed(2)} s: ours off the linear blend by ${diffs[0]?.cm.toFixed(2)} cm at most (${diffs[0]?.joint}), 95th percentile ${p95.toFixed(2)} cm`, data: diffs.slice(0, 20) };
}

function mmul4(a: M4, b: M4): M4 {
  const o = new Array(16).fill(0);
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) for (let k = 0; k < 4; k++) o[c * 4 + r] += a[k * 4 + r] * b[c * 4 + k];
  return o;
}

function main() {
  const args = process.argv.slice(2);
  const opt = (name: string, fallback?: string) => {
    const i = args.indexOf(`--${name}`);
    return i >= 0 ? args[i + 1] : fallback;
  };
  const body = new Glb(resolve(opt("body", `${CHAR}/base.glb`)!));
  const source = new Glb(resolve(opt("source", `${CHAR}/mannequin.glb`)!));
  const files = (opt("clips", `${CHAR}/base.glb,${CHAR}/fight.glb`) ?? "").split(",").map((f) => resolve(f));
  const only = opt("clip");
  const t = Number(opt("at", "0"));
  const { shapes } = skinOf(body, `${CHAR}/templates.json`);
  const skinClip = only ? files.map((f) => new Glb(f).clip(only)).find(Boolean) : undefined;
  const lines: Line[] = [];
  for (const stage of [
    () => stageRest(body, source),
    () => stageClips(body, source, files, only),
    () => stageMorph(body, shapes),
    () => stageGear(body),
    () => stageStrap(body, shapes),
    () => stageSkin(body, skinClip, t),
  ]) {
    const line = stage();
    lines.push(line);
    console.log(`${line.ok === null ? "·" : line.ok ? "✓" : "✗"} ${line.stage.padEnd(6)} ${line.text}`);
  }
  const out = opt("json");
  if (out) writeFileSync(out, JSON.stringify(lines, null, 1));
  process.exitCode = lines.some((l) => l.ok === false) ? 1 : 0;
}

if (import.meta.main) main();
