#!/usr/bin/env node
// Drives a running sandbox over Bevy's remote protocol: find things, move them, hold the
// camera on them, pause and step, press keys, read what is true and what the run said,
// wait for something to happen, and take screenshots, without restarting. See README.md.
// Node 22.18 or later, which runs TypeScript directly; no dependencies.

import { parseArgs } from "node:util";
import * as sandbox from "./client.ts";
import { call, describe } from "./client.ts";

const triple = (text: string | undefined) => {
  if (text === undefined) return undefined;
  const numbers = text.split(",").map(Number);
  if (numbers.length !== 3 || numbers.some(Number.isNaN)) throw new Error(`wanted three numbers like 0,0.1,0, not ${text}`);
  return numbers;
};
const eastSouth = (text: string) => {
  const numbers = text.split(",").map(Number);
  if (numbers.length !== 2 || numbers.some(Number.isNaN)) throw new Error(`wanted metres east and south like 70,-55, not ${text}`);
  return numbers as [number, number];
};
const number = (text: string | undefined) => (text === undefined ? undefined : Number(text));
const print = (value: unknown) => console.log(JSON.stringify(value, null, 1));

type Command = { usage: string; help: string; options?: Record<string, { type: "string" | "boolean"; help: string }>; run: (a: { values: Record<string, any>; positionals: string[] }) => Promise<void> };

const COMMANDS: Record<string, Command> = {
  start: {
    usage: "start [--windows] [-- sandbox options]",
    help: "start a sandbox listening, in the background, and wait until it answers; its log goes to shots/remote.log. The game moves a sixtieth of a second a frame, however fast frames are drawn, unless --real-time",
    options: { windows: { type: "boolean", help: "on Windows' GPU (cargo xtask win), from WSL" }, "real-time": { type: "boolean", help: "let the game's clock follow the real one" } },
    run: async ({ values, positionals }) => console.log(`ready on port ${await sandbox.start(positionals, { windows: values.windows, realTime: values["real-time"] })}`),
  },
  stop: { usage: "stop", help: "close the sandbox, which saves as it closes, and wait until it has gone", run: () => sandbox.stop() },
  call: {
    usage: "call METHOD [JSON]",
    help: "any method, with its params as JSON; `call rpc.discover` lists them",
    run: async ({ positionals: [method, params] }) => print(await call(method, params ? JSON.parse(params) : undefined)),
  },
  find: { usage: "find NAME", help: "entities whose name contains NAME, with where they are", run: async ({ positionals: [name] }) => print(await call("sandbox.find", { name })) },
  place: {
    usage: "place NAME [--translation x,y,z] [--rotation x,y,z] [--scale s]",
    help: "set the local transform of the entity called exactly NAME; rotation in degrees",
    options: { translation: { type: "string", help: "metres, in its parent's space" }, rotation: { type: "string", help: "degrees about X, Y, then Z" }, scale: { type: "string", help: "one number" } },
    run: async ({ values, positionals: [name] }) =>
      print(await call("sandbox.place", { name, translation: triple(values.translation), rotation: triple(values.rotation), scale: number(values.scale) })),
  },
  look: {
    usage: "look NAME|x,y,z [--distance m] [--around degrees] [--height m] [--aim m]",
    help: "hold the camera on an entity, following it, or on a point; around is from the model's front, aim how far above it to look",
    options: { distance: { type: "string", help: "metres away (2.5)" }, around: { type: "string", help: "degrees round from the front (30)" }, height: { type: "string", help: "metres above (0.3)" }, aim: { type: "string", help: "metres above it to look at (0)" } },
    run: async ({ values, positionals: [at] }) =>
      print(await call("sandbox.look", { at: /^-?[\d.]+,-?[\d.]+,-?[\d.]+$/.test(at) ? triple(at) : at, distance: number(values.distance), around: number(values.around), height: number(values.height), aim: number(values.aim) })),
  },
  release: { usage: "release", help: "let the camera go", run: async () => print(await call("sandbox.release")) },
  pause: { usage: "pause", help: "stop the clock, animations included", run: async () => print(await call("sandbox.pause", { paused: true })) },
  resume: { usage: "resume", help: "start the clock again", run: async () => print(await call("sandbox.pause", { paused: false })) },
  step: {
    usage: "step FRAMES",
    help: "stop the clock, let it run for FRAMES frames, stop it again, and wait until it has",
    run: async ({ positionals: [frames] }) => print(await sandbox.step(Number(frames ?? 1))),
  },
  press: {
    usage: "press KEY[@AFTER][:FRAMES] …",
    help: "hold keys, named as --press names them, for so many frames (one if not given), starting so many frames from now (none if not given): e, w:60, f@40, e@90:80",
    run: async ({ positionals }) => {
      await sandbox.press(...positionals);
      console.log("pressed");
    },
  },
  unpress: { usage: "unpress", help: "let go of every key held or queued by press", run: async () => print(await sandbox.unpress()) },
  status: { usage: "status", help: "the frame, the game's time, whether it is stopped, and the number of the next line the run will say", run: async () => print(await sandbox.status()) },
  mark: { usage: "mark", help: "start `said` and `wait` from now", run: async () => void (await sandbox.markNow()) },
  said: {
    usage: "said [--containing TEXT] [--all] [--from LINE]",
    help: "what the run has said since the mark, with the frame each was said on",
    options: { containing: { type: "string", help: "only lines with this in them" }, all: { type: "boolean", help: "from the start of the run" }, from: { type: "string", help: "from this line" } },
    run: async ({ values }) => {
      const since = values.all ? 0 : values.from !== undefined ? Number(values.from) : sandbox.readMark();
      const { lines, next_line } = await sandbox.said(since, values.containing, 1000);
      for (const line of lines) console.log(describe(line));
      if (lines.length === 1000) console.log(`… more; continue with --from ${lines[999].line + 1}`);
      else if (!lines.length) console.log(`(nothing since line ${since}; the next will be ${next_line})`);
    },
  },
  wait: {
    usage: "wait TEXT [--timeout seconds]",
    help: "wait until the run says something containing TEXT, since the mark; on timeout, show what it said instead and fail",
    options: { timeout: { type: "string", help: "real seconds to wait (60)" } },
    run: async ({ values, positionals }) => console.log(describe(await sandbox.waitFor(positionals.join(" "), { timeout: Number(values.timeout ?? 60) }))),
  },
  state: { usage: "state", help: "in play: where the player is, their body, what they hold, carry and wear, and who is near", run: async () => print(await sandbox.state()) },
  warp: {
    usage: "warp EAST,SOUTH [--facing EAST,SOUTH]",
    help: "in play: move the player, as --warp does, and wait until they are there",
    options: { facing: { type: "string", help: "a spot to face, in metres east and south of the start" } },
    run: async ({ values, positionals: [at] }) => sandbox.warp(eastSouth(at), values.facing ? eastSouth(values.facing) : undefined),
  },
  aim: {
    usage: "aim EAST,SOUTH [--up m]",
    help: "in play: turn the view until its middle, where a bow shoots and a tool reaches, is on a spot, and wait until it is",
    options: { up: { type: "string", help: "metres above the ground (1)" } },
    run: async ({ values, positionals: [at] }) => sandbox.aim(eastSouth(at), number(values.up)),
  },
  give: {
    usage: "give ITEM[:COUNT] …",
    help: "in play: carry something, as --give does: stone:10, iron_axe, leather_coat, or health:20 to set the body",
    run: async ({ positionals }) => sandbox.give(...positionals),
  },
  take: {
    usage: "take ITEM[:COUNT] …",
    help: "in play: take raw items away, all of a kind if no count: meat, stone:5",
    run: async ({ positionals }) => sandbox.take(...positionals),
  },
  pose: {
    usage: "pose CLIP [FRACTION]",
    help: "in the workbench: hold every model at FRACTION (0 to 1) through CLIP; without one, play it",
    run: async ({ positionals: [clip, fraction] }) => print(await call("workbench.pose", { clip, fraction: number(fraction) })),
  },
  shape: {
    usage: "shape [--model N] TEMPLATE | MORPH=WEIGHT …",
    help: "in the workbench: reshape a model's body live, as a template or a mix of morphs (heavy=0.5 thin=0.2)",
    options: { model: { type: "string", help: "which model, by its order on the command line (all if not given)" } },
    run: async ({ values, positionals }) => {
      const morphs = Object.fromEntries(positionals.filter((p) => p.includes("=")).map((p) => [p.split("=")[0], number(p.split("=")[1])]));
      const template = positionals.find((p) => !p.includes("="));
      print(await call("workbench.shape", { model: values.model === undefined ? undefined : number(values.model), template, morphs: Object.keys(morphs).length ? morphs : undefined }));
    },
  },
  shot: { usage: "shot FILE.png", help: "save a screenshot and wait for it", run: async ({ positionals: [file] }) => console.log(await sandbox.shot(file)) },
};

function usage() {
  console.log("node tools/sandbox/remote.ts COMMAND …\n");
  for (const command of Object.values(COMMANDS)) console.log(`  ${command.usage.padEnd(62)} ${command.help}`);
}

const [name, ...rest] = process.argv.slice(2);
const command = COMMANDS[name ?? ""];
if (!command || rest.includes("--help")) {
  usage();
  process.exit(command || name === "help" || !name ? 0 : 1);
}
// Places are often negative, like -30,33, which would otherwise read as an option. One
// after an option that takes a value becomes its value; any other is a positional.
const args: string[] = [];
const negatives: string[] = [];
const end = rest.indexOf("--");
for (const arg of end < 0 ? rest : rest.slice(0, end)) {
  const last = args.at(-1);
  if (!/^-[\d.]/.test(arg)) args.push(arg);
  else if (last?.startsWith("--") && !last.includes("=") && command.options?.[last.slice(2)]?.type === "string") args[args.length - 1] = `${last}=${arg}`;
  else negatives.push(arg);
}
const after = end < 0 ? [] : rest.slice(end + 1);
const { values, positionals } = parseArgs({ args: [...args, "--", ...negatives, ...after], options: command.options ?? {}, allowPositionals: true });
command.run({ values, positionals }).catch((error: Error) => {
  console.error(error.message);
  process.exit(1);
});
