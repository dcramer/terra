#!/usr/bin/env node
// Drives a running sandbox over Bevy's remote protocol: find things, move them, hold the
// camera on them, pause, and take screenshots, without restarting. See README.md.
// Node 22.18 or later, which runs TypeScript directly; no dependencies.

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, openSync, readFileSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { parseArgs } from "node:util";

const URL = "http://127.0.0.1:15702";
const ROOT = resolve(import.meta.dirname, "../..");
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

async function call(method: string, params?: unknown): Promise<unknown> {
  const body = JSON.stringify({ jsonrpc: "2.0", id: 1, method, params });
  const reply = await send(body);
  if (reply.error) throw new Error(`${method}: ${reply.error.message}`);
  return reply.result;
}

async function send(body: string): Promise<Reply> {
  if (through !== "windows") {
    try {
      const response = await fetch(URL, { method: "POST", body, headers: { "content-type": "application/json" } });
      through = "direct";
      return (await response.json()) as Reply;
    } catch (error) {
      if (through === "direct" || !onWsl) throw new Error(`no sandbox answering on ${URL}; start one with --remote`);
    }
  }
  const curl = spawnSync("/mnt/c/Windows/System32/curl.exe", ["-s", "-X", "POST", URL, "-H", "content-type: application/json", "--data-binary", "@-"], {
    input: body,
    encoding: "utf8",
  });
  if (curl.status !== 0 || !curl.stdout) throw new Error(`no sandbox answering on ${URL}, here or on Windows; start one with --remote`);
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

type Command = { usage: string; help: string; options?: Record<string, { type: "string" | "boolean"; help: string }>; run: (a: { values: Record<string, any>; positionals: string[] }) => Promise<void> };

const COMMANDS: Record<string, Command> = {
  start: {
    usage: "start [--windows] [-- sandbox options]",
    help: "start a sandbox listening, in the background, and wait until it answers; its log goes to shots/remote.log",
    options: { windows: { type: "boolean", help: "on Windows' GPU (cargo xtask win), from WSL" } },
    run: async ({ values, positionals }) => {
      const args = ["--remote", ...positionals];
      const command = values.windows
        ? ["cargo", ["xtask", "win", "-p", "terra_sandbox", "--copy", "remote", "--", ...args]]
        : ["cargo", ["run", "-q", "-p", "terra_sandbox", "--", ...args]];
      mkdirSync(resolve(ROOT, "shots"), { recursive: true });
      const log = openSync(resolve(ROOT, "shots/remote.log"), "w");
      spawn(command[0] as string, command[1] as string[], { cwd: ROOT, detached: true, stdio: ["ignore", log, log] }).unref();
      for (let i = 0; i < 600; i++) {
        try {
          await call("rpc.discover");
          console.log("ready");
          return;
        } catch {
          await sleep(1000);
        }
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
    usage: "look NAME|x,y,z [--distance m] [--around degrees] [--height m]",
    help: "hold the camera on an entity, following it, or on a point; around is from the model's front",
    options: { distance: { type: "string", help: "metres away (2.5)" }, around: { type: "string", help: "degrees round from the front (30)" }, height: { type: "string", help: "metres above (0.3)" } },
    run: async ({ values, positionals: [at] }) =>
      print(await call("sandbox.look", { at: /^-?[\d.]+,-?[\d.]+,-?[\d.]+$/.test(at) ? triple(at) : at, distance: number(values.distance), around: number(values.around), height: number(values.height) })),
  },
  release: { usage: "release", help: "let the camera go", run: async () => print(await call("sandbox.release")) },
  pause: { usage: "pause", help: "stop the clock, animations included", run: async () => print(await call("sandbox.pause", { paused: true })) },
  resume: { usage: "resume", help: "start the clock again", run: async () => print(await call("sandbox.pause", { paused: false })) },
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
const { values, positionals } = parseArgs({ args: rest, options: command.options ?? {}, allowPositionals: true });
command.run({ values, positionals }).catch((error: Error) => {
  console.error(error.message);
  process.exit(1);
});
