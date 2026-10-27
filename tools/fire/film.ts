// The motion gate's films (D472): the yard's campfire filmed still from RDR2's camp view for 20 s at
// dusk and at night, at seven placements of its flame (PLACEMENTS: the fire and camera moved together
// along x, so the view is the same and only the flame's seed changes, hearth.rs), each frame
// with its flame matte (--flame-matte, D433); then motion.ts's measures, each hour gated by their
// median. Exit 1 when either hour misses the gate.
//
// The gate measures the flame, so the light it casts is held: the build is filmed with every hearth
// light constant of crates/terra_world/src/fire/light.rs (`HEARTH_*`) set to REFERENCE, main's at
// 6741693a, and the file put back after the build. A brighter pit light lit the ground behind the
// flame's thin low skin past the flame cut and moved the lean and splits with the flame unchanged
// (fire round g54: y1, light only, failed at both hours where main passed). The sky behind it is
// held too, as the fire sheet's (`HELD_SKY`, D472): the dome's 2D clouds, not the marched volumes,
// whose arrival raised the dusk splits with no fire changed.
//
//   node tools/fire/film.ts [out=shots/motion] [--seconds 20]   # from the checkout's root
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { analyse, gate, GATE, load, said } from "./motion.ts";
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
  HEARTH_TOP_NIGHT_STOPS: "1.0",
  HEARTH_DUSK_SKY: "360.0",
};
const CONST = /^(\s*(?:pub(?:\([a-z]+\))? )?const (HEARTH_\w+): [\w:]+ = )(.+);$/;

/// The source with every hearth constant at its reference, and which changed. Throws where the
/// source's hearth constants are not REFERENCE's (one added or renamed): the hold would leave it free.
export function hold(source: string, reference = REFERENCE): { source: string; changed: string[] } {
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
  if (missing.length) throw new Error(`light.rs has no ${missing.join(", ")}: film.ts's REFERENCE is stale`);
  return { source: lines.join("\n"), changed };
}

/// The flame's placements, m along x (fire and camera moved together): seven, each its own seed
/// (hearth.rs: x * 0.731 + z * 0.377, hashed into the loop's offset, mirror, turn and scale, sim.rs).
/// Three (0, 0.3, 0.6) were the
/// three lowest-splitting of these seven at dusk, and one film's splits move by about 0.1 run to
/// run at one placement, so their median sat at the bar by luck and went either way (fire gate
/// round: main dbf2f473's dusk 1.01-1.23 over four runs of three).
export const PLACEMENTS = [0, 0.3, 0.6, 0.9, 1.2, 1.5, 1.8];
export const HOURS: [string, string][] = [["dusk", "18.4"], ["night", "22"]];
const CROP = "crop=600:600:500:250";

export function build(root: string): string[] {
  const path = join(root, "crates/terra_world/src/fire/light.rs");
  const was = readFileSync(path, "utf8");
  const { source, changed } = hold(was);
  writeFileSync(path, source);
  try {
    const b = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: root, stdio: "inherit" });
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

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const at = argv.indexOf("--seconds");
  const seconds = at >= 0 ? +argv[at + 1] : 20;
  const OUT = argv.find((a, i) => !a.startsWith("--") && argv[i - 1] !== "--seconds") ?? "shots/motion";
  const root = process.cwd();
  if (!existsSync(join(root, "crates/terra_world/src/fire/light.rs"))) throw new Error("run from the checkout's root");
  const changed = build(root);
  console.log(`✓ built, hearth light held at main's 6741693a${changed.length ? ` (${changed.join(", ")})` : " (the build's own)"}`);
  const bin = join(root, "target/release/terra_sandbox");
  const dirs = PLACEMENTS.map((dx) => join(OUT, `p${String(dx).replace(".", "")}`));
  PLACEMENTS.forEach((dx, i) => film(root, bin, dirs[i], dx, seconds));
  console.log(`✓ filmed ${PLACEMENTS.length} placements x ${HOURS.length} hours, ${seconds} s each, in ${OUT}`);
  let failed = 0;
  for (const [hour] of HOURS) {
    const ms = dirs.map((d) => {
      const src = (input: string) => ({ input, fps: 30, from: 0, seconds });
      const { clip, cut } = load(src(join(d, `${hour}.mkv`)), src(join(d, `${hour}-matte.mkv`)));
      return { d, m: analyse(clip, cut), cut };
    });
    writeFileSync(join(OUT, `${hour}.txt`), ms.map(({ d, m, cut }) => said(d, m, `flame luma >= ${cut.toFixed(0)} in the matte`)).join("\n") + "\n");
    const g = gate(...ms.map((x) => x.m));
    const met = g.filter((x) => x.met).length;
    if (met < GATE.length) failed++;
    console.log(`${met === GATE.length ? "✓" : "✗"} ${hour}: median of ${ms.length} met ${met} of ${GATE.length}: ${g.map((x) => `${x.name} ${x.value.toFixed(2)}${x.met ? "" : "x"}`).join(" ")} (${join(OUT, `${hour}.txt`)})`);
  }
  process.exit(failed ? 1 : 0);
}
