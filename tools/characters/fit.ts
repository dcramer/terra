#!/usr/bin/env node
// The fitter (D552, docs/attachment.md "Fitting a new item"): one command from an item's data
// to a checked look, a line a stage (D388), the makers' and the move check's output in
// tools/characters/out/fit-<item>.log.
//
//   1. read    its data (art/held/<item>.json, or art/gear/<item>/fit.json for a worn item), its
//              kind and its source
//   2. detect  a detector kind's turn and grip (detect.ts) into its data, unless its data gives a
//              turn or points, which always win; --turn DEG writes a person's turn first
//   3. make    the look by its kind's maker (fit_held.py, with its source_sha; a worn item
//              build_item.sh on a full build's bodies), against the committed file's bytes
//   4. check   attach_contract.ts on the look, its listed known faults allowed
//   5. moves   a held look in hand on every body template through every clip of its class
//              (moves.ts): its class's set with the set's partner in the other hand, else the
//              walker's clips, carried as the game carries; how deep the look goes into the body
//   6. show    its sheet and icon to make again, when the look's bytes changed
//
//   node tools/characters/fit.ts iron_axe            # a held item
//   node tools/characters/fit.ts quiver              # a worn item (needs build_bodies.sh's bodies)
//   node tools/characters/fit.ts hammer --turn 90    # a person's turn, kept in its data
//
// Exit 1 if a stage fails. Node 22.18 or later; no dependencies.

import { execFileSync, spawnSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";
import { KINDS, ROOT, against, holdOf, judge, knownFaults, readBody, readLook } from "./attach_contract.ts";
import { detect, sourceMesh } from "./detect.ts";
import { carriedClips } from "./held.ts";

const OUT = join(ROOT, "tools", "characters", "out");
/** A held look through the body, cm (moves.ts's HELD_CM). */
const HELD_CM = 1;
/** The walker's clips that use what is in hand (terra_walker::figure's Doing::clip), played
 * with the clips the game carries it in (held.ts's carriedClips), for a class with no set. */
const USED = ["Interact", "Sword_Attack", "Sword_Idle"];
const walkerClips = () => [...carriedClips(), ...USED];
/** moves.ts's own hands: the fighter's sword and shield, a set's partners. */
const PARTNER = { R: "assets/models/held/arming_sword.glb", L: "assets/models/held/heater_shield.glb" };

type Item = { name: string; worn: boolean; file: string; data: Record<string, unknown>; look: string };

/** A stage's line: ✓ or ✗, the item, the stage, what it found. */
function line(ok: boolean, item: string, stage: string, what: string): boolean {
  console.log(`${ok ? "✓" : "✗"} ${item} ${stage}: ${what}`);
  return ok;
}

/** One line a key, as the committed held data is written. */
function writeData(file: string, d: Record<string, unknown>) {
  const v = (x: unknown): string => (Array.isArray(x) ? `[${x.map(v).join(", ")}]` : x !== null && typeof x === "object" ? `{${Object.entries(x).map(([k, y]) => `${JSON.stringify(k)}: ${v(y)}`).join(", ")}}` : JSON.stringify(x));
  writeFileSync(file, `${v(d)}\n`);
}

function read(name: string): Item | string {
  const held = join(ROOT, "art", "held", `${name}.json`);
  const worn = join(ROOT, "art", "gear", name, "fit.json");
  const worn_ = existsSync(worn);
  if (!existsSync(held) && !worn_) return `no data: neither art/held/${name}.json nor art/gear/${name}/fit.json`;
  const file = worn_ ? worn : held;
  const data = JSON.parse(readFileSync(file, "utf8")) as Record<string, unknown>;
  if (!data.kind) return `${file.slice(ROOT.length + 1)} names no kind`;
  const source = worn_ ? join(ROOT, "art", "gear", name, "model.glb") : join(ROOT, "assets", "models", data.source as string);
  if (!worn_ && !existsSync(source)) return `its source ${source.slice(ROOT.length + 1)} is not there`;
  const look = join(ROOT, "assets", "models", worn_ ? "gear" : "held", `${name}.glb`);
  return { name, worn: worn_, file, data, look };
}

/** Detect, or keep what the data gives; writes what it finds into the data. */
function detectStage(it: Item, turn?: number): boolean {
  const d = it.data;
  if (it.worn) return line(true, it.name, "detect", `a ${d.kind} is fitted on the body by its data`);
  if (turn !== undefined) {
    d.turn = turn;
    writeData(it.file, d);
    return line(true, it.name, "detect", `a person's turn, ${turn}°, written into its data`);
  }
  if (d.fit !== "length" || d.turn !== undefined) return line(true, it.name, "detect", d.fit === "length" ? `its data's turn, ${d.turn}°, kept` : `its data's "${d.fit}" fit kept`);
  const f = detect(sourceMesh(d.source as string), d.kind as string, d.fit as string, d.length as number);
  if (!f.ok) return line(false, it.name, "detect", `${f.why}; set its turn once: fit.ts ${it.name} --turn DEG`);
  d.turn = f.turn;
  if (d.grip_from_end === undefined) d.grip_from_end = f.grip_from_end;
  writeData(it.file, d);
  return line(true, it.name, "detect", `turn ${f.turn}°, grip ${d.grip_from_end} m from its end, into its data (${f.measures})`);
}

/** What changed between two looks' files: nothing, or which of its parts. */
export function lookDiff(a: Buffer, b: Buffer): string {
  if (a.equals(b)) return "the committed file's bytes";
  const parts = (x: Buffer) => {
    const n = x.readUInt32LE(12);
    return { json: JSON.parse(x.subarray(20, 20 + n).toString()), bin: x.subarray(20 + n + 8) };
  };
  const [p, q] = [parts(a), parts(b)];
  const said: string[] = [p.bin.equals(q.bin) ? "its binary chunk the same" : `its binary chunk changed (${p.bin.length} bytes, was ${q.bin.length})`];
  const extras = (j: { nodes: { extras?: Record<string, unknown> }[] }) => Object.assign({}, ...j.nodes.map((n) => n.extras ?? {}));
  const [e, f] = [extras(p.json), extras(q.json)];
  for (const k of new Set([...Object.keys(e), ...Object.keys(f)]))
    if (JSON.stringify(e[k]) !== JSON.stringify(f[k])) said.push(k in f ? `extras ${k} ${JSON.stringify(e[k])}, was ${JSON.stringify(f[k])}` : `extras ${k} ${JSON.stringify(e[k])} added`);
  const rest = (j: Record<string, unknown>) => ({ ...j, nodes: (j.nodes as { extras?: unknown }[]).map((n) => ({ ...n, extras: undefined })) }) as Record<string, unknown>;
  const [r, s] = [rest(p.json), rest(q.json)];
  // Which of its glTF JSON's parts changed besides (a Blender name's .001 suffix shows in its meshes).
  const keys = [...new Set([...Object.keys(r), ...Object.keys(s)])].filter((k) => JSON.stringify(r[k]) !== JSON.stringify(s[k]));
  if (keys.length) said.push(`its glTF ${keys.join(", ")} otherwise changed`);
  return said.join("; ");
}

/** Make the look by its kind's maker; true and whether its bytes changed. */
function makeStage(it: Item, log: string): [boolean, boolean] {
  const was = (() => {
    try {
      return execFileSync("git", ["cat-file", "--filters", `HEAD:${it.look.slice(ROOT.length + 1)}`], { cwd: ROOT, maxBuffer: 1 << 30, stdio: ["ignore", "pipe", "ignore"] });
    } catch {
      return undefined;
    }
  })();
  const [cmd, args] = it.worn ? [join(ROOT, "tools", "characters", "build_item.sh"), [it.name]] : [join(ROOT, "tools", "blender", "run.sh"), [join(ROOT, "tools", "characters", "fit_held.py"), it.name]];
  const run = spawnSync(cmd, args, { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 });
  writeFileSync(log, `$ ${cmd} ${args.join(" ")}\n${run.stdout}${run.stderr}`);
  // build_item.sh's last line: "✓ ITEM pass: made, carried onto N bodies and checked, ...".
  const made = it.worn ? new RegExp(`^✓ ${it.name} pass\\b`, "m").test(run.stdout) :new RegExp(`^HELD ${it.name}\\b`, "m").test(run.stdout);
  if (run.status !== 0 || !made || !existsSync(it.look)) return [line(false, it.name, "make", `its maker failed; see ${log.slice(ROOT.length + 1)}`), false];
  const now = readFileSync(it.look);
  const diff = was ? lookDiff(now, was) : "a new look";
  return [line(true, it.name, "make", `${it.look.slice(ROOT.length + 1)}: ${diff}`), !was || !now.equals(was)];
}

function checkStage(it: Item): boolean {
  const look = readLook(it.look);
  const faults = judge(look, readBody());
  const { unlisted } = against(faults, knownFaults());
  if (unlisted.length) return line(false, it.name, "check", unlisted.map((f) => `${f.rule}: ${f.detail}`).join("; "));
  return line(true, it.name, "check", `the attachment contract${faults.length ? `, ${faults.length} known fault${faults.length > 1 ? "s" : ""} listed` : ""}`);
}

/** The set a class fights with, and the hand its item goes in (holding.md section 2): the off
 * hand for an off-hand grip or a kind that leads with it (a bow, attach_kinds.json's "lead"),
 * which then plays only the carried clips, as the walker's used ones swing the main hand. */
export function setOf(name: string): { set: string; hand: "R" | "L"; clips: string } {
  const hold = holdOf(`held/${name}`);
  const data = join(ROOT, "art", "held", `${name}.json`);
  const kind = existsSync(data) ? (JSON.parse(readFileSync(data, "utf8")) as { kind?: string }).kind : undefined;
  const hand = hold?.grip === "Off" || (kind && KINDS[kind]?.lead === "Off") ? "L" : "R";
  const sets = join(ROOT, "assets", "data", "moves");
  const families = readFileSync(join(sets, "families.ron"), "utf8");
  const family = hold && new RegExp(`"${hold.class}":\\s*"(\\w+)"`).exec(families)?.[1];
  const files = execFileSync("git", ["ls-files", "assets/data/moves"], { cwd: ROOT, encoding: "utf8" }).split("\n");
  // A shield fights in a set whose stance is "shielded".
  const set = hold && files.find((f) => [hold.class, family].includes(f.split("/").at(-1)!.split(".")[0]) || (hold.class === "shield" && f.endsWith(".shielded.ron")));
  return set ? { set, hand, clips: "" } : { set: "none", hand, clips: (hand === "L" ? carriedClips() : walkerClips()).join(",") };
}

/** A row moves.ts measures: a move of a set, or a clip, and what it measured. */
type MoveRow = { move?: string; clip?: string; measured?: Record<string, number> };

/** Runs `node <args>` (moves.ts, writing its rows to `json`), its output added to `log`: the
 * rows, or why there are none. The file of an earlier run is deleted first, so a run that
 * stopped before its rows (a missing clip once did) fails here and old rows are never read as
 * its own. moves.ts writes its rows and then exits 1 when some are over its own bars; those
 * rows are this run's and judged by fit's. */
export function runMoves(args: string[], json: string, log: string): MoveRow[] | string {
  rmSync(json, { force: true });
  const run = spawnSync(process.execPath, args, { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 });
  appendFileSync(log, `\n$ node ${args.join(" ")}\n${run.stdout}${run.stderr}`);
  if (existsSync(json)) return JSON.parse(readFileSync(json, "utf8")) as MoveRow[];
  const said = `${run.stderr ?? ""}\n${run.stdout ?? ""}`.split("\n").filter((l) => l.trim());
  const why = said.find((l) => /error/i.test(l)) ?? said.at(-1) ?? run.error?.message ?? "";
  const how = run.status === null ? `stopped by ${run.signal}` : `exit ${run.status}`;
  return `moves.ts wrote no rows (${how}): ${why.trim()}; see ${log.startsWith(ROOT) ? log.slice(ROOT.length + 1) : log}`;
}

function movesStage(it: Item, log: string, template?: string): boolean {
  const { set, hand, clips } = setOf(it.name);
  const look = it.look.slice(ROOT.length + 1);
  const hold = set === "none" ? [`socket.hand.${hand}=${look}`] : (["R", "L"] as const).map((h) => `socket.hand.${h}=${h === hand ? look : PARTNER[h]}`);
  const json = join(OUT, `fit-${it.name}-moves.json`);
  const args = [join(ROOT, "tools", "characters", "moves.ts"), "--set", set, "--carry", "--json", json, ...hold.flatMap((h) => ["--hold", h])];
  if (clips) args.push("--also", clips);
  if (template) args.push("--template", template);
  const rows = runMoves(args, json, log);
  if (typeof rows === "string") return line(false, it.name, "moves", rows);
  const key = `held_cm.${it.name}`;
  const deep = rows.filter((r) => r.measured?.[key] !== undefined).map((r) => ({ row: r.move ?? r.clip!, cm: r.measured![key] }));
  const over = deep.filter((r) => r.cm > HELD_CM).sort((a, b) => b.cm - a.cm);
  const worst = deep.reduce((a, b) => (b.cm > a.cm ? b : a), { row: "", cm: -Infinity });
  const where = `${deep.length} rows of ${set === "none" ? "the walker's clips" : set.split("/").at(-1)} in ${hand === "R" ? "the right" : "the left"} hand`;
  if (over.length) return line(false, it.name, "moves", `${where}: into the body ${over.map((r) => `${r.cm.toFixed(1)} cm in ${r.row}`).join(", ")} (bar ${HELD_CM} cm)`);
  return line(true, it.name, "moves", `${where}: ${worst.cm.toFixed(1)} cm deepest (${worst.row}), bar ${HELD_CM} cm`);
}

if (import.meta.main) {
  const { values: opt, positionals: items } = parseArgs({ allowPositionals: true, options: { turn: { type: "string" }, template: { type: "string" } } });
  if (!items.length) {
    console.error("usage: node tools/characters/fit.ts ITEM... [--turn DEG] [--template male,...]");
    process.exit(2);
  }
  mkdirSync(OUT, { recursive: true });
  let failed = 0;
  for (const name of items) {
    const it = read(name);
    if (typeof it === "string") {
      line(false, name, "read", it);
      failed++;
      continue;
    }
    line(true, name, "read", `${it.file.slice(ROOT.length + 1)}, a ${it.data.kind}`);
    const log = join(OUT, `fit-${name}.log`);
    if (!detectStage(it, opt.turn === undefined ? undefined : Number(opt.turn))) {
      failed++;
      continue;
    }
    const [made, changed] = makeStage(it, log);
    if (!made || !checkStage(it)) {
      failed++;
      continue;
    }
    if (it.worn) line(true, name, "moves", "a worn look has no held rows; build_item.sh's gear_check judged it on every body");
    else if (!movesStage(it, log, opt.template)) failed++;
    if (changed) line(true, name, "show", `its look changed: look at it (cargo xtask workbench --sheet ${it.worn ? "--wear" : "--hold"} ${name}) and paint its icon again (node tools/icons/paint.ts ${name}, or cargo xtask icons --keep)`);
    else line(true, name, "show", "its look's bytes are the committed ones: sheet and icon kept");
  }
  process.exit(failed ? 1 : 0);
}
