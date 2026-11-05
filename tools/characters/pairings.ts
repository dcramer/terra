#!/usr/bin/env node
// The pairing matrix (D611): every worn piece of a wardrobe body against every worn piece of its
// other slots, measured through the clips as layers.ts measures one piece over another, and
// written into the catalogue's `pairings` (assets/models/character/wardrobe.json), never by hand.
// The game composes no refused pairing (`terra_walker::wardrobe::compose`), nor may a look.
//
//   node tools/characters/pairings.ts [--frames N] [--dry] [--new]
//
// --new measures only the pairs of a piece no row names yet and of a piece whose outfit gained one
// (their own pairings judge the mixed ones), keeping every other row (toMeasure).
//
// For each two pieces: **apart** when neither comes within 8 cm of the other at rest. Else the
// outer is the one the other pokes through least at rest (as made, the tunic over the trousers,
// the boot over the hem); then through the mannequin's clips, `frames` moments each, the inner's
// vertices near the outer and its open edges within 8 cm of it (`layers.ts`, edges "near"): a
// poke past 2 mm, and the edges the outer leaves open. A clip is **bad** when it pokes past 5 mm
// or opens more edge than at rest (layers.ts's line). Their own outfits poke too (every pairing
// of theirs 40 to 77 mm in the roll and crouch, research/2026-11-02-character-base.md), so a
// pairing of two outfits is judged against the worse of the two outfits' own pairings of the
// same slots, as made: it **passes** when it has no more bad clips and no more vertices poking
// at rest than that; else it is **refused**, with its worst numbers. Pieces of one outfit are
// **theirs** and pass.
//
// A line a stage (D388); each pairing's numbers in the catalogue and in out/pairings.log.
// Node 22.18 or later; no dependencies.

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { WARDROBE, type Wardrobe } from "./build_steps.ts";
import { measure, pair } from "./layers.ts";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const LOG = join(ROOT, "tools", "characters", "out", "pairings.log");
const BAD_POKE_M = 0.005;

type Piece = Wardrobe["pieces"][string] & { outfit?: string | string[]; colourways?: string[] };

/** The outfits a piece belongs to as made (one of theirs may be in two: the Knight's arms). */
const outfits = (w: Wardrobe, p: string): string[] => [(w.pieces[p] as Piece).outfit ?? []].flat();

/** A piece that may be worn or shown with others: one with an item, or one of an optional slot
 *  (hair, a beard: the person's own); a bare piece is neither. */
const shows = (w: Wardrobe, p: string) => !!w.pieces[p].slot && (!!w.pieces[p].item || (w.optional ?? []).includes(w.pieces[p].slot!));

/** A row of the matrix, as the catalogue holds it. */
export type Pairing = {
  pieces: [string, string];
  verdict: "apart" | "theirs" | "pass" | "refused";
  over?: string;
  clips?: number;
  worse?: number;
  theirs_rest?: number;
  rest?: number;
  poke_mm?: number;
  worst?: string;
  open?: number;
  frames?: number;
};

/** Every two worn pieces (hair and beards among them) of one body in different slots, in a
 *  fixed order; never two that hold one region, which the game never shows together (a hood
 *  hides the hair). */
export function pairs(w: Wardrobe): [string, string][] {
  const worn = Object.keys(w.pieces)
    .filter((p) => shows(w, p))
    .sort();
  const out: [string, string][] = [];
  for (let i = 0; i < worn.length; i++)
    for (let j = i + 1; j < worn.length; j++) {
      const [a, b] = [w.pieces[worn[i]], w.pieces[worn[j]]];
      if (a.body === b.body && a.slot !== b.slot && !a.regions.some((r) => b.regions.includes(r))) out.push([worn[i], worn[j]]);
    }
  return out;
}

/** One pairing measured: the outer chosen at rest, then the clips. */
function measured(w: Wardrobe, a: string, b: string, frames: number) {
  const file = (p: string) => join(ROOT, w.pieces[p].source);
  const ab = pair(file(a), file(b), undefined, "near");
  const ba = pair(file(b), file(a), undefined, "near");
  if (!ab.near.length && !ba.near.length) return undefined;
  const [restAB, restBA] = [ab.one(undefined, 0), ba.one(undefined, 0)];
  const aOver = restAB.poking < restBA.poking || (restAB.poking === restBA.poking && restAB.uncovered <= restBA.uncovered);
  const [outer, inner] = aOver ? [a, b] : [b, a];
  const r = measure(file(outer), file(inner), undefined, frames, "near");
  const bad = [...r.clips].filter(([, c]) => c.poke > BAD_POKE_M || c.open > r.rest.uncovered);
  return {
    over: outer,
    clips: bad.length,
    rest: r.rest.poking,
    poke_mm: Math.round(r.worst.worst * 10000) / 10,
    worst: `${r.worst.clip}@${r.worst.t.toFixed(2)}`,
    open: r.open.uncovered,
    frames: r.frames,
    edge: r.rest.edge,
    bad: bad.map(([n, c]) => `${n} ${(c.poke * 1000).toFixed(0)}/${c.open}`),
    each: r.clips,
  };
}

/** The pairs `matrix` must measure again when `kept` holds the rest: those of a piece no kept row
 *  names (new), and those of a piece whose outfit has a new piece, whose own pairings judge them. */
export function toMeasure(w: Wardrobe, kept: Pairing[]): Set<string> {
  const named = new Set(kept.flatMap((r) => r.pieces));
  const fresh = Object.keys(w.pieces).filter((p) => shows(w, p) && !named.has(p));
  const touched = new Set(fresh.flatMap((p) => outfits(w, p)));
  const again = (p: string) => !named.has(p) || outfits(w, p).some((o) => touched.has(o));
  return new Set(pairs(w).filter(([a, b]) => again(a) || again(b)).map(([a, b]) => `${a}|${b}`));
}

/** The matrix: each pair measured, mixed ones judged against their outfits' own. With `kept` (the
 *  catalogue's rows), only `toMeasure`'s pairs are measured, with the pairings that judge them;
 *  the rest keep their rows. */
export function matrix(w: Wardrobe, frames = 12, log: string[] = [], kept?: Pairing[]): Pairing[] {
  const all = pairs(w);
  const runs = new Map<string, ReturnType<typeof measured>>();
  const run = (a: string, b: string) => {
    const [x, y] = a < b ? [a, b] : [b, a];
    const key = `${x}|${y}`;
    if (!runs.has(key)) runs.set(key, measured(w, x, y, frames));
    return runs.get(key);
  };
  const again = kept ? toMeasure(w, kept) : null;
  const old = new Map((kept ?? []).map((r) => [[...r.pieces].sort().join("|"), r]));
  const ofOutfit = (os: string[], body: string, slot: string | undefined) =>
    Object.keys(w.pieces).find((p) => w.pieces[p].body === body && w.pieces[p].slot === slot && w.pieces[p].item && outfits(w, p).some((o) => os.includes(o)));
  return all.map(([a, b]): Pairing => {
    const had = old.get(`${a}|${b}`);
    if (again && had && !again.has(`${a}|${b}`)) return had;
    const m = run(a, b);
    const [pa, pb] = [w.pieces[a], w.pieces[b]];
    const own = outfits(w, a).some((o) => outfits(w, b).includes(o));
    if (m) log.push(`${m.over === a ? a : b} over ${m.over === a ? b : a}: ${m.clips} bad clips of ${m.frames / frames}, ${m.rest} poking at rest, worst ${m.poke_mm} mm ${m.worst}, open edge worst ${m.open}/${m.edge}; ${m.bad.join(", ") || "none bad"}`);
    if (!m) return { pieces: [a, b], verdict: "apart" };
    const row = { over: m.over, clips: m.clips, rest: m.rest, poke_mm: m.poke_mm, worst: m.worst, open: m.open, frames: m.frames };
    if (own) return { pieces: [a, b], verdict: "theirs", ...row };
    // The two outfits' own pairings of these slots: a's outfit in b's slot, b's in a's.
    const refs = [
      [a, ofOutfit(outfits(w, a), pb.body, pb.slot)],
      [ofOutfit(outfits(w, b), pa.body, pa.slot), b],
    ].flatMap(([x, y]) => (x && y && x !== y ? [run(x, y)] : []));
    const { worse, restTheirs, ok } = judge(m, refs);
    log.push(`  against theirs (${refs.length} measured): ${worse.length} clips worse${worse.length ? ` (${worse.join(", ")})` : ""}, ${m.rest} poking at rest against their ${restTheirs}: ${ok ? "pass" : "refused"}`);
    return { pieces: [a, b], verdict: ok ? "pass" : "refused", ...row, worse: worse.length, theirs_rest: restTheirs };
  });
}

/** A pairing's measure: each clip's worst poke (m) and edge left open, and the inner's vertices
 *  poking at rest. */
export type Measure = { each: Map<string, { poke: number; open: number }>; rest: number };

/** A mixed pairing against its outfits' own (`refs`, undefined where theirs are apart). A clip is
 *  worse when it pokes past 5 mm where the worse of theirs stays within it (a poke mixing adds;
 *  one in a clip theirs poke in already is how their outfits behave), or leaves more of the edge
 *  open than the more open of theirs. It passes with no clip worse and no more poking at rest. */
export function judge(m: Measure, refs: (Measure | undefined)[]) {
  const worse = [...m.each]
    .filter(([clip, c]) => {
      const t = refs.map((r) => r?.each.get(clip) ?? { poke: 0, open: 0 });
      return (c.poke > BAD_POKE_M && Math.max(0, ...t.map((x) => x.poke)) <= BAD_POKE_M) || c.open > Math.max(0, ...t.map((x) => x.open));
    })
    .map(([clip]) => clip);
  const restTheirs = Math.max(0, ...refs.map((r) => r?.rest ?? 0));
  return { worse, restTheirs, ok: !worse.length && m.rest <= restTheirs };
}

/** The figures the matrix lets a body wear: a piece a slot (worn, or bare with `bare`), every two
 *  of them passing, and how many colourways those come in. */
export function figures(w: Wardrobe, rows: Pairing[], body: string, bare = false) {
  const refused = new Set(rows.filter((r) => r.verdict === "refused").map((r) => r.pieces.join("|")));
  const ok = (a: string, b: string) => !refused.has(a < b ? `${a}|${b}` : `${b}|${a}`);
  // An optional slot may be empty (""), and a piece in one is not chosen over a region a piece
  // chosen holds (the game hides the hair under a hood: that figure is the hood's alone).
  const optional = w.optional ?? [];
  const choices = [...w.slots, ...optional].map((s) => [
    ...Object.keys(w.pieces).filter((p) => w.pieces[p].body === body && w.pieces[p].slot === s && shows(w, p)),
    ...(bare && !optional.includes(s) ? [w.bodies[body].bare[s]] : []),
    ...(optional.includes(s) ? [""] : []),
  ]);
  const apart = (a: string, b: string) => !a || !b || !w.pieces[a].regions.some((r) => w.pieces[b].regions.includes(r));
  let count = 0;
  let coloured = 0;
  const walk = (k: number, chosen: string[]) => {
    if (k === choices.length) {
      count++;
      coloured += chosen.filter((p) => p).reduce((n, p) => n * Math.max(1, (w.pieces[p] as Piece).colourways?.length ?? 1), 1);
      return;
    }
    for (const p of choices[k]) if (chosen.every((c) => !p || !c || (ok(c, p) && apart(c, p)))) walk(k + 1, [...chosen, p]);
  };
  walk(0, []);
  const all = choices.reduce((n, c) => n * c.length, 1);
  return { count, all, coloured };
}

/** The catalogue's text with its `pairings` replaced, a row a line, the rest as written. */
export function written(text: string, rows: Pairing[]): string {
  const body = rows.length ? `[\n${rows.map((r) => `    ${JSON.stringify(r)}`).join(",\n")}\n  ]` : "[]";
  const at = text.indexOf('"pairings":');
  const end = text.indexOf('"populations":', at);
  if (at < 0 || end < 0) throw new Error(`pairings: ${WARDROBE} has no "pairings" before "populations"`);
  return `${text.slice(0, at)}"pairings": ${body},\n  ${text.slice(end)}`;
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const frames = args.includes("--frames") ? Number(args[args.indexOf("--frames") + 1]) : 12;
  const path = join(ROOT, WARDROBE);
  const text = readFileSync(path, "utf8");
  const w: Wardrobe = JSON.parse(text);
  const log: string[] = [];
  const kept = args.includes("--new") ? ((w as unknown as { pairings?: Pairing[] }).pairings ?? []) : undefined;
  const again = kept ? toMeasure(w, kept).size : null;
  const rows = matrix(w, frames, log, kept);
  const n = (v: Pairing["verdict"]) => rows.filter((r) => r.verdict === v).length;
  // Each refused pairing's numbers go in the log, never the terminal (D388).
  log.push("", ...rows.filter((r) => r.verdict === "refused").map((r) => `refused ${r.over} over ${r.pieces.find((p) => p !== r.over)}: ${r.worse} clips worse than theirs, ${r.rest} poking at rest against their ${r.theirs_rest}, worst ${r.poke_mm} mm ${r.worst}`));
  mkdirSync(dirname(LOG), { recursive: true });
  writeFileSync(LOG, `${log.join("\n")}\n`);
  console.log(`✓ measured: ${rows.length} pairings${again === null ? "" : ` (${again} measured again, the rest kept: --new)`}, ${n("apart")} apart, ${n("theirs")} theirs, ${n("pass")} pass, ${n("refused")} refused (${frames} moments a clip; each in ${LOG.slice(ROOT.length)})`);
  for (const body of Object.keys(w.bodies)) {
    const dressed = figures(w, rows, body);
    const any = figures(w, rows, body, true);
    console.log(`✓ figures of ${body}: ${dressed.count} of ${dressed.all} dressed a piece a slot (${dressed.coloured} in their colourways), ${any.count} of ${any.all} with bare slots, hair and headwear among the slots`);
  }
  if (args.includes("--dry")) console.log("  --dry: the catalogue is unchanged");
  else {
    writeFileSync(path, written(text, rows));
    console.log(`✓ wrote ${WARDROBE}'s pairings`);
  }
}
