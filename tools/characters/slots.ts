#!/usr/bin/env node
// D600's trial measured: what shows in an empty clothing slot on Quaternius's man, the two ways
// side by side, in each state of undress, through the mannequin's clips.
//
//   node tools/characters/slots.ts SEX OUTFIT [SLOT=OUTFIT ...] [--frames N]
//
// Slots are body, arms, legs and feet; `feet=Peasant` wears another outfit's piece there. States:
// dressed, coat off (body and arms off), legs off, feet off, bare.
//
// - **Swap**: each empty slot holds its bare piece, an exact cut of their full body
//   (`cut_slots.py`, out/quaternius-SEX-bare-*.glb); the head piece is their OnlyHead while the
//   arms are worn, else their UpperBody (head, shoulders, arms and hands).
// - **Hidden**: their whole body (out/quaternius-SEX-patched.glb) under what is worn, the patches
//   a worn piece covers at rest not drawn, as the game's `body_mask.wesl` does: a patch is hidden
//   when 90% of its vertices, looking straight out from 2 mm off the skin, meet a worn piece within
//   12 cm (`fit_gear.covers`, COVERED and REACH_M).
//
// Measured each frame, as `layers.ts` measures one piece under another (a ray from the bone out
// through each vertex): **poke**, skin past a worn piece by more than 2 mm, of the skin within
// 8 cm of one; **open border**, skin that should be covered and is not covered from outside (a
// swap piece's cut border with no twin in a neighbouring bare piece, or a hidden patch's skin),
// and of those the **see-through** ones, with no worn piece anywhere on the ray (a hidden patch
// whose skin stood proud of the cloth collapses onto the cloth: open, not see-through); **open rim**, a worn piece's rim with nothing inside it (looking
// in from the rim towards the bone meets no skin and no other piece: a see-through gap). At rest
// also **cracks**: cut-border vertices whose twin in the neighbouring bare piece has other
// weights (a crack can open in motion) or a normal more than 1 degree off (a shading seam). Draw
// cost is the files' triangles, vertices and primitives (draws); a hidden body still submits all
// its triangles, collapsed ones included.
//
// A line a state and way; out/slots-SEX-OUTFIT.json holds the numbers, the hidden patches of each
// state and each clip's worst, for `slot_stills.py`. A measurement, never a fix (D599).
//
// Node 22.18 or later; no dependencies.

import { readdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { type Clip, Glb, type Pose, type V3 } from "./glb.ts";
import { type Piece, pieces } from "./intact.ts";
import { anchor, cover, type Frame, Grid, hits, joined, MANNEQUIN, openEdge, retargeted, skinned, within } from "./layers.ts";

const at = (rel: string) => fileURLToPath(new URL(rel, import.meta.url));
const UBC = at("../../art/characters/quaternius/universal-base-characters/");
const MOF = at("../../art/characters/quaternius/modular-outfits-fantasy/");
const OUT = at("./out/");

export const SLOTS = ["body", "arms", "legs", "feet"] as const;
export type Slot = (typeof SLOTS)[number];
export const STATES: { name: string; worn: Slot[] }[] = [
  { name: "dressed", worn: ["body", "arms", "legs", "feet"] },
  { name: "coat off", worn: ["legs", "feet"] },
  { name: "legs off", worn: ["body", "arms", "feet"] },
  { name: "feet off", worn: ["body", "arms", "legs"] },
  { name: "bare", worn: [] },
];
const BARE: Partial<Record<Slot, string>> = { body: "torso", legs: "legs", feet: "feet" };
const NEAR_M = 0.08;
const REACH_M = 0.12;
const COVERED = 0.9;

/** A file worn or shown: its pieces joined for measuring, its draw cost. */
type Part = { label: string; file: string; piece: Piece; tris: number; verts: number; draws: number };

function part(label: string, file: string, only?: (p: Piece) => boolean): Part {
  const g = new Glb(file);
  const all = pieces(g);
  const prims = (g.gltf.meshes ?? []).reduce((a, m) => a + m.primitives.length, 0);
  const ps = only ? all.filter(only) : all;
  return {
    label,
    file,
    piece: ps.length === 1 ? ps[0] : joined(ps),
    tris: all.reduce((a, p) => a + p.tris.length / 3, 0),
    verts: all.reduce((a, p) => a + p.count, 0),
    draws: prims,
  };
}

/** Their outfit's piece for a slot, by its file name (`Male_Ranger_Feet_Boots.gltf`). */
function outfitFile(sex: string, outfit: string, slot: Slot): string {
  const re = new RegExp(`^${sex}_${outfit}_${slot[0].toUpperCase()}${slot.slice(1)}(_[A-Za-z]+)?\\.gltf$`);
  const found = readdirSync(MOF).filter((f) => re.test(f));
  if (found.length !== 1) throw new Error(`slots: ${found.length} files match ${re} in ${MOF}; name an outfit their pack has for every slot (Ranger, Peasant)`);
  return MOF + found[0];
}

const key = (p: Piece, i: number) => `${Math.round(p.pos[3 * i] * 1e4)},${Math.round(p.pos[3 * i + 1] * 1e4)},${Math.round(p.pos[3 * i + 2] * 1e4)}`;

/** A swap figure's borders at rest: each bare piece's open-edge vertices (their full body's own
 *  open edges left out) with no twin in another bare piece shown, which a worn piece must cover;
 *  and twins whose weights or normals differ. */
function borders(bare: Part[], own: Set<string>) {
  const at = new Map<string, [Part, number][]>();
  for (const b of bare) for (let i = 0; i < b.piece.count; i++) at.set(key(b.piece, i), [...(at.get(key(b.piece, i)) ?? []), [b, i]]);
  const lone = new Map<Part, number[]>();
  let twins = 0;
  let cracks = 0;
  let creases = 0;
  for (const b of bare) {
    const mine: number[] = [];
    for (const i of openEdge(b.piece)) {
      const k = key(b.piece, i);
      if (own.has(k)) continue;
      const other = (at.get(k) ?? []).filter(([o]) => o !== b);
      if (!other.length) {
        mine.push(i);
        continue;
      }
      twins++;
      const [o, j] = other[0];
      const [wa, wb] = [b.piece.weights[i], o.piece.weights[j]];
      if ([...new Set([...wa.keys(), ...wb.keys()])].some((n) => Math.abs((wa.get(n) ?? 0) - (wb.get(n) ?? 0)) > 1e-4)) cracks++;
      const [na, nb] = [b.piece.nrm, o.piece.nrm];
      if (na && nb && na[3 * i] * nb[3 * j] + na[3 * i + 1] * nb[3 * j + 1] + na[3 * i + 2] * nb[3 * j + 2] < Math.cos((1 * Math.PI) / 180)) creases++;
    }
    lone.set(b, mine);
  }
  return { lone, twins, cracks, creases };
}

/** The patches of `body` its worn pieces hide at rest, as `fit_gear.covers` finds them. */
function hiddenPatches(body: Part, patch: Int32Array, worn: Part[], joints: Map<string, Pose>): Set<number> {
  const { pos, tris } = gather(worn.map((w) => [w, skinned(w.piece, joints)]));
  const grid = new Grid(pos, tris);
  const v = skinned(body.piece, joints);
  const n = body.piece.nrm!;
  const hit = new Map<number, number>();
  const total = new Map<number, number>();
  const buf: number[] = [];
  const ts: number[] = [];
  for (let i = 0; i < body.piece.count; i++) {
    const d: V3 = [n[3 * i], n[3 * i + 1], n[3 * i + 2]];
    const o: V3 = [v[3 * i] + 0.002 * d[0], v[3 * i + 1] + 0.002 * d[1], v[3 * i + 2] + 0.002 * d[2]];
    total.set(patch[i], (total.get(patch[i]) ?? 0) + 1);
    if (hits(o, d, pos, tris, grid.along(o, d, REACH_M, buf), ts).some((t) => t <= REACH_M)) hit.set(patch[i], (hit.get(patch[i]) ?? 0) + 1);
  }
  return new Set([...total].filter(([p, t]) => (hit.get(p) ?? 0) / t >= COVERED).map(([p]) => p));
}

/** Several parts' skinned vertices as one array, with their triangles and each one's owner. */
function gather(list: [Part, Float64Array][], keep: (p: Part, k: number) => boolean = () => true) {
  const count = list.reduce((a, [, v]) => a + v.length / 3, 0);
  const pos = new Float64Array(3 * count);
  const tris: number[] = [];
  const owner: Part[] = [];
  const base = new Map<Part, number>();
  let b = 0;
  for (const [p, v] of list) {
    pos.set(v, 3 * b);
    base.set(p, b);
    for (let k = 0; k < p.piece.tris.length; k += 3) {
      if (!keep(p, k)) continue;
      tris.push(b + p.piece.tris[k], b + p.piece.tris[k + 1], b + p.piece.tris[k + 2]);
      owner.push(p);
    }
    b += v.length / 3;
  }
  return { pos, tris: Uint32Array.from(tris), owner, base };
}

/** A figure, one state shown one way. */
type Figure = {
  state: string;
  way: "swap" | "hidden";
  shown: Part[];
  worn: Part[];
  inner: { part: Part; near: number[]; edge: number[] }[];
  rims: Map<Part, number[]>;
  keep: (p: Part, k: number) => boolean;
  drawn: number;
  cracks?: { twins: number; cracks: number; creases: number };
  hidden?: number[];
};

type Look = { poke: number; poking: number; open: number; through: number; rim: number; where: string; by: Record<string, [number, number, number]> };

/** One frame of a figure: the worst poke, how many skin vertices poke past 2 mm, open border
 *  vertices, open rim vertices, and the inner vertices poking or open (for the stills). */
function look(f: Figure, joints: Map<string, Pose>, children: Map<string, string[]>, skin: Map<Part, Float64Array>): Look {
  const all = gather([...f.worn, ...f.inner.map((i) => i.part)].map((p) => [p, skin.get(p)!]), f.keep);
  const isWorn = new Set(f.worn);
  const grid = new Grid(all.pos, all.tris);
  const buf: number[] = [];
  const out: Look = { poke: 0, poking: 0, open: 0, through: 0, rim: 0, where: "", by: {} };
  const wornOnly = (o: V3, d: V3, l: number) => grid.along(o, d, l + REACH_M, buf).filter((k) => isWorn.has(all.owner[k / 3]));
  for (const { part, near, edge } of f.inner) {
    if (!f.worn.length) continue;
    const r: Frame = cover(part.piece, skin.get(part)!, all.pos, all.tris, wornOnly, joints, children, near, edge);
    if (r.worst > out.poke) {
      const [bone] = [...part.piece.weights[r.at]].reduce((a, b) => (b[1] > a[1] ? b : a), ["", -1]);
      out.where = `${part.label} on ${bone}, ${skin.get(part)![3 * r.at + 1].toFixed(2)} m up`;
    }
    out.poke = Math.max(out.poke, r.worst);
    out.poking += r.poking;
    out.open += r.uncovered;
    out.through += r.through.length;
    for (const i of r.holes) {
      const y = skin.get(part)![3 * i + 1];
      const b = (out.by[`${part.label} border`] ??= [0, Infinity, -Infinity]);
      [b[0], b[1], b[2]] = [b[0] + 1, Math.min(b[1], y), Math.max(b[2], y)];
    }
  }
  const ts: number[] = [];
  for (const [w, rim] of f.rims) {
    const v = skin.get(w)!;
    for (const i of rim) {
      const p: V3 = [v[3 * i], v[3 * i + 1], v[3 * i + 2]];
      const a = anchor(w.piece, i, p, joints, children);
      const to: V3 = [a[0] - p[0], a[1] - p[1], a[2] - p[2]];
      const L = Math.hypot(...to);
      if (L < 1e-6) continue;
      const d: V3 = [to[0] / L, to[1] / L, to[2] / L];
      // Backed: another piece inside it, between the rim and the bone. Covered: anything, its own
      // piece too (a lining's edge inside a boot), over it out from the bone. Neither: a gap.
      const inside = grid.along(p, d, L, buf).filter((k) => all.owner[k / 3] !== w);
      const backed = hits(p, d, all.pos, all.tris, inside, ts).some((t) => t > 1e-4 && t < L);
      const back: V3 = [-d[0], -d[1], -d[2]];
      const over = grid.along(p, back, REACH_M, buf);
      const covered = hits(p, back, all.pos, all.tris, over, ts).some((t) => t > 1e-4 && t < REACH_M);
      if (!backed && !covered) {
        out.rim++;
        const b = (out.by[w.label] ??= [0, Infinity, -Infinity]);
        [b[0], b[1], b[2]] = [b[0] + 1, Math.min(b[1], p[1]), Math.max(b[2], p[1])];
      }
    }
  }
  return out;
}

export function figures(sex: string, outfits: Record<Slot, string>) {
  const Sex = sex[0].toUpperCase() + sex.slice(1);
  // A head file's skin alone is measured (its brows are open cards, its eyes inside the head);
  // all its pieces count in the draw cost.
  const skinOf = (file: string) => {
    const big = pieces(new Glb(file)).reduce((a, b) => (b.tris.length > a.tris.length ? b : a));
    return (p: Piece) => p.node === big.node;
  };
  const headFile = `${UBC}Regular_${Sex}_OnlyHead.gltf`;
  const upperFile = `${UBC}Regular_${Sex}_UpperBody.gltf`;
  const head = part("OnlyHead", headFile, skinOf(headFile));
  const upper = part("UpperBody", upperFile, skinOf(upperFile));
  const bare = new Map<Slot, Part>();
  for (const [slot, name] of Object.entries(BARE) as [Slot, string][]) bare.set(slot, part(`bare ${name}`, `${OUT}quaternius-${sex}-bare-${name}.glb`));
  const patchedFile = `${OUT}quaternius-${sex}-patched.glb`;
  const body = part("full body", patchedFile, skinOf(patchedFile));
  // patches.py adds its layer after their own two UV sets: u = patch + 0.5, v = 0.5 throughout.
  const layer = body.piece.uvs.at(-1);
  if (body.piece.uvs.length < 3 || !layer || layer.some((x, n) => (n % 2 ? x !== 0.5 : x % 1 !== 0.5)))
    throw new Error(`slots: ${patchedFile} has no patch layer after their two UV sets (u = patch + 0.5, v = 0.5); run tools/blender/run.sh tools/characters/cut_slots.py ${sex}`);
  const patch = Int32Array.from({ length: body.piece.count }, (_, i) => Math.floor(layer[2 * i]));
  const worn = new Map<Slot, Part>(SLOTS.map((s) => [s, part(`${outfits[s]} ${s}`, outfitFile(Sex, outfits[s], s))]));
  const own = new Set(openEdge(body.piece).map((i) => key(body.piece, i)));
  const skel = new Glb(head.file);
  const rest = retargeted(skel, skel, undefined, 0);
  const nearCache = new Map<string, Set<number>>();
  const near = (inner: Part, outer: Part[]) => {
    const s = new Set<number>();
    for (const o of outer) {
      const k = `${inner.file}|${o.file}`;
      if (!nearCache.has(k)) nearCache.set(k, new Set(within(inner.piece, o.piece, NEAR_M)));
      for (const i of nearCache.get(k)!) s.add(i);
    }
    return [...s];
  };
  const out: Figure[] = [];
  for (const st of STATES) {
    const on = st.worn.map((s) => worn.get(s)!);
    const rims = new Map(on.map((w) => [w, openEdge(w.piece)]));
    const sum = (ps: Part[], k: "tris" | "verts" | "draws") => ps.reduce((a, p) => a + p[k], 0);
    // Swap.
    const shown = [st.worn.includes("arms") ? head : upper, ...SLOTS.filter((s) => !st.worn.includes(s) && bare.has(s)).map((s) => bare.get(s)!)];
    const b = borders(shown, own);
    out.push({
      state: st.name,
      way: "swap",
      shown: [...shown, ...on],
      worn: on,
      inner: shown.map((p) => ({ part: p, near: near(p, on), edge: b.lone.get(p)! })),
      rims,
      keep: () => true,
      drawn: sum([...shown, ...on], "tris"),
      cracks: { twins: b.twins, cracks: b.cracks, creases: b.creases },
    });
    // Hidden.
    const hid = on.length ? hiddenPatches(body, patch, on, rest) : new Set<number>();
    const collapsed = (k: number) => hid.has(patch[body.piece.tris[k]]);
    const close = new Set(near(body, on));
    const visible: number[] = [];
    const under: number[] = [];
    for (const i of close) (hid.has(patch[i]) ? under : visible).push(i);
    let gone = 0;
    for (let k = 0; k < body.piece.tris.length; k += 3) if (collapsed(k)) gone++;
    const whole = part("full body", body.file);
    out.push({
      state: st.name,
      way: "hidden",
      shown: [whole, ...on],
      worn: on,
      inner: [{ part: body, near: visible, edge: under }],
      rims,
      keep: (p, k) => p !== body || !collapsed(k),
      drawn: sum([whole, ...on], "tris") - gone,
      hidden: [...hid].sort((x, y) => x - y),
    });
  }
  return { skel, figures: out };
}

/** The clip played in place, as the game moves the body itself (pace.py): the mannequin's
 *  travelling clips carry the pelvis metres away while its root bone stays, and their full body
 *  weights its feet a little to the root (3% at the heel), which would leave those vertices
 *  tens of centimetres behind. The root is carried along the ground with the pelvis. */
function inPlace(joints: Map<string, Pose>, still: Map<string, Pose>): Map<string, Pose> {
  const [pelvis, root, rest] = [joints.get("pelvis"), joints.get("root"), still.get("pelvis")];
  if (!pelvis || !root || !rest) return joints;
  joints.set("root", { ...root, t: [root.t[0] + pelvis.t[0] - rest.t[0], root.t[1], root.t[2] + pelvis.t[2] - rest.t[2]] });
  return joints;
}

type Tally = { rest: Look; frames: number; poked: number; opened: number; holed: number; rimmed: number; worst: { poke: [number, string]; open: [number, string]; through: [number, string]; rim: [number, string] }; clips: Record<string, [number, number, number]> };

export function measure(sex: string, outfits: Record<Slot, string>, frames = 6) {
  const { skel, figures: figs } = figures(sex, outfits);
  const from = new Glb(MANNEQUIN);
  const children = new Map<string, string[]>();
  skel.gltf.nodes.forEach((n) => n.name && children.set(n.name, (n.children ?? []).map((c) => skel.gltf.nodes[c].name ?? "")));
  const parts = new Set(figs.flatMap((f) => [...f.worn, ...f.inner.map((i) => i.part)]));
  const still = retargeted(skel, skel, undefined, 0);
  const frame = (clip: Clip | undefined, t: number) => {
    const joints = inPlace(retargeted(skel, from, clip, t), still);
    const skin = new Map([...parts].map((p) => [p, skinned(p.piece, joints)]));
    return figs.map((f) => look(f, joints, children, skin));
  };
  const rest = frame(undefined, 0);
  const tallies: Tally[] = rest.map((r) => ({ rest: r, frames: 0, poked: 0, opened: 0, holed: 0, rimmed: 0, worst: { poke: [r.poke, "rest"], open: [r.open, "rest"], through: [r.through, "rest"], rim: [r.rim, "rest"] }, clips: {} }));
  for (const name of from.clipNames()) {
    const clip = from.clip(name)!;
    for (let k = 0; k < frames; k++) {
      const t = (clip.end * k) / frames;
      const where = `${name}@${t.toFixed(2)}`;
      frame(clip, t).forEach((l, n) => {
        const y = tallies[n];
        y.frames++;
        if (l.poking) y.poked++;
        if (l.open > y.rest.open) y.opened++;
        if (l.through > y.rest.through) y.holed++;
        if (l.through > y.worst.through[0]) y.worst.through = [l.through, where];
        if (l.rim > y.rest.rim) y.rimmed++;
        if (l.poke > y.worst.poke[0]) y.worst.poke = [l.poke, `${where} (${l.where})`];
        if (l.open > y.worst.open[0]) y.worst.open = [l.open, where];
        if (l.rim > y.worst.rim[0]) y.worst.rim = [l.rim, where];
        const c = (y.clips[name] ??= [0, 0, 0]);
        y.clips[name] = [Math.max(c[0], l.poke), Math.max(c[1], l.open), Math.max(c[2], l.rim)];
      });
    }
  }
  return { figures: figs, tallies };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const opt = (k: string) => (args.includes(k) ? args.splice(args.indexOf(k), 2)[1] : undefined);
  const frames = Number(opt("--frames") ?? 6);
  const [sex, outfit, ...mix] = args;
  if (!sex || !outfit || mix.some((m) => !/^(body|arms|legs|feet)=\w+$/.test(m))) {
    console.log("usage: node tools/characters/slots.ts SEX OUTFIT [SLOT=OUTFIT ...] [--frames N]   (e.g. male Ranger feet=Peasant)");
    process.exit(2);
  }
  const outfits = Object.fromEntries(SLOTS.map((s) => [s, outfit])) as Record<Slot, string>;
  for (const m of mix) outfits[m.split("=")[0] as Slot] = m.split("=")[1];
  const { figures: figs, tallies } = measure(sex, outfits, frames);
  const mm = (m: number) => `${(m * 1000).toFixed(0)} mm`;
  const k = (n: number) => `${(n / 1000).toFixed(1)}k`;
  const name = [outfit, ...mix].join("+");
  console.log(`${sex} ${name}, ${tallies[0].frames} frames of the mannequin's clips (poke past 2 mm; open border: a cut edge or hidden patch not covered from outside, see-through: no worn piece on its ray at all; open rim: a worn piece's rim with nothing inside)`);
  figs.forEach((f, n) => {
    const y = tallies[n];
    const cost = `${k(f.shown.reduce((a, p) => a + p.tris, 0))} tris (${k(f.drawn)} drawn) ${k(f.shown.reduce((a, p) => a + p.verts, 0))} verts ${f.shown.reduce((a, p) => a + p.draws, 0)} draws`;
    const seams = f.cracks ? `; borders ${f.cracks.twins} twinned, ${f.cracks.cracks} cracks, ${f.cracks.creases} creases` : `; ${f.hidden!.length} patches hidden`;
    console.log(
      `  ${f.state.padEnd(8)} ${f.way.padEnd(6)} ${cost} | poke ${mm(y.rest.poke)} at rest${y.rest.where ? ` (${y.rest.where})` : ""}, in ${y.poked}/${y.frames} frames, worst ${mm(y.worst.poke[0])} ${y.worst.poke[1]}` +
        ` | open border ${y.rest.open}/${f.inner.reduce((a, i) => a + i.edge.length, 0)} at rest, worse in ${y.opened}, worst ${y.worst.open[0]} ${y.worst.open[1]}; of them see-through ${y.rest.through} at rest, more in ${y.holed}, worst ${y.worst.through[0]} ${y.worst.through[1]}` +
        ` | open rim ${y.rest.rim}/${[...f.rims.values()].reduce((a, r) => a + r.length, 0)} at rest, worse in ${y.rimmed}, worst ${y.worst.rim[0]} ${y.worst.rim[1]}${seams}`,
    );
    const by = Object.entries(y.rest.by);
    if (by.length) console.log(`      open at rest: ${by.map(([n, [c, lo, hi]]) => `${n} ${c} (${lo.toFixed(2)}-${hi.toFixed(2)} m up)`).join(", ")}`);
  });
  const file = `${OUT}slots-${sex}-${name}.json`;
  writeFileSync(
    file,
    JSON.stringify(
      {
        sex,
        outfits,
        frames,
        figures: figs.map((f, n) => ({
          state: f.state,
          way: f.way,
          files: f.shown.map((p) => p.file.slice(p.file.indexOf("tools/") >= 0 ? p.file.indexOf("tools/") : p.file.indexOf("art/"))),
          worn: f.worn.map((p) => p.label),
          tris: f.shown.reduce((a, p) => a + p.tris, 0),
          drawn: f.drawn,
          verts: f.shown.reduce((a, p) => a + p.verts, 0),
          draws: f.shown.reduce((a, p) => a + p.draws, 0),
          ...(f.cracks ?? {}),
          hidden: f.hidden,
          ...tallies[n],
        })),
      },
      null,
      1,
    ),
  );
  console.log(`  wrote ${file.slice(file.indexOf("tools/"))}`);
}
