#!/usr/bin/env node
// The clip step (D450): a move's clip made from its spec in one command. For each clip named,
// an entry of a set's list with a spec (clip_spec.ts): its guard take converted to Kimodo's
// NPZ, the guard and hand keys solved (kimodo_keys.py), Kimodo's takes made from its prompt
// held to them, every take put onto the mannequin side by side (made_clips.py --try=CLIP#N),
// the base body exported with every clip of the list carried by aim (export_character.py
// --list), each take's active frames measured (phases.ts), every take scored by the move check
// against its spec (moves.ts --spec --json), the take kept the first that passes
// (clip_spec.ts's choose), and its phases written into a move set.
//
//   node tools/characters/clip.ts Keyed_Backhand                  # into a scratch set
//   node tools/characters/clip.ts Keyed_Backhand --measure        # score the takes made again
//   node tools/characters/clip.ts Keyed_Backhand --into assets/data/moves/sword_and_shield.ron
//
// --list is the set (art/animation/sets/fight.json by default), its kept takes in the folder
// named as it is. The scratch set, tools/characters/out/clip/<set>/moves.ron, holds each clip's
// chosen take as its blow's move, as it is named in the scored file beside it (takes.glb);
// moves.ts checks it with --set and --clips. --into, when every clip has a take that passes,
// keeps each take in the list (as motion.ts --keep does), rebuilds the game's fight clips
// (assets/models/character/fight.glb) from the list and writes the phases into that set; then
// runs the combat scorecard (tools/combat/scorecard.ts, D452), and when any criterion is worse
// than its baseline puts every file it wrote back as it was and refuses the set.
// A line a stage (D388); everything the steps print goes to the log it names. Exit 1 when a
// clip has no take that passes. Kimodo is set up as tools/characters/README.md, "Made clips"
// says. Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { appendFileSync, copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { choose, keysFor, readSpec, speedFor, writePhases, type Scored, type Spec } from "./clip_spec.ts";
import { Glb } from "./glb.ts";
import { KIMODO, KIMODO_ENV, MODEL, kept } from "./motion.ts";
import { measure } from "./phases.ts";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const HERE = join(ROOT, "tools", "characters");
const BLENDER = join(ROOT, "tools", "blender", "run.sh");
const BODY = join(HERE, "out", "base-male.glb");
const GAME_CLIPS = join(ROOT, "assets", "models", "character", "fight.glb");

/** A take's clip name in the scored file, and its move's name in the scored set. */
export const takeName = (clip: string, n: number) => `${clip}#${n}`;
const moveName = (clip: string, n: number) => `${clip}_${n}`;

export type Measured = { clip: string; take: number; end: number; active?: [number, number]; hit?: number };

/** The set moves.ts scores the takes by: each take a move, from its first frame to its last. */
export function scoringSet(takes: Measured[]): string {
  const f = (x: number) => x.toFixed(2);
  const rows = takes.map((t) => {
    const active = t.active ? ` active: (${f(t.active[0])}, ${f(t.active[1])}),` : "";
    return `        "${moveName(t.clip, t.take)}": (clip: "${takeName(t.clip, t.take)}", from: 0.00,${active} done: ${f(t.end)}, speed: 1.0),`;
  });
  return `// Written by clip.ts: each take a move, from its first frame to its last.\n(\n    moves: {\n${rows.join("\n")}\n    },\n)\n`;
}

export type Row = { move?: string; clip: string; bad: string[]; gaps?: [number, number] };

/** Each take of `clip` as moves.ts scored it (its rows), with what clip.ts found besides
 * (`more`, by take), for `choose`: off is its farther gap from the guard, metres. */
export function scores(rows: Row[], clip: string, more: Map<number, string[]> = new Map()): Scored[] {
  return rows
    .filter((r) => r.move && r.clip.startsWith(`${clip}#`))
    .map((r) => {
      const take = Number(r.clip.slice(clip.length + 1));
      return { take, failures: [...(more.get(take) ?? []), ...r.bad], off: Math.max(...(r.gaps ?? [0])) };
    });
}

const EMPTY = "// Written by clip.ts: each clip's chosen take as its blow's move.\n(\n    moves: {\n    },\n)\n";

class Failed extends Error {}

function main() {
  const { values, positionals: clips } = parseArgs({
    allowPositionals: true,
    options: { list: { type: "string", default: "art/animation/sets/fight.json" }, into: { type: "string" }, measure: { type: "boolean", default: false } },
  });
  if (clips.length === 0) throw new Failed("usage: clip.ts CLIP... [--list FILE.json] [--into SET.ron] [--measure]");
  const LIST = resolve(ROOT, values.list!);
  const set = basename(LIST, ".json");
  const listDir = LIST.replace(/\.json$/, "");
  const OUT = join(HERE, "out", "clip", set);
  mkdirSync(OUT, { recursive: true });
  const LOG = join(OUT, "clip.log");
  writeFileSync(LOG, `clip.ts ${process.argv.slice(2).join(" ")}\n`);
  // Paths in the repository relative to it, as a Windows Blender takes them (tools/blender/run.sh).
  const rel = (p: string) => (resolve(p).startsWith(resolve(ROOT)) ? relative(ROOT, p) : p);
  const say = (ok: boolean, stage: string, what: string) => console.log(`${ok ? "✓" : "✗"} ${stage}: ${what}`);

  // A step's output into the log; a failure says its last lines and where the rest is.
  const run = (stage: string, command: string, args: string[], env: Record<string, string> = {}, okStatus = [0]) => {
    appendFileSync(LOG, `\n$ ${[command, ...args].map(rel).join(" ")}\n`);
    const r = spawnSync(command, args, { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28, env: { ...process.env, ...env } });
    const out = (r.stdout ?? "") + (r.stderr ?? "");
    appendFileSync(LOG, out);
    if (r.status === null || !okStatus.includes(r.status)) {
      const tail = out.trim().split("\n").filter((l) => /error|Error|fail|no /.test(l)).slice(-3);
      say(false, stage, `${basename(command)} exited ${r.status}${tail.length ? `: ${tail.join(" | ")}` : ""} (log: ${rel(LOG)})`);
      throw new Failed();
    }
    return out;
  };
  const blender = (stage: string, script: string, args: string[]) => run(stage, BLENDER, [join(HERE, script), ...args]);
  const lines = (text: string, start: string) => text.split("\n").filter((l) => l.startsWith(start));

  const list: Record<string, Record<string, unknown>> = JSON.parse(readFileSync(LIST, "utf8"));
  const specs: Spec[] = clips.map((c) => {
    if (!list[c]) throw new Failed(`${c} is not in ${rel(LIST)}`);
    return readSpec(c, list[c]);
  });
  say(true, "spec", specs.map((s) => `${s.name} (${s.blow.move}, ${s.hand.length + s.left.length} keys, ${s.samples} takes)`).join(", "));
  const takeFile = (s: Spec, n: number) => join(OUT, s.name, s.name, `${s.name}_${String(n).padStart(2, "0")}.bvh`);
  const guardTake = (s: Spec) => join(listDir, `${s.guard.clip}.bvh`);

  if (!values.measure) {
    if (!existsSync(join(KIMODO, "venv", "bin", "kimodo_gen"))) throw new Failed(`no Kimodo at ${KIMODO}: see tools/characters/README.md, "Made clips"`);
    for (const s of specs) {
      const dir = join(OUT, s.name);
      mkdirSync(dir, { recursive: true });
      // The guard's kept take as Kimodo's NPZ, made again when the take is newer.
      const npz = join(OUT, `${s.guard.clip}.npz`);
      if (!existsSync(guardTake(s))) throw new Failed(`no ${rel(guardTake(s))}: keep a take of ${s.guard.clip} first (motion.ts --keep)`);
      if (!existsSync(npz) || statSync(npz).mtimeMs < statSync(guardTake(s)).mtimeMs)
        run("guard", join(KIMODO, "venv", "bin", "kimodo_convert"), [guardTake(s), npz, "--bvh_standard_tpose"]);
      writeFileSync(join(dir, "keys.json"), JSON.stringify(keysFor(s)));
      const keyed = run("keys", join(KIMODO, "venv", "bin", "python"), ["-I", join(HERE, "kimodo_keys.py"), npz, join(dir, "keys.json"), join(dir, "constraints.json")]);
      const short = lines(keyed, "KEY ").map((l) => Number(/([\d.]+) cm short/.exec(l)?.[1] ?? 0));
      say(true, "keys", `${s.name}: the guard at its first and last frames, ${short.length} hand keys, at most ${Math.max(0, ...short).toFixed(1)} cm short`);
      const started = Date.now();
      run("takes", join(KIMODO, "venv", "bin", "kimodo_gen"), [
        s.prompt, "--model", MODEL, "--duration", String(s.seconds), "--num_samples", String(s.samples),
        "--constraints", join(dir, "constraints.json"), "--output", join(dir, s.name), "--bvh", "--bvh_standard_tpose", "--seed", String(s.seed),
      ], KIMODO_ENV);
      say(true, "takes", `${s.name}: ${s.samples} from Kimodo in ${((Date.now() - started) / 1000).toFixed(0)} s`);
    }
  }
  const takes = specs.flatMap((s) => [...Array(s.samples).keys()].filter((n) => existsSync(takeFile(s, n))).map((n) => ({ s, n })));
  for (const s of specs) if (!takes.some((t) => t.s === s)) throw new Failed(`no takes of ${s.name} in ${rel(join(OUT, s.name))}: make them (without --measure)`);

  // Every take onto the mannequin side by side, with each guard's kept take.
  const mannequin = join(OUT, "mannequin.glb");
  const guards = [...new Set(specs.map((s) => s.guard.clip))];
  const made = blender("made", "made_clips.py", [
    rel(mannequin), ...takes.map(({ s, n }) => `--try=${takeName(s.name, n)}:${rel(takeFile(s, n))}`),
    ...guards.map((g) => `--try=${g}:${rel(join(listDir, `${g}.bvh`))}`), `--list=${rel(LIST)}`,
  ]);
  say(true, "made", `${lines(made, "ONCE ").length} takes and ${guards.join(", ")} onto the mannequin`);

  if (!existsSync(BODY)) {
    blender("body", "convert_rig.py", ["art/characters/base-male/rigged.glb", "base-male", "1.80", "2"]);
    blender("body", "lift.py", []);
    say(true, "body", `${rel(BODY)} converted from its rig`);
  }
  const scored = join(OUT, "takes.glb");
  const exported = blender("export", "export_character.py", [rel(BODY), rel(scored), `--clips=${rel(mannequin)}`, `--list=${rel(LIST)}`]);
  say(true, "export", `${rel(scored)}: ${lines(exported, "AIM ").length} clips carried by aim`);

  // Each take's active frames, from its first frame to its last.
  const glb = new Glb(scored);
  const more = new Map<string, Map<number, string[]>>(specs.map((s) => [s.name, new Map()]));
  const measured: Measured[] = takes.map(({ s, n }) => {
    const p = measure(glb, takeName(s.name, n));
    if (!p) throw new Failed(`no ${takeName(s.name, n)} in ${rel(scored)}`);
    const wrong: string[] = [];
    if (s.blow.sweep && !p.active) wrong.push("never strikes: the tip is never fast and ahead");
    if (p.jumps.length) wrong.push(`the tip jumps at ${p.jumps.map((j) => `${j.t.toFixed(2)} s (${j.speed.toFixed(0)} m/s)`).join(", ")}`);
    more.get(s.name)!.set(n, wrong);
    // Only a blow strikes: a guard's or a block's quick sword is no active span.
    const active = s.blow.sweep ? p.active : undefined;
    return { clip: s.name, take: n, end: p.end, active, hit: s.blow.sweep ? p.fastest?.t : s.blow.raised?.[0] };
  });
  say(true, "phases", measured.map((m) => `${takeName(m.clip, m.take)} ${m.active ? m.active.map((x) => x.toFixed(2)).join("-") : "none"} of ${m.end.toFixed(2)} s`).join(", "));

  // The move check on every take, against its spec.
  const scoring = join(OUT, "score.ron");
  const json = join(OUT, "score.json");
  writeFileSync(scoring, scoringSet(measured));
  run("moves", process.execPath, [join(HERE, "moves.ts"), "--set", scoring, "--clips", scored, "--spec", LIST, "--guard", guards[0], "--also=", "--json", json], {}, [0, 1]);
  const rows: Row[] = JSON.parse(readFileSync(json, "utf8"));
  const chosen = specs.map((s) => {
    const all = scores(rows, s.name, more.get(s.name));
    for (const t of all) say(t.failures.length === 0, "moves", `${takeName(s.name, t.take)}${t.failures.length ? `, ${t.failures.length} over: ${t.failures.join("; ")}` : " within the bars"}`);
    const pick = choose(all)!;
    return { s, pick, m: measured.find((m) => m.clip === s.name && m.take === pick.take)! };
  });
  for (const { s, pick } of chosen) say(pick.failures.length === 0, "chosen", pick.failures.length ? `${s.name}: no take passes; the nearest is take ${pick.take}` : `${s.name}: take ${pick.take}`);

  // Each chosen take's playback speed, to meet its spec's timing.
  const speeds = new Map<string, number>();
  for (const { s, m } of chosen) {
    if (!s.timing || m.hit === undefined) continue;
    const t = speedFor(s.timing, m.hit, m.end);
    speeds.set(s.name, t.speed);
    const span = (x: [number, number]) => `${x[0]}-${x[1]}`;
    say(t.ok, "timing", `${s.name} at speed ${t.speed.toFixed(2)}: hit ${t.hit.toFixed(2)} s (spec ${span(s.timing.hit)}), back on guard ${t.back.toFixed(2)} s after (spec ${span(s.timing.back)})`);
  }

  // The scratch set: each chosen take as its blow's move, as the scored file names it, with
  // its chain; checked as a set, so a chain's poses meet.
  let scratch = EMPTY;
  const hold = (s: Spec) => (s.blow.raised ? (s.blow.raised[0] + s.blow.raised[1]) / 2 : undefined);
  for (const { s, m } of chosen) {
    const next = specs.some((o) => o.blow.move === s.blow.next) ? s.blow.next : undefined;
    const phases = { clip: takeName(s.name, m.take), from: 0, done: m.end, active: m.active, speed: speeds.get(s.name), hold: hold(s), next };
    scratch = writePhases(scratch, s.blow.move, phases);
  }
  const setFile = join(OUT, "moves.ron");
  writeFileSync(setFile, scratch);
  const checked = run("set", process.execPath, [join(HERE, "moves.ts"), "--set", setFile, "--clips", scored, "--spec", LIST, "--guard", guards[0], "--also=",], {}, [0, 1]);
  const setBad = lines(checked, "✗ ").filter((l) => !/^✗ \d+ over the bars/.test(l));
  const chains = [...checked.matchAll(/ into (\w+) ([\d.]+) m/g)].map((x) => `${x[1]} ${x[2]} m`);
  const chained = chains.length ? `, chains into ${chains.join(", ")}` : "";
  say(setBad.length === 0, "set", `${rel(setFile)}${chained}, ${setBad.length ? `${setBad.length} over (log)` : "every move within the bars"} (moves.ts --set it --clips ${rel(scored)})`);
  const passed = chosen.every((c) => c.pick.failures.length === 0) && setBad.length === 0;
  if (values.into) {
    if (!passed) {
      say(false, "into", `not written: every clip needs a take that passes`);
      throw new Failed();
    }
    // What --into writes, as it was, put back if the scorecard finds the set worse.
    const into = resolve(ROOT, values.into);
    const written = [LIST, GAME_CLIPS, into, ...chosen.map(({ s }) => join(listDir, `${s.name}.bvh`))];
    const before = new Map(written.map((f) => [f, existsSync(f) ? readFileSync(f) : undefined]));
    const putBack = () => {
      for (const [f, bytes] of before) if (bytes === undefined) rmSync(f, { force: true });
      else writeFileSync(f, bytes);
    };
    for (const { s, pick } of chosen) {
      mkdirSync(listDir, { recursive: true });
      copyFileSync(takeFile(s, pick.take), join(listDir, `${s.name}.bvh`));
      list[s.name] = kept(list[s.name], pick.take);
    }
    writeFileSync(LIST, JSON.stringify(list, null, 2) + "\n");
    const figure = join(HERE, "out", `${set}-mannequin.glb`);
    blender("into", "made_clips.py", [rel(figure), `--list=${rel(LIST)}`]);
    blender("into", "export_character.py", [rel(BODY), rel(GAME_CLIPS), `--clips=${rel(figure)}`, `--list=${rel(LIST)}`]);
    let text = readFileSync(into, "utf8");
    for (const { s, m } of chosen) text = writePhases(text, s.blow.move, { clip: s.name, from: 0, done: m.end, active: m.active, speed: speeds.get(s.name), hold: hold(s) });
    writeFileSync(into, text);
    // The combat ratchet: a set that makes any criterion worse than its baseline is refused.
    appendFileSync(LOG, `\n$ node tools/combat/scorecard.ts\n`);
    const card = spawnSync(process.execPath, [join(ROOT, "tools", "combat", "scorecard.ts")], { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 });
    appendFileSync(LOG, (card.stdout ?? "") + (card.stderr ?? ""));
    if (card.status !== 0) {
      putBack();
      const worse = (card.stdout ?? "").split("\n").filter((l) => l.startsWith("  ✗ ")).map((l) => l.trim().slice(2));
      say(false, "into", `refused, every file put back: the combat scorecard is worse (${worse.length ? worse.join("; ") : "see the log"}); log ${rel(LOG)}`);
      throw new Failed();
    }
    say(true, "into", `${chosen.map((c) => `${c.s.name} take ${c.pick.take}`).join(", ")} kept in ${rel(listDir)}; ${rel(GAME_CLIPS)} rebuilt; phases in ${rel(into)}`);
  }
  console.log(`log: ${rel(LOG)}`);
  if (!passed) process.exitCode = 1;
}

if (import.meta.main) {
  try {
    main();
  } catch (e) {
    if (!(e instanceof Failed)) throw e;
    if (e.message) console.log(`✗ ${e.message}`);
    process.exitCode = 1;
  }
}
