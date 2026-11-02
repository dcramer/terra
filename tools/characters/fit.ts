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
//   4. check   attach_contract.ts on the look, its listed known faults allowed, and a bow's nock
//              on its string (nock.ts); a fault names the field of its data and the value to set
//   5. moves   a held look in hand on every body template through every clip of its class
//              (moves.ts, a template a process): its class's set with the set's partner in the
//              other hand, else the walker's clips, carried as the game carries; how deep the
//              look goes into the body, and which of its points, where and when
//   6. show    its sheet and icon to make again, when the look's bytes changed
//
// The make and moves stages are keyed on everything they read (fit_cache.ts: the item's data,
// its source's bytes, their scripts and every script those reach, the bodies and clips), so one
// whose inputs have not changed is not run again but its result put back. Items run side by
// side, and within an item the move check's templates, a process a core.
//
//   node tools/characters/fit.ts iron_axe            # a held item
//   node tools/characters/fit.ts quiver              # a worn item (needs build_bodies.sh's bodies)
//   node tools/characters/fit.ts --all               # every item with data
//   node tools/characters/fit.ts hammer --turn 90    # a person's turn, kept in its data
//   node tools/characters/fit.ts knife --fresh       # every stage run, the cache not read
//
// Exit 1 if a stage fails. Node 22.18 or later; no dependencies.

import { execFileSync, spawn } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { availableParallelism } from "node:os";
import { join } from "node:path";
import { parseArgs } from "node:util";
import { KINDS, ROOT, type Fault, against, holdOf, judge, knownFaults, readBody, readLook } from "./attach_contract.ts";
import { detect, sourceMesh } from "./detect.ts";
import { Cache, closure, key } from "./fit_cache.ts";
import { carriedClips } from "./held.ts";
import { nockOf } from "./nock.ts";

const OUT = join(ROOT, "tools", "characters", "out");
const CHARS = join(ROOT, "tools", "characters");
/** A held look through the body, cm (moves.ts's HELD_CM). */
const HELD_CM = 1;
/** A bow's nock off its string's middle as made, m (nock.ts). */
const NOCK_M = 0.005;
/** The walker's clips that use what is in hand (terra_walker::figure's Doing::clip), played
 * with the clips the game carries it in (held.ts's carriedClips), for a class with no set. */
const USED = ["Interact", "Sword_Attack", "Sword_Idle"];
const walkerClips = () => [...carriedClips(), ...USED];
/** moves.ts's own hands: the fighter's sword and shield, a set's partners. */
const PARTNER = { R: "assets/models/held/arming_sword.glb", L: "assets/models/held/heater_shield.glb" };
const rel = (f: string) => (f.startsWith(ROOT) ? f.slice(ROOT.length + 1) : f);

type Item = { name: string; worn: boolean; file: string; data: Record<string, unknown>; look: string };
/** One item's run: its lines, printed together when it is done, and its log. */
type Ctx = { cache: Cache; fresh: boolean; templates: string[]; lines: string[]; log: string };

// Processes side by side, as many as the machine has cores less two (a Blender, a move check each).
let free = Math.max(2, availableParallelism() - 2);
const queued: (() => void)[] = [];
async function slot<T>(f: () => Promise<T>): Promise<T> {
  if (free > 0) free--;
  else await new Promise<void>((r) => queued.push(r));
  try {
    return await f();
  } finally {
    const next = queued.shift();
    if (next) next();
    else free++;
  }
}
type Ran = { status: number | null; signal: string | null; stdout: string; stderr: string; error?: string };
/** Runs a command in a slot, its output kept whole. */
const exec = (cmd: string, args: string[]) =>
  slot(
    () =>
      new Promise<Ran>((done) => {
        const p = spawn(cmd, args, { cwd: ROOT, stdio: ["ignore", "pipe", "pipe"] });
        const [out, err]: Buffer[][] = [[], []];
        let error: string | undefined;
        p.stdout.on("data", (b) => out.push(b));
        p.stderr.on("data", (b) => err.push(b));
        p.on("error", (e) => (error = e.message));
        p.on("close", (status, signal) => done({ status, signal, stdout: Buffer.concat(out).toString(), stderr: Buffer.concat(err).toString(), error }));
      }),
  );

/** A stage's line: ✓ or ✗, the item, the stage, what it found, and its time or that it was kept. */
function line(ctx: Ctx, ok: boolean, item: string, stage: string, what: string, since?: number): boolean {
  const took = since === undefined ? "" : since < 0 ? " [cached]" : ` [${((performance.now() - since) / 1000).toFixed(1)} s]`;
  ctx.lines.push(`${ok ? "✓" : "✗"} ${item} ${stage}: ${what}${took}`);
  return ok;
}

/** One line a key, as the committed held data is written. */
function writeData(file: string, d: Record<string, unknown>) {
  const v = (x: unknown): string => (Array.isArray(x) ? `[${x.map(v).join(", ")}]` : x !== null && typeof x === "object" ? `{${Object.entries(x).map(([k, y]) => `${JSON.stringify(k)}: ${v(y)}`).join(", ")}}` : JSON.stringify(x));
  writeFileSync(file, `${v(d)}\n`);
}

/** Every item with data: art/held/*.json and art/gear/<item>/fit.json, sorted. */
export function allItems(root = ROOT): string[] {
  const held = readdirSync(join(root, "art", "held")).filter((f) => f.endsWith(".json")).map((f) => f.slice(0, -5));
  const worn = readdirSync(join(root, "art", "gear")).filter((d) => existsSync(join(root, "art", "gear", d, "fit.json")));
  return [...held, ...worn].sort();
}

function read(name: string): Item | string {
  const held = join(ROOT, "art", "held", `${name}.json`);
  const worn = join(ROOT, "art", "gear", name, "fit.json");
  const worn_ = existsSync(worn);
  if (!existsSync(held) && !worn_) return `no data: neither art/held/${name}.json nor art/gear/${name}/fit.json`;
  const file = worn_ ? worn : held;
  const data = JSON.parse(readFileSync(file, "utf8")) as Record<string, unknown>;
  if (!data.kind) return `${rel(file)} names no kind`;
  const source = worn_ ? join(ROOT, "art", "gear", name, "model.glb") : join(ROOT, "assets", "models", data.source as string);
  if (!worn_ && !existsSync(source)) return `its source ${rel(source)} is not there`;
  const look = join(ROOT, "assets", "models", worn_ ? "gear" : "held", `${name}.glb`);
  return { name, worn: worn_, file, data, look };
}

/** Detect, or keep what the data gives; writes what it finds into the data. */
function detectStage(ctx: Ctx, it: Item, turn?: number): boolean {
  const d = it.data;
  if (it.worn) return line(ctx, true, it.name, "detect", `a ${d.kind} is fitted on the body by its data`);
  if (turn !== undefined) {
    d.turn = turn;
    writeData(it.file, d);
    return line(ctx, true, it.name, "detect", `a person's turn, ${turn}°, written into its data`);
  }
  if (d.fit !== "length" || d.turn !== undefined) return line(ctx, true, it.name, "detect", d.fit === "length" ? `its data's turn, ${d.turn}°, kept` : `its data's "${d.fit}" fit kept`);
  const f = detect(sourceMesh(d.source as string), d.kind as string, d.fit as string, d.length as number);
  if (!f.ok) return line(ctx, false, it.name, "detect", `${f.why}; set its turn once: fit.ts ${it.name} --turn DEG`);
  d.turn = f.turn;
  if (d.grip_from_end === undefined) d.grip_from_end = f.grip_from_end;
  writeData(it.file, d);
  return line(ctx, true, it.name, "detect", `turn ${f.turn}°, grip ${d.grip_from_end} m from its end, into its data (${f.measures})`);
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

/** What a look's maker reads: its scripts and every script they reach, Blender's version, the
 * item's data and source; for a worn item every worn item's data (their layering), its own
 * files and the full build's bodies. */
export function makeInputs(it: Pick<Item, "name" | "worn" | "file" | "data">, root = ROOT): string[] {
  const chars = join(root, "tools", "characters");
  const blender = join(root, "tools", "blender", "version");
  if (!it.worn) return [...closure(join(chars, "fit_held.py"), root), blender, it.file, join(root, "assets", "models", it.data.source as string)];
  const gear = join(root, "art", "gear");
  const out = join(chars, "out");
  const bodies = existsSync(out) ? readdirSync(out).filter((f) => f.endsWith("-game.glb") || (f.endsWith(".npz") && !f.endsWith("-push.npz"))).map((f) => join(out, f)) : [];
  const data = readdirSync(gear).map((d) => join(gear, d, "fit.json")).filter((f) => existsSync(f));
  const own = readdirSync(join(gear, it.name)).map((f) => join(gear, it.name, f));
  return [...closure(join(chars, "build_item.sh"), root), blender, ...data, ...own, ...bodies];
}

/** Make the look by its kind's maker, or put back the one its inputs made before; true, and
 * whether its bytes differ from the committed file's. */
async function makeStage(ctx: Ctx, it: Item): Promise<[boolean, boolean]> {
  const t0 = performance.now();
  const was = (() => {
    try {
      return execFileSync("git", ["cat-file", "--filters", `HEAD:${rel(it.look)}`], { cwd: ROOT, maxBuffer: 1 << 30, stdio: ["ignore", "pipe", "ignore"] });
    } catch {
      return undefined;
    }
  })();
  const k = key("make", ROOT, makeInputs(it));
  const kept = ctx.fresh ? undefined : ctx.cache.get(k, ROOT);
  if (!kept) {
    const [cmd, args] = it.worn ? [join(CHARS, "build_item.sh"), [it.name]] : [join(ROOT, "tools", "blender", "run.sh"), [join(CHARS, "fit_held.py"), it.name]];
    const run = await exec(cmd, args);
    appendFileSync(ctx.log, `$ ${cmd} ${args.join(" ")}\n${run.stdout}${run.stderr}\n`);
    // build_item.sh's last line: "✓ ITEM pass: made, carried onto N bodies and checked, ...".
    const made = it.worn ? new RegExp(`^✓ ${it.name} pass\\b`, "m").test(run.stdout) : new RegExp(`^HELD ${it.name}\\b`, "m").test(run.stdout);
    if (run.status !== 0 || !made || !existsSync(it.look)) return [line(ctx, false, it.name, "make", `its maker failed; see ${rel(ctx.log)}`, t0), false];
    ctx.cache.put(k, ROOT, { ok: true, line: "" }, [it.look]);
  }
  const now = readFileSync(it.look);
  const diff = was ? lookDiff(now, was) : "a new look";
  return [line(ctx, true, it.name, "make", `${rel(it.look)}: ${diff}`, kept ? -1 : t0), !was || !now.equals(was)];
}

/** What sets a fault right: the field of the item's data and its value, where one does; `found`
 * the detector's measures of its source, when it has them. */
export function remedy(f: Fault, it: Pick<Item, "name" | "file" | "data">, found?: { turn?: number; grip_from_end?: number }): string {
  const file = rel(it.file);
  const d = it.data;
  const turnBy = (deg: number) => ((((((d.turn as number) ?? 0) + deg + 180) % 360) + 360) % 360) - 180;
  switch (f.rule) {
    case "facing":
      if (d.fit === "points") return `set its "points" in ${file} so its working end lies in Y-Z, toward -Z`;
      if (/ leans /.test(f.detail)) return `set "turn" ${turnBy(180)} in ${file}`;
      if (found?.turn !== undefined && found.turn !== d.turn) return `set "turn" ${found.turn} in ${file} (the detector's)`;
      return `set "turn" ${turnBy(90)} or ${turnBy(-90)} in ${file}, whichever puts its working end toward -Z`;
    case "grip":
      return found?.grip_from_end !== undefined ? `set "grip_from_end" ${found.grip_from_end} in ${file} (the detector's)` : `set "grip_from_end" in ${file} to where the fist closes, m from its butt`;
    case "size":
      return `set "length" in ${file} within its kind's range`;
    case "hold": {
      const m = /hold\.length is ([\d.]+) m/.exec(f.detail);
      return m ? `set "length" ${m[1]} in ${file}, or its item's hold.length in items.ron to its look's` : `set "kind" in ${file} to a kind whose classes hold it, or its hold.class in items.ron`;
    }
    case "second":
      return `set "second" in ${file}: m along +Y from the grip where the other hand closes, on its handle (holding.md section 3)`;
    case "nock": {
      const m = /string's middle (\[[^\]]+\])/.exec(f.detail);
      return m ? `set "nock" ${m[1]} in ${file}` : `give its look a String material (nock.ts measures its middle)`;
    }
    case "stale":
    case "marker":
      return `make it again: node tools/characters/fit.ts ${it.name} --fresh`;
    default:
      return `in ${file}`;
  }
}

/** A bow's nock off its string as made (nock.ts, the middle of its String at rest), over 5 mm. */
export function nockFault(it: Pick<Item, "name" | "data" | "look">): Fault | undefined {
  if (!Array.isArray(it.data.nock)) return undefined;
  const at = nockOf(it.look);
  const want = it.data.nock as number[];
  if (!at) return { look: `held/${it.name}`, rule: "nock", detail: "its look has no String material to measure its nock on" };
  const off = Math.hypot(...at.map((v, i) => v - want[i]));
  if (off <= NOCK_M) return undefined;
  return { look: `held/${it.name}`, rule: "nock", detail: `its data's nock [${want.join(", ")}] is ${(off * 100).toFixed(1)} cm off its string's middle [${at.join(", ")}] (bar ${NOCK_M * 100} cm)` };
}

function checkStage(ctx: Ctx, it: Item): boolean {
  const look = readLook(it.look);
  const faults = judge(look, readBody());
  const nock = it.worn ? undefined : nockFault(it);
  if (nock) faults.push(nock);
  const { unlisted } = against(faults, knownFaults());
  if (unlisted.length) {
    const found = (() => {
      if (it.worn || it.data.fit !== "length") return undefined;
      const f = detect(sourceMesh(it.data.source as string), it.data.kind as string, "length", it.data.length as number);
      return f.ok ? f : undefined;
    })();
    return line(ctx, false, it.name, "check", unlisted.map((f) => `${f.rule}: ${f.detail}; ${remedy(f, it, found)}`).join("; "));
  }
  const listed = faults.length ? `, ${faults.length} known fault${faults.length > 1 ? "s" : ""} listed` : "";
  return line(ctx, true, it.name, "check", `the attachment contract${listed}${Array.isArray(it.data.nock) ? ", its nock on its string" : ""}`);
}

/** The set a class fights with, and the hand its item goes in (holding.md section 2): the off
 * hand for an off-hand grip or a kind that leads with it (a bow, attach_kinds.json's "lead"),
 * which then plays only the carried clips, as the walker's used ones swing the main hand. */
export function setOf(name: string): { set: string; hand: "R" | "L"; clips: string } {
  const hold = holdOf(`held/${name}`);
  const data = join(ROOT, "art", "held", `${name}.json`);
  const { kind, strap } = existsSync(data) ? (JSON.parse(readFileSync(data, "utf8")) as { kind?: string; strap?: { fist?: string } }) : {};
  // A strapped look rides its strap's fist (a shield's left), whether or not an item names it.
  const hand = strap?.fist ? (strap.fist.endsWith(".L") ? "L" : "R") : hold?.grip === "Off" || (kind && KINDS[kind]?.lead === "Off") ? "L" : "R";
  const sets = join(ROOT, "assets", "data", "moves");
  const families = readFileSync(join(sets, "families.ron"), "utf8");
  const family = hold && new RegExp(`"${hold.class}":\\s*"(\\w+)"`).exec(families)?.[1];
  const files = execFileSync("git", ["ls-files", "assets/data/moves"], { cwd: ROOT, encoding: "utf8" }).split("\n");
  // A shield fights in a set whose stance is "shielded".
  const set = hold && files.find((f) => [hold.class, family].includes(f.split("/").at(-1)!.split(".")[0]) || (hold.class === "shield" && f.endsWith(".shielded.ron")));
  return set ? { set, hand, clips: "" } : { set: "none", hand, clips: (hand === "L" ? carriedClips() : walkerClips()).join(",") };
}

/** A row moves.ts measures: a move of a set, or a clip, what it measured and where (moves.ts --json). */
type MoveRow = { move?: string; clip?: string; measured?: Record<string, number>; where?: Record<string, string> };

/** Runs `node <args>` (moves.ts, writing its rows to `json`), its output added to `log`: the
 * rows, or why there are none. The file of an earlier run is deleted first, so a run that
 * stopped before its rows (a missing clip once did) fails here and old rows are never read as
 * its own. moves.ts writes its rows and then exits 1 when some are over its own bars; those
 * rows are this run's and judged by fit's. */
export async function runMoves(args: string[], json: string, log: string): Promise<MoveRow[] | string> {
  rmSync(json, { force: true });
  const run = await exec(process.execPath, args);
  appendFileSync(log, `\n$ node ${args.join(" ")}\n${run.stdout}${run.stderr}`);
  if (existsSync(json)) return JSON.parse(readFileSync(json, "utf8")) as MoveRow[];
  const said = `${run.stderr}\n${run.stdout}`.split("\n").filter((l) => l.trim());
  const why = said.find((l) => /error/i.test(l)) ?? said.at(-1) ?? run.error ?? "";
  const how = run.status === null ? `stopped by ${run.signal}` : `exit ${run.status}`;
  return `moves.ts wrote no rows (${how}): ${why.trim()}; see ${rel(log)}`;
}

/** What the move check reads besides its scripts and the files it is given: every file of ours
 * its scripts name ("assets/…", "art/…": its default body, clips, set, spec, targets) and the
 * walker's held.rs (held.ts's carried clips), with the data that picks its set and hand. */
export function movesReads(scripts: string[], root = ROOT): string[] {
  const named = new Set<string>();
  for (const s of scripts) for (const [, f] of readFileSync(s, "utf8").matchAll(/"((?:assets|art)\/[\w./-]+\.(?:json|ron|glb|npz))"/g)) named.add(join(root, f));
  const more = ["crates/terra_walker/src/character/held.rs", "assets/data/items.ron", "assets/data/moves/families.ron", "tools/characters/attach_kinds.json"];
  return [...named, ...more.map((f) => join(root, f))].sort();
}

/** One move check: these args on one template, kept by its inputs; its rows, or why none. */
async function movesRun(ctx: Ctx, it: Item, args: string[], files: string[], template: string): Promise<{ rows: MoveRow[] | string; kept: boolean }> {
  const scripts = closure(join(CHARS, "moves.ts"), ROOT);
  const k = key("moves", ROOT, [...scripts, ...movesReads(scripts), it.file, ...files], [args, template]);
  const kept = ctx.fresh ? undefined : ctx.cache.get(k, ROOT);
  if (kept) return { rows: kept.data as MoveRow[], kept: true };
  const json = join(OUT, `fit-${it.name}-${template}-moves.json`);
  const rows = await runMoves([join(CHARS, "moves.ts"), ...args, "--template", template, "--json", json], json, ctx.log);
  if (typeof rows !== "string") ctx.cache.put(k, ROOT, { ok: true, line: "", data: rows });
  return { rows, kept: false };
}

type Deep = { row: string; cm: number; where: string };
/** Each row's deepest for this look over every template's run, and where it went in. */
export function deepest(runs: MoveRow[][], name: string): Deep[] {
  const by = new Map<string, Deep>();
  const k = `held_cm.${name}`;
  for (const rows of runs)
    for (const r of rows) {
      const cm = r.measured?.[k];
      if (cm === undefined) continue;
      const row = r.move ?? r.clip!;
      if (!by.has(row) || cm > by.get(row)!.cm) by.set(row, { row, cm, where: r.where?.[k] ?? "" });
    }
  return [...by.values()];
}

async function movesStage(ctx: Ctx, it: Item): Promise<boolean> {
  const t0 = performance.now();
  const { set, hand, clips } = setOf(it.name);
  const look = rel(it.look);
  const hold = set === "none" ? [`socket.hand.${hand}=${look}`] : (["R", "L"] as const).map((h) => `socket.hand.${h}=${h === hand ? look : PARTNER[h]}`);
  const args = ["--set", set, "--carry", "--arms", ...hold.flatMap((h) => ["--hold", h])];
  if (clips) args.push("--also", clips);
  const files = [...hold.map((h) => join(ROOT, h.split("=")[1])), ...(set === "none" ? [] : [join(ROOT, set)])];
  const runs = await Promise.all(ctx.templates.map((t) => movesRun(ctx, it, args, files, t)));
  const since = runs.every((r) => r.kept) ? -1 : t0;
  const failed = runs.find((r) => typeof r.rows === "string");
  if (failed) return line(ctx, false, it.name, "moves", failed.rows as string, since);
  const deep = deepest(
    runs.map((r) => r.rows as MoveRow[]),
    it.name,
  );
  const over = deep.filter((r) => r.cm > HELD_CM).sort((a, b) => b.cm - a.cm);
  const worst = deep.reduce((a, b) => (b.cm > a.cm ? b : a), { row: "", cm: -Infinity, where: "" });
  const where = `${deep.length} rows of ${set === "none" ? "the walker's clips" : set.split("/").at(-1)} in ${hand === "R" ? "the right" : "the left"} hand on ${ctx.templates.length} ${ctx.templates.length === 1 ? "body" : "bodies"}`;
  if (over.length) return line(ctx, false, it.name, "moves", `${where}: into the body ${over.map((r) => `${r.cm.toFixed(1)} cm in ${r.row}${r.where ? ` (${r.where})` : ""}`).join(", ")} (bar ${HELD_CM} cm)`, since);
  return line(ctx, true, it.name, "moves", `${where}: ${worst.cm.toFixed(1)} cm deepest (${worst.row}), bar ${HELD_CM} cm`, since);
}

/** One item through every stage; its lines and whether it passed. */
async function fitItem(name: string, opt: { turn?: number; fresh: boolean; templates: string[]; cache: Cache }): Promise<{ lines: string[]; ok: boolean }> {
  const ctx: Ctx = { cache: opt.cache, fresh: opt.fresh, templates: opt.templates, lines: [], log: join(OUT, `fit-${name}.log`) };
  writeFileSync(ctx.log, "");
  const it = read(name);
  if (typeof it === "string") return { lines: ctx.lines, ok: line(ctx, false, name, "read", it) };
  line(ctx, true, name, "read", `${rel(it.file)}, a ${it.data.kind}`);
  if (!detectStage(ctx, it, opt.turn)) return { lines: ctx.lines, ok: false };
  const [made, changed] = await makeStage(ctx, it);
  if (!made || !checkStage(ctx, it)) return { lines: ctx.lines, ok: false };
  let ok = true;
  if (it.worn) line(ctx, true, name, "moves", "a worn look has no held rows; build_item.sh's gear_check judged it on every body");
  else ok = await movesStage(ctx, it);
  if (changed) line(ctx, true, name, "show", `its look changed: look at it (cargo xtask workbench --sheet ${it.worn ? "--wear" : "--hold"} ${name}) and paint its icon again (node tools/icons/paint.ts ${name}, or cargo xtask icons --keep)`);
  else line(ctx, true, name, "show", "its look's bytes are the committed ones: sheet and icon kept");
  return { lines: ctx.lines, ok };
}

if (import.meta.main) {
  const { values: opt, positionals } = parseArgs({
    allowPositionals: true,
    options: { turn: { type: "string" }, template: { type: "string" }, all: { type: "boolean", default: false }, fresh: { type: "boolean", default: false } },
  });
  const items = opt.all ? allItems() : positionals;
  if (!items.length) {
    console.error("usage: node tools/characters/fit.ts ITEM... | --all [--turn DEG] [--template male,...] [--fresh]");
    process.exit(2);
  }
  mkdirSync(OUT, { recursive: true });
  const t0 = performance.now();
  const templates = opt.template ? opt.template.split(",") : Object.keys(JSON.parse(readFileSync(join(ROOT, "assets", "models", "character", "templates.json"), "utf8")));
  const cache = new Cache(join(OUT, "fit-cache"));
  const turn = opt.turn === undefined ? undefined : Number(opt.turn);
  // Every item side by side; each one's lines together, as it finishes.
  const results = await Promise.all(
    items.map((name) =>
      fitItem(name, { turn, fresh: opt.fresh!, templates, cache }).then((r) => {
        for (const l of r.lines) console.log(l);
        return r;
      }),
    ),
  );
  const failed = results.filter((r) => !r.ok).length;
  console.log(`${failed ? "✗" : "✓"} ${items.length - failed} of ${items.length} fitted in ${((performance.now() - t0) / 1000).toFixed(1)} s`);
  process.exit(failed ? 1 : 0);
}
