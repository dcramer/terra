#!/usr/bin/env node
// The campfire (`hearth` in prop.json), a hero prop the player sits beside (D427), made as
// shipped games make theirs, from photoscans rather than generated: five charred logs leaned
// over its coal bed, a ring of fieldstones of mixed size seated unevenly round it, a few sticks,
// and the worn ground under it all, a hollow of grey ash and soot fading through trampled dirt
// into the grass out to 0.8 m (D513). Four rounds of a generated one left a harsh judge at 3/10
// against Red Dead Redemption 2's: an identical teepee lit like a leopard print, uniform potato
// stones necklace-spaced, a flat brown disc for its pit.
//
// Its sources are Poly Haven's scans (polyhaven.com, CC0: free to use, no credit needed):
// `dead_tree_trunk` cut into five sections for the logs, `rock_07`, `stone_01`, `rock_09` and
// `namaqualand_boulder_05` each reused at its own size, roll and tilt for the fieldstones, and
// `dry_branches_medium_01` for the sticks. `layout` says where each part lies;
// tools/blender/campfire.py cuts, scales and seats them there, chars the logs and bakes its maps;
// then coals.ts adds the coal bed. Its materials keep the glow contract (`props::Embers`,
// `staged::quench`): `Wood` (colour, normal, and an emissive map that is its glow's mask, the
// fissures only where it is charred and faces the fire), `Stone` (colour, normal), `Ground`
// (colour with a ragged alpha, glTF MASK), and the bed's `Coals`. The model contract holds:
// origin the middle of its base, front +Z, its catalogue `dims` (assets/data/builds.ron).
//
//   node tools/props/campfire.ts <out.glb>      (props.ts refine campfire runs it)
//
// The scans are fetched once into tools/props/out/scans, not committed. Node 22.18 or later.

import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { addBed, type Arm, random } from "./coals.ts";
import { fetchScan } from "./scans.ts";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const WORK = join(ROOT, "tools", "props", "out", "campfire");
const BLENDER = join(ROOT, "tools", "blender", "run.sh");

/// The triangles it may have, its coal bed's included: a hero prop's (art-style.md, D427).
export const BUDGET = 10_000;
/// The footprint's half width (its worn ground's reach, D513) and the height it is made within.
export const HALF = 0.8;
export const TALL = 0.55;
const LOGS = 5;
/// The first log's bearing; how far out a log's butt lies, least; and how far out it enters the
/// fire, where the flames are told its burning length starts (terra_world's `fire/hearth.rs`,
/// `Logs::CAMPFIRE`, which the test holds to this layout). The camp's eye, at the front (+Z,
/// bearing pi/2), looks in between two logs, a fifth of a turn either side: at 0.35 one log's
/// butt faced it square, its end read as a sawn stump (judge, round 5).
const TURN = 0.35 + Math.PI / 5;
const BUTT_R = 0.36;
export const BURN_R = 0.34;
/// How tall the stack stands, metres: 0.3-0.5 of the ring across.
export const STACK = 0.48;
/// The fieldstones: how many, the longest side of the middle-sized one, each one's size as a
/// share of it (0.75-1.4, mixed round the ring), and how far out their inner faces lie, least:
/// just past the logs' butts. Thirteen 0.2-0.4 m stones nearly touching close the ring, as
/// RDR2's 12-16 do; ten at uneven gaps read as a necklace of pebbles (judge, round 2), and at
/// 0.19-0.35 m, spaced by their longest sides, small and wide apart (judge, round 6).
const STONES = 13;
const MID = 0.28;
const SIZES = [1.0, 0.8, 1.3, 0.9, 1.15, 0.75, 1.4, 0.95, 1.2, 0.85, 1.25, 0.8, 1.1];
export const INNER_R = 0.45;
/// The ring's middle line, where a stone's long side lies along it.
const RING_R = 0.6;
/// The gaps between stones, metres: 1-2.5 cm, and one of 10 cm where the wood is fed in.
const GAP = [0.01, 0.025];
const FEED = 0.1;
/// Which scan each stone is cut from, and how many quarter turns it is rolled about its long side
/// first, so a scan used twice shows another face: the angular boulder and rock_07 most, stone_01
/// for a few, rock_09 a flat stone lying in the ring.
/// (stone_fire_pit's stones are fused into its ground, one loose part, so it is not used.)
const STONE_FROM: [string, number][] = [
  ["rock_07", 0], ["namaqualand_boulder_05", 0], ["stone_01", 0], ["rock_07", 1], ["namaqualand_boulder_05", 1], ["rock_09", 0], ["namaqualand_boulder_05", 2],
  ["rock_07", 2], ["stone_01", 2], ["namaqualand_boulder_05", 3], ["rock_07", 3], ["stone_01", 1], ["namaqualand_boulder_05", 0],
];
/// Where along the trunk scan each log is cut from (a share of its length), so no two show
/// the same bark.
const SECTIONS = [0.4, 0.02, 0.78, 0.21, 0.59];
/// How far each log bows at its middle, metres: a branch is seldom straight, and five straight
/// logs read as turned dowels (judge, round 4). Small beside its girth, so the flames' straight
/// line up it (`Logs::CAMPFIRE`) still lies inside it.
const BOWS = [0.012, -0.009, 0.007, -0.013, 0.01];
/// The triangles of each part: five logs, thirteen stones, three sticks and the ground, under the
/// budget with the coal bed's ~4,200. Their form is in their normal maps, baked from the full scans.
export const PART_BUDGET = { log: 400, stone: 170, stick: 120, ground: 600 };
/// The maps' sides: 2K for the wood and stone of a prop past 1.5 m (art-style.md), 1K for the
/// ground seen from above.
const MAPS = { wood: 2048, stone: 2048, ground: 1024 };

type V3 = [number, number, number];
const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const norm = (a: V3): V3 => {
  const l = Math.hypot(...a) || 1;
  return [a[0] / l, a[1] / l, a[2] / l];
};

/** A log: its radius, its butt's middle on the ground, the way it runs up into the stack and how
 * far, how it is turned about its run, where along the trunk scan it is cut from, and how far it
 * bows sideways at its middle (metres, the side its roll turns to). */
export type Log = { R: number; foot: V3; axis: V3; L: number; roll: number; from: number; flip: boolean; bow: number };
/** A stone: its scan and piece, its bearing round the ring, how far out its inner face lies, its
 * longest side, its turn off the ring's line, its tilt about the ring's line and across it
 * (radians), and how much of its height is sunk in the ground. */
export type Stone = { source: string; roll: number; bearing: number; inner: number; size: number; yaw: number; tilt: [number, number]; sink: number };
/** A stick lying on the ground: its piece of the branches scan and its two ends. */
export type Stick = { piece: number; a: V3; b: V3 };

/** Where its logs, stones and sticks lie, from a seed. */
export function layout(seed = 1): { logs: Log[]; stones: Stone[]; sticks: Stick[] } {
  const rnd = random(seed);
  const logs: Log[] = Array.from({ length: LOGS }, (_, i) => {
    // Logs 0.08-0.13 m through (a girth each, its mean the flames' `radius`), the thick ones
    // lowest: five of one 0.1 m girth read as tubes from one mould (judge, round 4), at 0.12-0.16 m
    // all through they dwarfed the stones (round 2). Leaned 20-50 deg into a stack `STACK` tall,
    // their butts on the ground inside the stones and their burnt ends crossing a little past the
    // middle at mixed heights; each bowed a centimetre or so sideways at its middle (`bow`, metres).
    const R = [0.05, 0.061, 0.043, 0.057, 0.04][i] + 0.004 * rnd();
    const phi = TURN + (i / LOGS) * 2 * Math.PI + 0.14 * (rnd() - 0.5);
    const rb = BUTT_R + 0.03 * rnd();
    const foot: V3 = [rb * Math.cos(phi), R * 0.55, rb * Math.sin(phi)];
    // Its end past the middle and turned on a little, so the logs cross rather than meet in a point.
    const pt = phi + 0.6 + 0.3 * rnd(), rt = 0.04 + 0.04 * rnd();
    const end: V3 = [rt * Math.cos(pt), STACK * [1, 0.84, 0.95, 0.78, 0.9][i] - 0.6 * R, rt * Math.sin(pt)];
    const roll = rnd() * 2 * Math.PI;
    return { R, foot, axis: norm(sub(end, foot)), L: Math.hypot(...sub(end, foot)), roll, from: SECTIONS[i], flip: i % 2 === 1, bow: BOWS[i] };
  });
  // The stones round the ring nearly touching: each takes the angle its long side spans at
  // `RING_R`, spaced so the gaps between them are `GAP`, one `FEED` where the wood is fed in
  // (their spans pressed together a little to fit, as a scan's ends taper). A third are sunk
  // deep, near half their height, the rest seated a tenth or so: sat on top they read unseated
  // (judge, round 6).
  const gaps = Array.from({ length: STONES }, (_, j) => (j === 3 ? FEED : GAP[0] + (GAP[1] - GAP[0]) * rnd()) / RING_R);
  const raw = SIZES.map((k) => 2 * Math.atan((MID * k) / (2 * RING_R)));
  const fit = (2 * Math.PI - gaps.reduce((a, g) => a + g, 0)) / raw.reduce((a, s) => a + s, 0);
  const spans = raw.map((s) => s * fit);
  let bearing = TURN + 0.9;
  const stones: Stone[] = [];
  const mids: [number, number][] = [];
  for (let j = 0; j < STONES; j++) {
    const [source, roll] = STONE_FROM[j];
    const sink = j % 3 === 0 ? 0.35 + 0.15 * rnd() : 0.08 + 0.1 * rnd();
    stones.push({ source, roll, bearing, inner: INNER_R + 0.03 * rnd(), size: MID * SIZES[j], yaw: 0.4 * (rnd() - 0.5), tilt: [0.5 * (rnd() - 0.5), 0.6 * (rnd() - 0.5)], sink });
    const gap = gaps[j];
    const next = bearing + spans[j] / 2 + gap + spans[(j + 1) % STONES] / 2;
    mids.push([bearing + spans[j] / 2 + gap / 2, gap]);
    bearing = next;
  }
  // Sticks in the three widest gaps: one laid in toward the fire, two lying across the gap.
  const wide = [...mids].sort((a, b) => b[1] - a[1]).slice(0, 3).map(([b]) => b);
  const at = (b: number, r: number, y = 0.012): V3 => [r * Math.cos(b), y, r * Math.sin(b)];
  const across = (b: number, r: number, half: number, skew: number): Stick["a"][] => {
    const [cx, , cz] = at(b, r), t = b + Math.PI / 2 + skew;
    return [[cx - half * Math.cos(t), 0.012, cz - half * Math.sin(t)], [cx + half * Math.cos(t), 0.012, cz + half * Math.sin(t)]];
  };
  const [a1, b1] = across(wide[1], 0.72, 0.16, 0.35), [a2, b2] = across(wide[2], 0.73, 0.13, -0.5);
  const sticks: Stick[] = [
    { piece: 0, a: at(wide[0] + 0.05, 0.5), b: at(wide[0] - 0.08, 0.77) },
    { piece: 1, a: a1, b: b1 },
    { piece: 2, a: a2, b: b2 },
  ];
  return { logs, stones, sticks };
}

/** Where the logs lie over the coal bed, seen from above: each one's line on the ground and its
 * half width, so the bed glows only under them (`coals.ts`, `under`). */
export function arms(seed = 1): Arm[] {
  return layout(seed).logs.map((l) => ({ a: [l.foot[0], l.foot[2]], b: [l.foot[0] + l.axis[0] * l.L, l.foot[2] + l.axis[2] * l.L], w: l.R }));
}

/** A glb with its JSON changed by `edit`, its binary chunk as it was. */
export function editGlb(glb: Buffer, edit: (json: any) => void): Buffer {
  const length = glb.readUInt32LE(12);
  const json = JSON.parse(glb.subarray(20, 20 + length).toString());
  edit(json);
  let text = Buffer.from(JSON.stringify(json));
  text = Buffer.concat([text, Buffer.alloc((4 - (text.length % 4)) % 4, 0x20)]);
  const rest = glb.subarray(20 + length);
  const head = Buffer.from(new Uint32Array([0x46546c67, 2, 12 + 8 + text.length + rest.length, text.length, 0x4e4f534a]).buffer);
  return Buffer.concat([head, text, rest]);
}

/** Makes the campfire into `out`: the scans fetched, assembled and baked in Blender, the
 * ground cut out by its alpha, the coal bed added. */
export async function campfire(out: string, seed = 1) {
  mkdirSync(WORK, { recursive: true });
  const here = (path: string) => relative(ROOT, path);
  const ids = ["dead_tree_trunk", "rock_07", "stone_01", "rock_09", "namaqualand_boulder_05", "dry_branches_medium_01"];
  const sources: Record<string, string> = {};
  for (const id of ids) sources[id] = here(await fetchScan(id, "2k"));
  const spec = join(WORK, "spec.json");
  writeFileSync(spec, JSON.stringify({ ...layout(seed), sources, budgets: PART_BUDGET, maps: MAPS, half: HALF, burn: BURN_R, work: here(WORK) }, null, 1));
  const result = spawnSync(BLENDER, [join(ROOT, "tools", "blender", "campfire.py"), here(spec), here(out)], { cwd: ROOT, stdio: "inherit" });
  if (result.status !== 0) throw new Error("Blender failed on the campfire");
  const cut = editGlb(readFileSync(out), (json) => {
    for (const m of json.materials) if (m.name === "Ground") Object.assign(m, { alphaMode: "MASK", alphaCutoff: 0.5 });
  });
  writeFileSync(out, addBed(cut, seed, arms(seed)));
  console.log(`campfire: ${here(out)}`);
}

if (import.meta.main) {
  const out = process.argv[2];
  if (!out) throw new Error("node tools/props/campfire.ts <out.glb>");
  await campfire(out);
}
