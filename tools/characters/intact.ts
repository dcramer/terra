#!/usr/bin/env node
// The intact-parts check (D599): every piece of Quaternius's in an output file is their piece
// as made, vertex for vertex and weight for weight. The developers: "their parts stay as made"
// (meshes, UVs, normals, weights, the split into pieces, the bones); our tools add paint and
// pieces of our own, never change theirs.
//
//   node tools/characters/intact.ts OUTPUT.glb|.gltf ... [--source FILE.gltf|DIR]...
//
// A piece is a node with a mesh. Each is matched to theirs by node or mesh name (Blender's
// ".001" suffixes aside) among the files under art/characters/quaternius/ (or `--source`),
// then compared without regard to vertex order or how it is split into primitives, since a
// re-export may reorder both: every vertex must have one of theirs with the same position,
// normal, UVs (each set theirs has) and weights by joint name, to float tolerance; the
// triangles must be theirs, every one, with their winding; and each joint it is weighted to
// must keep its bind matrix. A piece whose every vertex, triangle and bind is theirs but which
// leaves some of their triangles out is an exact cut of theirs, named as one with how many it
// keeps (their UpperBody is such a cut of their FullBody; the D600 trial cuts bare slot pieces
// so). A mesh with no name of theirs is ours, unless most of its vertices sit on one of their
// pieces without being an exact cut (theirs renamed, merged or changed), which fails. Materials, textures,
// vertex colours and shape keys are not compared: paint and build morphs are ours to add.
//
// Node 22.18 or later; no dependencies.

import { readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { Glb } from "./glb.ts";

/** How far a value may move and still be theirs: float noise through an export, far below
 *  anything an eye or a skin could tell. A Blender 5.2 import and export of their Ranger
 *  bodies and the whole woman moved positions 0.5 µm at most, normals 0.19° (one vertex; the
 *  rest under 0.05°), bind matrices 2e-6, weights under 5e-4, UVs not at all; and with its
 *  default of four influences it dropped up to 0.075 of a vertex's weight from their
 *  eight-influence pieces (the woman's Ranger body and legs), which this fails, as it should:
 *  export with `export_all_influences`. */
export type Tolerance = { position: number; normal: number; uv: number; weight: number; bind: number };
export const TOLERANCE: Tolerance = { position: 1e-5, normal: 0.5, uv: 1e-5, weight: 1e-3, bind: 1e-4 };

/** Where their files are. */
export const THEIRS = fileURLToPath(new URL("../../art/characters/quaternius/", import.meta.url));

/** One mesh's vertices and triangles, its primitives laid end to end. */
export type Piece = {
  file: string;
  node: string;
  mesh: string;
  count: number;
  pos: Float64Array;
  nrm?: Float64Array;
  uvs: Float64Array[];
  weights: Map<string, number>[];
  tris: Uint32Array;
  bind: Map<string, number[]>;
  canon?: Int32Array;
};

type Gltf = {
  nodes: { name?: string; mesh?: number; skin?: number }[];
  meshes?: { name?: string; primitives: { attributes: Record<string, number>; indices?: number; mode?: number }[] }[];
  skins?: { joints: number[]; inverseBindMatrices?: number }[];
};

/** Every piece of a file. */
export function pieces(glb: Glb, file = glb.file): Piece[] {
  const g = glb.gltf as unknown as Gltf;
  return g.nodes.flatMap((node, ni) => {
    if (node.mesh === undefined || !g.meshes) return [];
    const mesh = g.meshes[node.mesh];
    const skin = node.skin === undefined ? undefined : g.skins?.[node.skin];
    const joint = (j: number) => (skin && g.nodes[skin.joints[j]]?.name) || `joint ${j}`;
    const bind = new Map<string, number[]>();
    if (skin?.inverseBindMatrices !== undefined) {
      const m = glb.read(skin.inverseBindMatrices);
      skin.joints.forEach((_, j) => bind.set(joint(j), Array.from(m.subarray(j * 16, j * 16 + 16))));
    }
    const sets = Math.max(0, ...mesh.primitives.map((p) => Object.keys(p.attributes).filter((k) => k.startsWith("TEXCOORD_")).length));
    const pos: number[] = [];
    const nrm: number[] = [];
    const uvs: number[][] = Array.from({ length: sets }, () => []);
    const weights: Map<string, number>[] = [];
    const tris: number[] = [];
    let normals = true;
    for (const p of mesh.primitives) {
      if ((p.mode ?? 4) !== 4) throw new Error(`${file}: mesh ${mesh.name} has a primitive of mode ${p.mode}; only triangle lists are compared`);
      const base = pos.length / 3;
      const P = glb.read(p.attributes.POSITION);
      const n = P.length / 3;
      for (const x of P) pos.push(x);
      if (p.attributes.NORMAL === undefined) normals = false;
      else for (const x of glb.read(p.attributes.NORMAL)) nrm.push(x);
      for (let k = 0; k < sets; k++) {
        const at = p.attributes[`TEXCOORD_${k}`];
        if (at === undefined) for (let i = 0; i < 2 * n; i++) uvs[k].push(Number.NaN);
        else for (const x of glb.read(at)) uvs[k].push(x);
      }
      const w = Array.from({ length: n }, () => new Map<string, number>());
      for (let k = 0; p.attributes[`JOINTS_${k}`] !== undefined; k++) {
        const J = glb.read(p.attributes[`JOINTS_${k}`]);
        const W = glb.read(p.attributes[`WEIGHTS_${k}`]);
        for (let i = 0; i < J.length; i++) if (W[i] > 0) w[i >> 2].set(joint(J[i]), (w[i >> 2].get(joint(J[i])) ?? 0) + W[i]);
      }
      weights.push(...w);
      const I = p.indices === undefined ? Array.from({ length: n }, (_, i) => i) : glb.read(p.indices);
      for (const i of I) tris.push(base + i);
    }
    return [
      {
        file,
        node: node.name ?? `node ${ni}`,
        mesh: mesh.name ?? `mesh ${node.mesh}`,
        count: pos.length / 3,
        pos: Float64Array.from(pos),
        nrm: normals ? Float64Array.from(nrm) : undefined,
        uvs: uvs.map((u) => Float64Array.from(u)),
        weights,
        tris: Uint32Array.from(tris),
        bind,
      },
    ];
  });
}

/** The vertices of a piece by position, in cells as wide as the tolerance, so those within it
 *  of a point are in the 27 cells round it. */
class Grid {
  private readonly cells = new Map<string, number[]>();
  private readonly p: Piece;
  private readonly cell: number;
  constructor(p: Piece, cell: number) {
    this.p = p;
    this.cell = cell;
    for (let i = 0; i < p.count; i++) {
      const k = this.key(p.pos[3 * i], p.pos[3 * i + 1], p.pos[3 * i + 2]);
      this.cells.get(k)?.push(i) ?? this.cells.set(k, [i]);
    }
  }
  private key(x: number, y: number, z: number, dx = 0, dy = 0, dz = 0) {
    return `${Math.floor(x / this.cell) + dx},${Math.floor(y / this.cell) + dy},${Math.floor(z / this.cell) + dz}`;
  }
  /** The vertices within `r` of the point, by index. */
  near(x: number, y: number, z: number, r: number): number[] {
    const out: number[] = [];
    for (let dx = -1; dx <= 1; dx++)
      for (let dy = -1; dy <= 1; dy++)
        for (let dz = -1; dz <= 1; dz++)
          for (const i of this.cells.get(this.key(x, y, z, dx, dy, dz)) ?? [])
            if (Math.hypot(this.p.pos[3 * i] - x, this.p.pos[3 * i + 1] - y, this.p.pos[3 * i + 2] - z) <= r) out.push(i);
    return out.sort((a, b) => a - b);
  }
}

type Diff = { position: number; normal: number; uv: number[]; weight: number; joint: string };

/** How far vertex `i` of `a` is from vertex `s` of `b` in each attribute `b` has. */
function diff(a: Piece, i: number, b: Piece, s: number): Diff {
  const position = Math.hypot(a.pos[3 * i] - b.pos[3 * s], a.pos[3 * i + 1] - b.pos[3 * s + 1], a.pos[3 * i + 2] - b.pos[3 * s + 2]);
  let normal = 0;
  if (b.nrm) {
    if (!a.nrm) normal = Number.POSITIVE_INFINITY;
    else {
      const [x, y, z] = [a.nrm[3 * i], a.nrm[3 * i + 1], a.nrm[3 * i + 2]];
      const [u, v, w] = [b.nrm[3 * s], b.nrm[3 * s + 1], b.nrm[3 * s + 2]];
      const c = (x * u + y * v + z * w) / (Math.hypot(x, y, z) * Math.hypot(u, v, w) || 1);
      normal = (Math.acos(Math.min(1, Math.max(-1, c))) * 180) / Math.PI;
    }
  }
  const uv = b.uvs.map((set, k) => {
    const mine = a.uvs[k];
    if (!mine) return Number.POSITIVE_INFINITY;
    let d = 0;
    for (let j = 0; j < 2; j++) {
      const [p, q] = [mine[2 * i + j], set[2 * s + j]];
      d = Math.max(d, Number.isNaN(p) && Number.isNaN(q) ? 0 : Number.isNaN(p) || Number.isNaN(q) ? Number.POSITIVE_INFINITY : Math.abs(p - q));
    }
    return d;
  });
  let weight = 0;
  let joint = "";
  for (const name of new Set([...a.weights[i].keys(), ...b.weights[s].keys()])) {
    const d = Math.abs((a.weights[i].get(name) ?? 0) - (b.weights[s].get(name) ?? 0));
    if (d > weight) [weight, joint] = [d, name];
  }
  return { position, normal, uv, weight, joint };
}

/** Which attributes of a diff are past the tolerance. */
const over = (d: Diff, t: Tolerance) => [d.position > t.position, d.normal > t.normal, d.uv.some((x) => x > t.uv), d.weight > t.weight].filter(Boolean).length;

/** Each vertex of a piece as the lowest-numbered of its own vertices equal to it in everything,
 *  so two vertices a seam splits only in name are one when triangles are compared. */
function canon(p: Piece, t: Tolerance): Int32Array {
  if (p.canon) return p.canon;
  const grid = new Grid(p, Math.max(t.position, 1e-9));
  p.canon = new Int32Array(p.count);
  for (let s = 0; s < p.count; s++) p.canon[s] = grid.near(p.pos[3 * s], p.pos[3 * s + 1], p.pos[3 * s + 2], t.position).find((c) => over(diff(p, s, p, c), t) === 0) ?? s;
  return p.canon;
}

/** A triangle as a key the same however its corners are numbered round, its winding kept. */
const tri = (a: number, b: number, c: number) => (a <= b && a <= c ? `${a},${b},${c}` : b <= c ? `${b},${c},${a}` : `${c},${a},${b}`);

/** `cut`: every vertex, triangle and bind of it theirs, but only some of their triangles, as
 *  their own UpperBody is a cut of their FullBody; `kept` of their triangles are here. */
export type Verdict = { ok: boolean; misses: number; lines: string[]; cut: boolean; kept: number };

/** Whether `out` is `src` as made, and if not, what differs, a line an attribute. */
export function compare(out: Piece, src: Piece, t: Tolerance = TOLERANCE): Verdict {
  const sc = canon(src, t);
  const grid = new Grid(src, Math.max(t.position, 1e-9));
  const map = new Int32Array(out.count).fill(-1);
  type Worst = { n: number; max: number; at: number; note: string };
  const worst = new Map<string, Worst>();
  const note = (what: string, by: number, at: number, said: string) => {
    const w = worst.get(what) ?? { n: 0, max: -1, at, note: said };
    w.n++;
    if (by > w.max) Object.assign(w, { max: by, at, note: said });
    worst.set(what, w);
  };
  let far = 0;
  for (let i = 0; i < out.count; i++) {
    const [x, y, z] = [out.pos[3 * i], out.pos[3 * i + 1], out.pos[3 * i + 2]];
    const near = grid.near(x, y, z, t.position);
    const diffs = near.map((s) => ({ s, d: diff(out, i, src, s) }));
    const same = diffs.find(({ d }) => over(d, t) === 0);
    if (same) {
      map[i] = sc[same.s];
      continue;
    }
    if (!diffs.length) {
      // The nearest of theirs, by brute force, for the first few thousand only.
      let best = far++ < 2000 ? Number.POSITIVE_INFINITY : Number.NaN;
      if (!Number.isNaN(best)) for (let s = 0; s < src.count; s++) best = Math.min(best, Math.hypot(src.pos[3 * s] - x, src.pos[3 * s + 1] - y, src.pos[3 * s + 2] - z));
      note("position", best, i, `vertex ${i} at (${[x, y, z].map((v) => v.toFixed(4)).join(", ")}), the nearest of theirs ${mm(best)} away`);
      continue;
    }
    const { s, d } = diffs.reduce((a, b) => (over(b.d, t) < over(a.d, t) ? b : a));
    if (d.normal > t.normal) note("normal", d.normal, i, `vertex ${i}, turned ${d.normal.toFixed(2)}°`);
    d.uv.forEach((u, k) => u > t.uv && note(`TEXCOORD_${k}`, u, i, `vertex ${i}, moved ${u.toPrecision(3)} in UV`));
    if (d.weight > t.weight) note("weights", d.weight, i, `vertex ${i}, '${d.joint}' ${(src.weights[s].get(d.joint) ?? 0).toFixed(3)} -> ${(out.weights[i].get(d.joint) ?? 0).toFixed(3)}`);
  }
  const lines: string[] = [];
  // How many vertices is not compared: an exporter splits a vertex wherever any attribute
  // differs across it (vertex colours too), so a re-export has more, each still theirs.
  const tol: Record<string, string> = { position: `none of theirs within ${mm(t.position)}`, normal: `turned more than ${t.normal}°`, weights: `weights changed more than ${t.weight}` };
  for (const [what, w] of worst) lines.push(`${what}: ${n(w.n)} of ${n(out.count)} vertices ${tol[what] ?? `moved more than ${t.uv} in UV`}, the worst ${w.note}`);
  // Their triangles, each by its corners' canonical vertices, against ours mapped onto theirs.
  const theirs = new Map<string, number>();
  for (let k = 0; k < src.tris.length; k += 3) {
    const key = tri(sc[src.tris[k]], sc[src.tris[k + 1]], sc[src.tris[k + 2]]);
    theirs.set(key, (theirs.get(key) ?? 0) + 1);
  }
  // A triangle with a corner already counted above is left out of this count, and so is one of
  // theirs it stood for, so one moved vertex is not also a line of triangles.
  let strange = 0;
  let touched = 0;
  for (let k = 0; k < out.tris.length; k += 3) {
    const [a, b, c] = [map[out.tris[k]], map[out.tris[k + 1]], map[out.tris[k + 2]]];
    if (a < 0 || b < 0 || c < 0) {
      touched++;
      continue;
    }
    const key = tri(a, b, c);
    const left = theirs.get(key) ?? 0;
    if (left > 0) theirs.set(key, left - 1);
    else strange++;
  }
  const missing = Math.max(0, [...theirs.values()].reduce((a, b) => a + b, 0) - touched);
  if (strange || missing) lines.push(`indices: ${n(missing)} of their ${n(src.tris.length / 3)} triangles missing, ${n(strange)} of ${n(out.tris.length / 3)} here not theirs`);
  // The bones it is weighted to keep their bind.
  const used = new Set(src.weights.flatMap((w) => [...w.keys()]));
  const moved: string[] = [];
  for (const j of used) {
    const [a, b] = [out.bind.get(j), src.bind.get(j)];
    if (!b) continue;
    if (!a) moved.push(`'${j}' not in this skin`);
    else {
      const d = Math.max(...a.map((v, k) => Math.abs(v - b[k])));
      if (d > t.bind) moved.push(`'${j}' by ${d.toPrecision(3)}`);
    }
  }
  if (moved.length) lines.push(`bind: ${moved.length} of the ${used.size} joints it is weighted to moved: ${moved.slice(0, 5).join(", ")}${moved.length > 5 ? ", ..." : ""}`);
  const misses = [...worst.values()].reduce((a, w) => a + w.n, 0) + strange + missing + moved.length;
  const cut = worst.size === 0 && touched === 0 && strange === 0 && moved.length === 0 && missing > 0;
  return { ok: lines.length === 0, misses, lines, cut, kept: out.tris.length / 3 };
}

const n = (x: number) => x.toLocaleString("en");
const mm = (m: number) => (!Number.isFinite(m) ? "(not searched)" : m < 1e-5 ? `${(m * 1e6).toPrecision(2)} µm` : `${(m * 1000).toFixed(m < 0.001 ? 3 : 2)} mm`);

/** Every piece in their files under the paths given (files or folders). */
export function sources(paths: string[] = [THEIRS]): Piece[] {
  const files = (p: string): string[] => (statSync(p).isDirectory() ? readdirSync(p).flatMap((f) => files(join(p, f))) : /\.(gltf|glb)$/.test(p) ? [p] : []);
  return paths.flatMap(files).flatMap((f) => pieces(new Glb(f), relative(THEIRS, f).startsWith("..") ? f : relative(THEIRS, f)));
}

const stem = (s: string) => s.replace(/\.\d{3}$/, "");
const names = (p: Piece) => new Set([stem(p.node), stem(p.mesh)]);

export type Report = { ok: boolean; theirs: string[]; cuts: string[]; ours: string[]; lines: string[] };

/** A cut's line: its name, how many of their triangles it keeps, and whose. */
const cutOf = (p: Piece, s: Piece, v: Verdict) => `${p.node} (${n(v.kept)} of their ${n(s.tris.length / 3)} triangles of ${s.node}, ${s.file})`;

/** Every piece of `out` checked against theirs: theirs as made, an exact cut of theirs (every
 *  vertex, triangle and bind theirs, some of their triangles left out, as their own UpperBody is
 *  of their FullBody: the D600 trial's bare slot pieces), or ours. */
export function check(out: Glb | Piece[], srcs: Piece[], t: Tolerance = TOLERANCE): Report {
  const report: Report = { ok: true, theirs: [], cuts: [], ours: [], lines: [] };
  let at: Map<string, Set<number>> | undefined;
  for (const p of Array.isArray(out) ? out : pieces(out)) {
    const mine = names(p);
    // Theirs by name, the same node name first, so a tie in the report goes to it.
    const like = srcs.filter((s) => [...names(s)].some((x) => mine.has(x))).sort((a, b) => Number(b.node === p.node) - Number(a.node === p.node));
    if (like.length) {
      const verdicts = like.map((s) => ({ s, v: compare(p, s, t) }));
      const pass = verdicts.find(({ v }) => v.ok);
      if (pass) {
        report.theirs.push(p.node);
        continue;
      }
      const cut = verdicts.find(({ v }) => v.cut);
      if (cut) {
        report.cuts.push(cutOf(p, cut.s, cut.v));
        continue;
      }
      const { s, v } = verdicts.reduce((a, b) => (b.v.misses < a.v.misses ? b : a));
      report.ok = false;
      report.lines.push(`${p.node} is not their piece as made (theirs: ${s.file}, ${s.node})`, ...v.lines.map((l) => `  ${l}`));
      continue;
    }
    // A name of ours: theirs under another name if most of its vertices sit on one of theirs.
    if (!at) {
      at = new Map();
      srcs.forEach((s, si) => {
        for (let i = 0; i < s.count; i++) {
          const k = cell(s.pos, i);
          at!.get(k)?.add(si) ?? at!.set(k, new Set([si]));
        }
      });
    }
    const hits = new Array(srcs.length).fill(0);
    for (let i = 0; i < p.count; i++) for (const si of at.get(cell(p.pos, i)) ?? []) hits[si]++;
    const best = hits.indexOf(Math.max(...hits));
    // Mostly on theirs: an exact cut of one of them is named as one, of the smallest it fits.
    const cut = srcs
      .filter((_, si) => p.count && hits[si] / p.count > 0.5)
      .sort((a, b) => a.tris.length - b.tris.length)
      .map((s) => ({ s, v: compare(p, s, t) }))
      .find(({ v }) => v.cut);
    if (cut) report.cuts.push(cutOf(p, cut.s, cut.v));
    else if (p.count && hits[best] / p.count > 0.5) {
      report.ok = false;
      report.lines.push(`${p.node} (mesh ${p.mesh}) has ${Math.round((100 * hits[best]) / p.count)}% of its vertices on their ${srcs[best].node} (${srcs[best].file}): theirs renamed, merged or changed; keep their piece whole under its name, or an exact cut of it`);
    } else report.ours.push(p.node);
  }
  return report;
}

/** A position to a tenth of a millimetre, as a key. */
const cell = (pos: Float64Array, i: number) => `${Math.round(pos[3 * i] * 1e4)},${Math.round(pos[3 * i + 1] * 1e4)},${Math.round(pos[3 * i + 2] * 1e4)}`;

if (import.meta.main) {
  const args = process.argv.slice(2);
  const from: string[] = [];
  const outs: string[] = [];
  for (let i = 0; i < args.length; i++) (args[i] === "--source" ? from.push(args[++i]) : outs.push(args[i]));
  if (!outs.length || outs.some((a) => a.startsWith("--"))) {
    console.log("usage: node tools/characters/intact.ts OUTPUT.glb|.gltf ... [--source FILE.gltf|DIR]...");
    process.exit(2);
  }
  const srcs = sources(from.length ? from : [THEIRS]);
  let failed = false;
  for (const f of outs) {
    const r = check(new Glb(f), srcs);
    const list = (xs: string[]) => (xs.length > 6 ? `${xs.slice(0, 6).join(", ")}, ...` : xs.join(", "));
    const ours = r.ours.length ? `; ${r.ours.length} of ours not checked (${list(r.ours)})` : "";
    const said = [r.theirs.length && `keeps ${r.theirs.length} of their pieces as made (${r.theirs.join(", ")})`, r.cuts.length && `has ${r.cuts.length} exact cuts of theirs: ${r.cuts.join(", ")}`].filter(Boolean).join("; ");
    if (r.ok) console.log(said ? `✓ intact: ${f} ${said}${ours}` : `· intact: ${f} has none of their pieces${ours}`);
    else {
      failed = true;
      console.log(`✗ intact: ${f}\n  ${r.lines.join("\n  ")}`);
    }
  }
  if (failed)
    console.log(
      "Their parts stay as made (D599): fix the step that wrote the file so it carries their piece unchanged (a separate output, a new piece of ours beside it), never by loosening the tolerance or editing art/characters/quaternius/. A piece of ours must not take their name.",
    );
  process.exit(failed ? 1 : 0);
}
