// The attachment contract's check (docs/attachment.md, D546): every look the game hangs on a
// character, read from its committed file without Blender or a GPU, against the rules of its
// kind; and the body's sockets against their table. A look's kind is named in its data
// (art/held/<item>.json, art/gear/<item>/fit.json, "kind"; a skinned garment has none), and
// each kind's rules (how it goes on, its life size, where its grip falls) are attach_kinds.json.
//
//   node tools/characters/attach_contract.ts            every look in assets/models/{held,gear}
//   node tools/characters/attach_contract.ts FILE.glb   one look, judged by where it sits
//
// A fault already known is listed, with why, in attach_faults.json; a fix removes its line.
// Node 22.18 or later; no dependencies.

import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { Glb, place, posed, rot, type Pose, type V3 } from "./glb.ts";

export const ROOT = join(import.meta.dirname, "..", "..");
const MODELS = join(ROOT, "assets", "models");
export const BASE = join(MODELS, "character", "base.glb");

/** The five ways a look goes on (docs/attachment.md, "Mounts"). */
export type Mount = "in hand" | "hung" | "strapped" | "rigid" | "skinned";

/** A kind of look (docs/attachment.md, "Kinds"), from attach_kinds.json: how it goes on, its
 * life size and where its grip falls. Each item's data names its kind. */
export type KindRules = {
  mount: Mount;
  length_m?: [number, number];
  size_m?: [number, number];
  grip_from_end_m?: [number, number];
  grip_share?: [number, number];
  second?: string;
  classes?: string[];
  fits: "detector" | "person" | "body";
};
export const KINDS: Record<string, KindRules> = Object.fromEntries(
  Object.entries(JSON.parse(readFileSync(join(import.meta.dirname, "attach_kinds.json"), "utf8"))).filter(([k]) => k !== "about"),
) as Record<string, KindRules>;
/** The kind of a skinned look with no data of its own: clothing, armour, a cap. */
export const GARMENT = "garment";

/** What the check reads of a look's file: its extras, whether it is skinned and to what, its
 * morphs, and its vertices and triangles in its own space. */
export type Look = {
  name: string; // "held/knife", "gear/quiver"
  extras: Record<string, unknown>;
  joints: string[] | null;
  targets: string[];
  points: V3[];
  tris: number[];
  /** Each vertex's normal as the file gives it, placed as its vertex is; none if it gives none. */
  normals?: (V3 | undefined)[];
  /** A skinned look's: each vertex's most-weighted joint, by name. */
  heaviest?: string[];
};

/** What the check reads of the body: its node names, sockets and joints at rest, morphs and height. */
export type Body = { nodes: Set<string>; parent: Map<string, string>; sockets: Map<string, Pose>; targets: string[]; at?: Map<string, V3>; height?: number };

export type Fault = { look: string; rule: string; detail: string };

/** Each socket's bone and its axes at rest in the body's space (glTF's: +X the body's left,
 * +Y up, +Z its front), the table in docs/attachment.md. */
export const SOCKETS: Record<string, { bone: string; x: V3; y: V3; z: V3 }> = {
  "socket.hand.R": { bone: "hand_r", x: [0, 1, 0], y: [0, 0, 1], z: [1, 0, 0] },
  "socket.hand.L": { bone: "hand_l", x: [0, -1, 0], y: [0, 0, 1], z: [-1, 0, 0] },
  "socket.head": { bone: "Head", x: [1, 0, 0], y: [0, 1, 0], z: [0, 0, 1] },
  "socket.back": { bone: "spine_03", x: [1, 0, 0], y: [0, 1, 0], z: [0, 0, 1] },
  "socket.belt.R": { bone: "pelvis", x: [1, 0, 0], y: [0, 1, 0], z: [0, 0, 1] },
  "socket.belt.L": { bone: "pelvis", x: [1, 0, 0], y: [0, 1, 0], z: [0, 0, 1] },
};

/** How far a socket's axis may lie from its table's, degrees. */
export const SOCKET_AXIS_DEG = 10;
/** How far the middle of a held look's handle, where y = 0 cuts it, may sit from the origin. */
export const GRIP_M = 0.015;
/** How much wider across X than across Z a working end may be and still lie in the Y-Z plane:
 * a round end (a torch's) has no facing and passes. */
export const ACROSS = { ratio: 1.1, slack_m: 0.005 };
/** How much further one side of a working end must reach to count as one-sided (an axe's bit,
 * a hammer's face), which then faces -Z. */
export const ONE_SIDED_M = 0.02;
/** The share of a held look's length, from its far end, that is its working end. */
export const HEAD_SHARE = 0.25;
/** How far a rigid look's middle may sit from its socket: it is saved in the socket's space. */
export const RIGID_NEAR_M = 0.5;
/** Patches of skin a look may cover: 0 to 1,023 (body_mask.wesl). */
export const PATCHES = 1024;
/** How far a skinned look's vertex may lie from its most-weighted joint at rest. */
export const SKIN_REACH_M = 0.6;
/** How far to one side of the body's middle a vertex is on that side. */
export const SIDE_M = 0.05;
/** How far behind the body's middle (-Z) a pack's vertices lie on average, at least. */
export const PACK_BEHIND_M = 0.05;
/** The share of a look's surface that may face against its normals (insideOut). */
export const INSIDE_OUT = 0.25;

type Prim = { attributes: Record<string, number>; indices?: number; mode?: number };
type GNode = { name?: string; mesh?: number; skin?: number; extras?: Record<string, unknown> };

/** A look's file, read. Its meshes' vertices are placed by their nodes (a rigid look's node
 * sits where it hangs from); a skinned look's are left in bind space. */
export function readLook(file: string, name = lookName(file)): Look {
  const g = new Glb(file);
  const nodes = g.gltf.nodes as GNode[];
  const world = posed(g, undefined, 0);
  const extras: Record<string, unknown> = {};
  let joints: string[] | null = null;
  const targets: string[] = [];
  const points: V3[] = [];
  const tris: number[] = [];
  const normals: (V3 | undefined)[] = [];
  const heaviest: string[] = [];
  nodes.forEach((n, i) => {
    if (n.mesh === undefined) return;
    Object.assign(extras, n.extras ?? {});
    const mesh = g.gltf.meshes![n.mesh] as { primitives: Prim[]; extras?: { targetNames?: string[] } };
    for (const t of mesh.extras?.targetNames ?? []) if (!targets.includes(t)) targets.push(t);
    const skin = n.skin === undefined ? undefined : g.gltf.skins![n.skin].joints.map((j) => nodes[j].name ?? "");
    if (skin) joints = skin;
    // A normal goes as its vertex does, less the move: its node's turn and scale. A mirroring
    // node flips the winding's front too (glTF 2.0, "Transformations": a negative determinant
    // makes it clockwise), so the normal is flipped once more to be compared with the winding.
    const lin = (v: V3): V3 => {
      const [a, o] = [place(world[i], v), place(world[i], [0, 0, 0])];
      return [a[0] - o[0], a[1] - o[1], a[2] - o[2]];
    };
    const [ex, ey, ez] = [lin([1, 0, 0]), lin([0, 1, 0]), lin([0, 0, 1])];
    const mirrored = ex[0] * (ey[1] * ez[2] - ey[2] * ez[1]) - ex[1] * (ey[0] * ez[2] - ey[2] * ez[0]) + ex[2] * (ey[0] * ez[1] - ey[1] * ez[0]) < 0;
    const turn = (v: V3): V3 => lin(mirrored ? [-v[0], -v[1], -v[2]] : v);
    for (const p of mesh.primitives) {
      if ((p.mode ?? 4) !== 4) continue;
      const at = points.length;
      const a = g.read(p.attributes.POSITION);
      const nm = p.attributes.NORMAL === undefined ? undefined : g.read(p.attributes.NORMAL);
      for (let k = 0; k < a.length; k += 3) {
        const v: V3 = [a[k], a[k + 1], a[k + 2]];
        points.push(skin ? v : place(world[i], v));
        const nv: V3 | undefined = nm && [nm[k], nm[k + 1], nm[k + 2]];
        normals.push(nv && (skin ? nv : turn(nv)));
      }
      if (skin && p.attributes.JOINTS_0 !== undefined && p.attributes.WEIGHTS_0 !== undefined) {
        const [js, ws] = [g.read(p.attributes.JOINTS_0), g.read(p.attributes.WEIGHTS_0)];
        for (let k = 0; k < js.length; k += 4) {
          let best = k;
          for (let m = k + 1; m < k + 4; m++) if (ws[m] > ws[best]) best = m;
          heaviest[at + k / 4] = skin[js[best]];
        }
      }
      const idx = p.indices === undefined ? Array.from({ length: a.length / 3 }, (_, k) => k) : g.read(p.indices);
      for (const k of idx) tris.push(at + k);
    }
  });
  return { name, extras, joints, targets, points, tris, normals, heaviest: joints ? heaviest : undefined };
}

/** "held/knife" from assets/models/held/knife.glb. */
export function lookName(file: string): string {
  return `${basename(dirname(file))}/${basename(file, ".glb")}`;
}

export function readBody(file = BASE): Body {
  const g = new Glb(file);
  const nodes = g.gltf.nodes as GNode[];
  const world = posed(g, undefined, 0);
  const sockets = new Map<string, Pose>();
  const parent = new Map<string, string>();
  const at = new Map<string, V3>();
  let height = 0;
  nodes.forEach((n, i) => {
    if (n.name && g.parentOf[i] !== undefined) parent.set(n.name, nodes[g.parentOf[i]].name ?? "");
    if (n.name?.startsWith("socket.")) sockets.set(n.name, world[i]);
    if (n.name) at.set(n.name, world[i].t);
    if (n.mesh !== undefined && n.skin !== undefined)
      for (const p of g.gltf.meshes![n.mesh].primitives) {
        const a = g.read(p.attributes.POSITION);
        for (let k = 1; k < a.length; k += 3) height = Math.max(height, a[k]);
      }
  });
  const targets = (g.gltf.meshes ?? []).flatMap((m) => m.extras?.targetNames ?? []);
  return { nodes: new Set(nodes.map((n) => n.name ?? "")), parent, sockets, targets: [...new Set(targets)], at, height };
}

/** An item's numbers, art/held/<item>.json or art/gear/<item>/fit.json, if it has any. */
export function dataOf(look: string, root = ROOT): Record<string, unknown> | undefined {
  const [dir, item] = look.split("/");
  const file = dir === "held" ? join(root, "art", "held", `${item}.json`) : join(root, "art", "gear", item, "fit.json");
  return existsSync(file) ? JSON.parse(readFileSync(file, "utf8")) : undefined;
}

/** A look's kind, as its data names it ("kind"); a skinned look with no data is a garment.
 * Undefined when neither says. */
export function kindOf(look: Look, data?: Record<string, unknown>): string | undefined {
  if (typeof data?.kind === "string") return data.kind;
  if (!data && look.name.startsWith("gear/") && look.joints) return GARMENT;
  return undefined;
}

/** How a look's data has its maker fit it, which must be how its kind goes on. */
export function fittedAs(look: Look, data?: Record<string, unknown>): Mount | undefined {
  if (look.name.startsWith("held/")) {
    if (data?.fit === "hung") return "hung";
    if (data?.strap) return "strapped";
    return "in hand";
  }
  const fit = data?.fit;
  // A garment's data (gear_list.ts): made as a shell of the skin, or hung from the hips.
  if (fit === "back" || fit === "torso" || fit === "shell" || fit === "hung") return "skinned";
  if (fit === "belt" || fit === "head") return "rigid";
  return data ? undefined : "skinned";
}

const box = (pts: V3[]) => {
  const lo: V3 = [Infinity, Infinity, Infinity];
  const hi: V3 = [-Infinity, -Infinity, -Infinity];
  for (const p of pts) for (let k = 0; k < 3; k++) (lo[k] = Math.min(lo[k], p[k])), (hi[k] = Math.max(hi[k], p[k]));
  return { lo, hi };
};
const isV3 = (v: unknown) => Array.isArray(v) && v.length === 3 && v.every((x) => typeof x === "number");
const num = (v: unknown) => typeof v === "number" && Number.isFinite(v);
const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/** The share of a look's surface, by area, whose winding faces against its own normals: near
 * 0 made right, near 1 inside out (its vertices mirrored or its faces flipped without the
 * other). Undefined when its file gives no normals. */
export function insideOut(look: Look): number | undefined {
  if (!look.normals?.length) return undefined;
  let [against, all] = [0, 0];
  for (let t = 0; t + 2 < look.tris.length; t += 3) {
    const [a, b, c] = [0, 1, 2].map((k) => look.points[look.tris[t + k]]);
    const ns = [0, 1, 2].map((k) => look.normals![look.tris[t + k]]);
    if (ns.some((n) => !n)) continue;
    const u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    const v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    const w = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    const area = Math.hypot(w[0], w[1], w[2]);
    if (!area) continue;
    const d = [0, 1, 2].reduce((s, k) => s + w[k] * (ns[0]![k] + ns[1]![k] + ns[2]![k]), 0);
    all += area;
    if (d < 0) against += area;
  }
  return all ? against / all : undefined;
}

/** Which side of the body a joint is on by its name: +1 its left (+X), -1 its right, 0 neither. */
export function sideOf(joint: string): number {
  if (/(_l|\.L|Left|_L)$/.test(joint) || /^(left|Left)/.test(joint)) return 1;
  if (/(_r|\.R|Right|_R)$/.test(joint) || /^(right|Right)/.test(joint)) return -1;
  return 0;
}

/** A skinned look on the body as made on the base: the share of its vertices more than
 * SKIN_REACH_M from their most-weighted joint at rest (made elsewhere, at another scale, or
 * moved), and of those more than SIDE_M to one side weighted to a joint of the other side
 * (mirrored, its left on the body's right). */
export function onBody(look: Look, body: Body, reach = SKIN_REACH_M) {
  if (!look.heaviest || !body.at) return undefined;
  let [far, wrong, sided, n] = [0, 0, 0, 0];
  look.points.forEach((p, i) => {
    const j = look.heaviest![i];
    const at = j === undefined ? undefined : body.at!.get(j);
    if (!at) return;
    n++;
    if (Math.hypot(p[0] - at[0], p[1] - at[1], p[2] - at[2]) > reach) far++;
    const s = sideOf(j);
    if (s && Math.abs(p[0]) > SIDE_M) {
      sided++;
      if (Math.sign(p[0]) !== s) wrong++;
    }
  });
  return { far: n ? far / n : 0, mirrored: sided ? wrong / sided : 0 };
}

/** Where the plane y = 0 cuts the look's triangles: the middle of the crossings' extent in X
 * and Z, the middle of the handle a fist closes on. */
export function handleAt(look: Look): [number, number] | undefined {
  const cuts: V3[] = [];
  for (let t = 0; t + 2 < look.tris.length; t += 3) {
    for (const [i, j] of [[0, 1], [1, 2], [2, 0]]) {
      const a = look.points[look.tris[t + i]];
      const b = look.points[look.tris[t + j]];
      if ((a[1] < 0) === (b[1] < 0)) continue;
      const k = a[1] / (a[1] - b[1]);
      cuts.push([a[0] + (b[0] - a[0]) * k, 0, a[2] + (b[2] - a[2]) * k]);
    }
  }
  if (!cuts.length) return undefined;
  const { lo, hi } = box(cuts);
  return [(lo[0] + hi[0]) / 2, (lo[2] + hi[2]) / 2];
}

/** The working end (the far HEAD_SHARE of its length): how far it reaches each way across X
 * and Z from the handle's line. */
export function headReach(look: Look, handle: [number, number]) {
  const { lo, hi } = box(look.points);
  const from = hi[1] - HEAD_SHARE * (hi[1] - lo[1]);
  const head = box(look.points.filter((p) => p[1] >= from));
  return {
    x: head.hi[0] - head.lo[0],
    z: head.hi[2] - head.lo[2],
    plusX: head.hi[0] - handle[0],
    minusX: handle[0] - head.lo[0],
    plusZ: head.hi[2] - handle[1],
    minusZ: handle[1] - head.lo[2],
  };
}

function inHand(look: Look, f: (rule: string, detail: string) => void) {
  const { lo, hi } = box(look.points);
  const handle = handleAt(look);
  if (!handle) return f("grip", "nothing crosses y = 0: the grip is not at the origin");
  const off = Math.hypot(handle[0], handle[1]);
  if (off > GRIP_M)
    f("grip", `the handle's middle at y = 0 is ${(off * 100).toFixed(1)} cm from the origin (x ${(handle[0] * 100).toFixed(1)}, z ${(handle[1] * 100).toFixed(1)} cm), over ${GRIP_M * 100} cm`);
  if (hi[1] < -lo[1]) f("along +Y", `it reaches ${(-lo[1]).toFixed(2)} m down -Y but only ${hi[1].toFixed(2)} m up +Y, toward its working end`);
  const head = headReach(look, handle);
  if (head.x > ACROSS.ratio * head.z + ACROSS.slack_m)
    f("facing", `its working end is ${(head.x * 100).toFixed(1)} cm across X and ${(head.z * 100).toFixed(1)} cm across Z (reaching ${(head.plusX * 100).toFixed(1)} cm toward +X, ${(head.minusX * 100).toFixed(1)} toward -X): it lies across X, not in the Y-Z plane`);
  else if (head.plusZ - head.minusZ > ONE_SIDED_M)
    f("facing", `its working end reaches ${(head.plusZ * 100).toFixed(1)} cm toward +Z and ${(head.minusZ * 100).toFixed(1)} cm toward -Z: its striking side faces +Z, not -Z`);
}

const within = (v: number, [lo, hi]: [number, number]) => v >= lo - 1e-6 && v <= hi + 1e-6;
const range = ([lo, hi]: [number, number], unit = " m") => `${lo} to ${hi}${unit}`;

/** Its life size for its kind: an in-hand look's length along +Y, any other's longest extent;
 * and where an in-hand look's grip falls along it, from its -Y end (its butt or pommel). */
function sized(look: Look, name: string, rules: KindRules, f: (rule: string, detail: string) => void) {
  const { lo, hi } = box(look.points);
  const length = hi[1] - lo[1];
  const longest = Math.max(hi[0] - lo[0], length, hi[2] - lo[2]);
  if (rules.length_m && !within(length, rules.length_m))
    f("size", `it is ${length.toFixed(2)} m along +Y; a ${name} is ${range(rules.length_m)}`);
  if (rules.size_m && !within(longest, rules.size_m))
    f("size", `it is ${longest.toFixed(2)} m at its longest; a ${name} is ${range(rules.size_m)}`);
  const end = -lo[1];
  const off = [
    rules.grip_from_end_m && !within(end, rules.grip_from_end_m) && `${range(rules.grip_from_end_m.map((v) => v * 100) as [number, number], " cm")} from it`,
    rules.grip_share && length > 0 && !within(end / length, rules.grip_share) && `${range(rules.grip_share.map((v) => v * 100) as [number, number], "%")} of its length`,
  ].filter(Boolean);
  if (off.length)
    f("grip along", `its grip is ${(end * 100).toFixed(1)} cm (${((100 * end) / length).toFixed(0)}% of its length) from its -Y end; a ${name}'s is ${off.join(" and ")}`);
}

/** Where the other hand goes on a kind held in two (its rules' `second` names the key): a
 * number along +Y ("second") or a point ("nock"), in its data and, once made, its extras; for
 * "second", on the look's handle, ahead of its -Y end and short of its working end. */
function secondHand(look: Look, name: string, rules: KindRules, data: Record<string, unknown> | undefined, f: (rule: string, detail: string) => void) {
  const key = rules.second;
  if (!key) return;
  const want = data?.[key];
  const shaped = key === "nock" ? isV3(want) : num(want);
  if (!shaped) return f("second", `its data gives no "${key}", where a ${name}'s other hand goes (holding.md, section 3)`);
  if (!same(look.extras[key], want)) f("marker", `its extras' ${key} ${JSON.stringify(look.extras[key])} is not its data's ${JSON.stringify(want)}`);
  if (key === "second") {
    const { lo, hi } = box(look.points);
    const at = want as number;
    if (at < lo[1] + 0.04 || at > hi[1] - HEAD_SHARE * (hi[1] - lo[1])) f("second", `its second grip, ${at} m along +Y, is off its handle (${lo[1].toFixed(2)} to ${hi[1].toFixed(2)} m, its far quarter its working end)`);
  }
}

/** How an item is held, its `hold` in assets/data/items.ron (holding.md, section 1). */
export type Hold = { class: string; grip: string; length?: number };

/** Every item's hold in items.ron, by the id its looks are named by ("Arming sword" is
 * arming_sword, terra_walker::character::item_id). */
export function holds(root = ROOT): Map<string, Hold> {
  const out = new Map<string, Hold>();
  const text = readFileSync(join(root, "assets", "data", "items.ron"), "utf8");
  for (const entry of text.split(/\bname: "/).slice(1)) {
    const hold = /\bhold: \(([^)]*)\)/.exec(entry)?.[1];
    if (!hold) continue;
    const field = (k: string) => new RegExp(`\\b${k}: "?([\\w.]+)"?`).exec(hold)?.[1];
    const length = field("length");
    out.set(entry.slice(0, entry.indexOf('"')).toLowerCase().replaceAll(" ", "_"), { class: field("class") ?? "", grip: field("grip") ?? "", length: length === undefined ? undefined : Number(length) });
  }
  return out;
}
let held: Map<string, Hold> | undefined;
/** A held look's item's hold in the committed items.ron. */
export const holdOf = (look: string) => (look.startsWith("held/") ? (held ??= holds()).get(look.slice(5)) : undefined);

/** How far a number the look is made from may differ from what its data says now. */
export const MADE_M = 0.005;
/** How far an item's hold.length may differ from its look's, as a share. */
export const HOLD_SHARE = 0.03;

/** Its look is made from its data as it stands: the numbers its maker reads are what the look
 * measures (a data file changed and its look not made again, or a look made at another size). */
function madeFrom(look: Look, body: Body, data: Record<string, unknown>, root: string, f: (rule: string, detail: string) => void) {
  const { lo, hi } = box(look.points);
  const was = (what: string, got: number, want: number, tol: number, unit = "m") => {
    if (Math.abs(got - want) > tol) f("stale", `its ${what} is ${got.toFixed(3)} ${unit}, its data's ${want}: made again since?`);
  };
  if (data.fit === "length" && num(data.length) && num(data.grip_from_end)) {
    was("length along +Y", hi[1] - lo[1], data.length as number, MADE_M);
    was("grip from its -Y end", -lo[1], data.grip_from_end as number, MADE_M);
  }
  if (data.fit === "hung" && num(data.out)) {
    const top = look.points.filter((p) => p[2] > hi[2] - 0.02);
    was("handle's top across X", top.reduce((s, p) => s + p[0], 0) / top.length, data.out as number, MADE_M);
  }
  if (data.fit === "belt" && num(data.size) && num(data.hangs_at) && body.height) {
    was("height", hi[1] - lo[1], (data.size as number) * body.height, 0.03 * (data.size as number) * body.height);
    was("hanging point, as a share of its height from its foot,", -lo[1] / (hi[1] - lo[1]), data.hangs_at as number, 0.02, "");
  }
  const swing = data.swing as Record<string, unknown> | undefined;
  const made = look.extras.swing as Record<string, unknown> | undefined;
  if (swing && (!made || ["stiffness", "drag", "gravity"].some((k) => swing[k] !== made[k]))) f("stale", `its extras' swing ${JSON.stringify(made)} is not its data's ${JSON.stringify(swing)}`);
  const [dir, item] = look.name.split("/");
  const source =
    dir === "held" ? (typeof data.source === "string" ? join(root, "assets", "models", data.source) : undefined) : join(root, "art", "gear", typeof data.model === "string" ? data.model : item, "model.glb");
  if (source && !existsSync(source)) return f("file", `its data's source ${source.slice(root.length + 1)} is not there`);
  const sha = look.extras.source_sha;
  if (source && sha !== undefined && sha !== sourceSha(source)) f("stale", `its source ${source.slice(root.length + 1)} has changed since its look was made (source_sha ${sha})`);
}

/** The first 12 hex digits of the SHA-256 of a look's source's bytes, its extras' `source_sha`. */
export const sourceSha = (file: string) => createHash("sha256").update(readFileSync(file)).digest("hex").slice(0, 12);

/** Its item's hold (items.ron) agrees with its kind and its look: the class one its kind fights
 * as, its length the look's, and a look held in two saying where the other hand goes. */
function heldAs(look: Look, kind: string, rules: KindRules, hold: Hold, data: Record<string, unknown> | undefined, f: (rule: string, detail: string) => void) {
  if (rules.classes && !rules.classes.includes(hold.class))
    f("hold", `its item fights as ${hold.class} (items.ron), which a ${kind} does not; a ${kind}'s classes are ${rules.classes.join(", ") || "none"}`);
  const { lo, hi } = box(look.points);
  const length = rules.mount === "in hand" ? hi[1] - lo[1] : Math.max(hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]);
  if (hold.length !== undefined && Math.abs(length - hold.length) > HOLD_SHARE * hold.length)
    f("hold", `its item's hold.length is ${hold.length} m (items.ron), its look ${length.toFixed(2)} m`);
  if (hold.grip !== "Two" || rules.second) return;
  if (!num(data?.second)) f("second", `its item is held in two (items.ron), but its data gives no "second", where the other hand goes`);
  else if (!same(look.extras.second, data!.second)) f("marker", `its extras' second ${JSON.stringify(look.extras.second)} is not its data's ${data!.second}`);
}

function hung(look: Look, f: (rule: string, detail: string) => void) {
  const { lo, hi } = box(look.points);
  if (look.extras.hold !== "hung") f("marker", `its extras do not say {"hold": "hung"}, which its data's "fit" does`);
  if (hi[2] > 0.05) f("hangs", `it stands ${(hi[2] * 100).toFixed(1)} cm above its handle's top (+Z), over 5 cm`);
  if (lo[2] > -0.1) f("hangs", `it hangs only ${(-lo[2] * 100).toFixed(1)} cm below its handle (-Z)`);
  if (hi[1] - lo[1] < hi[0] - lo[0]) f("hangs", "its long way lies along X, not +Y (forward)");
}

function strapped(look: Look, body: Body, data: Record<string, unknown> | undefined, f: (rule: string, detail: string) => void) {
  // Physical and per body (D554): its hand strap in the fist, a hand socket on a hand that the
  // strap's bone carries, wherever that body's fist is; no place on the bone set for every body.
  const strap = look.extras.strap as { bone?: string; fist?: string; at?: unknown; face?: unknown; grip?: unknown } | undefined;
  if (!strap) return f("marker", "its extras have no strap, which its data gives");
  if (data && !same(strap, data.strap)) f("marker", "its extras' strap differs from its data's");
  if (!strap.bone || !body.nodes.has(strap.bone)) f("strap", `its strap's bone ${strap.bone} is not a bone of the body`);
  if (strap.at !== undefined) f("strap", "its strap sets a place on the bone (at), not the fist that closes on it");
  const hand = strap.fist === undefined ? undefined : body.parent.get(strap.fist);
  if (!strap.fist || !body.sockets.has(strap.fist) || !hand || body.parent.get(hand) !== strap.bone)
    f("strap", `its strap's fist ${strap.fist} is not a hand socket on a hand that ${strap.bone} carries`);
  if (!isV3(strap.face) || !isV3(strap.grip)) return f("strap", "its strap's face and grip are not each [x, y, z]");
  const [face, grip] = [strap.face as V3, strap.grip as V3];
  const d = face[0] * grip[0] + face[1] * grip[1] + face[2] * grip[2];
  if (Math.abs(Math.hypot(...face) - 1) > 0.01 || Math.abs(Math.hypot(...grip) - 1) > 0.01 || Math.abs(d) > 0.02)
    f("strap", "its strap's face and grip are not square unit directions");
  const { lo, hi } = box(look.points);
  const thick = hi[2] - lo[2];
  if (thick > 0.5 * Math.min(hi[0] - lo[0], hi[1] - lo[1])) f("board", `it is ${(thick * 100).toFixed(1)} cm along Z: its board does not lie across Z`);
  if ((lo[2] + hi[2]) / 2 >= 0) f("board", "its board is not toward -Z, in front of its grip");
}

function rigid(look: Look, body: Body, f: (rule: string, detail: string) => void) {
  const socket = look.extras.socket;
  if (typeof socket !== "string" || !SOCKETS[socket] || !body.sockets.has(socket)) return f("socket", `it names no socket of the body (${JSON.stringify(socket)})`);
  if (look.joints) f("socket", "it is skinned as well as on a socket");
  const { lo, hi } = box(look.points);
  const mid: V3 = [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2];
  if (Math.hypot(...mid) > RIGID_NEAR_M) f("socket space", `its middle is ${Math.hypot(...mid).toFixed(2)} m from its socket: not saved in the socket's space`);
  const morphs = look.extras.morphs as Record<string, unknown> | undefined;
  if (!morphs || !same(Object.keys(morphs).sort(), [...body.targets].sort()) || !Object.values(morphs).every(isV3))
    f("morphs", `its extras' morphs (${morphs ? Object.keys(morphs).join(", ") : "none"}) are not a shift for each of the body's (${body.targets.join(", ")})`);
  const swing = look.extras.swing as Record<string, unknown> | undefined;
  if (swing && !(num(swing.stiffness) && num(swing.drag) && num(swing.gravity) && isV3(swing.tail)))
    f("swing", "its swing lacks a number for stiffness, drag or gravity, or its tail");
}

/** The share of a skinned look's vertices that may lie out of its joints' reach, or on the
 * other side from their joint's. */
export const OFF_BODY = { far: 0.02, mirrored: 0.1 };

function skinned(look: Look, body: Body, kind: string, f: (rule: string, detail: string) => void) {
  if (!look.joints) return f("skin", "it has no skin");
  const strange = look.joints.filter((j) => !body.nodes.has(j));
  if (strange.length) f("skin", `joints not the body's: ${strange.slice(0, 5).join(", ")}`);
  // A loose garment's hem hangs free of the hips it is weighted to (hung_garment.py).
  const on = onBody(look, body, look.extras.hung ? 2 * SKIN_REACH_M : SKIN_REACH_M);
  if (on && on.far > OFF_BODY.far) f("on the body", `${(on.far * 100).toFixed(0)}% of its vertices lie over ${SKIN_REACH_M} m from their joint at rest: not made on the base, in its T-pose, at its size`);
  if (on && on.mirrored > OFF_BODY.mirrored) f("mirrored", `${(on.mirrored * 100).toFixed(0)}% of its vertices off the middle are weighted to a joint of the body's other side`);
  if (kind === "pack") {
    const z = look.points.reduce((s, p) => s + p[2], 0) / look.points.length;
    if (z > -PACK_BEHIND_M) f("facing", `its middle is ${(z * 100).toFixed(1)} cm along Z: a pack's bag lies behind the back (-Z), over ${PACK_BEHIND_M * 100} cm`);
  }
  if (!same([...look.targets].sort(), [...body.targets].sort())) f("morphs", `its morphs (${look.targets.join(", ")}) are not the body's (${body.targets.join(", ")})`);
  if ("socket" in look.extras) f("socket", "a skinned look names a socket");
}

function covers(look: Look, f: (rule: string, detail: string) => void) {
  const c = look.extras.covers;
  if (c === undefined) return;
  if (!Array.isArray(c) || !c.every((p, i) => Number.isInteger(p) && p >= 0 && p < PATCHES && (i === 0 || p > c[i - 1])))
    f("covers", `its covers are not ascending patch numbers under ${PATCHES}`);
}

/** Every rule a look breaks for its kind. */
export function judge(look: Look, body: Body, data = dataOf(look.name), hold = holdOf(look.name), root = ROOT): Fault[] {
  const faults: Fault[] = [];
  const f = (rule: string, detail: string) => faults.push({ look: look.name, rule, detail });
  const kind = kindOf(look, data);
  const rules = kind === undefined ? undefined : KINDS[kind];
  if (!kind || !rules)
    return [{ look: look.name, rule: "kind", detail: kind ? `its data's kind ${kind} is none of attach_kinds.json's` : "its data names no kind (\"kind\"), and it is no garment (skinned, with no data)" }];
  const mount = rules.mount;
  const fitted = fittedAs(look, data);
  if (fitted !== mount) f("kind", `a ${kind} goes on ${mount}, but its data has it fitted ${fitted ?? "no way known"}`);
  if (mount === "in hand" || mount === "hung" || mount === "strapped") {
    if (look.joints) f("kind", "a held look is skinned");
    // Carried by the game's rule (terra_walker::character::carry_rule) unless its data gives one.
    if (look.extras.carry_tilt !== undefined && !num(look.extras.carry_tilt)) f("carry_tilt", `its extras' carry_tilt ${look.extras.carry_tilt} is no number`);
    else if (data && data.carry_tilt !== look.extras.carry_tilt) f("marker", `its carry_tilt ${look.extras.carry_tilt ?? "(the rule's)"} is not its data's ${data.carry_tilt ?? "(none)"}`);
    if (mount !== "hung" && look.extras.hold === "hung") f("marker", `it says {"hold": "hung"}, which its data does not`);
  }
  if (mount === "in hand") inHand(look, f);
  if (mount === "hung") hung(look, f);
  if (mount === "strapped") strapped(look, body, data, f);
  if (mount === "rigid") rigid(look, body, f);
  if (mount === "skinned") skinned(look, body, kind, f);
  sized(look, kind, rules, f);
  secondHand(look, kind, rules, data, f);
  covers(look, f);
  const out = insideOut(look);
  if (out !== undefined && out > INSIDE_OUT) f("inside out", `${(out * 100).toFixed(0)}% of its surface faces against its normals: mirrored, or its faces flipped`);
  if (data) madeFrom(look, body, data, root, f);
  if (hold) heldAs(look, kind, rules, hold, data, f);
  return faults;
}

/** Every socket of the table on the body, on its bone, its axes within SOCKET_AXIS_DEG and
 * its scale 1. */
export function judgeSockets(body: Body, name = "character/base"): Fault[] {
  const faults: Fault[] = [];
  const cos = Math.cos((SOCKET_AXIS_DEG * Math.PI) / 180);
  for (const [socket, want] of Object.entries(SOCKETS)) {
    const pose = body.sockets.get(socket);
    const f = (detail: string) => faults.push({ look: name, rule: socket, detail });
    if (!pose) {
      f("missing");
      continue;
    }
    if (body.parent.get(socket) !== want.bone) f(`on ${body.parent.get(socket)}, not ${want.bone}`);
    for (const [axis, unit] of [["x", [1, 0, 0]], ["y", [0, 1, 0]], ["z", [0, 0, 1]]] as const) {
      const got = rot(pose.r, unit as V3);
      const w = want[axis];
      if (got[0] * w[0] + got[1] * w[1] + got[2] * w[2] < cos) f(`its +${axis.toUpperCase()} is ${got.map((v) => v.toFixed(2))}, not ${w}`);
    }
    if (pose.s.some((s) => Math.abs(s - 1) > 1e-3)) f(`its scale is ${pose.s}`);
  }
  return faults;
}

/** Every look in assets/models/held and gear, and every item with data, judged. */
export function audit(root = ROOT): { looks: number; faults: Fault[] } {
  const body = readBody(join(root, "assets", "models", "character", "base.glb"));
  const faults = judgeSockets(body);
  let looks = 0;
  const seen = new Set<string>();
  const items = holds(root);
  for (const dir of ["held", "gear"]) {
    for (const file of readdirSync(join(root, "assets", "models", dir)).filter((f) => f.endsWith(".glb")).sort()) {
      const look = readLook(join(root, "assets", "models", dir, file));
      seen.add(look.name);
      looks++;
      faults.push(...judge(look, body, dataOf(look.name, root), dir === "held" ? items.get(basename(file, ".glb")) : undefined, root));
    }
  }
  for (const json of readdirSync(join(root, "art", "held")).filter((f) => f.endsWith(".json"))) {
    const name = `held/${basename(json, ".json")}`;
    if (!seen.has(name)) faults.push({ look: name, rule: "file", detail: `art/held/${json} has no look, assets/models/${name}.glb` });
  }
  return { looks, faults };
}

/** The faults known and listed, attach_faults.json: { "held/hammer": { "grip": "why" } }. */
export function knownFaults(root = ROOT): Record<string, Record<string, string>> {
  return JSON.parse(readFileSync(join(root, "tools", "characters", "attach_faults.json"), "utf8"));
}

/** Faults found that are not listed, and listed faults no longer found (a fix: remove its line). */
export function against(faults: Fault[], known: Record<string, Record<string, string>>) {
  const found = new Set(faults.map((f) => `${f.look} ${f.rule}`));
  const listed = new Set(Object.entries(known).flatMap(([look, rules]) => Object.keys(rules).map((r) => `${look} ${r}`)));
  return {
    unlisted: faults.filter((f) => !listed.has(`${f.look} ${f.rule}`)),
    fixed: [...listed].filter((l) => !found.has(l)),
  };
}

if (import.meta.main) {
  const files = process.argv.slice(2);
  if (files.length) {
    const body = readBody();
    let bad = 0;
    for (const file of files) {
      const look = readLook(file);
      const faults = judge(look, body);
      bad += faults.length;
      console.log(faults.length ? `✗ ${look.name} (${kindOf(look, dataOf(look.name))})` : `✓ ${look.name} (${kindOf(look, dataOf(look.name))})`);
      for (const f of faults) console.log(`  ${f.rule}: ${f.detail}`);
    }
    process.exit(bad ? 1 : 0);
  }
  const { looks, faults } = audit();
  const { unlisted, fixed } = against(faults, knownFaults());
  const listed = faults.length - unlisted.length;
  if (!unlisted.length && !fixed.length) console.log(`✓ attachment contract: ${looks} looks and the body's sockets, ${listed} known faults listed`);
  for (const f of unlisted) console.log(`✗ ${f.look} ${f.rule}: ${f.detail}`);
  for (const l of fixed) console.log(`✗ ${l}: listed in attach_faults.json but no longer found; remove its line`);
  process.exit(unlisted.length || fixed.length ? 1 : 0);
}
