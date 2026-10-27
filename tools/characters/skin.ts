// The body's skin for the move check (moves.ts) and the composed-pose checks (tools/sandbox/duel/pose.ts):
// each body template's shape (its morph mix and joint offsets) and, at a pose, how deep a point is
// inside the torso, head or thighs, skinned. Node 22.18 or later; no dependencies.

import { readFileSync } from "node:fs";
import { type Clip, type Glb, type Pose, type Q, type V3, len, matrix, mmul, posed, qinv, rot, sub } from "./glb.ts";
import { surface, topology } from "./surface.ts";

export type Shape = { name: string; positions: Float64Array; moved: Map<number, V3>; shift: Map<number, V3> };
export type Inside = (q: V3) => { depth: number; part: string };

const mid = (a: V3, b: V3): V3 => [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2];

/** Points along one arm (`l` or `r`) at a pose, each with its half-thickness (m) and its name,
 * for how deep the arm goes into the body (the move check's `hands`, the composed pose's arms):
 * the palm, three fingertips' middles, the forearm, the elbow and the upper arm. `P` gives a
 * bone's place by name. */
export function armPoints(P: (bone: string) => V3, s: string): [V3, number, string][] {
  return [
    [mid(P(`hand_${s}`), P(`middle_01_${s}`)), 0.025, `hand_${s}`],
    [P(`middle_02_${s}`), 0.012, `hand_${s}`],
    [P(`index_02_${s}`), 0.012, `hand_${s}`],
    [P(`thumb_02_${s}`), 0.012, `hand_${s}`],
    [mid(P(`lowerarm_${s}`), P(`hand_${s}`)), 0.035, `forearm_${s}`],
    [P(`lowerarm_${s}`), 0.04, `elbow_${s}`],
    [mid(P(`upperarm_${s}`), P(`lowerarm_${s}`)), 0.045, `upper arm_${s}`],
  ];
}

/** The body's templates (all, or `only` these) and its skin at a pose. */
export function skinOf(body: Glb, templatesFile: string, only: string[] = []): { shapes: Shape[]; skinned: (shape: Shape, pose: Pose[]) => Inside } {
  // The body: its skin, which of its vertices are torso, head or thigh (by their heaviest joint),
  // and each template's morph mix and joint offsets.
  const skin = body.gltf.skins![0];
  const prim = body.gltf.meshes![0].primitives[0];
  const positions = body.read(prim.attributes.POSITION);
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
  const templates: Record<string, { morphs: Record<string, number> }> = JSON.parse(readFileSync(templatesFile, "utf8"));

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

  // The torso, head and thighs as a surface: the triangles whose corners are all theirs, a
  // vertex split at a seam (its UVs) welded back into one corner, so the surface is whole.
  const corner = new Map<string, number>();
  const cornerOf = new Int32Array(positions.length / 3).fill(-1);
  const first: number[] = [];
  for (const v of hit) {
    const key = [0, 1, 2].map((j) => Math.round(positions[v * 3 + j] * 1e5)).join(",");
    if (!corner.has(key)) corner.set(key, first.push(v) - 1);
    cornerOf[v] = corner.get(key)!;
  }
  const indices = body.read((prim as { indices?: number }).indices!);
  const tris: number[] = [];
  for (let t = 0; t < indices.length; t += 3) {
    const c = [cornerOf[indices[t]], cornerOf[indices[t + 1]], cornerOf[indices[t + 2]]];
    if (c.every((x) => x >= 0) && c[0] !== c[1] && c[1] !== c[2] && c[2] !== c[0]) tris.push(...c);
  }
  const top = topology(tris, first.map((v) => partOf[v]));

  // The skinned torso, head and thighs at a pose, and how deep a point is inside them (positive
  // inside): its distance to the nearest point of that skin, signed by its winding number
  // (surface.ts). A nearest vertex's normal gave false depths where the skin cuts through itself:
  // the female template's thumb in Walk_Loop read -6.4 then +8.8 cm as it moved 3 cm out of the hip.
  function skinned(shape: Shape, pose: Pose[]) {
    const mats = skin.joints.map((j, k) => {
      const s = shape.shift.get(j)!;
      const unbind = mmul(ibm.subarray(k * 16, k * 16 + 16) as unknown as number[], matrix({ t: [-s[0], -s[1], -s[2]], r: [0, 0, 0, 1], s: [1, 1, 1] }));
      return mmul(matrix(pose[j]), unbind);
    });
    const at = new Float64Array(first.length * 3);
    first.forEach((v, i) => {
      const [x, y, z] = [shape.positions[v * 3], shape.positions[v * 3 + 1], shape.positions[v * 3 + 2]];
      for (let k = 0; k < 4; k++) {
        const w = weights[v * 4 + k];
        if (w === 0) continue;
        const m = mats[joints[v * 4 + k]];
        for (let j = 0; j < 3; j++) at[i * 3 + j] += w * (m[j] * x + m[4 + j] * y + m[8 + j] * z + m[12 + j]);
      }
    });
    return surface(top, at);
  }
  return { shapes, skinned };
}

/** The body posed by `clip` at `t` as a template shapes it: its morph's joints moved over the
 * clip (`terra_walker::appearance`). */
export function shaped(body: Glb, shape: Shape, clip: Clip, t: number, bends: Map<string, Q> = new Map()): Pose[] {
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

/** How deep an arm goes into the body over a clip, on every template (the move check's
 * `hands`, for any clip): the worst depth past the arm's half-thickness (cm) and where, and the
 * share of poses (a template at a time) with an arm more than `bar` cm in. */
export function armsInBody(body: Glb, templatesFile: string, clip: Clip, bar = 2, step = 1 / 15): { cm: number; where: string; over: number } {
  const { shapes, skinned } = skinOf(body, templatesFile);
  const node = new Map(body.gltf.nodes.map((n, i) => [n.name, i]));
  let worst = { cm: -Infinity, where: "" };
  let over = 0;
  let poses = 0;
  for (const shape of shapes) {
    for (let t = 0; t <= clip.end + 1e-6; t += step) {
      const pose = shaped(body, shape, clip, t);
      const inside = skinned(shape, pose);
      const P = (n: string) => pose[node.get(n)!].t;
      let most = -Infinity;
      for (const s of ["l", "r"]) {
        for (const [q, pad, what] of armPoints(P, s)) {
          const { depth, part } = inside(q);
          const cm = (depth - pad) * 100;
          most = Math.max(most, cm);
          if (cm > worst.cm) worst = { cm, where: `${what} in ${part} at ${t.toFixed(2)} s, ${shape.name}` };
        }
      }
      poses++;
      if (most > bar) over++;
    }
  }
  return { ...worst, over: over / poses };
}
