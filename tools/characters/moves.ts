#!/usr/bin/env node
// The move check (D447, docs/movement.md "Second round"): what a move set's clips get wrong on
// the bodies that play them, measured from the exported models without Blender or the game.
// For every move of the set, and the clips a drawn fighter shows besides, on every body
// template (thin man, woman, heavy, strong):
//
//   hands   how deep a hand, forearm, elbow or upper arm goes into the body (torso, head,
//           thighs), skinned
//   held    how deep a held look (sword, shield) goes into it
//   arc     a blow's tip at its fastest in its active frames: its bearing from ahead (+ left)
//           and how far ahead, which way it sweeps, how far the hips have turned
//   guard   a block's shield at its hold: how far its face is turned from ahead, how far
//           in front of the hips it stands
//   chain   how far a move's last pose is from the next one's first, and each blow's first
//           and last pose from the guard's: hands, forearms, head and blade tip, metres
//
// Bends the game lays over the clips (the prototype's shield arm) are applied as the game does.
// A line a row; ✗ marks what is over its bar; exit 1 if any is.
//
//   node tools/characters/moves.ts
//   node tools/characters/moves.ts [--set assets/data/moves/sword_and_shield.ron] [--fps 15] [--verbose]
//
// Node 22.18 or later; no dependencies.

import { readFileSync } from "node:fs";
import { parseArgs } from "node:util";
import { Glb, type Clip, type Pose, type Q, type V3, dot, len, matrix, mmul, place, posed, qinv, rot, sub, turn } from "./glb.ts";

const { values: opt } = parseArgs({
  options: {
    set: { type: "string", default: "assets/data/moves/sword_and_shield.ron" },
    clips: { type: "string", default: "assets/models/character/fight.glb" },
    body: { type: "string", default: "assets/models/character/base.glb" },
    templates: { type: "string", default: "assets/models/character/templates.json" },
    // What the prototype hangs on the hands (`terra_creatures::melee`, SWORD and SHIELD).
    hold: { type: "string", multiple: true, default: ["socket.hand.R=assets/models/held/arming_sword.glb", "socket.hand.L=assets/models/held/round_shield.glb"] },
    // The prototype's shield arm bend (`terra_creatures::melee`, SHIELD_ARM), degrees.
    bend: { type: "string", multiple: true, default: ["upperarm_l=-30,0,14"] },
    also: { type: "string", default: "Fight_Guard,Fight_Strafe_L,Fight_Strafe_R,Walk_Loop,Idle_Loop" },
    guard: { type: "string", default: "Fight_Guard" },
    fps: { type: "string", default: "15" },
    // Only these body templates, comma-separated; all of them otherwise.
    template: { type: "string", default: "" },
    verbose: { type: "boolean", default: false },
  },
});

// The bars: what a person sees as wrong.
const HAND_CM = 2; // a hand or arm through the skin
const HELD_CM = 1; // a sword or shield through the body
const ARC_DEG = 35; // a blow's contact off ahead
const AHEAD_M = 0.5; // a blow's contact this far in front of the hips at least
const FACE_DEG = 35; // a raised shield's face off ahead
const CHAIN_M = 0.1; // a pose jump between moves that a blend shows

type Move = { name: string; clip: string; from: number; speed: number; done: number; active?: [number, number]; next?: string; hold?: number };

function readSet(file: string): Move[] {
  const text = readFileSync(file, "utf8").replace(/\/\/.*$/gm, "");
  const moves: Move[] = [];
  for (const [, name, body] of text.matchAll(/"(\w+)":\s*\(([\s\S]*?)\),\s*(?="\w+":|\})/g)) {
    const num = (field: string) => {
      const m = body.match(new RegExp(`\\b${field}:\\s*(-?[\\d.]+)`));
      return m ? Number(m[1]) : undefined;
    };
    const active = body.match(/\bactive:\s*\((-?[\d.]+),\s*(-?[\d.]+)\)/);
    moves.push({
      name,
      clip: body.match(/\bclip:\s*"([^"]+)"/)![1],
      from: num("from") ?? 0,
      speed: num("speed") ?? 1,
      done: num("done")!,
      active: active ? [Number(active[1]), Number(active[2])] : undefined,
      next: body.match(/\bnext:\s*"([^"]+)"/)?.[1],
      hold: num("hold"),
    });
  }
  return moves;
}

const clipsFile = new Glb(opt.clips!);
const body = new Glb(opt.body!);
const moves = readSet(opt.set!);
const fps = Number(opt.fps);
const bends = new Map<string, Q>(
  opt.bend!.map((b) => {
    const [bone, deg] = b.split("=");
    return [bone, turn(deg.split(",").map(Number) as V3)];
  }),
);
const clips = new Map<string, Clip>();
const clipOf = (name: string): Clip => {
  if (!clips.has(name)) {
    const c = clipsFile.clip(name);
    if (!c) throw new Error(`no clip ${name} in ${opt.clips}`);
    clips.set(name, c);
  }
  return clips.get(name)!;
};

// The body: its skin, which of its vertices are torso, head or thigh (by their heaviest joint),
// and each template's morph mix and joint offsets.
const skin = body.gltf.skins![0];
const prim = body.gltf.meshes![0].primitives[0];
const positions = body.read(prim.attributes.POSITION);
const normals = body.read(prim.attributes.NORMAL);
const joints = body.read(prim.attributes.JOINTS_0);
const weights = body.read(prim.attributes.WEIGHTS_0);
const ibm = body.read(skin.inverseBindMatrices);
const targetNames = body.gltf.meshes![0].extras?.targetNames ?? [];
const TARGET = /^(pelvis|spine_0\d|neck_01|Head|thigh_[lr]|thigh_twist_01_[lr])$/;
const hit: number[] = [];
const partOf: string[] = [];
for (let v = 0; v < positions.length / 3; v++) {
  let best = 0;
  for (let k = 1; k < 4; k++) if (weights[v * 4 + k] > weights[v * 4 + best]) best = k;
  const name = body.gltf.nodes[skin.joints[joints[v * 4 + best]]].name ?? "";
  if (TARGET.test(name)) {
    hit.push(v);
    partOf[v] = name.replace(/_0\d|_twist_01/g, "");
  }
}
const rigNode = body.gltf.nodes.find((n) => (n as { extras?: { morphs?: string } }).extras?.morphs) as { extras: { morphs: string } } | undefined;
const offsets: Record<string, Record<string, V3>> = rigNode ? JSON.parse(rigNode.extras.morphs) : {};
const templates: Record<string, { morphs: Record<string, number> }> = JSON.parse(readFileSync(opt.templates!, "utf8"));

type Shape = { name: string; positions: Float64Array; moved: Map<number, V3>; shift: Map<number, V3> };
const only = opt.template!.split(",").filter(Boolean);
const shapes: Shape[] = Object.entries(templates).filter(([name]) => only.length === 0 || only.includes(name)).map(([name, t]) => {
  const pos = Float64Array.from(positions);
  for (const [morph, w] of Object.entries(t.morphs)) {
    const k = targetNames.indexOf(morph);
    if (k < 0 || !prim.targets) continue;
    const d = body.read(prim.targets[k].POSITION);
    for (let i = 0; i < pos.length; i++) pos[i] += w * d[i];
  }
  // A morph moves joints in the model's space (`terra_walker::appearance`): the joint's rest
  // translation in its parent's frame takes the difference from its parent's move.
  const shift = new Map<number, V3>();
  body.gltf.nodes.forEach((n, i) => {
    const by: V3 = [0, 0, 0];
    for (const [morph, w] of Object.entries(t.morphs)) {
      const d = n.name ? offsets[morph]?.[n.name] : undefined;
      if (d) for (let j = 0; j < 3; j++) by[j] += w * d[j];
    }
    shift.set(i, by);
  });
  // As the game does: the move less the parent's, in the parent's rest frame (its rotation
  // and scale), added to the joint's translation, the clip's where it sets one.
  const rest = posed(body, undefined, 0);
  const moved = new Map<number, V3>();
  body.gltf.nodes.forEach((_, i) => {
    const p = body.parentOf[i];
    const d = sub(shift.get(i)!, p === undefined ? [0, 0, 0] : shift.get(p)!);
    if (len(d) === 0) return;
    const local = p === undefined ? d : rot(qinv(rest[p].r), d).map((x, j) => x / rest[p].s[j]);
    moved.set(i, local as V3);
  });
  return { name, positions: pos, moved, shift };
});

function poseOf(shape: Shape, clip: Clip, t: number): Pose[] {
  // A morph's joints moved over the clip (`terra_walker::appearance`).
  const keep = { ...clip, tracks: new Map(clip.tracks) };
  for (const [node, by] of shape.moved) {
    const name = body.gltf.nodes[node].name;
    if (!name) continue;
    const track = keep.tracks.get(name);
    const rest = body.rest(node).t;
    const from = track?.t ?? (() => rest);
    keep.tracks.set(name, { ...track, t: (time: number) => from(time).map((x, j) => x + by[j]) });
  }
  return posed(body, keep, t, bends);
}

// Held looks: a few hundred of their vertices in their socket's frame, and the blade's tip.
type Held = { socket: string; name: string; points: V3[]; tip: V3; thin: V3; centre: V3 };
const held: Held[] = opt.hold!.map((h) => {
  const [socket, file] = h.split("=");
  const g = new Glb(file);
  const node = g.gltf.nodes.findIndex((n) => n.mesh !== undefined);
  const pose = g.rest(node);
  const p = g.read(g.gltf.meshes![g.gltf.nodes[node].mesh!].primitives[0].attributes.POSITION);
  const all: V3[] = [];
  for (let i = 0; i < p.length; i += 3) all.push(place(pose, [p[i], p[i + 1], p[i + 2]]));
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
  return { socket, name: file.split("/").at(-1)!.replace(".glb", ""), points, tip, thin, centre: lo.map((x, j) => (x + hi[j]) / 2) as V3 };
});

// The skinned torso, head and thighs at a pose, gridded for the nearest-vertex search.
const CELL = 0.1;
function skinned(shape: Shape, pose: Pose[]) {
  const mats = skin.joints.map((j, k) => {
    const s = shape.shift.get(j)!;
    const unbind = mmul(ibm.subarray(k * 16, k * 16 + 16) as unknown as number[], matrix({ t: [-s[0], -s[1], -s[2]], r: [0, 0, 0, 1], s: [1, 1, 1] }));
    return mmul(matrix(pose[j]), unbind);
  });
  const grid = new Map<string, { p: V3; n: V3; part: string }[]>();
  for (const v of hit) {
    const p: V3 = [0, 0, 0];
    const n: V3 = [0, 0, 0];
    for (let k = 0; k < 4; k++) {
      const w = weights[v * 4 + k];
      if (w === 0) continue;
      const m = mats[joints[v * 4 + k]];
      const [x, y, z] = [shape.positions[v * 3], shape.positions[v * 3 + 1], shape.positions[v * 3 + 2]];
      const [a, b, c] = [normals[v * 3], normals[v * 3 + 1], normals[v * 3 + 2]];
      for (let j = 0; j < 3; j++) {
        p[j] += w * (m[j] * x + m[4 + j] * y + m[8 + j] * z + m[12 + j]);
        n[j] += w * (m[j] * a + m[4 + j] * b + m[8 + j] * c);
      }
    }
    const l = len(n) || 1;
    const key = p.map((x) => Math.floor(x / CELL)).join(",");
    if (!grid.has(key)) grid.set(key, []);
    grid.get(key)!.push({ p, n: [n[0] / l, n[1] / l, n[2] / l], part: partOf[v] });
  }
  // How deep a point is inside the skin (positive inside), by its nearest vertex's normal.
  return (q: V3): { depth: number; part: string } => {
    const [cx, cy, cz] = q.map((x) => Math.floor(x / CELL));
    let best: { p: V3; n: V3; part: string } | undefined;
    let bestD = Infinity;
    for (let i = -2; i <= 2; i++)
      for (let j = -2; j <= 2; j++)
        for (let k = -2; k <= 2; k++)
          for (const v of grid.get(`${cx + i},${cy + j},${cz + k}`) ?? []) {
            const d = len(sub(v.p, q));
            if (d < bestD) (bestD = d), (best = v);
          }
    return best ? { depth: dot(sub(best.p, q), best.n), part: best.part } : { depth: -1, part: "" };
  };
}

const at = (name: string) => {
  const i = body.named(name);
  if (i < 0) throw new Error(`no ${name} in ${opt.body}`);
  return i;
};
const node = Object.fromEntries(
  ["pelvis", "Head", "thigh_l", "thigh_r", ...["l", "r"].flatMap((s) => [`hand_${s}`, `lowerarm_${s}`, `upperarm_${s}`, `middle_01_${s}`, `middle_02_${s}`, `index_02_${s}`, `thumb_02_${s}`])].map((n) => [n, at(n)]),
);
const mid = (a: V3, b: V3): V3 => [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2];

type Worst = { cm: number; where: string };
const worse = (w: Worst, cm: number, where: string) => (cm > w.cm ? { cm, where } : w);

// Hands, forearms and held looks against the body, over a span of a clip, on every template.
function penetration(clip: Clip, from: number, to: number) {
  let hands: Worst = { cm: -Infinity, where: "" };
  let heldW: Worst = { cm: -Infinity, where: "" };
  for (const shape of shapes) {
    for (let t = from; t <= to + 1e-6; t += 1 / fps) {
      const pose = poseOf(shape, clip, t);
      const inside = skinned(shape, pose);
      const P = (n: string) => pose[node[n]].t;
      for (const s of ["l", "r"]) {
        const pts: [V3, number, string][] = [
          [mid(P(`hand_${s}`), P(`middle_01_${s}`)), 0.025, `hand_${s}`],
          [P(`middle_02_${s}`), 0.012, `hand_${s}`],
          [P(`index_02_${s}`), 0.012, `hand_${s}`],
          [P(`thumb_02_${s}`), 0.012, `hand_${s}`],
          [mid(P(`lowerarm_${s}`), P(`hand_${s}`)), 0.035, `forearm_${s}`],
          [P(`lowerarm_${s}`), 0.04, `elbow_${s}`],
          [mid(P(`upperarm_${s}`), P(`lowerarm_${s}`)), 0.045, `upper arm_${s}`],
        ];
        for (const [q, pad, what] of pts) {
          const { depth, part } = inside(q);
          hands = worse(hands, (depth - pad) * 100, `${what} in ${part} at ${t.toFixed(2)} s, ${shape.name}`);
        }
      }
      for (const h of held) {
        const socket = pose[at(h.socket)];
        for (const q of h.points) {
          const { depth, part } = inside(place(socket, q));
          heldW = worse(heldW, depth * 100, `${h.name} in ${part} at ${t.toFixed(2)} s, ${shape.name}`);
        }
      }
    }
  }
  return { hands, held: heldW };
}

// Points that say where a pose is, relative to the hips: hands, forearms, head, blade tip.
function marks(clip: Clip, t: number): V3[] {
  const pose = poseOf(shapes[0], clip, t);
  const hips = pose[node.pelvis].t;
  const sword = held.find((h) => h.socket === "socket.hand.R");
  const pts = ["hand_l", "hand_r", "lowerarm_l", "lowerarm_r", "Head"].map((n) => pose[node[n]].t);
  if (sword) pts.push(place(pose[at(sword.socket)], sword.tip));
  return pts.map((p) => sub(p, hips));
}
const gap = (a: V3[], b: V3[]) => Math.max(...a.map((p, i) => len(sub(p, b[i]))));

// A blow's tip in its active frames, in the model's space from the hips at the move's start.
function arc(move: Move) {
  const clip = clipOf(move.clip);
  const sword = held.find((h) => h.socket === "socket.hand.R")!;
  const start = poseOf(shapes[0], clip, move.from)[node.pelvis].t;
  let fastest = { speed: -1, tip: [0, 0, 0] as V3, t: 0, hips: 0 };
  let first: V3 | undefined;
  let last: V3 = [0, 0, 0];
  let prev: V3 | undefined;
  const [a, b] = move.active!;
  for (let t = a - 1 / fps; t <= b + 1e-6; t += 1 / 120) {
    const pose = poseOf(shapes[0], clip, t);
    const tip = sub(place(pose[at(sword.socket)], sword.tip), start);
    if (prev && t >= a) {
      const speed = len(sub(tip, prev)) * 120;
      const across = sub(pose[node.thigh_l].t, pose[node.thigh_r].t);
      const hips = (Math.atan2(-across[2], across[0]) * 180) / Math.PI;
      if (speed > fastest.speed) fastest = { speed, tip, t, hips };
      first ??= tip;
      last = tip;
    }
    prev = tip;
  }
  const bearing = (Math.atan2(fastest.tip[0], fastest.tip[2]) * 180) / Math.PI;
  const sweep = first && last[0] > first[0] ? "to the left" : "to the right";
  return { bearing, ahead: fastest.tip[2], up: fastest.tip[1], sweep, hips: fastest.hips, speed: fastest.speed, t: fastest.t };
}

function guard(move: Move) {
  const shield = held.find((h) => h.socket === "socket.hand.L");
  if (!shield || move.hold === undefined) return undefined;
  const pose = poseOf(shapes[0], clipOf(move.clip), move.hold);
  const socket = pose[at(shield.socket)];
  const face = rot(socket.r, shield.thin);
  const off = (Math.acos(Math.min(1, Math.abs(face[2]) / len(face))) * 180) / Math.PI;
  const centre = sub(place(socket, shield.centre), pose[node.pelvis].t);
  const head = sub(pose[node.Head].t, pose[node.pelvis].t);
  return { off, ahead: centre[2], aboveHead: centre[1] - head[1] };
}

const cm = (x: number) => `${x.toFixed(1)} cm`;
const failures: string[] = [];
const row = (what: string, parts: string[], bad: string[]) => {
  console.log(`${bad.length ? "✗" : "✓"} ${what}: ${parts.join("; ")}`);
  for (const b of bad) failures.push(`${what}: ${b}`);
};
const guardClip = clipOf(opt.guard!);
const guardMarks = marks(guardClip, 0);
const byName = new Map(moves.map((m) => [m.name, m]));

for (const move of moves) {
  const clip = clipOf(move.clip);
  const parts: string[] = [];
  const bad: string[] = [];
  const pen = penetration(clip, move.from, move.done);
  parts.push(`hands ${cm(pen.hands.cm)}`, `held ${cm(pen.held.cm)}`);
  if (pen.hands.cm > HAND_CM) bad.push(`hands ${cm(pen.hands.cm)} deep, ${pen.hands.where}`);
  if (pen.held.cm > HELD_CM) bad.push(`held ${cm(pen.held.cm)} deep, ${pen.held.where}`);
  if (move.active) {
    const a = arc(move);
    parts.push(`contact ${a.bearing.toFixed(0)}° ${a.bearing > 0 ? "left" : "right"} of ahead, ${a.ahead.toFixed(2)} m ahead, sweeping ${a.sweep}, hips turned ${a.hips.toFixed(0)}°`);
    if (Math.abs(a.bearing) > ARC_DEG || a.ahead < AHEAD_M) bad.push(`contact ${Math.abs(a.bearing).toFixed(0)}° ${a.bearing > 0 ? "left" : "right"} of ahead and ${a.ahead.toFixed(2)} m ahead at ${a.t.toFixed(2)} s (bar ${ARC_DEG}°, ${AHEAD_M} m)`);
    const startGap = gap(marks(clip, move.from), guardMarks);
    const endGap = gap(marks(clip, move.done), guardMarks);
    parts.push(`from guard ${startGap.toFixed(2)} m, back to guard ${endGap.toFixed(2)} m`);
    if (startGap > CHAIN_M) bad.push(`starts ${startGap.toFixed(2)} m from the guard pose`);
    if (endGap > CHAIN_M) bad.push(`ends ${endGap.toFixed(2)} m from the guard pose`);
  }
  if (move.next) {
    const next = byName.get(move.next);
    if (next) {
      const g = gap(marks(clip, move.done), marks(clipOf(next.clip), next.from));
      parts.push(`into ${move.next} ${g.toFixed(2)} m`);
      if (g > CHAIN_M) bad.push(`its last pose is ${g.toFixed(2)} m from ${move.next}'s first`);
      if (next.clip === move.clip && next.from === move.from) bad.push(`chains into ${move.next}, the same clip from the same place`);
    }
  }
  const g = guard(move);
  if (g) {
    parts.push(`shield face ${g.off.toFixed(0)}° off ahead, ${g.ahead.toFixed(2)} m ahead, ${g.aboveHead.toFixed(2)} m above the head`);
    if (g.off > FACE_DEG) bad.push(`shield face ${g.off.toFixed(0)}° off ahead (bar ${FACE_DEG}°)`);
  }
  row(`${move.name} (${move.clip})`, parts, bad);
  if (opt.verbose) for (const b of bad) console.log(`    ${b}`);
}
for (const name of opt.also!.split(",").filter(Boolean)) {
  const clip = clipOf(name);
  const pen = penetration(clip, 0, clip.end);
  const bad: string[] = [];
  if (pen.hands.cm > HAND_CM) bad.push(`hands ${cm(pen.hands.cm)} deep, ${pen.hands.where}`);
  if (pen.held.cm > HELD_CM) bad.push(`held ${cm(pen.held.cm)} deep, ${pen.held.where}`);
  row(name, [`hands ${cm(pen.hands.cm)}`, `held ${cm(pen.held.cm)}`], bad);
  if (opt.verbose) for (const b of bad) console.log(`    ${b}`);
}
if (failures.length) {
  console.log(`✗ ${failures.length} over the bars:`);
  for (const f of failures) console.log(`  ${f}`);
  process.exit(1);
}
console.log("✓ every move within the bars");
