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
};

/** What the check reads of the body: its node names, sockets at rest, and morphs. */
export type Body = { nodes: Set<string>; parent: Map<string, string>; sockets: Map<string, Pose>; targets: string[] };

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
  nodes.forEach((n, i) => {
    if (n.mesh === undefined) return;
    Object.assign(extras, n.extras ?? {});
    const mesh = g.gltf.meshes![n.mesh] as { primitives: Prim[]; extras?: { targetNames?: string[] } };
    for (const t of mesh.extras?.targetNames ?? []) if (!targets.includes(t)) targets.push(t);
    if (n.skin !== undefined) joints = g.gltf.skins![n.skin].joints.map((j) => nodes[j].name ?? "");
    for (const p of mesh.primitives) {
      if ((p.mode ?? 4) !== 4) continue;
      const at = points.length;
      const a = g.read(p.attributes.POSITION);
      for (let k = 0; k < a.length; k += 3) {
        const v: V3 = [a[k], a[k + 1], a[k + 2]];
        points.push(n.skin === undefined ? place(world[i], v) : v);
      }
      const idx = p.indices === undefined ? Array.from({ length: a.length / 3 }, (_, k) => k) : g.read(p.indices);
      for (const k of idx) tris.push(at + k);
    }
  });
  return { name, extras, joints, targets, points, tris };
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
  nodes.forEach((n, i) => {
    if (n.name && g.parentOf[i] !== undefined) parent.set(n.name, nodes[g.parentOf[i]].name ?? "");
    if (n.name?.startsWith("socket.")) sockets.set(n.name, world[i]);
  });
  const targets = (g.gltf.meshes ?? []).flatMap((m) => m.extras?.targetNames ?? []);
  return { nodes: new Set(nodes.map((n) => n.name ?? "")), parent, sockets, targets: [...new Set(targets)] };
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
  if (fit === "back" || fit === "torso") return "skinned";
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

function hung(look: Look, f: (rule: string, detail: string) => void) {
  const { lo, hi } = box(look.points);
  if (look.extras.hold !== "hung") f("marker", `its extras do not say {"hold": "hung"}, which its data's "fit" does`);
  if (hi[2] > 0.05) f("hangs", `it stands ${(hi[2] * 100).toFixed(1)} cm above its handle's top (+Z), over 5 cm`);
  if (lo[2] > -0.1) f("hangs", `it hangs only ${(-lo[2] * 100).toFixed(1)} cm below its handle (-Z)`);
  if (hi[1] - lo[1] < hi[0] - lo[0]) f("hangs", "its long way lies along X, not +Y (forward)");
}

function strapped(look: Look, body: Body, data: Record<string, unknown> | undefined, f: (rule: string, detail: string) => void) {
  const strap = look.extras.strap as { bone?: string; at?: unknown; face?: unknown; grip?: unknown } | undefined;
  if (!strap) return f("marker", "its extras have no strap, which its data gives");
  if (data && !same(strap, data.strap)) f("marker", "its extras' strap differs from its data's");
  if (!strap.bone || !body.nodes.has(strap.bone)) f("strap", `its strap's bone ${strap.bone} is not a bone of the body`);
  if (!isV3(strap.at) || !isV3(strap.face) || !isV3(strap.grip)) return f("strap", "its strap's at, face and grip are not each [x, y, z]");
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

function skinned(look: Look, body: Body, f: (rule: string, detail: string) => void) {
  if (!look.joints) return f("skin", "it has no skin");
  const strange = look.joints.filter((j) => !body.nodes.has(j));
  if (strange.length) f("skin", `joints not the body's: ${strange.slice(0, 5).join(", ")}`);
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
export function judge(look: Look, body: Body, data = dataOf(look.name)): Fault[] {
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
    if (!num(look.extras.carry_tilt)) f("carry_tilt", "its extras have no carry_tilt");
    else if (data && data.carry_tilt !== look.extras.carry_tilt) f("marker", `its carry_tilt ${look.extras.carry_tilt} is not its data's ${data.carry_tilt}`);
    if (mount !== "hung" && look.extras.hold === "hung") f("marker", `it says {"hold": "hung"}, which its data does not`);
  }
  if (mount === "in hand") inHand(look, f);
  if (mount === "hung") hung(look, f);
  if (mount === "strapped") strapped(look, body, data, f);
  if (mount === "rigid") rigid(look, body, f);
  if (mount === "skinned") skinned(look, body, f);
  sized(look, kind, rules, f);
  secondHand(look, kind, rules, data, f);
  covers(look, f);
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
  for (const dir of ["held", "gear"]) {
    for (const file of readdirSync(join(root, "assets", "models", dir)).filter((f) => f.endsWith(".glb")).sort()) {
      const look = readLook(join(root, "assets", "models", dir, file));
      seen.add(look.name);
      looks++;
      faults.push(...judge(look, body, dataOf(look.name, root)));
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
