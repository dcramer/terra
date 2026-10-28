#!/usr/bin/env node
// The character build, incremental and resumable (D71, D388): the steps of build_steps.ts run
// as make and ninja run theirs. Each step's stamp (out/stamps/STEP.json) is the hash of what it
// read: its scripts with the modules they import and the data those read, its art, the command,
// and the outputs of the steps it needs. A step whose stamp still holds and whose outputs are as
// it left them is skipped; a changed one runs, and so does what reads its outputs, unless they
// came out the same. A step killed half-way has no stamp, so the next run starts it again.
//
//   node tools/characters/build.ts [--jobs N]     # every step (build_bodies.sh)
//   node tools/characters/build.ts --item ITEM    # what ITEM's game file needs (build_item.sh)
//   node tools/characters/build.ts --plan         # each step's state, running nothing
//   node tools/characters/build.ts --again S1,S2  # S1 and S2 run a second time: the same bytes?
//
// A line a step: built (and why), skipped (unchanged), restored (from the step cache), failed
// (and why), or not run. Each step's output is kept in out/steps/STEP.log; out/build.log is them
// all in the build's order, skipped steps' from when they ran, so build_verdict.ts gives the
// verdict a full build would.
//
// The step cache (~/.cache/terra/steps, or $TERRA_STEP_CACHE; --no-cache for none) is shared by
// every worktree: each step's outputs by its stamp's key, so a fresh worktree restores the steps
// another built from the same inputs rather than building them again. A step's outputs must be
// a function of its key, the same bytes every run (--again checks one).

import { execFileSync, spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, openSync, closeSync, readdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import { availableParallelism, homedir } from "node:os";
import { isAbsolute, join } from "node:path";
import { FAILED, itemVerdict, verdict } from "./build_verdict.ts";
import { glbDiff } from "./glb_diff.ts";
import { bodiesData, buildSteps, builtLine, closure, gearData, morphNames, SECTIONS, type Step } from "./build_steps.ts";

const ROOT = join(import.meta.dirname, "..", "..");

type Stamp = { key: string; inputs: Record<string, string>; needs: Record<string, string>; outputs: Record<string, string>; seconds: number };
export type Outcome = { state: "built" | "skipped" | "restored" | "failed" | "not run"; why: string };
/** A failure in the step's own run, not in what it checked: it is run again next time. */
const CRASHED = /Traceback|^run\.sh: .* exited \d+|Error: script failed/m;

const sha = (data: string | Buffer) => createHash("sha256").update(data).digest("hex");
const safe = (name: string) => name.replace(/[^\w.-]+/g, "-");

/** A file's hash, a folder's (every file under it, by path), or "missing". */
function hashPath(root: string, p: string, cache: Map<string, string>): string {
  const hit = cache.get(p);
  if (hit) return hit;
  const abs = isAbsolute(p) ? p : join(root, p);
  let h = "missing";
  if (existsSync(abs))
    h = statSync(abs).isDirectory()
      ? sha(readdirSync(abs, { recursive: true, withFileTypes: true }).filter((e) => e.isFile()).map((e) => join(e.parentPath, e.name).slice(abs.length + 1)).sort().map((f) => `${f} ${hashPath(root, `${p}/${f}`, cache)}`).join("\n"))
      : sha(readFileSync(abs));
  cache.set(p, h);
  return h;
}

export class Builder {
  readonly stamps: string;
  readonly logs: string;
  readonly outcomes = new Map<string, Outcome>();
  /** Each finished step's outputs' hashes, which the steps that need it read as inputs. */
  private readonly made = new Map<string, Record<string, string>>();
  private readonly inputCache = new Map<string, string>();

  readonly steps: Step[];
  readonly root: string;
  readonly env: Record<string, string>;
  readonly print: (line: string) => void;
  /** The step cache's folder, shared by worktrees; none, no cache. */
  readonly cache: string | undefined;

  constructor(steps: Step[], root = ROOT, out = join(ROOT, "tools/characters/out"), env: Record<string, string> = {}, print = (l: string) => console.log(l), cache?: string) {
    this.steps = steps;
    this.root = root;
    this.env = env;
    this.print = print;
    this.cache = cache;
    this.stamps = join(out, "stamps");
    this.logs = join(out, "steps");
  }

  logOf(s: Step) {
    return join(this.logs, `${safe(s.name)}.log`);
  }

  private stampOf(s: Step): Stamp | undefined {
    const f = join(this.stamps, `${safe(s.name)}.json`);
    try {
      return JSON.parse(readFileSync(f, "utf8"));
    } catch {
      return undefined;
    }
  }

  private outputsNow(s: Step): Record<string, string> {
    const cache = new Map<string, string>();
    return Object.fromEntries([...s.outputs, ...(s.inPlace ?? [])].map((p) => [p, hashPath(this.root, p, cache)]));
  }

  /** What the step reads now: its inputs (imports followed) and its needs' outputs. */
  private readsNow(s: Step) {
    // A file the step changes in place is its output, not its input (the mannequin the clips step rewrites).
    const own = new Set(s.inPlace ?? []);
    const inputs = Object.fromEntries(closure(s.inputs, this.root).filter((p) => !own.has(p)).map((p) => [p, hashPath(this.root, p, this.inputCache)]));
    const needs = Object.fromEntries(s.needs.map((n) => [n, sha(JSON.stringify(this.made.get(n)))]));
    const key = sha(JSON.stringify({ run: s.run, inputs, needs }));
    return { key, inputs, needs };
  }

  /** Why the step must run, or undefined when its stamp holds. */
  private stale(s: Step, now: ReturnType<Builder["readsNow"]>): string | undefined {
    const was = this.stampOf(s);
    if (!was) return "never built, or stopped part-way";
    if (was.key !== now.key) {
      const input = Object.keys({ ...was.inputs, ...now.inputs }).find((p) => was.inputs[p] !== now.inputs[p]);
      if (input) return `${input} changed`;
      const need = Object.keys(now.needs).find((n) => was.needs[n] !== now.needs[n]);
      if (need) return `${need}'s outputs changed`;
      return "its command changed";
    }
    // A file a later step changes in place (a body its skin is painted onto) is that step's to check.
    const later = new Set(this.steps.filter((t) => t !== s).flatMap((t) => t.inPlace ?? []));
    const outputs = this.outputsNow(s);
    const changed = Object.keys(was.outputs).find((p) => !later.has(p) && was.outputs[p] !== outputs[p]);
    return changed ? `${changed} is not as it left it` : undefined;
  }

  private exec(s: Step, extra: Record<string, string>): Promise<number> {
    mkdirSync(this.logs, { recursive: true });
    const fd = openSync(this.logOf(s), "w");
    const child = spawn("bash", ["-c", `set -euo pipefail\n${s.run}`], { cwd: this.root, stdio: ["ignore", fd, fd], env: { ...process.env, ...this.env, ...extra } });
    return new Promise((done) => {
      child.on("close", (code, signal) => {
        closeSync(fd);
        done(code ?? (signal ? 128 : 1));
      });
    });
  }

  /** Whether git has these files as committed (none changed, added or untracked), so a committed output is as built from them. */
  private asCommitted(paths: string[]): boolean {
    try {
      return execFileSync("git", ["status", "--porcelain", "--", ...paths], { cwd: this.root, encoding: "utf8" }).trim() === "";
    } catch {
      return false;
    }
  }

  private async runStep(s: Step, why: string, adoptOk = true): Promise<Outcome> {
    rmSync(join(this.stamps, `${safe(s.name)}.json`), { force: true });
    for (const p of s.outputs) rmSync(join(this.root, p), { force: true });
    const now = this.readsNow(s);
    const adopt = adoptOk && s.committed && this.asCommitted([...Object.keys(now.inputs), ...(s.inPlace ?? [])]);
    if (adopt) why = `${(s.inPlace ?? []).join(", ")} as committed, from its inputs as committed`;
    const t0 = performance.now();
    const code = await this.exec(s, adopt ? { ADOPT: "1" } : {});
    const seconds = Math.round((performance.now() - t0) / 1000);
    for (const p of [...s.outputs, ...(s.inPlace ?? [])]) this.inputCache.delete(p);
    const log = readFileSync(this.logOf(s), "utf8");
    const missing = s.outputs.find((p) => !existsSync(join(this.root, p)));
    const crashed = log.match(CRASHED)?.[0];
    const rel = this.logOf(s).slice(this.root.length + 1);
    if (code !== 0 || missing || crashed)
      return { state: "failed", why: `${code !== 0 ? `exited ${code}` : missing ? `wrote no ${missing}` : `${crashed} in its output`} after ${clock(seconds)}: see ${rel}` };
    const outputs = this.outputsNow(s);
    this.made.set(s.name, outputs);
    mkdirSync(this.stamps, { recursive: true });
    const f = join(this.stamps, `${safe(s.name)}.json`);
    writeFileSync(`${f}.tmp`, JSON.stringify({ ...now, outputs, seconds } satisfies Stamp, null, 1));
    renameSync(`${f}.tmp`, f);
    this.share(s, now.key, outputs, seconds);
    return { state: "built", why: `in ${clock(seconds)} (${why})${checks(log)}` };
  }

  // The step cache, shared by every worktree (D563): a step's outputs and log by its stamp's key,
  // each file kept once by its hash. The key is everything the step reads, so a step with the
  // same key anywhere made the same files. A step that changes a file in place also reads what
  // it changes, which its key does not hold, so it is never shared.

  shareable(s: Step) {
    return this.cache !== undefined && !s.inPlace?.length;
  }

  /** A file into the cache under its hash; its hash. */
  private keep(abs: string, hash?: string): string {
    const data = readFileSync(abs);
    const h = hash ?? sha(data);
    const obj = join(this.cache!, "objects", h.slice(0, 2), h);
    if (!existsSync(obj)) {
      mkdirSync(join(obj, ".."), { recursive: true });
      const tmp = `${obj}.${process.pid}.tmp`;
      writeFileSync(tmp, data);
      renameSync(tmp, obj);
    }
    return h;
  }

  /** Keeps a step's outputs and log under its key, unless the cache has them. */
  private share(s: Step, key: string, outputs: Record<string, string>, seconds: number) {
    if (!this.shareable(s)) return;
    const entry = join(this.cache!, "keys", `${key}.json`);
    if (existsSync(entry)) return;
    try {
      for (const [p, h] of Object.entries(outputs)) this.keep(join(this.root, p), h);
      const log = existsSync(this.logOf(s)) ? this.keep(this.logOf(s)) : undefined;
      mkdirSync(join(entry, ".."), { recursive: true });
      writeFileSync(`${entry}.${process.pid}.tmp`, JSON.stringify({ name: s.name, outputs, log, seconds }));
      renameSync(`${entry}.${process.pid}.tmp`, entry);
    } catch (e) {
      this.print(`  (not kept in the step cache: ${(e as Error).message})`);
    }
  }

  /** The step's outputs and log from the cache, each checked against its hash, with its stamp; undefined when the cache has not all of them. */
  private restore(s: Step): Outcome | undefined {
    if (!this.shareable(s)) return undefined;
    const now = this.readsNow(s);
    let entry: { outputs: Record<string, string>; log?: string; seconds: number };
    try {
      entry = JSON.parse(readFileSync(join(this.cache!, "keys", `${now.key}.json`), "utf8"));
    } catch {
      return undefined;
    }
    const files = Object.entries(entry.outputs).map(([p, h]) => [p, h, join(this.cache!, "objects", h.slice(0, 2), h)] as const);
    const read = new Map<string, Buffer>();
    for (const [p, h, obj] of files) {
      if (h === "missing" || !existsSync(obj)) return undefined;
      const data = readFileSync(obj);
      if (sha(data) !== h) return undefined;
      read.set(p, data);
    }
    rmSync(join(this.stamps, `${safe(s.name)}.json`), { force: true });
    for (const [p, data] of read) {
      mkdirSync(join(this.root, p, ".."), { recursive: true });
      writeFileSync(join(this.root, p), data);
      this.inputCache.delete(p);
    }
    mkdirSync(this.logs, { recursive: true });
    const log = entry.log ? join(this.cache!, "objects", entry.log.slice(0, 2), entry.log) : undefined;
    writeFileSync(this.logOf(s), log && existsSync(log) ? readFileSync(log) : "");
    this.made.set(s.name, entry.outputs);
    mkdirSync(this.stamps, { recursive: true });
    const f = join(this.stamps, `${safe(s.name)}.json`);
    writeFileSync(`${f}.tmp`, JSON.stringify({ ...now, outputs: entry.outputs, seconds: entry.seconds } satisfies Stamp, null, 1));
    renameSync(`${f}.tmp`, f);
    return { state: "restored", why: `from the step cache, built elsewhere in ${clock(entry.seconds)}${checks(readFileSync(this.logOf(s), "utf8"))}` };
  }

  /** The step's state without running it: skipped when its stamp holds, else why it would run. */
  private check(s: Step): Outcome {
    const now = this.readsNow(s);
    const why = this.stale(s, now);
    if (why) return { state: "built", why };
    const was = this.stampOf(s)!;
    this.made.set(s.name, was.outputs);
    this.share(s, was.key, was.outputs, was.seconds);
    return { state: "skipped", why: `unchanged${checks(existsSync(this.logOf(s)) ? readFileSync(this.logOf(s), "utf8") : "")}` };
  }

  /** Runs `wanted` (every step by default) and what they need, `jobs` at a time, the most-needed first. */
  async build(wanted = this.steps.map((s) => s.name), jobs = 6, plan = false): Promise<boolean> {
    const byName = new Map(this.steps.map((s) => [s.name, s]));
    const todo = new Set<string>();
    const want = (n: string) => {
      if (todo.has(n)) return;
      if (!byName.has(n)) throw new Error(`no step ${n}`);
      todo.add(n);
      byName.get(n)!.needs.forEach(want);
    };
    wanted.forEach(want);
    const order = this.steps.filter((s) => todo.has(s.name));
    const reach = new Map(order.map((s) => [s.name, order.filter((t) => this.reaches(t, s.name, byName)).length]));
    const running = new Map<string, Promise<void>>();
    const say = (s: Step, o: Outcome) => {
      this.outcomes.set(s.name, o);
      const mark = { built: "✓", skipped: "·", restored: "↓", failed: "✗", "not run": "-" }[o.state];
      this.print(`${mark} ${s.name} ${plan && o.state === "built" ? "would run" : o.state}${o.why.startsWith("in ") ? " " : ": "}${o.why}`);
    };
    while (this.outcomes.size < order.length) {
      const ready = order
        .filter((s) => !this.outcomes.has(s.name) && !running.has(s.name) && s.needs.every((n) => this.outcomes.has(n)) && (s.after ?? []).every((n) => !todo.has(n) || this.outcomes.has(n)))
        .sort((a, b) => reach.get(b.name)! - reach.get(a.name)!);
      for (const s of ready) {
        const broken = s.needs.find((n) => this.outcomes.get(n)!.state === "failed" || this.outcomes.get(n)!.state === "not run");
        if (broken) {
          say(s, { state: "not run", why: `needs ${broken}, which ${this.outcomes.get(broken)!.state === "failed" ? "failed" : "did not run"}` });
          continue;
        }
        const pending = s.needs.some((n) => plan && this.outcomes.get(n)!.state === "built");
        let o: Outcome = pending ? { state: "built" as const, why: "what it needs would run" } : this.check(s);
        if (o.state === "built" && !plan) o = this.restore(s) ?? o;
        else if (o.state === "built" && !pending && this.shareable(s) && existsSync(join(this.cache!, "keys", `${this.readsNow(s).key}.json`))) o = { state: "built", why: `${o.why}; the step cache has it` };
        if (o.state === "skipped" || o.state === "restored" || plan) {
          say(s, o);
          continue;
        }
        if (running.size >= jobs) break;
        running.set(s.name, this.runStep(s, o.why).then((r) => {
          say(s, r);
          running.delete(s.name);
        }));
      }
      if (running.size) await Promise.race(running.values());
    }
    return [...this.outcomes.values()].every((o) => o.state === "built" || o.state === "skipped" || o.state === "restored");
  }

  /** Builds `names` and what they need, then runs each named step again on the same inputs,
   * `jobs` at a time, and says which outputs came out other bytes: a step must make the same
   * files from the same inputs, or its rerun rebuilds all it reaches and the step cache could
   * not be shared (D563). The step's outputs of the first run are kept in out/again/STEP/.
   * Lines of what differed; none when every output came out the same. */
  async again(names: string[], jobs = 6, out = join(this.stamps, "..")): Promise<string[]> {
    const byName = new Map(this.steps.map((s) => [s.name, s]));
    const clash = names.find((a) => names.some((b) => a !== b && this.reaches(byName.get(a)!, b, byName)));
    if (clash) return [`${clash} reads another of the steps named; name them in separate runs`];
    if (!(await this.build(names, jobs))) return ["the steps named, or what they need, did not build"];
    const bad: string[] = [];
    const one = async (s: Step) => {
      const first = this.outputsNow(s);
      const kept = join(out, "again", safe(s.name));
      rmSync(kept, { recursive: true, force: true });
      for (const p of Object.keys(first)) {
        mkdirSync(join(kept, p, ".."), { recursive: true });
        if (existsSync(join(this.root, p))) writeFileSync(join(kept, p), readFileSync(join(this.root, p)));
      }
      const o = await this.runStep(s, "again", false);
      if (o.state === "failed") {
        bad.push(`${s.name}: ${o.why}`);
        this.print(`✗ ${s.name} again: ${o.why}`);
        return;
      }
      const second = this.outputsNow(s);
      const differ = Object.keys(first).filter((p) => first[p] !== second[p]);
      if (!differ.length) {
        this.print(`✓ ${s.name} again: ${Object.keys(first).length} outputs the same bytes`);
        return;
      }
      this.print(`✗ ${s.name} again: ${differ.length} of ${Object.keys(first).length} outputs differ (the first run's in ${kept.slice(this.root.length + 1)})`);
      for (const p of differ) {
        bad.push(`${s.name}: ${p} differs`);
        const why = p.endsWith(".glb") ? glbDiff(join(kept, p), join(this.root, p)).slice(0, 8) : [];
        this.print([`  ${p}`, ...why.map((l) => `    ${l}`)].join("\n"));
      }
    };
    const queue = names.map((n) => byName.get(n)!);
    await Promise.all(Array.from({ length: Math.min(jobs, queue.length) }, async () => {
      for (let s = queue.shift(); s; s = queue.shift()) await one(s);
    }));
    return bad;
  }

  private reaches(from: Step, to: string, byName: Map<string, Step>): boolean {
    return from.needs.some((n) => n === to || this.reaches(byName.get(n)!, to, byName));
  }

  /** The steps' logs in the build's order, under their sections' headings; a failed step's ✗ line where it failed. */
  joinedLog(names: string[]): string {
    const lines: string[] = [];
    for (const [i, heading] of SECTIONS.entries()) {
      const here = this.steps.filter((s) => s.section === i && names.includes(s.name));
      if (!here.length) continue;
      lines.push(heading);
      for (const s of here) {
        const o = this.outcomes.get(s.name);
        if (o && o.state !== "not run" && existsSync(this.logOf(s))) lines.push(readFileSync(this.logOf(s), "utf8").trimEnd());
        if (o?.state === "failed") lines.push(`✗ ${s.name} failed: ${o.why}`);
      }
    }
    return `${lines.filter(Boolean).join("\n")}\n`;
  }
}

const clock = (s: number) => (s < 60 ? `${s}s` : `${Math.floor(s / 60)}m${String(s % 60).padStart(2, "0")}s`);

/** The checks its output says failed, the first named: a result, kept, not a reason to run it again. */
function checks(log: string): string {
  const bad = log.split("\n").filter((l) => FAILED.some((re) => re.test(l)));
  return bad.length ? `; ${bad.length} check${bad.length > 1 ? "s" : ""} failed: ${bad[0].slice(0, 100)}` : "";
}

async function main() {
  const args = process.argv.slice(2);
  const flag = (f: string) => {
    const i = args.indexOf(f);
    return i < 0 ? undefined : args[i + 1];
  };
  const item = flag("--item");
  const plan = args.includes("--plan");
  const jobs = Number(flag("--jobs") ?? Math.max(2, Math.min(8, Math.floor(availableParallelism() / 4))));
  const bodies = bodiesData();
  const items = gearData();
  // A morph tried before it is listed in bodies.json (make_morph.sh): its design, or the body given.
  const morph = flag("--morph");
  if (morph && !morphNames(bodies).includes(morph)) bodies.designed.push(morph);
  if (morph && flag("--design")) bodies.designs = { [morph]: flag("--design")! };
  const steps = buildSteps(bodies, items);
  const out = join(ROOT, "tools/characters/out");
  const cache = args.includes("--no-cache") ? undefined : (process.env.TERRA_STEP_CACHE ?? join(homedir(), ".cache", "terra", "steps"));
  const b = new Builder(steps, ROOT, out, { blender: "tools/blender/run.sh", here: "tools/characters", out: "tools/characters/out" }, undefined, cache);
  const base = bodies.base.name;
  const again = flag("--again");
  if (again) {
    const bad = await b.again(again.split(","), jobs);
    console.log(bad.length ? `AGAIN fail: ${bad.length} outputs not the same bytes\n${bad.join("\n")}` : "AGAIN pass: every output the same bytes");
    process.exit(bad.length ? 1 : 0);
  }
  if (morph) {
    // The morph and every look carried onto it, pushed clear and checked (make_morph.sh's steps 4 to 6).
    const own = steps.filter((s) => s.name === `morph:${morph}` || s.name.startsWith(`fit:${morph}:`)).map((s) => s.name);
    const ok = await b.build(own, jobs, plan);
    if (plan) return;
    const log = join(out, `build-morph-${morph}.log`);
    writeFileSync(log, b.joinedLog(own));
    const failed = readFileSync(log, "utf8").split("\n").filter((l) => FAILED.some((re) => re.test(l)));
    const rel = log.slice(ROOT.length + 1);
    console.log(!ok || failed.length ? `✗ ${morph} fail: ${ok ? failed[0] : "a step failed"}; see ${rel}` : `✓ ${morph} pass: made and every look carried onto it, pushed clear and checked (${rel})`);
    process.exit(!ok || failed.length ? 1 : 0);
  }
  if (item) {
    if (!items.has(item)) throw new Error(`${item}: no art/gear/${item}/fit.json`);
    if (!plan && !existsSync(join(out, "stamps", "base.json"))) {
      console.log(`✗ ${item}: no full build here (tools/characters/out/stamps/base.json); run build_bodies.sh first`);
      process.exit(1);
    }
    const ok = await b.build([`look:${item}`], jobs, plan);
    if (plan) return;
    // The item's own steps: made (its garment, or the rigid pieces), fitted on each body, its game file.
    const own = steps.filter((s) => [`look:${item}`, `garment:${item}`].includes(s.name) || (s.name.startsWith("fit:") && s.name.endsWith(`:${item}`)) || (s.name === "rigid" && s.outputs.some((o) => o.endsWith(`/${item}.glb`)))).map((s) => s.name);
    const log = join(out, `build-${item}.log`);
    writeFileSync(log, b.joinedLog(own));
    const failed = itemVerdict(readFileSync(log, "utf8"), 1 + morphNames(bodies).length, ok ? 0 : 1);
    const rel = log.slice(ROOT.length + 1);
    if (failed.length) {
      console.log(`✗ ${item} fail: ${failed[0]}; see ${rel}`);
      for (const f of failed.slice(1, 10)) console.log(f);
      process.exit(1);
    }
    console.log(`✓ ${item} pass: made, carried onto ${1 + morphNames(bodies).length} bodies and checked, assets/models/gear/${item}.glb written (${rel})`);
    return;
  }
  const ok = await b.build(undefined, jobs, plan);
  if (plan) return;
  const log = join(out, "build.log");
  writeFileSync(log, b.joinedLog(steps.map((s) => s.name)) + (ok ? `${builtLine(bodies, items)}\n` : ""));
  const rig = join(out, `rig-check-${base}.json`);
  const failed = verdict(readFileSync(log, "utf8"), existsSync(rig) ? readFileSync(rig, "utf8") : undefined, ok ? 0 : 1);
  console.log(failed.length ? `BUILD fail\n${failed.join("\n")}` : "BUILD pass");
  process.exit(failed.length ? 1 : 0);
}

if (import.meta.main) await main();
