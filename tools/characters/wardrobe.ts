#!/usr/bin/env node
// The wardrobe check (D611): every body, piece and look of the catalogue
// (assets/models/character/wardrobe.json) as the game loads them, a line a stage (D388).
//
//   node tools/characters/wardrobe.ts
//
// - **catalogue**: every piece's source is there under art/, and every body's head and bare
//   pieces and every look's pieces are made on that body for that slot (the game's own reading,
//   `terra_walker::wardrobe::Wardrobe::parse`, refuses the rest).
// - **files**: every body's file and every piece's game file, with the buffer and textures it
//   names, is in assets/ (build_bodies.sh's `wardrobe:<body>` step writes them).
// - **intact**: each piece is theirs as made, or an exact cut of theirs (intact.ts, D599).
// - **bound**: each piece stands on its body's skeleton: every joint it names is the body's,
//   and its bind pose is the inverse of that joint's rest in the body's file (to 1e-4), so it
//   moves with him in the game and stands where it was made.
// - **looks**: every look in every state of undress, composed by the game's own rule
//   (`terra_walker::wardrobe::compose`, its tests run here): each slot filled or bare, every
//   region drawn once, no piece twice, no pairing the matrix refuses.
// - **pairings**: every two worn pieces of a body's different slots measured (pairings.ts writes
//   the rows; the game composes no refused one).
//
// Node 22.18 or later; no dependencies. Exits 1 on any failure, its details under the stage.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { gltfFiles, type Wardrobe, wardrobeData } from "./build_steps.ts";
import { Glb, matrix, mmul, posed } from "./glb.ts";
import { check, sources } from "./intact.ts";
import { figures, type Pairing, pairs } from "./pairings.ts";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const LOG = join(ROOT, "target", "check", "wardrobe.log");
const BIND = 1e-4;

type Stage = { name: string; faults: string[]; said: string };

function catalogue(w: Wardrobe): Stage {
  const faults: string[] = [];
  for (const [name, p] of Object.entries(w.pieces)) {
    if (!existsSync(join(ROOT, p.source))) faults.push(`${name}: its source ${p.source} is not there`);
    if (!w.bodies[p.body]) faults.push(`${name}: made on ${p.body}, which is not a body`);
  }
  const on = (piece: string, body: string, slot: string | undefined, what: string) => {
    const p = w.pieces[piece];
    if (!p) faults.push(`${what}: no piece ${piece}`);
    else if (p.body !== body || p.slot !== slot) faults.push(`${what}: ${piece} is made on ${p.body} for slot ${p.slot ?? "head"}`);
  };
  for (const [name, b] of Object.entries(w.bodies)) {
    on(b.head, name, undefined, `${name}'s head`);
    for (const s of w.slots) on(b.bare[s], name, s, `${name}'s bare ${s}`);
  }
  for (const [name, l] of Object.entries(w.looks)) for (const [s, p] of Object.entries(l.pieces)) on(p, l.body, s, `look ${name}'s ${s}`);
  const n = (o: object) => Object.keys(o).length;
  return { name: "catalogue", faults, said: `${n(w.bodies)} bodies, ${n(w.pieces)} pieces, ${n(w.looks)} looks` };
}

/** Every two worn pieces of a body's different slots have their measured row, so a new piece
 *  waits on the matrix (pairings.ts) before the game may compose it with another. */
function pairings(w: Wardrobe): Stage {
  const rows = (w as Wardrobe & { pairings?: Pairing[] }).pairings ?? [];
  const has = new Set(rows.map((r) => [...r.pieces].sort().join("|")));
  const want = pairs(w);
  const faults = want.filter(([a, b]) => !has.has([a, b].sort().join("|"))).map(([a, b]) => `${a} with ${b} is not measured: node tools/characters/pairings.ts`);
  for (const r of rows) for (const p of r.pieces) if (!w.pieces[p]) faults.push(`a pairing names ${p}, which is not a piece`);
  const n = (v: string) => rows.filter((r) => r.verdict === v).length;
  const dressed = Object.keys(w.bodies).map((b) => `${b} ${figures(w, rows, b).count}`);
  return { name: "pairings", faults, said: `${want.length} pairings measured: ${n("apart")} apart, ${n("theirs")} theirs, ${n("pass")} pass, ${n("refused")} refused; dressed figures they let through: ${dressed.join(", ")}` };
}

const pieceFile = (w: Wardrobe, piece: string) => `assets/${w.bodies[w.pieces[piece].body].pieces}/${piece}.gltf`;

function files(w: Wardrobe): Stage {
  const want = [...new Set([...Object.values(w.bodies).map((b) => `assets/${b.model}`), ...Object.keys(w.pieces).flatMap((p) => (existsSync(join(ROOT, pieceFile(w, p))) ? gltfFiles(pieceFile(w, p)) : [pieceFile(w, p)]))])];
  const faults = want.filter((f) => !existsSync(join(ROOT, f))).map((f) => `${f} is missing: build it, tools/characters/build_bodies.sh --step wardrobe:<body>`);
  return { name: "files", faults, said: `${want.length} files` };
}

function intact(w: Wardrobe): Stage {
  const srcs = sources();
  const faults: string[] = [];
  let theirs = 0;
  let cuts = 0;
  for (const piece of Object.keys(w.pieces)) {
    const r = check(new Glb(join(ROOT, pieceFile(w, piece))), srcs);
    theirs += r.theirs.length;
    cuts += r.cuts.length;
    if (!r.ok) faults.push(`${piece}: ${r.lines.join("; ")}`);
    if (r.ours.length) faults.push(`${piece}: ${r.ours.join(", ")} not theirs; a piece of ours goes in as its own row, never under their source`);
  }
  return { name: "intact", faults, said: `${theirs} meshes theirs as made, ${cuts} exact cuts` };
}

function bound(w: Wardrobe): Stage {
  const faults: string[] = [];
  let joints = 0;
  for (const [name, b] of Object.entries(w.bodies)) {
    const body = new Glb(join(ROOT, "assets", b.model));
    const rest = posed(body, undefined, 0);
    const world = new Map(body.gltf.nodes.map((n, i) => [n.name ?? "", matrix(rest[i])]));
    for (const piece of Object.keys(w.pieces).filter((p) => w.pieces[p].body === name)) {
      const g = new Glb(join(ROOT, pieceFile(w, piece)));
      let worst = 0;
      for (const skin of g.gltf.skins ?? []) {
        const ibm = g.read(skin.inverseBindMatrices);
        skin.joints.forEach((j, k) => {
          const joint = g.gltf.nodes[j].name ?? "";
          const at = world.get(joint);
          if (!at) return void faults.push(`${piece}: joint ${joint} is not on ${name}'s skeleton`);
          const m = mmul(at, ibm, k * 16);
          worst = Math.max(worst, ...m.map((x, i) => Math.abs(x - (i % 5 === 0 ? 1 : 0))));
          joints++;
        });
      }
      if (worst > BIND) faults.push(`${piece}: its bind poses are ${worst.toExponential(1)} off ${name}'s skeleton at rest (over ${BIND})`);
    }
  }
  return { name: "bound", faults, said: `${joints} joints of every piece on their body's skeleton` };
}

function looks(w: Wardrobe): Stage {
  const n = Object.values(w.looks).reduce((a, l) => a + 2 ** new Set(Object.values(l.pieces).map((p) => w.pieces[p]?.item)).size, 0);
  const args = ["test", "-q", "-p", "terra_walker", "--lib", "--", "wardrobe::"];
  try {
    const out = execFileSync("cargo", args, { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
    writeFileSync(LOG, out, { flag: "a" });
    return { name: "looks", faults: [], said: `${Object.keys(w.looks).length} looks in ${n} states of undress composed by the game's rule: every slot filled or bare, every region drawn once` };
  } catch (e) {
    const err = e as { stdout?: string; stderr?: string };
    writeFileSync(LOG, `${err.stdout ?? ""}${err.stderr ?? ""}`, { flag: "a" });
    return { name: "looks", faults: [`cargo ${args.join(" ")} failed; its output is in ${LOG}`], said: "" };
  }
}

if (import.meta.main) {
  mkdirSync(dirname(LOG), { recursive: true });
  writeFileSync(LOG, "");
  const w = wardrobeData(ROOT);
  let failed = false;
  for (const stage of [catalogue, pairings, files, intact, bound, looks]) {
    const s = stage(w);
    console.log(s.faults.length ? `✗ ${s.name}\n  ${s.faults.join("\n  ")}` : `✓ ${s.name}: ${s.said}`);
    if (s.faults.length) {
      failed = true;
      if (s.name === "files") break;
    }
  }
  console.log(failed ? "WARDROBE fail" : "WARDROBE pass");
  process.exit(failed ? 1 : 0);
}
