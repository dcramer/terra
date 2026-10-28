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
//
// A line a step: built (and why), skipped (unchanged), failed (and why), or not run. Each step's
// output is kept in out/steps/STEP.log; out/build.log is them all in the build's order, skipped
// steps' from when they ran, so build_verdict.ts gives the verdict a full build would.

import { execFileSync, spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, openSync, closeSync, readdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import { availableParallelism } from "node:os";
import { join } from "node:path";
import { FAILED, itemVerdict, verdict } from "./build_verdict.ts";
import { bodiesData, buildSteps, builtLine, closure, gearData, morphNames, SECTIONS, type Step } from "./build_steps.ts";

const ROOT = join(import.meta.dirname, "..", "..");

type Stamp = { key: string; inputs: Record<string, string>; needs: Record<string, string>; outputs: Record<string, string>; seconds: number };
export type Outcome = { state: "built" | "skipped" | "failed" | "not run"; why: string };
/** A failure in the step's own run, not in what it checked: it is run again next time. */
const CRASHED = /Traceback|^run\.sh: .* exited \d+|Error: script failed/m;

const sha = (data: string | Buffer) => createHash("sha256").update(data).digest("hex");
const safe = (name: string) => name.replace(/[^\w.-]+/g, "-");

/** A file's hash, a folder's (every file under it, by path), or "missing". */
function hashPath(root: string, p: string, cache: Map<string, string>): string {
  const hit = cache.get(p);
  if (hit) return hit;
  const abs = join(root, p);
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

  constructor(steps: Step[], root = ROOT, out = join(ROOT, "tools/characters/out"), env: Record<string, string> = {}, print = (l: string) => console.log(l)) {
    this.steps = steps;
    this.root = root;
    this.env = env;
    this.print = print;
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
    const inputs = Object.fromEntries(closure(s.inputs, this.root).map((p) => [p, hashPath(this.root, p, this.inputCache)]));
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

  private async runStep(s: Step, why: string): Promise<Outcome> {
    rmSync(join(this.stamps, `${safe(s.name)}.json`), { force: true });
    for (const p of s.outputs) rmSync(join(this.root, p), { force: true });
    const now = this.readsNow(s);
    const adopt = s.committed && this.asCommitted([...Object.keys(now.inputs), ...(s.inPlace ?? [])]);
    if (adopt) why = `${(s.inPlace ?? []).join(", ")} as committed, from its inputs as committed`;
    const t0 = performance.now();
    const code = await this.exec(s, adopt ? { ADOPT: "1" } : {});
    const seconds = Math.round((performance.now() - t0) / 1000);
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
    return { state: "built", why: `in ${clock(seconds)} (${why})${checks(log)}` };
  }

  /** The step's state without running it: skipped when its stamp holds, else why it would run. */
  private check(s: Step): Outcome {
    const now = this.readsNow(s);
    const why = this.stale(s, now);
    if (why) return { state: "built", why };
    this.made.set(s.name, this.stampOf(s)!.outputs);
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
      const mark = { built: "✓", skipped: "·", failed: "✗", "not run": "-" }[o.state];
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
        const o = pending ? { state: "built" as const, why: "what it needs would run" } : this.check(s);
        if (o.state === "skipped" || plan) {
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
    return [...this.outcomes.values()].every((o) => o.state === "built" || o.state === "skipped");
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
  const steps = buildSteps(bodies, items);
  const out = join(ROOT, "tools/characters/out");
  const b = new Builder(steps, ROOT, out, { blender: "tools/blender/run.sh", here: "tools/characters", out: "tools/characters/out" });
  const base = bodies.base.name;
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
