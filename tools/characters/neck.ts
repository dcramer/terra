#!/usr/bin/env node
// The neck seam (D600's swap): our bare torso, cut from a copy of their full body, against each
// piece that shows the head beside it (their OnlyHead while the arms are worn, their UpperBody
// while they are bare), through the mannequin's clips. Their outfits are all made on the same
// head, so this seam, where their head meets a cut of ours, is the one that is ours to hold.
//
//   node tools/characters/neck.ts [--frames N]
//
// Per head and torso of each wardrobe body, two numbers in mm, their worst over the clips:
// - **gap**: each point of the head's open edge (its neck ring) against the torso vertex that
//   meets it at rest (the nearest), as both move: a torso that stops short, a ring moved, or a
//   twin skinned otherwise, which opens as he moves, all read as a gap.
// - **overlap**: each point of the torso's open edge that lies on the head's skin (a vertex of the
//   head within 0.1 mm) yet off the head's ring: how far the torso runs on under the head, skin
//   drawn twice.
// The bar, 1 mm each, is one their own full body as shipped (uncut) passes against their head:
// 0 and 0 (neck.test.ts holds it, and fails a torso whose neck ring is moved). A measurement,
// never a fix of theirs (D599): a failing seam is our cut's (cut_slots.py).
//
// Node 22.18 or later; no dependencies.

import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { type Wardrobe, wardrobeData } from "./build_steps.ts";
import { type Clip, Glb } from "./glb.ts";
import { type Piece, pieces } from "./intact.ts";
import { inPlace, MANNEQUIN, openEdge, retargeted, skinned } from "./layers.ts";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
/** The bar, from their full body against their head (0 and 0): a millimetre each. */
export const BAR_M = 0.001;
const ON_M = 1e-4;

export type Seam = { gap: number; gapAt: string; overlap: number; overlapAt: string; ring: number; frames: number };

/** A file's skin: its largest mesh (a head's brows are open cards, its eyes inside it). */
export const skinOf = (file: string): Piece => pieces(new Glb(file)).reduce((a, b) => (b.tris.length > a.tris.length ? b : a));

const d2 = (a: Float64Array, i: number, b: Float64Array, j: number) => (a[3 * i] - b[3 * j]) ** 2 + (a[3 * i + 1] - b[3 * j + 1]) ** 2 + (a[3 * i + 2] - b[3 * j + 2]) ** 2;
const nearest = (from: Float64Array, i: number, to: Float64Array, n: number, among?: number[]) => {
  let [best, at] = [Infinity, -1];
  for (const j of among ?? Array.from({ length: n }, (_, k) => k)) {
    const d = d2(from, i, to, j);
    if (d < best) [best, at] = [d, j];
  }
  return { at, d: Math.sqrt(best) };
};

/** The seam of `torso` against `head`, both on the skeleton of `skel`, at rest and through every
 *  clip of `clips`, `frames` moments each. */
export function seam(head: Piece, torso: Piece, skel: Glb, clips: Glb | undefined, frames = 12): Seam {
  const ring = openEdge(head);
  const twin = ring.map((r) => nearest(head.pos, r, torso.pos, torso.count).at);
  // A place on the ring by its position: a mesh splits a vertex where its UVs part.
  const key = (p: Float64Array, i: number) => `${Math.round(p[3 * i] * 1e4)},${Math.round(p[3 * i + 1] * 1e4)},${Math.round(p[3 * i + 2] * 1e4)}`;
  const headRing = new Set(ring.map((r) => key(head.pos, r)));
  // The torso's own edge where it lies on the head's skin, and off the head's ring: the torso
  // runs on under the head.
  const under: [number, number][] = [];
  for (const q of openEdge(torso)) {
    const on = nearest(torso.pos, q, head.pos, head.count);
    if (on.d > ON_M || headRing.has(key(head.pos, on.at))) continue;
    under.push([q, nearest(torso.pos, q, head.pos, head.count, ring).at]);
  }
  const s: Seam = { gap: 0, gapAt: "rest", overlap: 0, overlapAt: "rest", ring: ring.length, frames: 0 };
  const still = retargeted(skel, skel, undefined, 0);
  const one = (clip: Clip | undefined, t: number, at: string) => {
    const joints = clip ? inPlace(retargeted(skel, clips!, clip, t), still) : still;
    const [h, b] = [skinned(head, joints), skinned(torso, joints)];
    s.frames++;
    ring.forEach((r, k) => {
      const g = Math.sqrt(d2(h, r, b, twin[k]));
      if (g > s.gap) [s.gap, s.gapAt] = [g, at];
    });
    for (const [q, r] of under) {
      const o = Math.sqrt(d2(b, q, h, r));
      if (o > s.overlap) [s.overlap, s.overlapAt] = [o, at];
    }
  };
  one(undefined, 0, "rest");
  for (const name of clips?.clipNames() ?? []) {
    const clip = clips!.clip(name)!;
    for (let k = 0; k < frames; k++) one(clip, (clip.end * k) / frames, `${name}@${((clip.end * k) / frames).toFixed(2)}`);
  }
  return s;
}

export const passes = (s: Seam) => s.gap <= BAR_M && s.overlap <= BAR_M;

/** Each body's neck seams: the bare piece holding the torso against every piece that shows the
 *  head (the body's head, and a bare piece holding the head too), a pair each. */
export function necks(w: Wardrobe): { body: string; head: string; torso: string }[] {
  return Object.entries(w.bodies).flatMap(([body, b]) => {
    const torso = Object.values(b.bare).find((p) => w.pieces[p]?.regions.includes("torso"));
    if (!torso) return [];
    const heads = [b.head, ...Object.values(b.bare).filter((p) => p !== torso && w.pieces[p]?.regions.includes("head"))];
    return heads.map((head) => ({ body, head, torso }));
  });
}

const mm = (m: number) => (m * 1000).toFixed(1);

/** The wardrobe check's stage: a fault a seam past the bar, said with its numbers. */
export function neckStage(w: Wardrobe, frames = 12, root = ROOT) {
  const clips = new Glb(MANNEQUIN);
  const faults: string[] = [];
  const read: string[] = [];
  for (const n of necks(w)) {
    const file = (p: string) => join(root, "assets", w.bodies[n.body].pieces, `${p}.gltf`);
    const s = seam(skinOf(file(n.head)), skinOf(file(n.torso)), new Glb(file(n.head)), clips, frames);
    const line = `${n.torso} against ${n.head}: gap ${mm(s.gap)} mm (${s.gapAt}), overlap ${mm(s.overlap)} mm (${s.overlapAt}), ${s.ring} ring points, ${s.frames} frames`;
    read.push(line);
    if (!passes(s))
      faults.push(`${line}; bar ${mm(BAR_M)} mm each, which their full body passes against their head (D600). The cut is ours: recut it in cut_slots.py so the torso's neck edge meets the head's ring, never their parts (D599); the neck task in cargo xtask characters says how`);
  }
  return { name: "neck", faults, said: read.map((l) => l.replace(/, \d+ ring points.*$/, "")).join("; "), read };
}

if (import.meta.main) {
  const at = process.argv.indexOf("--frames");
  const r = neckStage(wardrobeData(ROOT), at > 0 ? Number(process.argv[at + 1]) : 12);
  for (const l of r.read) console.log(`${r.faults.some((f) => f.startsWith(l)) ? "✗" : "✓"} ${l}`);
  console.log(r.faults.length ? "NECK fail" : "NECK pass");
  process.exit(r.faults.length ? 1 : 0);
}
