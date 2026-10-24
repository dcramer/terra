#!/usr/bin/env node
// The trailer (docs/trailer.md, D357): one run through each place, the game's own player
// running a long, nearly straight route and the game's own third-person camera following,
// filmed with a cinematic look and cut together with crossfades and a sound bed of the
// game's own sounds. A shot file (trailer/runs/<name>.json) is the place, the hour and the
// date, the player's route and the look; the sandbox films it with `--cine`
// (crates/terra_world/src/cine.rs): the exposure held from when the run starts so it does
// not pump, the clock held at the hour, motion blur, depth of field if asked. As Unreal's
// Movie Render Queue renders a sequence, each run is stepped a sixtieth of a second a frame,
// warmed up where it starts, and every second frame kept, for a film at 30 a second however
// slowly it is drawn. The route is smoothed (Catmull-Rom, a spot every few metres) so the
// player, and the camera behind them, turn gently.
//
// A run may instead be a scene (`scene`, trailer/scenes/<name>.ts): a timeline of the game's
// remote calls on given frames and the camera's moves, written as a QA film's is
// (qa/script.ts), made on their frames in the same scripted run (`--calls`), with the same
// look; the player stands where it starts, hidden. A dragon burning a castle is one. A run
// with a route and a scene is the player's run with the scene's calls made over it, the game's
// camera following: figures staged along the way, a troll sat on the shore as the player runs by.
//
//   node tools/sandbox/trailer.ts [--only lake,forest] [--again] [--stills] [--no-render]
//     [--size 1920x1080] [--cut tools/sandbox/trailer/cut.json] [--out ~/.cache/terra-review/trailer/cut.mp4]
//
// Each run is left beside --out as <name>.mp4 with its own sound; a run whose clip is newer
// than its shot file, its scene and the sandbox is kept, --again renders it again, --only renders just
// those, --stills only a first, middle and last frame of each on one sheet (stills.jpg), to
// judge a route fast, --no-render cuts what is there. The cut has a contact sheet,
// <out>-sheet.jpg. Needs ffmpeg. Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { ROOT } from "./client.ts";
import { place } from "./places.ts";
import { bars, joined, lay, type Piece, smoothed } from "./timeline.ts";
import { Script } from "./qa/script.ts";
import { withProgress } from "./progress.ts";
import { pathToFileURL } from "node:url";

type Shot = {
  about: string;
  place: string;
  hour: number;
  date?: string;
  /// How long the film of it runs.
  seconds: number;
  /// Frames the run waits where it starts before it rolls, for streaming and the eye.
  warmup?: number;
  /// Seconds of the run before the film starts, so the player is already running.
  lead_in?: number;
  /// Where the runner starts, the spots they run through, and how many degrees the view
  /// is tilted up from the game's own, by the arrow keys while the run warms up.
  player: { at: string; face?: string; route?: string; tilt?: number; hidden?: boolean };
  /// A scene's timeline instead of the player's run (trailer/scenes/<name>.ts), or, with a
  /// route, made over it.
  scene?: string;
  look?: { letterbox?: number };
  /// Wildlife meetings the film asks for, put out and timed by the game (play's wildlife/asked.rs,
  /// D384): the kind, what it does, when (seconds of the run) and where (metres ahead of the camera,
  /// degrees off its axis to the right), and how many.
  meetings?: { kind: string; behaviour: "crossing" | "grazing" | "fleeing"; at: number; ahead: number; bearing?: number; group?: number }[];
  /// Its sound bed's layers, as `terra_sandbox --sound-bed` takes them, without their times.
  sound?: string[];
  music?: string;
};
/// A scene: what it needs of the sandbox beyond the place's, the frame its film rolls on
/// (what comes before, its setting up, is not filmed), and its timeline.
type Scene = { about: string; sandbox: string[]; rolls: number; script: (s: Script) => void };
type Cut = { title: string; line?: string; end?: string; letterbox: number; crossfade: number; card_seconds: number; shots: string[] };

const FPS = 30;
const args = process.argv.slice(2);
const arg = (name: string, fallback: string) => {
  const at = args.indexOf(name);
  return at >= 0 ? args[at + 1] : fallback;
};
const home = (path: string) => path.replace(/^~(?=\/)/, homedir());
const cutFile = resolve(ROOT, arg("--cut", "tools/sandbox/trailer/cut.json"));
const shotsDir = resolve(dirname(cutFile), "runs");
const cut: Cut = JSON.parse(readFileSync(cutFile, "utf8"));
const size = arg("--size", "1920x1080");
const [width, height] = size.split("x").map(Number);
const out = resolve(home(arg("--out", "~/.cache/terra-review/trailer/cut.mp4")));
const work = dirname(out);
const clips = join(work, "clips");
mkdirSync(clips, { recursive: true });
const only = args.includes("--only") ? arg("--only", "").split(",") : undefined;
const again = args.includes("--again");
const render = !args.includes("--no-render");
const stills = args.includes("--stills");

const names = only ? cut.shots.filter((name) => only.includes(name)) : cut.shots;
const shots = new Map<string, Shot>(cut.shots.map((name) => [name, JSON.parse(readFileSync(join(shotsDir, `${name}.json`), "utf8"))]));
const clipOf = (name: string) => join(clips, `${name}-${size}.mp4`);
const scenes = new Map<string, Scene>();
for (const shot of shots.values()) {
  if (shot.scene && !scenes.has(shot.scene)) {
    scenes.set(shot.scene, (await import(pathToFileURL(join(dirname(cutFile), "scenes", `${shot.scene}.ts`)).href)).default as Scene);
  }
}
const sandbox = resolve(ROOT, "target/release/terra_sandbox");
// Without it every shot fails with no output ("undefinedundefined"), after the GPU queue's wait.
if (!existsSync(sandbox)) {
  console.error(`✗ trailer: ${sandbox} is missing; run cargo build --release -p terra_sandbox first`);
  process.exit(1);
}

/// A font for the cards: a light serif, as trailers' titles are, found by fontconfig.
const font = (() => {
  const found = spawnSync("fc-match", ["-f", "%{file}", "Noto Serif Display:light"], { encoding: "utf8" });
  return found.status === 0 && existsSync(found.stdout) ? found.stdout : undefined;
})();

const ffmpeg = (list: string[]) => {
  const run = spawnSync("ffmpeg", ["-y", "-loglevel", "error", ...list], { stdio: "inherit" });
  if (run.status !== 0) throw new Error(`ffmpeg failed: ${list.join(" ").slice(0, 300)}`);
};
const duration = (file: string) => {
  const probe = spawnSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", file], { encoding: "utf8" });
  return Number(probe.stdout.trim());
};

/// The view turns up or down at the arrow keys' 1.6 radians a second (play's camera.rs):
/// held so many frames early in the warm-up, it tilts so many degrees.
const KEY_LOOK_SPEED = 1.6;
function tilted(degrees: number): string[] {
  const frames = Math.round((Math.abs(degrees) * Math.PI) / 180 / KEY_LOOK_SPEED * 60);
  if (!frames) return [];
  return ["--press", `${degrees > 0 ? "up" : "down"}@60-${60 + frames}`];
}

/// A trailer's runner runs empty-handed: the action bar's last slot, empty in the starting
/// kit, taken early in the warm-up. The kit's first tool, the hammer, is in hand otherwise,
/// and the held tools are untextured stand-ins, flat colour that read as a debug tint.
const EMPTY_HANDED = ["--press", "8@60"];

/// The sandbox's options for a run: its place's, the play experiment for the game's own
/// player (never walk's test body), the hour, the date, where the player starts and the
/// route, smoothed.
function sandboxArgs(shot: Shot): string[] {
  const own = [...place(shot.place).args];
  if (shot.scene && !shot.player.route) {
    const date = own.indexOf("--date");
    if (shot.date && date >= 0) own.splice(date, 2);
    return [
      ...own,
      ...(own.includes("--mute") ? [] : ["--mute"]),
      "--time",
      `${shot.hour}`,
      ...(shot.date ? ["--date", shot.date] : []),
      "--at",
      shot.player.at,
      ...(shot.player.face ? ["--face", shot.player.face] : []),
      ...(scenes.get(shot.scene)?.sandbox ?? []),
    ];
  }
  const walk = own.indexOf("walk");
  if (walk > 0 && own[walk - 1] === "--experiment") own[walk] = "play";
  const date = own.indexOf("--date");
  if (shot.date && date >= 0) own.splice(date, 2);
  const spots = [shot.player.at, ...(shot.player.route ?? "").split(";")].map((s) => s.split(",").map(Number) as [number, number]);
  const route = smoothed(spots, 4).slice(1);
  return [
    ...own,
    ...(own.includes("--mute") ? [] : ["--mute"]),
    "--time",
    `${shot.hour}`,
    ...(shot.date ? ["--date", shot.date] : []),
    "--at",
    shot.player.at,
    "--face",
    shot.player.face ?? route[0].map((v) => v.toFixed(1)).join(","),
    ...route.flatMap(([e, s]) => ["--route", `${e.toFixed(1)},${s.toFixed(1)}`]),
    ...tilted(shot.player.tilt ?? 0),
    ...EMPTY_HANDED,
    ...(shot.scene ? (scenes.get(shot.scene)?.sandbox ?? []) : []),
  ];
}

/// The player's jog, metres a second (play's movement.rs, JOG_SPEED).
const JOG = 4.3;

/// Why a run's route is too short for its film, or undefined: a player who reaches its last
/// spot stands there idle for the rest of it, which on film reads as a figure not animated.
function tooShort(shot: Shot): string | undefined {
  if (!shot.player.route) return undefined;
  const spots = [shot.player.at, ...shot.player.route.split(";")].map((s) => s.split(",").map(Number) as [number, number]);
  const length = spots.slice(1).reduce((sum, [e, s], i) => sum + Math.hypot(e - spots[i][0], s - spots[i][1]), 0);
  const needs = ((shot.lead_in ?? 1.5) + shot.seconds) * JOG;
  if (length >= needs) return undefined;
  return `its route is ${length.toFixed(0)} m, run in ${(length / JOG).toFixed(1)} s, but the film needs ${needs.toFixed(0)} m: the player would stand for its last ${((needs - length) / JOG).toFixed(1)} s`;
}

/// A sound bed of `seconds` from the game's own sounds and music, or undefined if it could
/// not be made.
function bedFor(layers: string[], seconds: number, file: string): string | undefined {
  const made = spawnSync(sandbox, ["--sound-bed", file, "--seconds", seconds.toFixed(2), ...layers], { encoding: "utf8" });
  if (made.status === 0 && existsSync(file)) return file;
  console.log(`trailer: no sound bed (${`${made.stderr}`.trim()}); silent`);
  return undefined;
}
const layersOf = (shot: Shot, from: number, to: number) => [
  ...(shot.sound ?? []).map((sound) => `${sound}@${from.toFixed(2)}-${to.toFixed(2)}`),
  // Phrase after phrase to the end of its run, so its sound stays full to the last frame.
  ...(shot.music ? [`${shot.music}@${(from + 1).toFixed(2)}-${to.toFixed(2)}`] : []),
];

/// A place needs about this much of the GPU's memory at 1080p (the lush forest's run took
/// 4 to 5 GB, the mountain lake's with the lush forest's pines 6); with less free, a shot
/// sharing the GPU runs out and dies.
const NEEDS_MB = 7000;

/// Waits, up to half an hour, until an NVIDIA GPU has room for a shot; elsewhere, where
/// nvidia-smi does not answer, at once.
function roomOnTheGpu() {
  for (let waited = 0; waited < 1800; waited += 30) {
    const smi = spawnSync("nvidia-smi", ["--query-gpu=memory.free", "--format=csv,noheader,nounits"], { encoding: "utf8" });
    const free = Number(`${smi.stdout}`.trim().split("\n")[0]);
    if (smi.status !== 0 || !Number.isFinite(free) || free >= NEEDS_MB) return;
    if (waited === 0) console.log(`trailer: waiting for room on the GPU: ${free} MB free of the ${NEEDS_MB} a shot needs`);
    spawnSync("sleep", ["30"]);
  }
}

/// What the film saw of the wildlife, from the run's log (play's sighted.rs, twice a second of
/// the shot): how much of the film has an animal in it, how many at once, and how big the
/// biggest is on the picture. An encounter staged well is seen for seconds at tens of pixels.
function wildlifeSeen(log: string, from: number, seconds: number): string | undefined {
  const samples = [...log.matchAll(/Wildlife seen at ([\d.]+) s: (\d+) \((\d+) hidden\)(?:: (.*))?/g)]
    .map((m) => ({ t: Number(m[1]), count: Number(m[2]), hidden: Number(m[3]), px: [...(m[4] ?? "").matchAll(/(\d+) px/g)].map((p) => Number(p[1])) }))
    .filter((s) => s.t >= from && s.t <= from + seconds);
  if (!samples.length) return undefined;
  const share = (keep: (s: (typeof samples)[number]) => boolean) => samples.filter(keep).length / samples.length;
  const biggest = samples.map((s) => s.px[0] ?? 0).filter((px) => px > 0).sort((a, b) => a - b);
  const median = biggest.length ? biggest[Math.floor(biggest.length / 2)] : 0;
  const mean = samples.reduce((sum, s) => sum + s.count, 0) / samples.length;
  const pct = (x: number) => `${Math.round(x * 100)}%`;
  return (
    `wildlife seen in ${pct(share((s) => s.count > 0))} of the film, ${pct(share((s) => (s.px[0] ?? 0) >= 20))} at 20 px or more, ` +
    `${pct(share((s) => (s.px[0] ?? 0) >= 40))} at 40 px or more; ${mean.toFixed(1)} in view at once on average; ` +
    `the biggest ${median} px at the median, ${biggest[biggest.length - 1] ?? 0} px at most; ${(samples.reduce((sum, s) => sum + s.hidden, 0) / samples.length).toFixed(1)} drawn but hidden`
  );
}

/// An asked meeting is seen when it is so big on the picture for so long (seconds) of the film.
const MEETING_PX = 40;
const MEETING_S = 2;

/// Whether each meeting the run asked for was seen, a line each, from the run's log (play's
/// sighted.rs tells each asked one's size twice a second; wildlife's asked.rs when it was staged).
function meetingsSeen(log: string, shot: Shot, from: number): string[] {
  return (shot.meetings ?? []).map((meeting, index) => {
    const name = `meeting ${meeting.kind}`;
    const gaveUp = log.match(new RegExp(`Wildlife: meeting ${index} \\([^)]*\\) not staged: ([^\\n]*)`));
    if (gaveUp) return `✗ ${name}: not staged, ${gaveUp[1].replace(/\.$/, "")}`;
    if (!new RegExp(`Wildlife: meeting ${index} \\(`).test(log)) return `✗ ${name}: never staged (its time, ${meeting.at} s, not reached?)`;
    const px = [...log.matchAll(new RegExp(`Meeting ${index} seen at ([\\d.]+) s: (\\d+) px`, "g"))]
      .map((m) => ({ t: Number(m[1]), px: Number(m[2]) }))
      .filter((s) => s.t >= from && s.t <= from + shot.seconds)
      .map((s) => s.px);
    if (!px.length) return `✗ ${name}: staged, never seen unhidden in the film`;
    const big = px.filter((p) => p >= MEETING_PX);
    const seconds = big.length * 0.5;
    const range = (all: number[]) => `${Math.min(...all)}-${Math.max(...all)} px`;
    if (seconds >= MEETING_S) return `${name}: seen ${seconds.toFixed(1)} s at ${range(big)}`;
    return `✗ ${name}: seen ${(px.length * 0.5).toFixed(1)} s at ${range(px)}, only ${seconds.toFixed(1)} s at ${MEETING_PX} px or more (${MEETING_S} s asked)`;
  });
}

async function renderShot(name: string, n: number) {
  const shot = shots.get(name) as Shot;
  const file = join(shotsDir, `${name}.json`);
  const clip = clipOf(name);
  // A run is stale when its shot file, its scene or the sandbox changed after it was filmed.
  const sceneFile = shot.scene ? join(dirname(cutFile), "scenes", `${shot.scene}.ts`) : file;
  const stale = !existsSync(clip) || statSync(clip).mtimeMs < Math.max(statSync(file).mtimeMs, statSync(sceneFile).mtimeMs, statSync(sandbox).mtimeMs);
  if (!render || !(stills || again || only || stale)) return;
  const short = tooShort(shot);
  if (short) console.log(`trailer: ${name}: ${short}; lengthen it in ${file}`);
  const scene = shot.scene ? scenes.get(shot.scene) : undefined;
  const warmup = scene?.rolls ?? shot.warmup ?? 360;
  const first = scene && !shot.player.route ? warmup : warmup + Math.round((shot.lead_in ?? 1.5) * 60);
  const frames = Math.round(shot.seconds * FPS);
  const last = first + (frames - 1) * 2;
  const dir = join(work, stills ? "stills" : "frames", name);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const started = Date.now();
  // A scene's timeline, its calls on their frames, the camera's among them.
  const calls: string[] = [];
  if (scene) {
    const timeline = new Script();
    scene.script(timeline);
    const timed = join(work, `${name}.calls.jsonl`);
    writeFileSync(timed, timeline.all(last).map(({ frame, method, params }) => JSON.stringify({ frame, method, params })).join("\n") + "\n");
    calls.push("--calls", timed);
  }
  const options = [
    "--offscreen",
    "--clean",
    "--picture",
    size,
    ...sandboxArgs(shot),
    ...calls,
    "--cine",
    `${file}@${warmup}`,
    "--shots",
    stills ? `${dir}/f@${first},${first + 2 * Math.floor(frames / 2)},${last}` : `${dir}/f@${first}-${last}/2`,
  ];
  console.log(`trailer: ${n}/${names.length} ${name}: ${shot.about}`);
  // Shots share the GPU with other runs (D319), so one may find no room for a heavy place;
  // it waits a minute and tries again rather than failing the trailer.
  const env = { ...process.env, BEVY_ASSET_ROOT: ROOT };
  const attempt = () => (roomOnTheGpu(), withProgress(sandbox, options, { dir, name, frames: stills ? 3 : frames, cwd: ROOT, env }));
  let run = await attempt();
  for (let tries = 1; tries < 6 && /Out ?Of ?Memory/i.test(`${run.stdout}${run.stderr}`); tries++) {
    console.log(`trailer: ${name}: the GPU had no room (out of memory); trying again in a minute`);
    spawnSync("sleep", ["60"]);
    run = await attempt();
  }
  writeFileSync(join(work, `${name}.log`), `${sandbox} ${options.join(" ")}\n\n${run.stdout}${run.stderr}`);
  if (run.status !== 0 || !`${run.stdout}`.includes("SANDBOX saved")) {
    console.error(`${run.stdout}${run.stderr}`.split("\n").slice(-25).join("\n"));
    throw new Error(`${name} failed; its log is ${join(work, `${name}.log`)}`);
  }
  const late = `${run.stdout}${run.stderr}`.split("\n").filter((line) => line.includes("Still loading"));
  if (late.length) console.log(`trailer: ${name}: ${late[late.length - 1].replace(/.*Still loading/, "still loading")}`);
  const gaveUp = `${run.stdout}`.split("\n").filter((line) => line.includes("Route: gave up"));
  if (gaveUp.length) console.log(`trailer: ${name}: the route was blocked: ${gaveUp.join("; ")}`);
  const seen = wildlifeSeen(`${run.stdout}${run.stderr}`, shot.scene && !shot.player.route ? 0 : (shot.lead_in ?? 1.5), shot.seconds);
  if (seen) console.log(`trailer: ${name}: ${seen}`);
  const filmFrom = shot.scene && !shot.player.route ? 0 : (shot.lead_in ?? 1.5);
  for (const line of meetingsSeen(`${run.stdout}${run.stderr}`, shot, filmFrom)) console.log(`trailer: ${name}: ${line}`);
  if (stills) {
    console.log(`trailer: ${name}: stills in ${((Date.now() - started) / 1000).toFixed(0)} s`);
    return;
  }
  const bar = bars(width, height, shot.look?.letterbox ?? cut.letterbox);
  const box = bar > 0 ? `drawbox=x=0:y=0:w=iw:h=${bar}:color=black:t=fill,drawbox=x=0:y=ih-${bar}:w=iw:h=${bar}:color=black:t=fill,` : "";
  ffmpeg(["-framerate", `${FPS}`, "-pattern_type", "glob", "-i", `${dir}/f-*.png`, "-vf", `${box}format=yuv420p`, "-c:v", "libx264", "-preset", "slow", "-crf", "15", "-movflags", "+faststart", clip]);
  rmSync(dir, { recursive: true, force: true });
  // The run on its own, to send round: faded in and out, with its own sound.
  const alone = join(work, `${name}.mp4`);
  const bed = bedFor(layersOf(shot, 0, shot.seconds), shot.seconds, join(work, `${name}.wav`));
  const fades = `fade=t=in:d=0.8,fade=t=out:st=${(shot.seconds - 1).toFixed(2)}:d=1`;
  ffmpeg([
    "-i",
    clip,
    ...(bed ? ["-i", bed, "-af", `afade=t=out:st=${(shot.seconds - 2).toFixed(2)}:d=2,loudnorm=I=-16:TP=-1.5`, "-c:a", "aac", "-b:a", "192k"] : []),
    "-vf",
    fades,
    "-c:v",
    "libx264",
    "-preset",
    "slow",
    "-crf",
    "17",
    "-pix_fmt",
    "yuv420p",
    "-movflags",
    "+faststart",
    "-shortest",
    alone,
  ]);
  console.log(`trailer: ${alone}, ${frames} frames in ${((Date.now() - started) / 1000).toFixed(0)} s`);
}

/// A title card: black, its words fading in and out.
function card(name: string, title: string, line: string | undefined, seconds: number): string {
  const file = join(clips, `card-${name}-${size}.mp4`);
  const text = (words: string, big: boolean, y: string) => {
    const textFile = join(clips, `card-${name}-${big ? "title" : "line"}.txt`);
    writeFileSync(textFile, words);
    const fade = `if(lt(t,0.8),t/0.8,if(lt(t,${seconds - 0.8}),1,(${seconds}-t)/0.8))`;
    return `drawtext=${font ? `fontfile='${font}':` : ""}textfile='${textFile}':fontsize=${Math.round(height * (big ? 0.075 : 0.028))}:fontcolor=0xEDE6D8:x=(w-text_w)/2:y=${y}:alpha='${fade}'`;
  };
  const filters = [text(title, true, line ? "h*0.44-text_h/2" : "(h-text_h)/2"), ...(line ? [text(line, false, "h*0.56")] : [])];
  ffmpeg(["-f", "lavfi", "-i", `color=c=black:s=${size}:r=${FPS}:d=${seconds}`, "-vf", `${filters.join(",")},format=yuv420p`, "-c:v", "libx264", "-crf", "15", file]);
  return file;
}

/// A sheet of rows of pictures, three a row, the first of each row named.
function sheet(rows: { name: string; pictures: { file: string; at?: number }[] }[], file: string) {
  const thumbs = join(work, "thumbs");
  rmSync(thumbs, { recursive: true, force: true });
  mkdirSync(thumbs, { recursive: true });
  let tile = 0;
  for (const row of rows) {
    for (const [k, picture] of row.pictures.entries()) {
      const label =
        k === 0
          ? `,drawtext=${font ? `fontfile='${font}':` : ""}text='${row.name.replace(/[':\\%]/g, "")}':fontsize=22:fontcolor=white:box=1:boxcolor=black@0.6:x=10:y=h-th-12`
          : "";
      const seek = picture.at === undefined ? [] : ["-ss", picture.at.toFixed(2)];
      ffmpeg([...seek, "-i", picture.file, "-frames:v", "1", "-vf", `scale=640:-2${label}`, join(thumbs, `${String(tile++).padStart(3, "0")}.png`)]);
    }
  }
  if (tile) ffmpeg(["-framerate", "1", "-i", join(thumbs, "%03d.png"), "-vf", `tile=3x${rows.length}:padding=4:color=black`, "-frames:v", "1", "-q:v", "3", file]);
  rmSync(thumbs, { recursive: true, force: true });
}

for (const [i, name] of names.entries()) await renderShot(name, i + 1);

if (stills) {
  const rows = names
    .map((name) => {
      const dir = join(work, "stills", name);
      const files = existsSync(dir) ? readdirSync(dir).sort() : [];
      return { name: `${name}  ${shots.get(name)?.hour}h`, pictures: files.map((f) => ({ file: join(dir, f) })) };
    })
    .filter((row) => row.pictures.length);
  sheet(rows, join(work, "stills.jpg"));
  console.log(`trailer: stills of ${rows.length} runs in ${join(work, "stills.jpg")}`);
  process.exit(0);
}

// The cut: a title, each run crossfaded into the next, an end card.
const filmed = cut.shots.filter((name) => existsSync(clipOf(name)));
if (filmed.length < cut.shots.length) console.log(`trailer: cut without ${cut.shots.filter((n) => !filmed.includes(n)).join(", ")}, not filmed yet`);
const files: string[] = [];
const pieces: Piece[] = [];
const add = (file: string, act: number, overlap: number) => {
  files.push(file);
  pieces.push({ name: file, seconds: duration(file), overlap, act });
};
// A cut with no card time (`card_seconds` 0) or no title has no cards: a card of no length is
// an empty file, whose length reads as NaN and breaks the crossfades.
const cards = cut.card_seconds > 0 && cut.title !== "";
if (cards) add(card("open", cut.title, cut.line, cut.card_seconds), -1, 0);
for (const [i, name] of filmed.entries()) add(clipOf(name), i, i === 0 ? 0.8 : cut.crossfade);
if (cards) add(card("end", cut.title, cut.end, cut.card_seconds + 1), filmed.length, 1.0);
const { starts, seconds } = lay(pieces);
const layers = filmed.flatMap((name, i) => {
  const at = i + (cards ? 1 : 0);
  return layersOf(shots.get(name) as Shot, Math.max(0, starts[at] - 0.5), Math.min(seconds, starts[at] + pieces[at].seconds + 0.5));
});
const bed = bedFor(layers, seconds, join(work, "cut.wav"));
const filter =
  joined(pieces, FPS, size) + (bed ? `;[${files.length}:a]afade=t=in:d=1,afade=t=out:st=${(seconds - 3).toFixed(2)}:d=3,loudnorm=I=-16:TP=-1.5[bed]` : "");
ffmpeg([
  ...files.flatMap((file) => ["-i", file]),
  ...(bed ? ["-i", bed] : []),
  "-filter_complex",
  filter,
  "-map",
  "[cut]",
  ...(bed ? ["-map", "[bed]", "-c:a", "aac", "-b:a", "192k"] : []),
  "-c:v",
  "libx264",
  "-preset",
  "slow",
  "-crf",
  "17",
  "-pix_fmt",
  "yuv420p",
  "-movflags",
  "+faststart",
  "-t",
  seconds.toFixed(2),
  out,
]);
const contact = out.replace(/\.mp4$/, "-sheet.jpg");
sheet(
  filmed.map((name) => {
    const length = duration(clipOf(name));
    return { name: `${name}  ${shots.get(name)?.hour}h`, pictures: [0.1, 0.5, 0.9].map((at) => ({ file: clipOf(name), at: length * at })) };
  }),
  contact,
);
console.log(`trailer: ${out}, ${seconds.toFixed(1)} s, ${filmed.length} runs, ${(statSync(out).size / 1e6).toFixed(1)} MB${bed ? ", with its sound" : ""}`);
console.log(`trailer: contact sheet ${contact}`);
