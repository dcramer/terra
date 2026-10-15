#!/usr/bin/env node
// QA films (D69, D71): a scenario of building, harm or materials played by script in a set
// of kits, filmed the same every time, with a contact sheet and the same run timed in real
// time, frame by frame, so hitches are measured and not only seen.
//
//   node tools/sandbox/qa.ts <scenario> [--set <kit>|mixes|all] [--part <stage>] [--no-trace]
//   node tools/sandbox/qa.ts --list      the scenarios, their stages, and the sets
//   node tools/sandbox/qa.ts --index     write shots/qa/index.html again
//
// A scenario is a file in tools/sandbox/qa/ (not script.ts): its sandbox options and a
// timeline of remote calls on given frames and camera moves, written against a set (see
// qa/script.ts). A set is a wall kit in assets/data/kits/ with its roof, read from the folder,
// or `mixes`, every wall kit meeting the others; `all` is every set. For each set:
//
// 1. A first pass, live over the remote protocol: the timeline's calls made in order, to
//    learn how many steps each fill takes in this set and when the game said what the
//    scenario waits for, and to report every refusal.
// 2. The film: the timeline, now timed, made on its frames in a scripted run (`--calls`), the
//    game moving a sixtieth of a second a frame and every section laid before a frame is
//    drawn, every second frame saved and joined by ffmpeg at 30 a second with each stage's
//    name on it: shots/qa/<scenario>/<set>.mp4, and a contact sheet of each stage,
//    <set>.jpg. `--part` films one stage, the run the same up to it.
// 3. The trace: the same calls on the same frames in a benchmark run, which lays sections on
//    the workers as play does, every frame's time written (`--frame-times`): <set>-frames.csv,
//    and in <set>.txt each stage's frames against the budget and the worst frames with what
//    was done on them.
//
// shots/qa/index.html lists the latest of each. Runs on the Windows GPU from WSL. Needs
// ffmpeg and ImageMagick. Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { basename, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import * as game from "./client.ts";
import { buildOffscreen, imageMagick, offscreenPath, offscreenRun, ROOT } from "./client.ts";
import { type Call, type Known, type KitSet, readSets, type Scenario, Script, type Stage } from "./qa/script.ts";

const SCENARIOS = resolve(import.meta.dirname, "qa");
const OUT = resolve(ROOT, "shots/qa");
/// The frame budget (D25), and a hitch: twice it.
const BUDGET_MS = 1000 / 60;
const HITCH_MS = 2 * BUDGET_MS;

const args = process.argv.slice(2);
const option = (name: string) => {
  const at = args.indexOf(name);
  return at >= 0 ? args[at + 1] : undefined;
};
const scenarios = readdirSync(SCENARIOS)
  .filter((file) => file.endsWith(".ts") && file !== "script.ts")
  .map((file) => basename(file, ".ts"))
  .sort();
const load = async (name: string) => (await import(pathToFileURL(resolve(SCENARIOS, `${name}.ts`)).href)).default as Scenario;
const sets = readSets();

if (args.includes("--list")) {
  for (const name of scenarios) {
    const scenario = await load(name);
    const s = new Script();
    scenario.script(s, sets[0]);
    console.log(`${name}: ${scenario.about}\n  stages: ${s.stages.map((stage) => stage.name).join(", ")}`);
  }
  console.log(`sets: ${sets.map((set) => set.name).join(", ")}, all`);
  process.exit(0);
}
if (args.includes("--index")) {
  index();
  process.exit(0);
}

const name = args.find((arg, i) => !arg.startsWith("--") && !args[i - 1]?.startsWith("--"));
if (!name || !scenarios.includes(name)) {
  console.error(`usage: cargo xtask qa <scenario> [--set <kit>|mixes|all] [--part <stage>] [--no-trace]\nscenarios: ${scenarios.join(", ")}`);
  process.exit(1);
}
const wanted = option("--set") ?? (sets.find((set) => set.name === "logs") ?? sets[0]).name;
const chosen = wanted === "all" ? sets : sets.filter((set) => set.name === wanted);
if (!chosen.length) {
  console.error(`no set ${wanted}; the sets are ${sets.map((set) => set.name).join(", ")} and all`);
  process.exit(1);
}
const part = option("--part");
const scenario = await load(name);
const folder = resolve(OUT, name);
mkdirSync(folder, { recursive: true });

if (!buildOffscreen()) process.exit(1);
let failed = false;
for (const set of chosen) {
  try {
    await film(set);
  } catch (error) {
    console.log(`FAIL  ${name} in ${set.name}: ${(error as Error).message}`);
    failed = true;
  }
}
index();
process.exit(failed ? 1 : 0);

async function film(set: KitSet) {
  const started = Date.now();
  const seconds = () => `${((Date.now() - started) / 1000).toFixed(0)} s`;
  console.log(`qa: ${name} in ${set.name}: the first pass…`);
  const { known, refusals } = await firstPass(set);
  for (const refusal of refusals) console.log(`  refused: ${refusal}`);
  writeFileSync(resolve(folder, `${set.name}.known.json`), JSON.stringify(known, null, 1));
  if (args.includes("--first")) {
    for (const [fill, steps] of Object.entries(known.fills)) console.log(`  ${fill}: ${steps.length} steps`);
    return;
  }

  const s = new Script(known);
  scenario.script(s, set);
  const last = s.frame;
  const calls = s.all(last);
  if (s.clipping.length) console.log(`  the camera is within 40 cm of a solid at frames ${spans(s.clipping)}`);
  const stage = part ? s.stages.find((each) => each.name === part) : undefined;
  if (part && !stage) throw new Error(`no stage ${part}; ${name} has ${s.stages.map((each) => each.name).join(", ")}`);
  const [from, to] = stage ? [stage.from, stage.to] : [1, last];
  const callsFile = resolve(folder, `${set.name}.calls.jsonl`);
  writeFileSync(callsFile, calls.map(({ frame, method, params }) => JSON.stringify({ frame, method, params })).join("\n") + "\n");
  const label = part ? `${set.name}-${part}` : set.name;
  console.log(`qa: ${name} in ${set.name}: ${((last - 1) / 60).toFixed(0)} s, ${s.stages.length} stages; filming ${((to - from) / 60).toFixed(0)} s of it (${seconds()})`);

  // The film: every second frame, in a folder of its own on this side.
  const frames = resolve(folder, `frames-${label}`);
  rmSync(frames, { recursive: true, force: true });
  mkdirSync(frames, { recursive: true });
  const sandbox = [...scenario.sandbox, "--clean", "--calls", offscreenPath(callsFile)];
  const [command, words] = offscreenRun(`qa-film`, [...sandbox, "--shots", `${offscreenPath(frames)}/f@${from}-${to}/2`]);
  const run = spawnSync(command, words, { cwd: ROOT, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: ROOT }, maxBuffer: 256 * 1024 * 1024 });
  const said = `${run.stdout}${run.stderr}`;
  writeFileSync(resolve(folder, `${label}-film.log`), said);
  if (!said.includes("SANDBOX saved")) throw new Error(`the film did not finish:\n${said.split("\n").slice(-25).join("\n")}`);
  const warned = said.split("\n").filter((line) => /Calls: .*refused|Calls: .*no method|ERROR|panicked/.test(line));
  for (const line of warned) console.log(`  ${line.replace(/^.*?(Calls:|ERROR)/, "$1")}`);
  console.log(`qa: filmed (${seconds()})`);

  const mp4 = resolve(folder, `${label}.mp4`);
  encode(frames, mp4, s.stages, from, set);
  const sheet = resolve(folder, `${label}.jpg`);
  contactSheet(frames, sheet, s.stages, from, to, set);
  rmSync(frames, { recursive: true, force: true });
  console.log(`qa: ${relative(ROOT, mp4)}, ${(statSync(mp4).size / 1e6).toFixed(1)} MB; ${relative(ROOT, sheet)}, ${(statSync(sheet).size / 1e6).toFixed(2)} MB (${seconds()})`);

  let trace: Trace | undefined;
  if (!args.includes("--no-trace")) {
    trace = timeIt(set, sandbox, calls, s.stages, last, label);
    console.log(`qa: traced (${seconds()})`);
  }
  writeFileSync(
    resolve(folder, `${label}.json`),
    JSON.stringify({ scenario: name, set: set.name, part, made: new Date().toISOString(), seconds: (to - from) / 60, refusals: [...refusals, ...warned], trace: trace?.summary }, null, 1),
  );
}

/// Makes the timeline's calls in order in a live run, stepping the clock between them as
/// far as the timeline says, to learn what each fill builds and when expected words are said.
async function firstPass(set: KitSet): Promise<{ known: Known; refusals: string[] }> {
  const s = new Script();
  scenario.script(s, set);
  const known: Known = { fills: {}, said: {} };
  const refusals: string[] = [];
  // With --first, a picture at the end of each stage, the camera where the film's will be.
  const looks = new Map(s.all().filter((call) => call.method === "build.look").map((call) => [call.frame, call.params]));
  const ends = s.stages.map((stage) => ({ stage, at: stage.to }));
  const picture = async (upTo: number) => {
    while (ends.length && ends[0].at < upTo) {
      const { stage, at } = ends.shift()!;
      if (!args.includes("--first")) continue;
      const view = looks.get(at);
      if (view) await game.call("build.look", view);
      await game.step(20);
      console.log(`  ${stage.name}: ${await game.shot(resolve(folder, `first-${set.name}`, `${stage.name}.png`))}`);
    }
  };
  await game.start([...scenario.sandbox, "--offscreen", "--fresh", "--save", `qa-${name}.ron`], { windows: game.onWsl, release: true, copy: "qa-first" });
  try {
    let frame = 1;
    const calls = [...s.calls].sort((a, b) => a.frame - b.frame);
    for (const call of calls) {
      await picture(call.frame);
      if (call.frame > frame) await game.step(call.frame - frame);
      frame = call.frame;
      const since = (await game.status()).next_line;
      let answer: unknown;
      try {
        answer = await game.call(call.method, call.params);
      } catch (error) {
        refusals.push(`frame ${call.frame} ${call.method}: ${(error as Error).message}`);
        continue;
      }
      const each = (Array.isArray(answer) ? answer : [answer]) as { done?: boolean; what?: string; refused?: string }[];
      // What each call answered, to write a scenario by.
      if (args.includes("--answers")) console.log(`  frame ${call.frame} ${call.method} ${JSON.stringify(call.params)}: ${JSON.stringify(answer).slice(0, 300)}`);
      for (const refused of each.filter((one) => one?.done === false)) refusals.push(`frame ${call.frame} ${call.method} ${JSON.stringify(call.params)}: ${refused.what ?? ""}: ${refused.refused}`);
      if (call.tag) known.fills[call.tag] = each.filter((one) => one.done).map((one) => one.what ?? "");
      if (call.expect) {
        // A frame at a time, until it is said.
        let after = 0;
        for (; after < 1200; after++) {
          const { lines } = await game.said(since, call.expect.text, 1);
          if (lines.length) break;
          await game.step(1);
        }
        if (after === 1200) refusals.push(`frame ${call.frame}: never heard "${call.expect.text}"`);
        known.said[call.expect.name] = after;
        frame += after;
      }
    }
    await picture(Number.MAX_SAFE_INTEGER);
  } finally {
    await game.stop().catch(() => {});
  }
  return { known, refusals };
}

/// Joins the frames into a video at 30 a second, the game's own speed, each stage's name in
/// a corner while it lasts. Under 100 MB for a few minutes.
function encode(frames: string, out: string, stages: Stage[], from: number, set: KitSet) {
  const font = spawnSync("fc-match", ["-f", "%{file}", "DejaVu Sans:bold"], { encoding: "utf8" }).stdout.trim();
  const captions = stages
    .filter((stage) => stage.to >= from)
    .map((stage, i) => {
      const file = resolve(frames, `caption-${i}.txt`);
      writeFileSync(file, `${set.name}  ·  ${stage.title}`);
      const [a, b] = [Math.max(0, (stage.from - from) / 60), (stage.to - from + 1) / 60];
      return `drawtext=fontfile=${font}:textfile=${file}:x=48:y=h-80:fontsize=34:fontcolor=white:borderw=3:bordercolor=black@0.7:enable='between(t,${a.toFixed(3)},${b.toFixed(3)})'`;
    });
  const encoded = spawnSync(
    "ffmpeg",
    [
      ...["-y", "-loglevel", "error", "-framerate", "30", "-pattern_type", "glob", "-i", `${frames}/f-*.png`],
      ...(captions.length ? ["-vf", captions.join(",")] : []),
      ...["-c:v", "libx264", "-preset", "slow", "-crf", "23", "-maxrate", "4.5M", "-bufsize", "9M", "-pix_fmt", "yuv420p", "-movflags", "+faststart", out],
    ],
    { stdio: "inherit" },
  );
  if (encoded.status !== 0) throw new Error("ffmpeg failed");
}

/// Three frames of each stage, at its start, middle and end, labelled, in one picture under
/// 2 MB.
function contactSheet(frames: string, out: string, stages: Stage[], from: number, to: number, set: KitSet) {
  const saved = readdirSync(frames)
    .filter((file) => file.endsWith(".png"))
    .map((file) => ({ file: resolve(frames, file), frame: Number(/-(\d+)\.png$/.exec(file)![1]) }))
    .sort((a, b) => a.frame - b.frame);
  const nearest = (frame: number) => saved.reduce((best, each) => (Math.abs(each.frame - frame) < Math.abs(best.frame - frame) ? each : best));
  const picks: string[] = [];
  for (const stage of stages.filter((each) => each.to >= from && each.from <= to)) {
    const [a, b] = [Math.max(stage.from, from), Math.min(stage.to, to)];
    for (const share of [0.15, 0.55, 0.97]) {
      const pick = nearest(a + (b - a) * share);
      picks.push("-label", `${stage.name} ${pick.frame}`, pick.file);
    }
  }
  for (const quality of [80, 70, 60, 50]) {
    const [command, words] = imageMagick([
      "montage",
      ...["-font", "DejaVu-Sans", "-pointsize", "18", "-fill", "white", "-background", "#16181c"],
      ...picks,
      ...["-tile", "6x", "-geometry", "480x270+4+4", "-title", `${name} in ${set.name}`, "-quality", String(quality), out],
    ]);
    const made = spawnSync(command, words, { stdio: "inherit" });
    if (made.status !== 0) throw new Error("montage failed");
    if (statSync(out).size < 2e6) return;
  }
}

type Trace = { summary: { median: number; p99: number; worst: number; over: number; hitches: number; frames: number; torn: number; edits: number; edit_median: number; edit_worst: number } };

/// The same calls on the same frames in a benchmark run, every frame's time written, read
/// against the budget stage by stage, its worst frames beside what was done on them.
function timeIt(set: KitSet, sandbox: string[], calls: Call[], stages: Stage[], last: number, label: string): Trace {
  const csv = resolve(folder, `${label}-frames.csv`);
  rmSync(csv, { force: true });
  const settle = 30;
  const [command, words] = offscreenRun(`qa-trace`, [...sandbox, "--bench", "--settle", String(settle), "--bench-frames", String(Math.max(100, last - settle)), "--frame-times", offscreenPath(csv)]);
  const run = spawnSync(command, words, { cwd: ROOT, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: ROOT }, maxBuffer: 256 * 1024 * 1024 });
  writeFileSync(resolve(folder, `${label}-trace.log`), `${run.stdout}${run.stderr}`);
  if (!existsSync(csv)) throw new Error(`the trace did not finish:\n${`${run.stdout}${run.stderr}`.split("\n").slice(-25).join("\n")}`);
  const rows = readFileSync(csv, "utf8")
    .trim()
    .split("\n")
    .slice(1)
    .map((line) => line.split(","))
    .map(([frame, ms, game, render, gpu]) => ({ frame: Number(frame), ms: Number(ms), game: Number(game), render: Number(render), gpu: Number(gpu) }));
  const stat = (values: number[]) => {
    const sorted = [...values].sort((a, b) => a - b);
    return { median: sorted[Math.floor(sorted.length / 2)] ?? 0, p99: sorted[Math.floor(sorted.length * 0.99)] ?? 0, worst: sorted.at(-1) ?? 0 };
  };
  const lines: string[] = [`${name} in ${set.name}, in real time on ${/SANDBOX frames on ([^:]+?) at/.exec(run.stdout)?.[1] ?? "the GPU"}: frames against the ${BUDGET_MS.toFixed(1)} ms budget`, ""];
  const row = (title: string, ms: number[]) => {
    const { median, p99, worst } = stat(ms);
    lines.push(`${title.padEnd(24)} median ${median.toFixed(1).padStart(5)}  p99 ${p99.toFixed(1).padStart(5)}  worst ${worst.toFixed(1).padStart(6)} ms  over ${ms.filter((m) => m > BUDGET_MS).length}  hitches ${ms.filter((m) => m > HITCH_MS).length}`);
  };
  row("all", rows.map((r) => r.ms));
  for (const stage of stages) row(stage.name, rows.filter((r) => r.frame >= stage.from && r.frame <= stage.to).map((r) => r.ms));
  // Each change from the call that made it to all its sections on screen, as the run said
  // it: the stage of the call said last before it.
  // Torn: sections of one change put on screen before the rest came back, so for a frame a
  // piece is twice or missing where they meet. Fragments: how many each fall made, and how
  // long making them took.
  const edits: { stage: string; ms: number }[] = [];
  const debris: string[] = [];
  let torn = 0;
  let lastFrame = 0;
  for (const line of `${run.stdout}${run.stderr}`.split("\n")) {
    const call = /Calls: frame (\d+)/.exec(line);
    if (call) lastFrame = Number(call[1]);
    const change = /Build: change on screen in ([\d.]+) ms/.exec(line);
    if (change) edits.push({ stage: stages.find((each) => lastFrame >= each.from && lastFrame <= each.to)?.name ?? "-", ms: Number(change[1]) });
    const made = /Build: fragments: (\d+) cubes came down, \d+ destroyed, (\d+) fragments made in ([\d.]+) ms/.exec(line);
    if (made) debris.push(`${made[1]} cubes, ${made[2]} fragments in ${Number(made[3]).toFixed(1)} ms`);
    if (line.includes("sections shown before the rest")) torn++;
  }
  lines.push("", `Changes shown torn, a section before the rest of its change: ${torn}`);
  if (debris.length) lines.push(`Falls broken into fragments: ${debris.join("; ")}`);
  if (edits.length) {
    lines.push("", "Edits to the screen, from the change to all its sections drawn:");
    for (const stage of [...stages.map((each) => each.name), "-"]) {
      const ms = edits.filter((edit) => edit.stage === stage).map((edit) => edit.ms);
      if (!ms.length) continue;
      const { median, worst } = stat(ms);
      lines.push(`${stage.padEnd(24)} ${String(ms.length).padStart(4)} changes  median ${median.toFixed(0).padStart(5)}  worst ${worst.toFixed(0).padStart(6)} ms`);
    }
  }
  lines.push("", "Worst frames, and what was done on them and the two before:");
  const done = calls.filter((call) => !["build.look", "sandbox.look"].includes(call.method));
  for (const worst of [...rows].sort((a, b) => b.ms - a.ms).slice(0, 12)) {
    const stage = stages.find((each) => worst.frame >= each.from && worst.frame <= each.to)?.name ?? "-";
    const near = done.filter((call) => call.frame <= worst.frame && call.frame >= worst.frame - 2).map((call) => `${call.method} ${JSON.stringify(call.params ?? {})}`);
    lines.push(`  frame ${worst.frame} (${stage}): ${worst.ms.toFixed(1)} ms, game ${worst.game.toFixed(1)}, render ${worst.render.toFixed(1)}, GPU ${worst.gpu.toFixed(1)}${near.length ? `; ${near.join("; ").slice(0, 200)}` : ""}`);
  }
  writeFileSync(resolve(folder, `${label}.txt`), lines.join("\n") + "\n");
  console.log(lines.join("\n"));
  const all = stat(rows.map((r) => r.ms));
  const edit = stat(edits.map((each) => each.ms));
  return { summary: { ...all, over: rows.filter((r) => r.ms > BUDGET_MS).length, hitches: rows.filter((r) => r.ms > HITCH_MS).length, frames: rows.length, torn, edits: edits.length, edit_median: edit.median, edit_worst: edit.worst } };
}

/// shots/qa/index.html: the latest film of each scenario in each set, its contact sheet
/// linking to the video, and how its frames held, at a glance.
function index() {
  mkdirSync(OUT, { recursive: true });
  const names = readdirSync(OUT).filter((each) => statSync(resolve(OUT, each)).isDirectory()).sort();
  const columns = sets.map((set) => set.name);
  const cell = (scenario: string, set: string) => {
    const json = resolve(OUT, scenario, `${set}.json`);
    if (!existsSync(json)) return "<td class=none>not filmed</td>";
    const made = JSON.parse(readFileSync(json, "utf8"));
    const t = made.trace;
    const timing = t ? `median ${t.median.toFixed(1)} · p99 ${t.p99.toFixed(1)} · worst ${t.worst.toFixed(0)} ms · <span class=${t.hitches ? "bad" : "good"}>${t.hitches} hitches</span>${t.edits ? `<br>edits to screen median ${t.edit_median.toFixed(0)} · worst ${t.edit_worst.toFixed(0)} ms · <span class=${t.torn ? "bad" : "good"}>${t.torn} torn</span>` : ""}` : "not traced";
    const refused = made.refusals?.length ? `<br><span class=bad>${made.refusals.length} refused</span>` : "";
    return `<td><a href="${scenario}/${set}.mp4"><img src="${scenario}/${set}.jpg" alt="${scenario} in ${set}"></a><br>${timing}${refused}<br><a href="${scenario}/${set}.txt">trace</a> · ${made.made.slice(0, 16).replace("T", " ")}</td>`;
  };
  const html = `<!doctype html><meta charset=utf-8><title>QA films</title>
<style>body{font:14px system-ui;background:#16181c;color:#ddd;margin:16px}table{border-collapse:collapse}td,th{padding:6px;vertical-align:top;border:1px solid #333}img{width:320px;display:block}a{color:#9cf}.bad{color:#f88}.good{color:#8f8}.none{color:#666}</style>
<h1>QA films</h1><p>The latest film of each scenario in each set: click a sheet for its video. <code>cargo xtask qa &lt;scenario&gt; --set &lt;set&gt;</code> makes one again (tools/sandbox/README.md).</p>
<table><tr><th></th>${columns.map((c) => `<th>${c}</th>`).join("")}</tr>
${names.map((scenario) => `<tr><th>${scenario}</th>${columns.map((set) => cell(scenario, set)).join("")}</tr>`).join("\n")}
</table>`;
  writeFileSync(resolve(OUT, "index.html"), html);
  console.log(`qa: ${relative(ROOT, resolve(OUT, "index.html"))}`);
}

/// Frames as runs, like 120-180, 400.
function spans(frames: number[]) {
  const out: string[] = [];
  for (let i = 0; i < frames.length; ) {
    let j = i;
    while (j + 1 < frames.length && frames[j + 1] === frames[j] + 1) j++;
    out.push(i === j ? `${frames[i]}` : `${frames[i]}-${frames[j]}`);
    i = j + 1;
  }
  return out.join(", ");
}
