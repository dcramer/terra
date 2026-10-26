// The body's skin for the move check (moves.ts) and the composed-pose checks (tools/sandbox/duel/pose.ts):
// each body template's shape (its morph mix and joint offsets) and, at a pose, how deep a point is
// inside the torso, head or thighs, skinned. Node 22.18 or later; no dependencies.

import { readFileSync } from "node:fs";
import { type Glb, type Pose, type V3, dot, len, matrix, mmul, posed, qinv, rot, sub } from "./glb.ts";

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
  return { shapes, skinned };
}
