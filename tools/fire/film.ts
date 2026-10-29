// The motion gate's films (D472): the yard's campfire filmed still from RDR2's camp view for 20 s at
// dusk and at night, at seven placements of its flame (PLACEMENTS: the fire and camera moved together
// along x, so the view is the same and only the flame's seed changes, hearth.rs), each frame
// with its flame matte (--flame-matte, D433), each placement filmed TAKES times; then motion.ts's
// measures, each hour gated by their middle-half mean (`held`). Exit 1 when either hour misses.
//
// The gate is paired (paired.ts): the base (`--base <sha>`, else where HEAD leaves origin/main) is
// built in a worktree of its own and filmed beside the build in the same run, and a measure the
// build misses passes when the build is no further outside its bar than the base, within the
// run-to-run spread repeat films of one flame show. `--alone` films the build alone, bars only.
// A base that falls to the build itself (HEAD, nothing uncommitted: on the worker, a --dirty job's
// patch) is refused unless named (`--base HEAD`, an A/A run).
// Each side's measures are kept as `<out>/{build,base}/<hour>.json`; `--judge <out>` judges them
// again. The films themselves (~12 GB a run) are deleted once measured unless `--films`.
//
// The gate measures the flame, so the light it casts is held: the build is filmed with every hearth
// light constant of crates/terra_world/src/fire/light.rs (`HEARTH_*`) set to REFERENCE, main's at
// 6741693a, and the file put back after the build. A brighter pit light lit the ground behind the
// flame's thin low skin past the flame cut and moved the lean and splits with the flame unchanged
// (fire round g54: y1, light only, failed at both hours where main passed). The sky behind it is
// held too, as the fire sheet's (`HELD_SKY`, D472): the dome's 2D clouds, not the marched volumes,
// whose arrival raised the dusk splits with no fire changed.
//
//   node tools/fire/film.ts [out=shots/motion] [--base <sha> | --alone] [--seconds 20] [--films]   # from the checkout's root
//   node tools/fire/film.ts --judge <out> [--alone]     # judge kept films again
//   node tools/fire/film.ts --aa <side dir>...          # pool A/A films into gate-aa.json, print tolerances
import { spawn, spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, openSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { analyse, GATE, load, said } from "./motion.ts";
import { type Film, paired, type Side, told, tolerance, TOLERANCE } from "./paired.ts";
import { HELD_SKY } from "./sheet.ts";

/// The hearth light the films are lit by: main's at 6741693a, as `const NAME: type = value;`'s value.
export const REFERENCE: Record<string, string> = {
  HEARTH_LIGHT: "Color::srgb(1.0, 0.50, 0.08)",
  HEARTH_LUMENS_A_METRE: "60_000.0",
  HEARTH_RANGE_M: "6.0",
  HEARTH_NIGHT_STOPS: "3.0",
  HEARTH_DUSK_STOPS: "2.5",
  HEARTH_SPREAD: "1.0",
  HEARTH_LIGHT_UP: "0.4",
  HEARTH_TOP: "0.6",
  HEARTH_TOP_GAIN: "3.0",
  HEARTH_TOP_UP: "1.5",
  HEARTH_TOP_RANGE_M: "10.0",
  HEARTH_TOP_SPREAD: "2.0",
  HEARTH_TOP_DUSK: "7.0",
  // The pit key's dusk gain and the top light's dusk reach (g82), as main lit before either
  // was: no gain, and a reach past the top light's range (it keeps at most all of it).
  HEARTH_KEY_DUSK: "1.0",
  HEARTH_TOP_DUSK_REACH: "100.0",
  HEARTH_TOP_NIGHT_STOPS: "1.0",
  HEARTH_DUSK_SKY: "360.0",
};
const CONST = /^(\s*(?:pub(?:\([a-z]+\))? )?const (HEARTH_\w+): [\w:]+ = )(.+);$/;

/// The source with every hearth constant at its reference, and which changed. Throws where the
/// source's hearth constants are not REFERENCE's (one added or renamed): the hold would leave it free.
/// `partial` (the base's light.rs, older than the build's REFERENCE) lets it lack some of them.
export function hold(source: string, reference = REFERENCE, partial = false): { source: string; changed: string[] } {
  const seen = new Set<string>();
  const changed: string[] = [];
  const lines = source.split("\n").map((line) => {
    const m = CONST.exec(line);
    if (!m) return line;
    const want = reference[m[2]];
    if (want === undefined) throw new Error(`${m[2]} is not in film.ts's REFERENCE: add it at main's value`);
    seen.add(m[2]);
    if (m[3] !== want) changed.push(`${m[2]} ${m[3]} -> ${want}`);
    return `${m[1]}${want};`;
  });
  const missing = Object.keys(reference).filter((k) => !seen.has(k));
  if (missing.length && !partial) throw new Error(`light.rs has no ${missing.join(", ")}: film.ts's REFERENCE is stale`);
  return { source: lines.join("\n"), changed };
}

/// The flame's placements, m along x (fire and camera moved together): seven, each its own seed
/// (hearth.rs: x * 0.731 + z * 0.377, hashed into the loop's offset, mirror, turn and scale, sim.rs).
/// Three (0, 0.3, 0.6) were the
/// three lowest-splitting of these seven at dusk, and one film's splits move by about 0.1 run to
/// run at one placement, so their median sat at the bar by luck and went either way (fire gate
/// round: main dbf2f473's dusk 1.01-1.23 over four runs of three).
export const PLACEMENTS = [0, 0.3, 0.6, 0.9, 1.2, 1.5, 1.8];
/// Films of each placement. Two films of one build are not the same film (aa.ts: whole frames
/// and mattes differ run to run, determinism rounds 2-3), so a placement's splits move by up to
/// 0.15 from one run to the next; each is filmed twice and all fourteen held together.
export const TAKES = 2;
/// Every film's folder: each placement's, a take each.
export const takes = (out: string) =>
  PLACEMENTS.flatMap((dx) => Array.from({ length: TAKES }, (_, t) => ({ dx, dir: join(out, `p${String(dx).replace(".", "")}${t ? `-${t + 1}` : ""}`) })));
export const HOURS: [string, string][] = [["dusk", "18.4"], ["night", "22"]];
const CROP = "crop=600:600:500:250";

/// Builds the checkout at `root` with its hearth light held, into `target` (its own by default;
/// never another checkout's, `prepareBase`). `partial` as `hold`.
export function build(root: string, target = join(root, "target"), partial = false): string[] {
  const path = join(root, "crates/terra_world/src/fire/light.rs");
  const was = readFileSync(path, "utf8");
  const { source, changed } = hold(was, REFERENCE, partial);
  writeFileSync(path, source);
  try {
    const env = { ...process.env, CARGO_TARGET_DIR: target };
    const b = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: root, stdio: "inherit", env });
    if (b.status !== 0) throw new Error("cargo build failed");
  } finally {
    writeFileSync(path, was);
  }
  return changed;
}

/// The sandbox's options for one placement's film at an hour into `out`, its stills in `frames`:
/// the yard, the campfire's calls, the camera, every other frame with its matte, under the sky
/// the fire sheet holds too (`HELD_SKY`: the dome's 2D clouds, never the marched ones).
export function filmArgs(out: string, frames: string, time: string, seconds: number, from = 120): string[] {
  return [
    ...["--offscreen", "--clean", "--picture", "1600x900", "--experiment", "play", "--level", "yard", "--free-build", "--mute"],
    ...["--at", "0,0", "--face", "0,30", "--time", time, "--date", "09-22", "--calls", join(out, "calls.jsonl")],
    ...["--cine", `${join(out, "shot.json")}@120`, "--shots", `${join(frames, "f")}@${from}-${120 + seconds * 60 - 2}/2`, "--flame-matte"],
    ...HELD_SKY,
  ];
}

export function film(root: string, bin: string, out: string, dx: number, seconds: number, hours = HOURS, keep = false, from = 120) {
  const C = [-10 + dx, 0, 10];
  const key = (t: number) => ({ t, at: [-10 + dx, 2.6, 13.95], look: [C[0], 0.65, 10], lens: 29, focus: "look", aperture: 5.6 });
  const shot = { about: `The motion gate's campfire, still, ${seconds} s (tools/fire/film.ts)`, seconds, keys: [key(0), key(seconds)], look: { shutter: 180 }, player: { hidden: true } };
  const spawn = { frame: 2, method: "staged.spawn", params: { name: "campfire", model: "models/props/Campfire.glb", at: C, facing: [C[0], 0, C[2] + 5], base: true, burns: [1.5, 0.7, 0.9] } };
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "shot.json"), JSON.stringify(shot, null, 1));
  writeFileSync(join(out, "calls.jsonl"), `${JSON.stringify(spawn)}\n`);
  const ff = (args: string[]) => spawnSync("ffmpeg", ["-y", "-v", "error", ...args], { encoding: "utf8" });
  for (const [hour, time] of hours) {
    const frames = join(out, `${hour}-frames`);
    rmSync(frames, { recursive: true, force: true });
    mkdirSync(frames, { recursive: true });
    const r = spawnSync("timeout", [String(1500 + seconds * 120), bin, ...filmArgs(out, frames, time, seconds, from)],
      { cwd: root, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: root }, maxBuffer: 1 << 28 });
    writeFileSync(join(out, `${hour}-run.log`), `${r.stdout}${r.stderr}`);
    if (r.status !== 0) throw new Error(`the ${hour} run exited ${r.status}: ${join(out, `${hour}-run.log`)}`);
    const enc = (glob: string, vf: string, to: string) => {
      const e = ff(["-framerate", "30", "-pattern_type", "glob", "-i", join(frames, glob), "-vf", vf, "-an", "-c:v", "libx264", "-qp", "0", "-pix_fmt", "yuv444p", join(out, to)]);
      if (e.status !== 0) throw new Error(`ffmpeg ${to}: ${e.stderr.slice(0, 300)}`);
    };
    enc("f-*[0-9].png", CROP, `${hour}.mkv`);
    enc("f-*.matte.png", `${CROP},format=gray`, `${hour}-matte.mkv`);
    if (!keep) rmSync(frames, { recursive: true, force: true });
  }
}

/// Films one side's every take at each hour into `out` and measures them: `<hour>.json` (a `Side`:
/// each film's placement and GATE values) and `<hour>.txt` (motion.ts's report of each film).
/// The films (lossless, about 12 GB a run) are deleted once measured unless `keepFilms`: the gate
/// and `--judge` read only the measures, and a worker job collects whatever is left.
export function filmSide(root: string, bin: string, out: string, seconds: number, rev: string, keepFilms = false) {
  const all = takes(out);
  for (const { dx, dir } of all) film(root, bin, dir, dx, seconds);
  for (const [hour] of HOURS) {
    const ms = all.map(({ dx, dir }) => {
      const src = (input: string) => ({ input, fps: 30, from: 0, seconds });
      const { clip, cut } = load(src(join(dir, `${hour}.mkv`)), src(join(dir, `${hour}-matte.mkv`)));
      return { dx, dir, m: analyse(clip, cut), cut };
    });
    writeFileSync(join(out, `${hour}.txt`), ms.map(({ dir, m, cut }) => said(dir, m, `flame luma >= ${cut.toFixed(0)} in the matte`)).join("\n") + "\n");
    const side: Side = { rev, hour, films: ms.map(({ dx, dir, m }) => ({ dx, dir, v: GATE.map((g) => g.of(m)) })) };
    writeFileSync(join(out, `${hour}.json`), JSON.stringify(side));
    if (!keepFilms) for (const { dir } of all) for (const f of [`${hour}.mkv`, `${hour}-matte.mkv`]) rmSync(join(dir, f), { force: true });
  }
}

const git = (cwd: string, args: string[]) => spawnSync("git", args, { cwd, encoding: "utf8" });

/// The commit the build is judged beside: `given`, else where HEAD leaves origin/main (HEAD itself
/// on main). `same` when that is HEAD with nothing changed in the tree, so one build serves both.
/// Throws when the base was not given and is the build itself (HEAD, nothing uncommitted): on the
/// worker a `--dirty` job's patch is HEAD and its origin/main may lack it or be absent, so the
/// default fell to HEAD and the gate judged the build beside itself, which passes any change.
/// An A/A run (one build filmed twice, for `--aa`) names the base: `--base HEAD`.
export function baseOf(root: string, given?: string): { rev: string; same: boolean } {
  const head = git(root, ["rev-parse", "HEAD"]).stdout.trim();
  const want = given ?? (git(root, ["merge-base", "HEAD", "origin/main"]).stdout.trim() || head);
  const r = git(root, ["rev-parse", "--verify", "-q", `${want}^{commit}`]);
  if (r.status !== 0) throw new Error(`no commit ${want} in this checkout: pass --base <sha> of HEAD's history`);
  const rev = r.stdout.trim();
  const same = rev === head && git(root, ["status", "--porcelain", "--untracked-files=no"]).stdout.trim() === "";
  if (same && given === undefined)
    throw new Error(
      `film.ts: the base defaulted to the build itself (${head.slice(0, 9)}, where HEAD leaves origin/main with nothing uncommitted), ` +
        "so the paired gate (D472) would judge the build beside itself and pass any change. On the worker a --dirty job's patch is HEAD: " +
        "pass --base <sha> of main before the change (the patch's base), or --alone to hold the build to the bars only; " +
        "for an A/A run of one build filmed twice, say so with --base HEAD.",
    );
  return { rev, same };
}

/// The base's checkout (a worktree at target/gate/base, its own assets) and its held build, built
/// into a target of its own (target/gate/base-target, kept between runs so the registry's crates
/// build once), its binary kept beside it, and its textures cooked as the worker cooks a job's.
export function prepareBase(root: string, rev: string): { dir: string; bin: string; changed: string[] } {
  const dir = join(root, "target/gate/base");
  git(root, ["worktree", "remove", "--force", dir]);
  rmSync(dir, { recursive: true, force: true });
  git(root, ["worktree", "prune"]);
  const add = git(root, ["worktree", "add", "-q", "--detach", dir, rev]);
  if (add.status !== 0) throw new Error(`git worktree add ${rev.slice(0, 9)}: ${add.stderr.trim()}`);
  // The base builds into a target of its own, never the build's: cargo gives a path crate (the
  // workspace's and patches/') the same unit in any checkout of the workspace, and judges it
  // fresh by the files its last build read, named relative to the target when they lie under
  // it, as this worktree does. So the build linked the base's copy of every crate whose files
  // the gate did not touch, and a gate that failed before removing this worktree left the
  // build's target linking it on later jobs (tundra rounds 12 and 14: bevy_pbr's new field
  // missing on worker slot 3 for hours).
  const target = join(root, "target/gate/base-target");
  const changed = build(dir, target, true);
  const bin = join(root, "target/gate/base-sandbox");
  copyFileSync(join(target, "release/terra_sandbox"), bin);
  const cook = spawnSync("cargo", ["run", "-q", "--release", "-p", "terra_bake", "--"], { cwd: dir, encoding: "utf8", env: { ...process.env, CARGO_TARGET_DIR: target } });
  if (cook.status !== 0) throw new Error(`the base's texture cook failed: ${`${cook.stdout}${cook.stderr}`.trim().split("\n").at(-1)}`);
  return { dir, bin, changed };
}

const readSide = (dir: string, hour: string): Side => JSON.parse(readFileSync(join(dir, `${hour}.json`), "utf8"));

/// Judges the films kept in `out` (build/, and base/ unless `alone`), a line an hour; the hours failed.
export function judge(out: string, alone = false): number {
  let failed = 0;
  for (const [hour] of HOURS) {
    const b = readSide(join(out, "build"), hour);
    const a = alone ? undefined : readSide(join(out, "base"), hour);
    const j = paired(b.films, a?.films, TOLERANCE[hour]);
    const passed = j.filter((x) => x.pass).length;
    const met = j.filter((x) => x.met).length;
    if (passed < GATE.length) failed++;
    const beside = a ? `, ${passed - met} no worse than base ${a.rev.slice(0, 9)}'s ${a.films.length} (~)` : "";
    console.log(`${passed === GATE.length ? "✓" : "✗"} ${hour}: ${b.films.length} films met ${met} of ${GATE.length}${beside}: ${told(j)} (${join(out, "build", `${hour}.txt`)})`);
  }
  return failed;
}

/// Pools A/A sides (each a folder of `<hour>.json`, films of one flame) into gate-aa.json and
/// prints the tolerances they give, for paired.ts's TOLERANCE.
function pool(dirs: string[]) {
  const path = new URL("gate-aa.json", import.meta.url);
  const aa: { about: string; takes: number; sources: string[]; hours: Record<string, Film[]> } = existsSync(path)
    ? JSON.parse(readFileSync(path, "utf8"))
    : { about: "The motion gate's A/A films: repeat films of one flame, each film's placement and GATE values (paired.ts, D472)", takes: TAKES, sources: [], hours: {} };
  for (const dir of dirs)
    for (const [hour] of HOURS) {
      if (!existsSync(join(dir, `${hour}.json`))) continue;
      const s = readSide(dir, hour);
      (aa.hours[hour] ??= []).push(...s.films.map((f) => ({ dx: f.dx, v: f.v.map((x) => Math.round(1e4 * x) / 1e4) })));
      // A worker job's films are named by its job id, not this machine's copy of them.
      aa.sources.push(`${hour} ${s.rev.slice(0, 9)} ${dir.replace(/^.*\/remote\/([^/]+)\//, "job $1 ")}`);
    }
  writeFileSync(path, JSON.stringify(aa, null, 0).replace(/\{"dx"/g, '\n{"dx"') + "\n");
  for (const [hour, films] of Object.entries(aa.hours)) console.log(`${hour}: ${films.length} films, tolerance ${JSON.stringify(tolerance(films, aa.takes))}`);
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const opt = (k: string) => (argv.includes(k) ? argv[argv.indexOf(k) + 1] : undefined);
  const seconds = +(opt("--seconds") ?? 20);
  if (argv[0] === "--side") {
    // A side's films, run by the gate as a process of its own so base and build film side by side.
    const [, root, bin, out, secs, rev, films] = argv;
    filmSide(root, bin, out, +secs, rev, films === "--films");
    process.exit(0);
  }
  if (argv[0] === "--aa") pool(argv.slice(1));
  else if (argv[0] === "--judge") process.exit(judge(resolve(argv[1]), argv.includes("--alone")) ? 1 : 0);
  else {
    const OUT = resolve(argv.find((a, i) => !a.startsWith("--") && !["--seconds", "--base"].includes(argv[i - 1])) ?? "shots/motion");
    const alone = argv.includes("--alone");
    const root = process.cwd();
    if (!existsSync(join(root, "crates/terra_world/src/fire/light.rs"))) throw new Error("run from the checkout's root");
    const head = git(root, ["rev-parse", "HEAD"]).stdout.trim();
    let base: ReturnType<typeof baseOf> | undefined;
    try {
      base = alone ? undefined : baseOf(root, opt("--base"));
    } catch (e) {
      console.log(`✗ ${(e as Error).message}`);
      process.exit(1);
    }
    const held = (changed: string[]) => (changed.length ? changed.join(", ") : "the build's own");
    let other = { root, bin: join(root, "target/gate/build-sandbox") };
    if (base && !base.same) {
      const b = prepareBase(root, base.rev);
      console.log(`✓ built base ${base.rev.slice(0, 9)}, hearth light held at main's 6741693a (${held(b.changed)})`);
      other = { root: b.dir, bin: b.bin };
    }
    const changed = build(root);
    const bin = join(root, "target/release/terra_sandbox");
    mkdirSync(join(root, "target/gate"), { recursive: true });
    copyFileSync(bin, join(root, "target/gate/build-sandbox"));
    console.log(`✓ built ${head.slice(0, 9)}, hearth light held at main's 6741693a (${held(changed)})`);
    const side = (r: string, b: string, out: string, rev: string) =>
      new Promise<number>((done) => {
        rmSync(out, { recursive: true, force: true });
        mkdirSync(out, { recursive: true });
        const log = openSync(join(out, "side.log"), "w");
        const keep = argv.includes("--films") ? ["--films"] : [];
        const p = spawn(process.execPath, [fileURLToPath(import.meta.url), "--side", r, b, out, String(seconds), rev, ...keep], { cwd: r, stdio: ["ignore", log, log] });
        p.on("exit", (code) => done(code ?? 1));
      });
    // The base films beside the build, at once, so whatever the box does meanwhile falls on both.
    const sides = [side(root, join(root, "target/gate/build-sandbox"), join(OUT, "build"), head)];
    if (base) sides.push(side(other.root, other.bin, join(OUT, "base"), base.rev));
    const codes = await Promise.all(sides);
    const bad = codes.map((c, i) => (c ? join(OUT, ["build", "base"][i], "side.log") : "")).filter(Boolean);
    if (bad.length) {
      console.log(`✗ films failed: ${bad.join(", ")}`);
      process.exit(1);
    }
    if (base && !base.same) git(root, ["worktree", "remove", "--force", other.root]);
    const what = base ? `the build and base ${base.rev.slice(0, 9)}${base.same ? " (HEAD, one build filmed twice)" : ""} side by side` : "the build alone";
    console.log(`✓ filmed ${what}: ${PLACEMENTS.length} placements x ${TAKES} takes x ${HOURS.length} hours, ${seconds} s each, in ${OUT} (${argv.includes("--films") ? "films kept" : "measures only; --films keeps the films"})`);
    process.exit(judge(OUT, alone) ? 1 : 0);
  }
}
