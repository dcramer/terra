#!/usr/bin/env node
// Drives a running sandbox over Bevy's remote protocol: find things, move them, hold the
// camera on them, pause and step, press keys, read what is true and what the run said,
// wait for something to happen, and take screenshots, without restarting. See README.md.
// Node 22.18 or later, which runs TypeScript directly; no dependencies.

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, openSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { parseArgs } from "node:util";

const ROOT = resolve(import.meta.dirname, "../..");
// The sandbox this checkout started listens on a port of its own and answers sandbox.whoami
// with a token of its own, both kept here, so commands never reach a sandbox another session
// is driving, even one that takes the same port. TERRA_REMOTE_PORT overrides the port.
const PORT_FILE = resolve(ROOT, "shots/remote.port");
const FIRST_PORT = 15702;
const saved = existsSync(PORT_FILE) ? readFileSync(PORT_FILE, "utf8").trim().split(" ") : [];
let port = Number(process.env.TERRA_REMOTE_PORT) || Number(saved[0]) || FIRST_PORT;
let token = process.env.TERRA_REMOTE_PORT ? undefined : saved[1];
const url = () => `http://127.0.0.1:${port}`;
const onWsl = (() => {
  try {
    return readFileSync("/proc/version", "utf8").toLowerCase().includes("microsoft");
  } catch {
    return false;
  }
})();

type Reply = { result?: unknown; error?: { code: number; message: string } };

// Under WSL a sandbox running on Windows listens on Windows' own localhost, which WSL
// cannot reach, so the request goes through Windows' curl. A sandbox on Linux is reached
// directly; whichever answers is the one used.
let through: "direct" | "windows" | undefined;

// Whether the sandbox answering is the one this checkout started.
async function ours(): Promise<boolean> {
  if (!token) return true;
  const answer = (await raw("sandbox.whoami")) as { token?: string } | undefined;
  return answer?.token === token;
}

async function call(method: string, params?: unknown): Promise<unknown> {
  if (!(await ours())) throw new Error(`the sandbox on port ${port} is not the one this checkout started; start one with \`start\``);
  return raw(method, params);
}

async function raw(method: string, params?: unknown): Promise<unknown> {
  const body = JSON.stringify({ jsonrpc: "2.0", id: 1, method, params });
  const reply = await send(body);
  if (reply.error) throw new Error(`${method}: ${reply.error.message}`);
  return reply.result;
}

async function send(body: string): Promise<Reply> {
  if (through !== "windows") {
    try {
      const response = await fetch(url(), { method: "POST", body, headers: { "content-type": "application/json" } });
      through = "direct";
      return (await response.json()) as Reply;
    } catch (error) {
      if (through === "direct" || !onWsl) throw new Error(`no sandbox answering on ${url()}; start one with \`start\``);
    }
  }
  const curl = spawnSync("/mnt/c/Windows/System32/curl.exe", ["-s", "--max-time", "5", "-X", "POST", url(), "-H", "content-type: application/json", "--data-binary", "@-"], {
    input: body,
    encoding: "utf8",
  });
  if (curl.status !== 0 || !curl.stdout) throw new Error(`no sandbox answering on ${url()}, here or on Windows; start one with \`start\``);
  through = "windows";
  return JSON.parse(curl.stdout) as Reply;
}

// A path the sandbox can write to: as it is for a sandbox on Linux, through WSL's share for
// one on Windows.
function forSandbox(path: string): string {
  if (through !== "windows") return path;
  return spawnSync("wslpath", ["-w", path], { encoding: "utf8" }).stdout.trim();
}

const sleep = (ms: number) => new Promise((done) => setTimeout(done, ms));

async function waitForFile(path: string, seconds = 30) {
  let last = -1;
  for (let i = 0; i < seconds * 10; i++) {
    if (existsSync(path)) {
      const size = statSync(path).size;
      if (size > 0 && size === last) return;
      last = size;
    }
    await sleep(100);
  }
  throw new Error(`the sandbox did not save ${path}`);
}

const triple = (text: string | undefined) => {
  if (text === undefined) return undefined;
  const numbers = text.split(",").map(Number);
  if (numbers.length !== 3 || numbers.some(Number.isNaN)) throw new Error(`wanted three numbers like 0,0.1,0, not ${text}`);
  return numbers;
};
const number = (text: string | undefined) => (text === undefined ? undefined : Number(text));
const print = (value: unknown) => console.log(JSON.stringify(value, null, 1));

// The mark: the number of the first line of the run's record that has not been looked at
// yet. Commands that act move it to now before they act, so `wait` and `said` see only
// what happened since; `wait` moves it past what it found, so waiting twice for the same
// words waits for them to be said twice.
const MARK = resolve(ROOT, "shots/remote.mark");
const readMark = () => (existsSync(MARK) ? Number(readFileSync(MARK, "utf8")) || 0 : 0);
const writeMark = (line: number) => {
  mkdirSync(dirname(MARK), { recursive: true });
  writeFileSync(MARK, String(line));
};
type Status = { frame: number; seconds: number; paused: boolean; stepping: number; next_line: number };
type Line = { line: number; frame: number; level: string; source: string; text: string };
const status = async () => (await call("sandbox.status")) as Status;
const markNow = async () => writeMark((await status()).next_line);
const said = async (since: number, containing?: string, most?: number) =>
  (await call("sandbox.said", { since, containing, most })) as { lines: Line[]; next_line: number };
const show = (line: Line) => console.log(`frame ${String(line.frame).padStart(5)}  ${line.level === "INFO" ? "" : line.level + " "}${line.text}`);
const eastSouth = (text: string) => {
  const numbers = text.split(",").map(Number);
  if (numbers.length !== 2 || numbers.some(Number.isNaN)) throw new Error(`wanted metres east and south like 70,-55, not ${text}`);
  return numbers;
};

type Command = { usage: string; help: string; options?: Record<string, { type: "string" | "boolean"; help: string }>; run: (a: { values: Record<string, any>; positionals: string[] }) => Promise<void> };

const COMMANDS: Record<string, Command> = {
  start: {
    usage: "start [--windows] [-- sandbox options]",
    help: "start a sandbox listening, in the background, and wait until it answers; its log goes to shots/remote.log. The game moves a sixtieth of a second a frame, however fast frames are drawn, unless --real-time",
    options: { windows: { type: "boolean", help: "on Windows' GPU (cargo xtask win), from WSL" }, "real-time": { type: "boolean", help: "let the game's clock follow the real one" } },
    run: async ({ values, positionals }) => {
      // A random port nothing answers on, well away from the usual one other runs use, and a
      // token to know this run by.
      for (let tries = 0; tries < 20; tries++) {
        port = 20000 + Math.floor(Math.random() * 40000);
        through = undefined;
        try {
          await raw("rpc.discover");
        } catch {
          break;
        }
      }
      through = undefined;
      token = Math.random().toString(36).slice(2);
      mkdirSync(dirname(PORT_FILE), { recursive: true });
      writeFileSync(PORT_FILE, `${port} ${token}`);
      const args = ["--remote-port", String(port), "--remote-token", token, ...(values["real-time"] ? [] : ["--steady"]), ...positionals];
      writeMark(0);
      const command = values.windows
        ? ["cargo", ["xtask", "win", "-p", "terra_sandbox", "--copy", `remote-${port}`, "--", ...args]]
        : ["cargo", ["run", "-q", "-p", "terra_sandbox", "--", ...args]];
      mkdirSync(resolve(ROOT, "shots"), { recursive: true });
      const log = openSync(resolve(ROOT, "shots/remote.log"), "w");
      spawn(command[0] as string, command[1] as string[], { cwd: ROOT, detached: true, stdio: ["ignore", log, log] }).unref();
      for (let i = 0; i < 600; i++) {
        let mine: boolean;
        try {
          mine = await ours();
        } catch {
          await sleep(1000);
          continue;
        }
        if (!mine) throw new Error(`another sandbox answered on port ${port}; run start again`);
        console.log(`ready on port ${port}`);
        return;
      }
      throw new Error("the sandbox did not answer within ten minutes; see shots/remote.log");
    },
  },
  stop: { usage: "stop", help: "close the sandbox", run: async () => void (await call("sandbox.quit")) },
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
    run: async ({ positionals: [frames] }) => {
      await markNow();
      await call("sandbox.step", { frames: Number(frames ?? 1) });
      for (let i = 0; i < 6000; i++) {
        const now = await status();
        if (now.stepping === 0) return print(now);
        await sleep(100);
      }
      throw new Error("the steps did not finish");
    },
  },
  press: {
    usage: "press KEY[:FRAMES] …",
    help: "hold keys down for so many frames (one if not given), all starting on the same frame, as --press names them: e, w:60, F3, esc, left:20",
    run: async ({ positionals }) => {
      await markNow();
      for (const each of positionals) {
        const [key, frames] = each.split(":");
        await call("sandbox.press", { key, frames: number(frames) });
      }
      console.log("pressed");
    },
  },
  status: { usage: "status", help: "the frame, the game's time, whether it is stopped, and the number of the next line the run will say", run: async () => print(await status()) },
  mark: { usage: "mark", help: "start `said` and `wait` from now", run: async () => void (await markNow()) },
  said: {
    usage: "said [--containing TEXT] [--all] [--from LINE]",
    help: "what the run has said since the mark, with the frame each was said on",
    options: { containing: { type: "string", help: "only lines with this in them" }, all: { type: "boolean", help: "from the start of the run" }, from: { type: "string", help: "from this line" } },
    run: async ({ values }) => {
      const since = values.all ? 0 : values.from !== undefined ? Number(values.from) : readMark();
      const { lines, next_line } = await said(since, values.containing, 1000);
      lines.forEach(show);
      if (lines.length === 1000) console.log(`… more; continue with --from ${lines[999].line + 1}`);
      else if (!lines.length) console.log(`(nothing since line ${since}; the next will be ${next_line})`);
    },
  },
  wait: {
    usage: "wait TEXT [--timeout seconds]",
    help: "wait until the run says something containing TEXT, since the mark; on timeout, show what it said instead and fail",
    options: { timeout: { type: "string", help: "real seconds to wait (60)" } },
    run: async ({ values, positionals }) => {
      const text = positionals.join(" ");
      const since = readMark();
      const deadline = Date.now() + 1000 * Number(values.timeout ?? 60);
      while (Date.now() < deadline) {
        const { lines } = await said(since, text, 1);
        if (lines.length) {
          show(lines[0]);
          writeMark(lines[0].line + 1);
          return;
        }
        await sleep(100);
      }
      const { lines } = await said(since, undefined, 1000);
      console.log(`never heard "${text}"; what was said since line ${since}:`);
      lines.slice(-40).forEach(show);
      const now = await status();
      throw new Error(`timed out on frame ${now.frame}${now.paused ? ", with the clock stopped" : ""}`);
    },
  },
  state: { usage: "state", help: "in play: where the player is, their body, what they hold, carry and wear, and who is near", run: async () => print(await call("play.state")) },
  warp: {
    usage: "warp EAST,SOUTH [--facing EAST,SOUTH]",
    help: "in play: move the player, as --warp does, and wait until they are there",
    options: { facing: { type: "string", help: "a spot to face, in metres east and south of the start" } },
    run: async ({ values, positionals: [at] }) => {
      await markNow();
      await call("play.warp", { at: eastSouth(at), facing: values.facing ? eastSouth(values.facing) : undefined });
      await COMMANDS.wait.run({ values: { timeout: "30" }, positionals: ["Warped to"] });
    },
  },
  give: {
    usage: "give ITEM[:COUNT] …",
    help: "in play: carry something, as --give does: stone:10, iron_axe, leather_coat, or health:20 to set the body",
    run: async ({ positionals }) => {
      await markNow();
      for (const each of positionals) {
        const [item, count] = each.split(":");
        await call("play.give", { item, count: number(count) });
      }
      for (const _ of positionals) await COMMANDS.wait.run({ values: { timeout: "30" }, positionals: ["Given "] });
    },
  },
  pose: {
    usage: "pose CLIP [FRACTION]",
    help: "in the workbench: hold every model at FRACTION (0 to 1) through CLIP; without one, play it",
    run: async ({ positionals: [clip, fraction] }) => print(await call("workbench.pose", { clip, fraction: number(fraction) })),
  },
  shot: {
    usage: "shot FILE.png",
    help: "save a screenshot and wait for it",
    run: async ({ positionals: [file] }) => {
      const path = resolve(file);
      mkdirSync(dirname(path), { recursive: true });
      await call("rpc.discover");
      await call("sandbox.shot", { path: forSandbox(path) });
      await waitForFile(path);
      console.log(path);
    },
  },
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
