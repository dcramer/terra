// Dials hold parameters in by search, so nobody sweeps a constant by hand (D71, D632):
//
//   node tools/characters/solve.ts tools/characters/specs/basket.json [--json FILE] [--apply]
//
// A spec names the looks to improve, the looks to hold passing, the clips each is measured in,
// the measures and the parameters (tune.ts's names) with their bounds and step:
//
//   { "seed": 1, "bodies": ["female", "base"],            bodies: every one when left out
//     "looks": { "basket": ["Crouch_Idle_Loop", "Jump_Loop"] },   improved: their measures below
//     "hold":  { "stone_axe": ["Sprint_Loop"] },                  held: every barred measure of theirs
//     "measures": ["carry.*"],                                    of `looks`; * is any run of letters
//     "parameters": [{ "name": "give.back", "lo": 0, "hi": 60, "step": 1 }],
//     "samples": 16, "starts": 2, "budget": 300 }         the search's size; defaults below
//
// A point is every parameter at a value, measured by holds.ts's own rows and bars with moves.ts
// --tune (no clip baked, no source edited), one process a look, body and clip on every core.
// Every point, the start included, is measured with its poses laid by tune.ts, and points are
// compared with each other only, never with a baked number.
//
// The objective, least first, in order (lexicographic):
//   1. over: the sum, over every barred measure of every look, of its worst value's amount over
//      its bar (centimetres and degrees summed as they are); the bars are holds.ts's.
//   2. the thinnest margin under a bar among the improved looks' measures, the wider the better.
//   3. the distance from the start, each parameter's move as a share of its range.
//
// The search is seeded and deterministic, on the grid start + k * step, so the answer is the edit:
//   - a Latin hypercube sample of the bounds (McKay, Beckman and Conover 1979, "A Comparison of
//     Three Methods for Selecting Values of Input Variables in the Analysis of Output from a
//     Computer Code", Technometrics 21), then
//   - compass search from the best few: poll every parameter a step each way at once, move to the
//     best if it is better, else halve the step, until the grid's own step fails (Kolda, Lewis
//     and Torczon 2003, "Optimization by Direct Search", SIAM Review 45, section 1.1 and 3).
//
// stdout is the same bytes for the same spec and tree; seconds and progress go to stderr.
// --apply writes the edits (tune.ts editsOf) and never bakes. Node 22.18 or later; no dependencies.

import { mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { availableParallelism } from "node:os";
import { basename, join } from "node:path";
import { parseArgs } from "node:util";
import { carriedClips, carryHold, carryUpright } from "./held.ts";
import { type Ctx, type Measure, bodies, judged, pool, rowsOf } from "./holds.ts";
import { type EditOf, FIGHT, type Sets, type Tune, editsOf, paramOf, readSets } from "./tune.ts";

const ROOT = join(import.meta.dirname, "..", "..");
const OUT = join(ROOT, "tools", "characters", "out");
const HELD = join(ROOT, "assets", "models", "held");

export type Dim = { name: string; start: number; lo: number; hi: number; step: number };
export type Spec = {
  about?: string;
  seed?: number;
  samples?: number;
  starts?: number;
  budget?: number;
  bodies?: string[];
  looks: Record<string, string[]>;
  hold?: Record<string, string[]>;
  measures: string[];
  parameters: { name: string; lo: number; hi: number; step: number }[];
};
/** One evaluated point: its grid offsets, its values, its objective and what was measured. */
export type Found<T> = { k: number[]; at: number[]; score: number[]; detail: T };

const r6 = (x: number) => Math.round(x * 1e6) / 1e6 + 0;
/** Lexicographic: negative when `a` is the better. */
const cmp = (a: number[], b: number[]) => {
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return a[i] - b[i];
  return 0;
};

/** mulberry32 (Tommy Ettinger's, public domain): a seeded stream in [0, 1). */
function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** The search: a Latin hypercube pass, then compass search from the best few (the header's
 * sources). `evaluate` scores a batch of points at once; the distance from the start is added as
 * the last term. The same seed and evaluator give the same trace. */
export async function search<T>(
  dims: Dim[],
  evaluate: (points: number[][]) => Promise<{ score: number[]; detail: T }[]>,
  opts: { seed: number; samples: number; starts: number; budget: number; progress?: (line: string) => void },
): Promise<{ start: Found<T>; best: Found<T>; trace: Found<T>[] }> {
  const lo = dims.map((d) => Math.ceil((d.lo - d.start) / d.step - 1e-9));
  const hi = dims.map((d) => Math.floor((d.hi - d.start) / d.step + 1e-9));
  const span = dims.map((d) => d.hi - d.lo || 1);
  const seen = new Map<string, Found<T>>();
  const trace: Found<T>[] = [];
  const better = (a: Found<T>, b: Found<T>) => cmp(a.score, b.score) < 0;
  const run = async (ks: number[][]): Promise<Found<T>[]> => {
    const fresh = [...new Map(ks.filter((k) => !seen.has(k.join())).map((k) => [k.join(), k])).values()].slice(0, Math.max(0, opts.budget - seen.size));
    const at = fresh.map((k) => k.map((n, i) => r6(dims[i].start + n * dims[i].step)));
    const got = fresh.length ? await evaluate(at) : [];
    fresh.forEach((k, i) => {
      const far = Math.sqrt(k.reduce((s, n, j) => s + ((n * dims[j].step) / span[j]) ** 2, 0));
      const f = { k, at: at[i], score: [...got[i].score.map(r6), r6(far)], detail: got[i].detail };
      seen.set(k.join(), f);
      trace.push(f);
    });
    return ks.flatMap((k) => seen.get(k.join()) ?? []);
  };
  const [start] = await run([dims.map(() => 0)]);
  // The Latin hypercube: each parameter's range cut in `samples` strata, each stratum used once.
  const rng = mulberry32(opts.seed);
  const strata = dims.map(() => {
    const p = Array.from({ length: opts.samples }, (_, i) => i);
    for (let i = p.length - 1; i > 0; i--) {
      const j = Math.floor(rng() * (i + 1));
      [p[i], p[j]] = [p[j], p[i]];
    }
    return p.map((s) => (s + rng()) / opts.samples);
  });
  await run(Array.from({ length: opts.samples }, (_, s) => dims.map((_, i) => lo[i] + Math.round(strata[i][s] * (hi[i] - lo[i])))));
  opts.progress?.(`sampled ${seen.size} points`);
  // Compass search from the best few; a stable sort, so ties go to the earlier point.
  const from = [...trace].sort((a, b) => cmp(a.score, b.score)).slice(0, opts.starts);
  for (const first of from) {
    let here = first;
    for (let level = 2; seen.size < opts.budget; ) {
      const s = dims.map((_, i) => Math.max(1, Math.floor((hi[i] - lo[i]) / 2 ** level)));
      const poll = dims.flatMap((_, i) => [-s[i], s[i]].map((d) => here.k.map((n, j) => (j === i ? n + d : n)))).filter((k) => k.every((n, i) => n >= lo[i] && n <= hi[i]));
      const best = (await run(poll)).reduce((a, b) => (better(b, a) ? b : a), here);
      if (best !== here) here = best;
      else if (s.every((n) => n === 1)) break;
      else level++;
    }
    opts.progress?.(`compass from ${first.at.join(" ")} ended at ${here.at.join(" ")} after ${seen.size} points`);
  }
  const best = trace.reduce((a, b) => (better(b, a) ? b : a), start);
  return { start, best, trace };
}

const matcher = (patterns: string[]) => {
  const res = patterns.map((p) => new RegExp(`^${p.replace(/[.+?^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*")}$`));
  return (name: string) => res.some((r) => r.test(name));
};

/** A point's measures as its objective: the amount over holds.ts's bars and the thinnest margin,
 * with the worst value of every look's measure (the first of equals, in a fixed order). */
export function scored(ms: Measure[], spec: Spec): { over: number; margin: number; worst: Map<string, Measure> } {
  const wanted = matcher(spec.measures);
  const order = (m: Measure) => `${m.look} ${m.measure} ${m.body} ${m.clip}`;
  const worst = new Map<string, Measure>();
  for (const m of [...ms].sort((a, b) => (order(a) < order(b) ? -1 : order(a) > order(b) ? 1 : 0))) {
    if (typeof m.bar !== "number" && m.bar !== null) continue;
    if (!(m.look in (spec.hold ?? {})) && !(m.look in spec.looks && wanted(m.measure))) continue;
    const k = `${m.look} ${m.measure}`;
    if (!worst.has(k) || m.value > worst.get(k)!.value) worst.set(k, m);
  }
  let over = 0;
  let margin = Number.POSITIVE_INFINITY;
  for (const m of worst.values()) {
    if (typeof m.bar !== "number") continue;
    over += Math.max(0, m.value - m.bar);
    if (m.look in spec.looks) margin = Math.min(margin, m.bar - m.value);
  }
  return { over, margin, worst };
}

export type Solved = { spec: Spec; dims: Dim[]; start: Found<Measure[]>; best: Found<Measure[]>; trace: Found<Measure[]>[]; edits: EditOf[] };

/** The spec's parameters as the search's dimensions; a bad one is an error that says what to do. */
export function dimsOf(spec: Spec, sets: Sets = readSets()): Dim[] {
  if (!spec.parameters?.length) throw new Error("the spec names no parameters (D556): give `parameters`, each { name, lo, hi, step }, a name as tools/characters/tune.ts's header lists them");
  return spec.parameters.map((p) => {
    const start = paramOf(p.name, sets).start;
    if (!(p.step > 0) || !(p.lo <= p.hi)) throw new Error(`parameter ${p.name} needs lo <= hi and a step above 0 (D556): it has lo ${p.lo}, hi ${p.hi}, step ${p.step}`);
    if (start < p.lo || start > p.hi) throw new Error(`parameter ${p.name} starts at ${r6(start)}, its committed value, outside its bounds ${p.lo} to ${p.hi} (D556): widen the bounds in the spec to hold it; the search is compared with the start`);
    return { name: p.name, start, lo: p.lo, hi: p.hi, step: p.step };
  });
}

/** Search a spec with `measure` (a point's measures; holds.ts's through `holdsMeasure`). */
export async function solve(spec: Spec, measure: (tune: Tune) => Promise<Measure[]>, opts: { sets?: Sets; progress?: (line: string) => void } = {}): Promise<Solved> {
  const sets = opts.sets ?? readSets();
  const dims = dimsOf(spec, sets);
  const n = dims.length;
  const evaluate = (points: number[][]) =>
    Promise.all(
      points.map(async (at) => {
        const detail = await measure(Object.fromEntries(dims.map((d, i) => [d.name, at[i]])));
        const { over, margin, worst } = scored(detail, spec);
        if (!Number.isFinite(margin)) {
          const names = [...new Set(detail.filter((m) => typeof m.bar === "number").map((m) => `${m.look} ${m.measure}`))].sort();
          throw new Error(`the spec's measures (${spec.measures.join(", ")}) match no barred measure of ${Object.keys(spec.looks).join(", ")} in the clips it names (D556): name patterns that match one of: ${names.join(", ") || "none measured"}`);
        }
        return { score: [over, -margin], detail: [...worst.values()] };
      }),
    );
  const found = await search(dims, evaluate, { seed: spec.seed ?? 1, samples: spec.samples ?? 8 * n, starts: spec.starts ?? 3, budget: spec.budget ?? 60 * n + 40, progress: opts.progress });
  const moved = Object.fromEntries(dims.flatMap((d, i) => (found.best.k[i] ? [[d.name, found.best.at[i]]] : [])));
  return { spec, dims, ...found, edits: editsOf(moved, sets) };
}

const num = (x: number) => String(Number(x.toFixed(3)) + 0);
const one = (x: number) => (x + 0).toFixed(1);

/** The report (D388): at most 24 lines, the same bytes for the same search. */
export function report(name: string, s: Solved): string[] {
  const { dims, start, best, spec } = s;
  const held = Object.keys(spec.hold ?? {}).length;
  const out = [`solve ${name}: ${dims.length} parameters, ${Object.keys(spec.looks).join(", ")} improved, ${held} looks held passing; ${s.trace.length} evaluations, seed ${spec.seed ?? 1}`];
  const obj = (f: Found<Measure[]>) => `over bars ${num(f.score[0])} (cm and deg summed), thinnest margin ${num(-f.score[1])}, distance ${num(f.score[2])}`;
  out.push(`  start:  ${obj(start)}`, `  answer: ${obj(best)}`);
  dims.forEach((d, i) => {
    const edge = best.at[i] <= d.lo + d.step / 2 ? "lower" : best.at[i] >= d.hi - d.step / 2 ? "upper" : "";
    out.push(`  ${d.name}: ${num(d.start)} -> ${num(best.at[i])}${edge && best.k[i] ? ` (left at its ${edge} bound ${edge === "lower" ? d.lo : d.hi}: widen it in the spec to search past)` : ""}`);
  });
  const before = new Map(start.detail.map((m) => [`${m.look} ${m.measure}`, m]));
  const changed = best.detail
    .map((m) => ({ m, was: before.get(`${m.look} ${m.measure}`) }))
    .filter(({ m, was }) => !was || one(was.value) !== one(m.value))
    .sort((a, b) => Math.abs(b.m.value - (b.was?.value ?? 0)) - Math.abs(a.m.value - (a.was?.value ?? 0)));
  const edits = s.edits.map((e) => `  edit ${e.file}: ${e.key} = ${Array.isArray(e.value) ? `[${e.value.join(", ")}]` : num(e.value)}`);
  const room = Math.max(1, 24 - out.length - 2 - Math.min(edits.length, 6));
  const cut = <T>(rows: T[], n: number) => (rows.length > n ? rows.slice(0, n - 1) : rows);
  const shown = cut(changed, room);
  for (const { m, was } of shown) {
    const of = (x: Measure) => `${one(x.value)} ${x.unit} (${x.body}, ${x.clip})`;
    const state = typeof m.bar !== "number" ? "a reading" : `bar ${m.bar}, ${m.value <= m.bar ? "passes" : "fails"}`;
    out.push(`  ${m.look} ${m.measure}: ${was ? of(was) : "unmeasured"} -> ${of(m)}; ${state}`);
  }
  if (shown.length < changed.length) out.push(`  and ${changed.length - shown.length} more measures changed: --json FILE has every one`);
  const some = cut(edits, 6);
  out.push(...some);
  if (some.length < edits.length) out.push(`  and ${edits.length - some.length} more edits: --json FILE has every one`);
  const next = edits.length ? `--apply writes the ${edits.length} edits; then bake with tools/characters/build_bodies.sh --step fight and run holds.ts` : "the start is the best point found: nothing to edit";
  out.push(`${best.score[0] > 0 ? "✗" : "✓"} solve: ${best.score[0] > 0 ? `${num(best.score[0])} still over a bar` : "every barred measure within its bar"}; ${next}`);
  return out;
}

/** Write the edits under `root`: fight.json's keys, the Rust constants and grip.ts's. */
export function apply(edits: EditOf[], root = ROOT): void {
  const texts = new Map<string, string>();
  const text = (file: string) => texts.get(file) ?? readFileSync(join(root, file), "utf8");
  for (const e of edits) {
    if (e.file === FIGHT) {
      // Only the lines the edit changes are rewritten: the file keeps its own way of writing a
      // number (8.0), which JSON.stringify would not.
      const was = text(e.file).split("\n");
      const json = JSON.parse(was.join("\n"));
      const lines = () => `${JSON.stringify(json, null, 2)}\n`.split("\n");
      const before = lines();
      const path = e.key.split(/[.[\]]+/).filter(Boolean);
      const last = path.pop()!;
      const at = path.reduce((o, k) => o?.[k], json);
      if (at?.[last] === undefined) throw new Error(`${e.file} has no ${e.key} to set (D556): check tune.ts editsOf against the file`);
      at[last] = e.value;
      const after = lines();
      if (before.length !== was.length || after.length !== was.length) throw new Error(`${e.file} is not laid out a value a line, as JSON.stringify(x, null, 2) lays it, so --apply cannot change ${e.key} alone (D556): make the edits by hand (they are listed above)`);
      texts.set(e.file, was.map((line, i) => (before[i] === after[i] ? line : after[i])).join("\n"));
      continue;
    }
    const rust = e.file.endsWith(".rs");
    const re = rust ? new RegExp(`(pub const ${e.key}: f32 = )[^;]+;`) : new RegExp(`(export const ${e.key} = )[^;]+;`);
    const v = e.value as number;
    const was = text(e.file);
    if (!re.test(was)) throw new Error(`${e.file} has no constant ${e.key} declared as ${rust ? `pub const ${e.key}: f32 = N;` : `export const ${e.key} = N;`} (D556): set it to ${v} by hand`);
    texts.set(e.file, was.replace(re, `$1${rust && Number.isInteger(v) ? `${v}.0` : r6(v)};`));
  }
  for (const [file, t] of texts) writeFileSync(join(root, file), t);
}

/** A spec read and checked: an unknown look is an error that lists what there is. */
export function readSpec(file: string): Spec {
  const spec = JSON.parse(readFileSync(file, "utf8")) as Spec;
  const all = readdirSync(HELD).filter((f) => f.endsWith(".glb")).map((f) => f.slice(0, -4)).sort();
  if (!spec.looks || !Object.keys(spec.looks).length || !spec.measures?.length) throw new Error(`${file} needs \`looks\` (a look to its clips) and \`measures\` (patterns like carry.*) (D556): see the head of tools/characters/solve.ts`);
  const looks = { ...spec.hold, ...spec.looks };
  const unknown = Object.keys(looks).filter((l) => !all.includes(l));
  if (unknown.length) throw new Error(`${file} names no held look ${unknown.join(", ")} in assets/models/held (D556): name any of ${all.join(", ")}`);
  const carried = carriedClips();
  for (const [look, clips] of Object.entries(looks)) {
    const bad = Array.isArray(clips) ? clips.filter((c) => !carried.includes(c)) : ["(no list)"];
    if (bad.length || !clips.length) throw new Error(`${file}: ${look} needs a list of the clips it is measured in, each one the game carries a look in; ${bad.join(", ") || "it has none"} is not (D556): name any of ${carried.join(", ")}`);
  }
  const known = bodies();
  const off = (spec.bodies ?? []).filter((b) => !known.includes(b));
  if (off.length) throw new Error(`${file} names no body ${off.join(", ")} (D556): name any of ${known.join(", ")}, or leave \`bodies\` out for all`);
  return spec;
}

/** holds.ts's measures of a point: one moves.ts process a look, body and clip, a look given only
 * the parameters it depends on (its own carry pose's, the give unless it is carried upright, the
 * seat), so a point that changes nothing of a look measures it once. */
export function holdsMeasure(spec: Spec, ctx: Ctx, limit: <T>(job: () => Promise<T>) => Promise<T>): (tune: Tune) => Promise<Measure[]> {
  const carried = carriedClips();
  const on = spec.bodies ?? bodies();
  const looks = Object.entries({ ...spec.hold, ...spec.looks }).map(([name, clips]) => ({ name, clips, file: join("assets", "models", "held", `${name}.glb`), pose: carryHold(join(HELD, `${name}.glb`)), upright: carryUpright(join(HELD, `${name}.glb`)) }));
  const memo = new Map<string, Promise<Measure[]>>();
  return async (tune) => {
    const runs: Promise<Measure[]>[] = [];
    for (const look of looks) {
      const mine = Object.fromEntries(
        Object.keys(tune)
          .sort()
          .filter((k) => (k.startsWith("give.") ? !look.upright : !k.startsWith("pose.Carry_") || k.split(".")[1] === look.pose))
          .map((k) => [k, tune[k]]),
      );
      for (const body of on)
        for (const clip of look.clips) {
          const k = `${look.name} ${body} ${clip} ${JSON.stringify(mine)}`;
          if (!memo.has(k)) memo.set(k, limit(() => rowsOf(look.file, body, { ...ctx, tune: mine, clips: [clip] })).then((rows) => judged(look.name, body, rows, carried)));
          runs.push(memo.get(k)!);
        }
    }
    return (await Promise.all(runs)).flat();
  };
}

if (import.meta.main) {
  const { values: opt, positionals } = parseArgs({ allowPositionals: true, options: { json: { type: "string" }, apply: { type: "boolean", default: false } } });
  if (positionals.length !== 1) {
    console.error("✗ solve: give one spec (D556): node tools/characters/solve.ts tools/characters/specs/NAME.json [--json FILE] [--apply]");
    process.exit(2);
  }
  const name = basename(positionals[0], ".json");
  const scratch = join(OUT, `solve-${name}`);
  const log = join(OUT, `solve-${name}.log`);
  const began = Date.now();
  const secs = () => ((Date.now() - began) / 1000).toFixed(0);
  try {
    const spec = readSpec(positionals[0]);
    mkdirSync(scratch, { recursive: true });
    writeFileSync(log, "");
    const measure = holdsMeasure(spec, { log, out: scratch }, pool(availableParallelism()));
    const solved = await solve(spec, measure, { progress: (line) => console.error(`solve ${name}: ${line}, ${secs()} s`) });
    console.log(report(name, solved).join("\n"));
    if (opt.json) writeFileSync(opt.json, `${JSON.stringify({ spec, parameters: solved.dims, start: solved.start, answer: solved.best, edits: solved.edits, trace: solved.trace.map((f) => ({ at: f.at, score: f.score })) }, null, 1)}\n`);
    if (opt.apply) apply(solved.edits);
    console.error(`solve ${name}: ${solved.trace.length} evaluations in ${secs()} s${opt.apply ? `; ${solved.edits.length} edits written, nothing baked` : ""}`);
    rmSync(scratch, { recursive: true, force: true });
    process.exit(solved.best.score[0] > 0 ? 1 : 0);
  } catch (e) {
    rmSync(scratch, { recursive: true, force: true });
    console.error(`✗ solve: ${(e as Error).message}`);
    process.exit(2);
  }
}
