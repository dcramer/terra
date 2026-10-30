#!/usr/bin/env node
// The character controller's scorecard (D457, tools/controller/README.md): every criterion of
// criteria.json measured, a line each, and held to baseline.tsv by the shared ratchet
// (tools/scorecard, D452's).
//
//   cargo xtask controller                    the cheap criteria: the plain-Rust course
//                                             (terra_motion::course) driving the mover, seconds
//   cargo xtask controller --json FILE        the results also as JSON
//   cargo xtask controller --bench            also the heavy cost rows: the mover's release bench
//                                             (terra_walker's movement::bench, D185's CPU half)
//   cargo xtask controller --film             also the in-game filmed course's rows (yard.ts: the GPU,
//                                             minutes; `cargo xtask gpu film -- cargo xtask controller --film`);
//                                             a run that did not finish leaves its rows not measured
//   cargo xtask controller --film --out DIR   the film somewhere other than ~/.cache/terra-review/controller-yard
//   cargo xtask controller --filmed          the film's rows from main's own film, not filming: the
//                                             newest clean one nothing it reads changed since
//                                             (tools/scorecard/films.ts; nightly's, so it banks them)
//   cargo xtask controller --raise "<why>"    write the rows that got worse with why (new rows:
//                                             their first number, a "first number" why their own)
//   cargo xtask controller --lower            write the rows that got better (nightly's, D414)
//   cargo xtask controller --first ID,...     write held rows again as first numbers
//
// The heavy criteria: the mover's bench runs with --bench (release, one core, medians); the
// in-game filmed course with --film (yard.ts writes checks.json beside its film). What cargo printed
// goes to target/controller/scorecard.log.
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { MARKER_BAKES } from "../characters/build_steps.ts";
import { cutClips } from "../characters/glb_clips.ts";
import { bakeFile } from "../characters/markers.ts";
import { CONTROLLER_FILMS, mainFilm } from "../scorecard/films.ts";
import { type Criterion, finish, type Held, judge, readBaseline, round, type Status, TEST_BUILD } from "../scorecard/ratchet.ts";

const ROOT = resolve(import.meta.dirname, "../..");
export const CRITERIA = resolve(import.meta.dirname, "criteria.json");
export const BASELINE = resolve(import.meta.dirname, "baseline.tsv");
/// The mover's bench: one body walked on the course and the planet's ground at 2 and 30 km.
const BENCH = "movement::bench::the_mover_bench_times_a_body_walking";

/// The course's numbers from the test's output: `scorecard`, an id and a number, tab-separated.
export function courseValues(out: string): Map<string, number> {
  const values = new Map<string, number>();
  for (const m of out.matchAll(/^scorecard\t(\S+)\t(\S+)$/gm)) {
    const value = Number(m[2]);
    if (Number.isFinite(value)) values.set(m[1], round(value));
  }
  return values;
}

/// The clip files of `files` whose baked foot-lock markers (D455, tools/characters/markers.ts)
/// are missing or not what the build's bake makes now (its MARKER_BAKES): a re-export that drops
/// them leaves every body's feet unlocked. A clip that skates is the bake's own fault, not
/// counted here.
export function staleMarkers(files: string[]): string[] {
  return files.filter((file) => {
    const bake = MARKER_BAKES[file];
    const roots = bake && cutClips(readFileSync(resolve(ROOT, bake.pack)), bake.clips);
    const { count, same } = bakeFile(resolve(ROOT, file), false, false, roots, bake?.plants);
    return !count || !same;
  });
}

/// The film rows' numbers from a film's checks (yard.ts's checks.json), by each row's check name;
/// a check with no number (null: a run that did not finish) leaves its line as the row's note.
export function filmValues(criteria: Criterion[], checks: { name: string; value: number | null; line?: string }[], values: Map<string, number>, notes: Map<string, string>) {
  for (const c of criteria) {
    const check = c.check as { by?: string; name?: string } | null;
    const found = check?.by === "film" ? checks.find((each) => each.name === check.name) : undefined;
    if (found && Number.isFinite(found.value)) values.set(c.id, round(found.value!));
    else if (found?.line) notes.set(c.id, found.line.replace(/^✗ (not measured: )?/, ""));
  }
}

function main() {
  const { values: opt } = parseArgs({
    options: {
      json: { type: "string" },
      raise: { type: "string" },
      lower: { type: "boolean", default: false },
      first: { type: "string" },
      bench: { type: "boolean", default: false },
      film: { type: "boolean", default: false },
      filmed: { type: "boolean", default: false },
      out: { type: "string" },
    },
  });
  if (opt.raise !== undefined && !opt.raise.trim()) throw new Error("--raise needs its why");
  const file: { course: { crate: string; test: string }; criteria: Criterion[] } = JSON.parse(readFileSync(CRITERIA, "utf8"));
  const criteria = file.criteria;
  const held = existsSync(BASELINE) ? readBaseline(readFileSync(BASELINE, "utf8")) : new Map<string, Held>();
  const logDir = resolve(ROOT, "target/controller");
  mkdirSync(logDir, { recursive: true });
  const log = join(logDir, "scorecard.log");

  // The course: one test drives the mover through every case and prints a number a criterion.
  const args = [...TEST_BUILD, "--", "--exact", file.course.test, "--nocapture"];
  const r = spawnSync("cargo", args, { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 });
  const out = (r.stdout ?? "") + (r.stderr ?? "");
  const values = courseValues(out);
  let logged = `scorecard.ts ${process.argv.slice(2).join(" ")}\n\n$ cargo ${args.join(" ")}\n${out}`;

  // The game's clip files keep their baked foot locks.
  for (const c of criteria) {
    const check = c.check as { by?: string; files?: string[] } | null;
    if (check?.by !== "markers" || !check.files) continue;
    const stale = staleMarkers(check.files);
    values.set(c.id, stale.length);
    if (stale.length) console.log(`  markers stale or missing (run node tools/characters/markers.ts on them): ${stale.join(", ")}`);
  }

  // A criterion another crate's test measures (the figure's gaits, terra_walker::pace).
  for (const c of criteria) {
    const check = c.check as { by?: string; crate?: string; test?: string } | null;
    if (check?.by !== "test" || !check.crate || !check.test) continue;
    const t = [...TEST_BUILD, "--", "--exact", check.test, "--nocapture"];
    const tr = spawnSync("cargo", t, { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 });
    const tout = (tr.stdout ?? "") + (tr.stderr ?? "");
    logged += `\n$ cargo ${t.join(" ")}\n${tout}`;
    const found = courseValues(tout).get(c.id);
    if (found !== undefined) values.set(c.id, found);
  }

  // The bench: the mover's cost a body a tick, in release on one core (D185's CPU half).
  if (opt.bench) {
    const bench = ["test", "-q", "--release", "-p", "terra_walker", "--lib", "--", "--ignored", "--exact", BENCH, "--nocapture", "--test-threads=1"];
    const b = spawnSync("cargo", bench, { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 });
    const bout = (b.stdout ?? "") + (b.stderr ?? "");
    logged += `\n$ cargo ${bench.join(" ")}\n${bout}`;
    for (const [id, v] of courseValues(bout)) values.set(id, v);
    for (const m of bout.matchAll(/^bench\t(.*)$/gm)) console.log(`  ${m[1].replaceAll("\t", ": ")}`);
  }
  // The in-game filmed course: its checks' numbers, a row each.
  const notes = new Map<string, string>();
  if (opt.film) {
    const out = resolve(opt.out ?? join(homedir(), ".cache/terra-review/controller-yard"));
    const json = join(out, "checks.json");
    rmSync(json, { force: true });
    // Its stderr, the runs' newest line every 30 s, passes straight on: `cargo xtask gpu` sees
    // the film going by it (yard.ts's PASS_ON_MS).
    const y = spawnSync(process.execPath, [resolve(import.meta.dirname, "yard.ts"), "--out", out], { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28, stdio: ["ignore", "pipe", "inherit"] });
    logged += `\n$ node tools/controller/yard.ts --out ${out}\n${(y.stdout ?? "") + (y.stderr ?? "")}`;
    for (const m of (y.stdout ?? "").matchAll(/^yard: (.*)$/gm)) console.log(`  ${m[1]}`);
    const checks = existsSync(json) ? JSON.parse(readFileSync(json, "utf8")) : [];
    filmValues(criteria, checks, values, notes);
  }
  // Main's film without filming (nightly's): the newest clean film of this commit's history
  // that nothing the film reads changed since (tools/scorecard/films.ts).
  let filmed = false;
  if (opt.filmed && !opt.film) {
    const found = mainFilm(CONTROLLER_FILMS);
    if ("why" in found) console.log(`  film rows not judged: ${found.why}`);
    else {
      filmed = true;
      console.log(`  film rows from main's film at ${found.commit.slice(0, 10)}, ${found.behind} commits back, nothing the film reads changed since`);
      filmValues(criteria, JSON.parse(readFileSync(join(found.dir, "checks.json"), "utf8")), values, notes);
    }
  }
  writeFileSync(log, logged);

  const ran = (c: Criterion) => ({ bench: opt.bench, film: opt.film || filmed })[(c.check as { by?: string } | null)?.by ?? ""] === true;
  const results = criteria.map((c) => (c.measurable && c.cost === "heavy" && !ran(c) ? { id: c.id, status: "skipped" as Status } : judge(c, values.get(c.id), held.get(c.id))));
  for (const r of results) if (r.status === "missing" && notes.has(r.id)) r.note = notes.get(r.id);
  finish({
    name: "controller",
    criteria,
    results,
    held,
    baseline: BASELINE,
    baselineName: "tools/controller/baseline.tsv",
    log: "target/controller/scorecard.log",
    heavy: "run with --bench, the filmed course with --film",
    heavyRun: false,
    json: opt.json,
    raise: opt.raise,
    lower: opt.lower,
    first: opt.first?.split(",").filter(Boolean),
  });
}

if (import.meta.main) main();
