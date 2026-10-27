// The detector (docs/attachment.md, "Fitting a new item"; D552): how a new held model of a
// detector kind (hafted, blade, polearm) is turned, measured from its source without Blender, so
// its data gets its turn and grip from a measurement rather than a person's eye. It refuses
// rather than guesses when a measure is not decisive, and names which; a person then sets the
// turn once (fit.ts --turn), kept in the data.
//
// In the source's own glTF axes:
// - its long axis is +Y for a length fit, else its vertices' principal axis, at least
//   LONG_RATIO times its next extent;
// - its handle is the longest run of sections a fist closes on (round, HANDLE_M across, even
//   within HANDLE_EVEN), and its working end the end away from it (a sword's guard is wider
//   than its blade, so "the end whose sections are wider" alone picks the hilt);
// - its head (past the handle, from the first section HEAD_RATIO times as wide as the handle or
//   flat) is HEAD_RATIO times as wide as the handle (a blade instead flat, FLAT_RATIO times as
//   wide as thick); the way it spreads across the axis goes to the look's Z and its
//   further-reaching side to -Z, one-sided by ONE_SIDED_M or even within EVEN_M; a round head
//   has no side, and keeps its turn;
// - its grip by its kind's rule from the butt: hafted HAFT_GRIP_M up its handle, blade and
//   polearm the middle of their grip or shaft.
//
// A "length" fit (its source standing on its butt along +Y, fit_held.py) gets "turn", degrees
// about +Y, and "grip_from_end" if its data gives none; a "points" fit gets "grip", "toward"
// and "out". Node 22.18 or later; no dependencies.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { ROOT, readLook } from "./attach_contract.ts";
import type { V3 } from "./glb.ts";

const MODELS = join(ROOT, "assets", "models");

/** A points fit's long axis against its next extent (a length fit's is +Y by its convention). */
export const LONG_RATIO = 3;
/** A handle's section across: a fist closes on 0.8 cm, and the free pickaxe's haft is 7.5 to 9.5 cm. */
export const HANDLE_M: [number, number] = [0.008, 0.1];
/** How much wider than thick a handle's section may be: the sword's grip 3.4, its blade 6.4 and over. */
export const HANDLE_ROUND = 4;
/** How much a handle's section may grow over the run's thinnest: a torch's head is 2.1 times. */
export const HANDLE_EVEN = 1.4;
export const HANDLE_RUN_M = 0.06;
export const HEAD_RATIO = 1.5;
export const FLAT_RATIO = 3;
export const ROUND_HEAD = 1.25;
export const ONE_SIDED_M = 0.02;
export const EVEN_M = 0.01;
/** A hafted grip above where its handle starts (attach_kinds.json's 4 to 12 cm). */
export const HAFT_GRIP_M = 0.07;

export type Mesh = { points: V3[]; tris: number[] };
export type Found =
  | { ok: true; turn?: number; grip_from_end?: number; grip?: V3; toward?: V3; out?: V3; measures: string }
  | { ok: false; why: string };

const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const scale = (a: V3, k: number): V3 => [a[0] * k, a[1] * k, a[2] * k];
const add = (a: V3, b: V3): V3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const unit = (a: V3): V3 => scale(a, 1 / Math.hypot(...a));
const cm = (m: number) => `${(m * 100).toFixed(1)} cm`;
const r3 = (v: number) => Math.round(v * 1000) / 1000 + 0;

/** The eigenvectors of a symmetric 3x3 matrix, largest eigenvalue first (Jacobi). */
export function principal(points: V3[]): V3[] {
  const n = points.length;
  const c = points.reduce((s, p) => add(s, p), [0, 0, 0] as V3).map((x) => x / n) as V3;
  const a = [0, 0, 0].map(() => [0, 0, 0]);
  for (const p of points) for (let i = 0; i < 3; i++) for (let j = 0; j < 3; j++) a[i][j] += ((p[i] - c[i]) * (p[j] - c[j])) / n;
  const v = [
    [1, 0, 0],
    [0, 1, 0],
    [0, 0, 1],
  ];
  for (let sweep = 0; sweep < 50; sweep++) {
    for (const [p, q] of [
      [0, 1],
      [0, 2],
      [1, 2],
    ]) {
      if (Math.abs(a[p][q]) < 1e-15) continue;
      const th = (a[q][q] - a[p][p]) / (2 * a[p][q]);
      const t = Math.sign(th || 1) / (Math.abs(th) + Math.sqrt(th * th + 1));
      const [cs, sn] = [1 / Math.sqrt(t * t + 1), 0];
      const s = t * cs + sn;
      for (let k = 0; k < 3; k++) {
        const [akp, akq] = [a[k][p], a[k][q]];
        a[k][p] = cs * akp - s * akq;
        a[k][q] = s * akp + cs * akq;
      }
      for (let k = 0; k < 3; k++) {
        const [apk, aqk] = [a[p][k], a[q][k]];
        a[p][k] = cs * apk - s * aqk;
        a[q][k] = s * apk + cs * aqk;
      }
      for (let k = 0; k < 3; k++) {
        const [vkp, vkq] = [v[k][p], v[k][q]];
        v[k][p] = cs * vkp - s * vkq;
        v[k][q] = s * vkp + cs * vkq;
      }
    }
  }
  return [0, 1, 2].sort((i, j) => a[j][j] - a[i][i]).map((i) => [v[0][i], v[1][i], v[2][i]] as V3);
}

type P2 = [number, number];
/** Points' extent across 2-D, every degree: the widest and thinnest, and the way it is widest,
 * square to its thinnest (a box's widest caliper is its diagonal, its thinnest square to a side). */
function across(pts: P2[]): { size: number; thin: number; way: P2 } {
  let [size, thin, way] = [0, Infinity, [1, 0] as P2];
  for (let deg = 0; deg < 180; deg += 1) {
    const d: P2 = [Math.cos((deg * Math.PI) / 180), Math.sin((deg * Math.PI) / 180)];
    let [lo, hi] = [Infinity, -Infinity];
    for (const p of pts) {
      const x = p[0] * d[0] + p[1] * d[1];
      lo = Math.min(lo, x);
      hi = Math.max(hi, x);
    }
    size = Math.max(size, hi - lo);
    if (hi - lo < thin) [thin, way] = [hi - lo, [-d[1], d[0]]];
  }
  return { size, thin, way };
}

/** The way 2-D points spread most: their covariance's first axis. A box's or a diamond's lies
 * along its symmetry, where calipers tilt (a diamond's thinnest is square to a side, a box's
 * widest its diagonal). */
function spread(pts: P2[]): P2 {
  const [mx, my] = [pts.reduce((a, p) => a + p[0], 0) / pts.length, pts.reduce((a, p) => a + p[1], 0) / pts.length];
  let [xx, yy, xy] = [0, 0, 0];
  for (const [x, y] of pts) {
    xx += (x - mx) ** 2;
    yy += (y - my) ** 2;
    xy += (x - mx) * (y - my);
  }
  const angle = Math.atan2(2 * xy, xx - yy) / 2;
  return [Math.cos(angle), Math.sin(angle)];
}

export type Section ={ s: number; size: number; thin: number; mid: P2 };
/** Where the plane s along `axis` cuts the mesh's edges, in the cross-plane (u, w), every 1/n of its length. */
export function sections(mesh: Mesh, axis: V3, u: V3, w: V3, lo: number, hi: number, n: number): Section[] {
  const out: Section[] = [];
  for (let k = 1; k < n; k++) {
    const s = lo + ((hi - lo) * k) / n;
    const cuts: P2[] = [];
    for (let t = 0; t + 2 < mesh.tris.length; t += 3)
      for (const [i, j] of [
        [0, 1],
        [1, 2],
        [2, 0],
      ]) {
        const [a, b] = [mesh.points[mesh.tris[t + i]], mesh.points[mesh.tris[t + j]]];
        const [sa, sb] = [dot(a, axis) - s, dot(b, axis) - s];
        if (sa < 0 === sb < 0) continue;
        const p = add(a, scale(add(b, scale(a, -1)), sa / (sa - sb)));
        cuts.push([dot(p, u), dot(p, w)]);
      }
    if (cuts.length < 3) continue;
    const { size, thin } = across(cuts);
    const [us, ws] = [cuts.map((c) => c[0]), cuts.map((c) => c[1])];
    out.push({ s, size, thin, mid: [(Math.min(...us) + Math.max(...us)) / 2, (Math.min(...ws) + Math.max(...ws)) / 2] });
  }
  return out;
}

/** How a model of `kind` is turned and gripped, measured; `length` scales a "length" fit's source. */
export function detect(mesh: Mesh, kind: string, fit: string, length?: number): Found {
  if (!["hafted", "blade", "polearm"].includes(kind)) return { ok: false, why: `a ${kind} is not measured by the detector` };
  if (fit !== "length" && fit !== "points") return { ok: false, why: `a "${fit}" fit has nothing for the detector to find` };
  const extent = (d: V3): [number, number] => {
    let [lo, hi] = [Infinity, -Infinity];
    for (const p of mesh.points) {
      const x = dot(p, d);
      lo = Math.min(lo, x);
      hi = Math.max(hi, x);
    }
    return [lo, hi];
  };
  const size = (d: V3) => extent(d)[1] - extent(d)[0];
  // A length fit stands on its butt along +Y (fit_held.py), so its axis is +Y and its
  // cross-plane X and Z, a turn about +Y read straight off: it need only be longest along Y
  // (a pickaxe's head is nearly as wide as its haft is long, so a ratio would refuse it). A
  // points fit lies any way: its long axis is its vertices' principal axis, decisive at
  // LONG_RATIO times its next extent.
  let [axis, u, w]: V3[] = [
    [0, 1, 0],
    [1, 0, 0],
    [0, 0, 1],
  ];
  if (fit === "length") {
    const [y, x, z] = [size(axis), size(u), size(w)];
    if (y < Math.max(x, z)) return { ok: false, why: `a length fit's source stands along +Y, but it is ${cm(Math.max(x, z))} across and ${cm(y)} along Y` };
  } else {
    [axis, u, w] = principal(mesh.points);
    const [l1, l2] = [size(axis), size(u)];
    if (l1 < LONG_RATIO * l2) return { ok: false, why: `its long axis, ${cm(l1)}, is under ${LONG_RATIO} times the next, ${cm(l2)}` };
  }
  const [lo, hi] = extent(axis);
  const k = fit === "length" && length ? length / (hi - lo) : 1;
  const secs = sections(mesh, axis, u, w, lo, hi, 120);
  const handleLike = (x: Section) => x.size * k >= HANDLE_M[0] && x.size * k <= HANDLE_M[1] && x.size / Math.max(x.thin, 1e-6) <= HANDLE_ROUND;
  // The longest run of handle sections, a missing section or one odd one let through, each
  // within HANDLE_EVEN of the run's thinnest so far (a torch's head is as round as its handle).
  let best: [number, number] | undefined;
  for (let i = 0; i < secs.length; i++) {
    if (!handleLike(secs[i])) continue;
    let [j, thinnest] = [i, secs[i].size];
    const fits = (x: Section | undefined) => !!x && handleLike(x) && x.size <= HANDLE_EVEN * thinnest;
    while (fits(secs[j + 1]) || fits(secs[j + 2])) {
      j += fits(secs[j + 1]) ? 1 : 2;
      thinnest = Math.min(thinnest, secs[j].size);
    }
    if (!best || secs[j].s - secs[i].s > secs[best[1]].s - secs[best[0]].s) best = [i, j];
  }
  if (!best || (secs[best[1]].s - secs[best[0]].s) * k < HANDLE_RUN_M)
    return { ok: false, why: `no handle: no run of round sections ${cm(HANDLE_M[0])} to ${cm(HANDLE_M[1])} across, ${cm(HANDLE_RUN_M)} long` };
  const run = secs.slice(best[0], best[1] + 1);
  const handleMid = (run[0].s + run.at(-1)!.s) / 2;
  const [toLo, toHi] = [handleMid - lo, hi - handleMid];
  if (Math.abs(toHi - toLo) < 0.1 * (hi - lo)) return { ok: false, why: `its handle lies near its middle (${cm(toLo * k)} from one end, ${cm(toHi * k)} from the other): which end works is not decisive` };
  const up = toHi > toLo ? 1 : -1;
  if (fit === "length" && up < 0) return { ok: false, why: "it stands on its working end: a length fit's source stands on its butt, its working end up +Y" };
  const along = (s: number) => (up > 0 ? s - lo : hi - s); // from the butt
  const handle = [...run.map((x) => x.size)].sort((a, b) => a - b)[run.length >> 1];
  const centre: P2 = [run.reduce((a, x) => a + x.mid[0], 0) / run.length, run.reduce((a, x) => a + x.mid[1], 0) / run.length];
  const runFrom = Math.min(along(run[0].s), along(run.at(-1)!.s));
  const runTo = Math.max(along(run[0].s), along(run.at(-1)!.s));
  // The head: every vertex from the first section past the handle that is a head's (HEAD_RATIO
  // times the handle across, or flat), across the axis from the handle's line. A far share of
  // its length would take in a spear's shaft, and its round binding the blade's flat.
  const L = hi - lo;
  const isHead = (x: Section) => x.size >= HEAD_RATIO * handle || x.size >= FLAT_RATIO * x.thin;
  const past = secs.filter((x) => along(x.s) > runTo).sort((a, b) => along(a.s) - along(b.s));
  const first = past.findIndex(isHead);
  const from = first > 0 ? along(past[first - 1].s) : runTo;
  const head: P2[] = mesh.points.filter((p) => along(dot(p, axis)) > from).map((p) => [dot(p, u) - centre[0], dot(p, w) - centre[1]]);
  if (head.length < 3) return { ok: false, why: `no head: its handle runs to its working end (${cm(runFrom * k)} to ${cm(runTo * k)} of ${cm(L * k)})` };
  const h = { ...across(head), way: spread(head) };
  const said = `long ${cm(L * k)}; handle ${cm(handle * k)} across, ${cm(runFrom * k)} to ${cm(runTo * k)} from the butt; head ${cm(h.size * k)} by ${cm(h.thin * k)}`;
  if (kind === "blade" && h.size < FLAT_RATIO * h.thin) return { ok: false, why: `its blade is not flat: ${cm(h.size * k)} wide, ${cm(h.thin * k)} thick, under ${FLAT_RATIO} times` };
  if (kind !== "blade" && h.size < HEAD_RATIO * handle) return { ok: false, why: `its head's section, ${cm(h.size * k)}, is under ${HEAD_RATIO} times the handle's, ${cm(handle * k)}` };
  // Its side toward -Z: the further-reaching way along its widest, or either for an even head.
  let side: P2 | undefined;
  let how = "round, no side to face";
  if (h.size >= ROUND_HEAD * h.thin) {
    const reach = (d: P2) => Math.max(...head.map((p) => p[0] * d[0] + p[1] * d[1]));
    const [plus, minus] = [reach(h.way), reach([-h.way[0], -h.way[1]])];
    const diff = (plus - minus) * k;
    if (Math.abs(diff) > EVEN_M && Math.abs(diff) < ONE_SIDED_M)
      return { ok: false, why: `its head reaches ${cm(plus * k)} one way and ${cm(minus * k)} the other: neither one-sided by ${cm(ONE_SIDED_M)} nor even within ${cm(EVEN_M)}` };
    side = diff >= ONE_SIDED_M ? h.way : diff <= -ONE_SIDED_M ? [-h.way[0], -h.way[1]] : undefined;
    how = side ? `one-sided by ${cm(Math.abs(diff))}` : "even";
    if (!side) {
      // Even, so either way is the same look turned half round: a length fit takes the whole
      // degrees in (-180, 0] (a source's widest way along X, the common case, then turns -90 as
      // the axes do, never ±90 by a hair of noise), a points fit the side that leaves its "out"
      // mostly positive.
      const flip: P2 = [-h.way[0], -h.way[1]];
      if (fit === "length") side = norm(Math.round(turnOf(h.way))) <= 0 ? h.way : flip;
      else {
        const o = cross(unit(add(scale(u, h.way[0]), scale(w, h.way[1]))), axis);
        const big = o.reduce((b, x, i) => (Math.abs(x) > Math.abs(o[b]) ? i : b), 0);
        side = o[big] * up > 0 ? h.way : flip;
      }
    }
  }
  // Its grip, from the butt, by its kind's rule: hafted a fist up its handle; blade and polearm
  // the middle of their grip or shaft.
  const grip = kind === "hafted" ? runFrom + HAFT_GRIP_M / k : (runFrom + runTo) / 2;
  const measures = `${said}; ${how}`;
  if (fit === "length") {
    const turn = side ? norm(Math.round(turnOf(side))) : 0;
    return { ok: true, turn, grip_from_end: r3(grip * k), measures };
  }
  // A points fit: its grip on the handle's line, toward its working end, its side to -Z.
  const s = up > 0 ? lo + grip : hi - grip;
  const at = add(add(scale(axis, s), scale(u, centre[0])), scale(w, centre[1]));
  const a = scale(axis, up);
  const t: V3 = side ? unit(add(scale(u, side[0]), scale(w, side[1]))) : u;
  const out = cross(t, a);
  return { ok: true, grip: at.map(r3) as V3, toward: add(at, scale(a, 0.5)).map(r3) as V3, out: out.map(r3) as V3, measures };
}

/** The turn about +Y, degrees, that brings a way (x, z) across a length fit's handle to -Z. */
function turnOf(d: P2): number {
  return norm(180 - (Math.atan2(d[0], d[1]) * 180) / Math.PI);
}

/** Degrees into (-180, 180]. */
function norm(deg: number): number {
  let d = deg % 360;
  if (d > 180) d -= 360;
  if (d <= -180) d += 360;
  return d + 0;
}

/** An item's source, read in its own glTF axes: every mesh placed by its node. */
export function sourceMesh(source: string): Mesh {
  const look = readLook(join(MODELS, source), source);
  return { points: look.points, tris: look.tris };
}

/** An item's data, art/held/<item>.json. */
export function heldData(item: string): Record<string, unknown> {
  return JSON.parse(readFileSync(join(ROOT, "art", "held", `${item}.json`), "utf8"));
}

/** What the detector finds for a held item, its data's own turn and grip ignored. */
export function detectItem(item: string): Found {
  const d = heldData(item);
  return detect(sourceMesh(d.source as string), d.kind as string, d.fit as string, d.length as number | undefined);
}

if (import.meta.main) {
  const items = process.argv.slice(2);
  if (!items.length) {
    console.error("usage: node tools/characters/detect.ts ITEM...  (art/held/<item>.json)");
    process.exit(2);
  }
  for (const item of items) {
    const f = detectItem(item);
    console.log(f.ok ? `✓ ${item}: ${JSON.stringify({ ...f, ok: undefined, measures: undefined })}  (${f.measures})` : `✗ ${item}: ${f.why}`);
  }
}
