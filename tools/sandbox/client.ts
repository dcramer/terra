// A client for a sandbox run over Bevy's remote protocol (D69), for the command line
// (remote.ts) and for scripts that test the game, such as the demo's loop. See README.md.
// Node 22.18 or later, which runs TypeScript directly; no dependencies.

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, openSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, resolve } from "node:path";

export const ROOT = resolve(import.meta.dirname, "../..");
// The sandbox this checkout started listens on a port of its own and answers sandbox.whoami
// with a token of its own, both kept here, so commands never reach a sandbox another session
// is driving, even one that takes the same port. TERRA_REMOTE_PORT overrides the port.
const PORT_FILE = resolve(ROOT, "shots/remote.port");
const FIRST_PORT = 15702;
const saved = existsSync(PORT_FILE) ? readFileSync(PORT_FILE, "utf8").trim().split(" ") : [];
let port = Number(process.env.TERRA_REMOTE_PORT) || Number(saved[0]) || FIRST_PORT;
let token = process.env.TERRA_REMOTE_PORT ? undefined : saved[1];
const url = () => `http://127.0.0.1:${port}`;
export const onWsl = (() => {
  try {
    return readFileSync("/proc/version", "utf8").toLowerCase().includes("microsoft");
  } catch {
    return false;
  }
})();

/// A scripted run of the sandbox to its end, offscreen, for a check that compares its
/// frames (lodcheck, pops): the command and its words. Under WSL it is the Windows build, on
/// the real GPU, in a copy of its own named `copy`, since WSL's own build draws on the CPU,
/// far too slowly for a forest (a frame every few seconds); elsewhere the release build.
/// `buildOffscreen` builds it first, and paths given to it go through `offscreenPath`.
export const offscreenRun = (copy: string, args: string[]): [string, string[]] =>
  onWsl
    ? ["cargo", ["xtask", "win", "-p", "terra_sandbox", "--release", "--copy", own(copy), "--", "--offscreen", ...args]]
    : [resolve(ROOT, "target/release/terra_sandbox"), ["--offscreen", ...args]];

/// A Windows copy's name, this checkout's own: runs from two worktrees under one name would
/// otherwise share the copy and fight over its program and its files.
const checkout = (() => {
  const name = basename(ROOT);
  return name === "terra" ? "main" : name.slice(-8);
})();
export const own = (copy: string) => `${copy}-${checkout}`;

/// Builds what `offscreenRun` runs, once, before runs that may start at once.
export function buildOffscreen(): boolean {
  const target = onWsl ? ["--target", "x86_64-pc-windows-gnu"] : [];
  return spawnSync("cargo", ["build", "-q", "--release", ...target, "-p", "terra_sandbox"], { cwd: ROOT, stdio: "inherit" }).status === 0;
}

/// A path a run started by `offscreenRun` can write to: through WSL's share for Windows.
export const offscreenPath = (path: string) => (onWsl ? spawnSync("wslpath", ["-w", path], { encoding: "utf8" }).stdout.trim() : path);

/// ImageMagick's command for `words` as version 7 takes them after `magick` (`montage …`
/// for a montage): version 6, which Ubuntu ships and `cargo xtask doctor` asks for, has no
/// `magick`, only `convert` and `montage`, which take the same words.
export const imageMagick = (words: string[]): [string, string[]] =>
  hasMagick ? ["magick", words] : words[0] === "montage" ? ["montage", words.slice(1)] : ["convert", words];
const hasMagick = spawnSync("magick", ["-version"]).status === 0;

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

export async function call(method: string, params?: unknown): Promise<unknown> {
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
  const curl = spawnSync("/mnt/c/Windows/System32/curl.exe", ["-s", "--max-time", "60", "-X", "POST", url(), "-H", "content-type: application/json", "--data-binary", "@-"], {
    input: body,
    encoding: "utf8",
  });
  if (curl.status !== 0 || !curl.stdout) throw new Error(`no sandbox answering on ${url()}, here or on Windows; start one with \`start\``);
  through = "windows";
  return JSON.parse(curl.stdout) as Reply;
}

export const sleep = (ms: number) => new Promise((done) => setTimeout(done, ms));

/// `copy` names the Windows copy the run goes in, which is also where it keeps files given
/// by a relative path, such as a save: a script that restarts a run to load its save keeps
/// the same copy. `foreground` lets a person use the window too; otherwise the run stays
/// out of their way (`--background`).
/// `release` runs an optimised build, to time things as a player would meet them.
export type Start = { windows?: boolean; realTime?: boolean; foreground?: boolean; copy?: string; release?: boolean };

/// Starts a sandbox listening, in the background, on a port nothing else answers on and with
/// a token to know it by, and waits until it answers. Its log goes to shots/remote.log.
export async function start(sandboxArgs: string[], { windows, realTime, foreground, copy, release }: Start = {}) {
  for (let tries = 0; tries < 20; tries++) {
    // A random port away from the usual one other runs use, below the dynamic range, where
    // Windows reserves blocks of ports for Hyper-V and WSL and a run would start without a
    // listener.
    port = 20000 + Math.floor(Math.random() * 10000);
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
  const args = ["--remote-port", String(port), "--remote-token", token, ...(realTime ? [] : ["--steady"]), ...(foreground ? [] : ["--background"]), ...sandboxArgs];
  writeMark(0);
  // A dev build keeps the line behind every change, for `why` (D548); `win` adds it itself.
  const profile = release ? ["--release"] : ["--features", "track_location"];
  const command = windows
    ? ["cargo", ["xtask", "win", "-p", "terra_sandbox", ...(release ? ["--release"] : []), "--copy", own(copy ?? `remote-${port}`), "--", ...args]]
    : ["cargo", ["run", "-q", ...profile, "-p", "terra_sandbox", "--", ...args]];
  // Windows will not replace a program that is still closing, such as the last run of this
  // one, so a start that ends at once for that reason is tried again.
  const logPath = resolve(ROOT, "shots/remote.log");
  for (let attempt = 0; attempt < 30; attempt++) {
    const log = openSync(logPath, "w");
    const child = spawn(command[0] as string, command[1] as string[], { cwd: ROOT, detached: true, stdio: ["ignore", log, log] });
    let ended = false;
    child.on("exit", () => (ended = true));
    child.unref();
    for (let i = 0; i < 600 && !ended; i++) {
      let mine: boolean;
      try {
        mine = await ours();
      } catch {
        await sleep(1000);
        continue;
      }
      if (!mine) throw new Error(`another sandbox answered on port ${port}; start again`);
      // The clock is held while the loading screen is up, so nothing a script does means
      // anything until it has gone.
      // A debug build can be too busy loading to answer for a while, which is not an end.
      const settled = async () => {
        try {
          return await status();
        } catch {
          return { loading: true } as Awaited<ReturnType<typeof status>>;
        }
      };
      await waitUntil("the loading screen to go", settled, (now) => !now.loading, 300);
      return port;
    }
    if (!ended) throw new Error("the sandbox did not answer within ten minutes; see shots/remote.log");
    const said = readFileSync(logPath, "utf8");
    if (!said.includes("Could not copy the program")) throw new Error(`the sandbox stopped before it answered:\n${said.split("\n").slice(-15).join("\n")}`);
    await sleep(1000);
  }
  throw new Error("the last run of this sandbox never let go of its program; see shots/remote.log");
}

/// Closes the sandbox, which saves as it closes, and waits until it has gone.
export async function stop() {
  await call("sandbox.quit");
  for (let i = 0; i < 300; i++) {
    try {
      await raw("rpc.discover");
      await sleep(100);
    } catch {
      return;
    }
  }
  throw new Error("the sandbox did not close");
}

// A path the sandbox can write to: as it is for a sandbox on Linux, through WSL's share for
// one on Windows.
function forSandbox(path: string): string {
  if (through !== "windows") return path;
  return spawnSync("wslpath", ["-w", path], { encoding: "utf8" }).stdout.trim();
}

/// Saves a screenshot and waits for the file.
export async function shot(file: string) {
  const path = resolve(file);
  mkdirSync(dirname(path), { recursive: true });
  await call("rpc.discover");
  await call("sandbox.shot", { path: forSandbox(path) });
  let last = -1;
  for (let i = 0; i < 300; i++) {
    if (existsSync(path)) {
      const size = statSync(path).size;
      if (size > 0 && size === last) return path;
      last = size;
    }
    await sleep(100);
  }
  throw new Error(`the sandbox did not save ${path}`);
}

/// Saves the next so many frames one after another, as FILE-00000.png on, to see a change
/// as it happens; returns at once, and `filmed` waits for the last.
export async function film(file: string, frames: number) {
  const path = resolve(file);
  mkdirSync(dirname(path), { recursive: true });
  await call("sandbox.shot", { path: forSandbox(path), frames });
  return path.replace(/\.png$/, `-${String(frames - 1).padStart(5, "0")}.png`);
}

/// Waits for a file a run is saving to be whole.
export async function filmed(path: string) {
  let last = -1;
  for (let i = 0; i < 600; i++) {
    if (existsSync(path)) {
      const size = statSync(path).size;
      if (size > 0 && size === last) return path;
      last = size;
    }
    await sleep(100);
  }
  throw new Error(`the sandbox did not save ${path}`);
}

// The mark: the number of the first line of the run's record that has not been looked at
// yet. Commands that act move it to now before they act, so `waitFor` and `said` see only
// what happened since; `waitFor` moves it past what it found, so waiting twice for the same
// words waits for them to be said twice.
const MARK = resolve(ROOT, "shots/remote.mark");
export const readMark = () => (existsSync(MARK) ? Number(readFileSync(MARK, "utf8")) || 0 : 0);
export const writeMark = (line: number) => {
  mkdirSync(dirname(MARK), { recursive: true });
  writeFileSync(MARK, String(line));
};

export type Status = { frame: number; seconds: number; paused: boolean; loading: boolean; stepping: number; next_line: number };
export type Line = { line: number; frame: number; level: string; source: string; text: string };

export const status = async () => (await call("sandbox.status")) as Status;
export const markNow = async () => {
  const line = (await status()).next_line;
  writeMark(line);
  return line;
};
export const said = async (since: number, containing?: string, most?: number) =>
  (await call("sandbox.said", { since, containing, most })) as { lines: Line[]; next_line: number };
export const describe = (line: Line) => `frame ${String(line.frame).padStart(5)}  ${line.level === "INFO" ? "" : line.level + " "}${line.text}`;

/// Waits until the run says something containing `text`, from line `since` (the mark if not
/// given), and moves the mark past it. Fails at once if it says something containing
/// `unless`, and on timeout, with what was said instead.
export async function waitFor(text: string, { timeout = 60, since, unless }: { timeout?: number; since?: number; unless?: string } = {}): Promise<Line> {
  const from = since ?? readMark();
  const deadline = Date.now() + 1000 * timeout;
  while (Date.now() < deadline) {
    const { lines } = await said(from, text, 1);
    if (lines.length) {
      writeMark(Math.max(readMark(), lines[0].line + 1));
      return lines[0];
    }
    if (unless) {
      const { lines: refused } = await said(from, unless, 1);
      if (refused.length) throw new Error(describe(refused[0]));
    }
    await sleep(100);
  }
  const { lines } = await said(from, undefined, 1000);
  const now = await status();
  throw new Error(
    [`never heard "${text}" by frame ${now.frame}${now.paused ? ", with the clock stopped" : ""}; what was said since line ${from}:`, ...lines.slice(-40).map(describe)].join("\n"),
  );
}

/// Waits until `test` holds, asking every so often, or fails saying what it was waiting for.
export async function waitUntil<T>(what: string, ask: () => Promise<T>, test: (value: T) => boolean, timeout = 60): Promise<T> {
  const deadline = Date.now() + 1000 * timeout;
  let value = await ask();
  while (!test(value)) {
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}; last saw ${JSON.stringify(value)}`);
    await sleep(250);
    value = await ask();
  }
  return value;
}

/// Waits until `test` holds, asking every so often, and fails only once the game's clock has
/// moved on by `seconds` without it: a wait on what the game does, in its own time, so a
/// loaded machine drawing slowly does not fail it. Answers what was seen and how many
/// seconds of game time it took. A game that stops answering, or whose clock stops, fails
/// after `stalled` seconds on this machine's clock.
export async function waitUntilInGame<T>(what: string, ask: () => Promise<T>, test: (value: T) => boolean, seconds: number, stalled = 120): Promise<{ value: T; seconds: number }> {
  const from = (await status()).seconds;
  let [last, since] = [from, Date.now()];
  for (;;) {
    const value = await ask();
    const now = (await status()).seconds;
    if (test(value)) return { value, seconds: now - from };
    if (now - from > seconds) throw new Error(`${what} not seen in ${seconds} s of game time; last saw ${JSON.stringify(value)}`);
    if (now !== last) [last, since] = [now, Date.now()];
    else if (Date.now() - since > stalled * 1000) throw new Error(`the game's clock stood still for ${stalled} s waiting for ${what}`);
    await sleep(100);
  }
}

/// Waits until the game's clock has moved on by so many seconds.
export async function waitSeconds(seconds: number) {
  const from = (await status()).seconds;
  await waitUntil(`${seconds} s of game time`, status, (now) => now.seconds >= from + seconds, 60 + seconds * 10);
}

/// Presses keys, each written as --press names it, optionally `@after` frames from now and
/// `:frames` held: `e`, `w:60`, `f@40`, `e@90:80`.
export async function press(...keys: string[]) {
  await markNow();
  for (const each of keys) {
    const match = /^([^@:]+)(?:@(\d+))?(?::(\d+))?$/.exec(each);
    if (!match) throw new Error(`wanted a key like e, w:60, f@40 or e@90:80, not ${each}`);
    const [, key, after, frames] = match;
    await call("sandbox.press", { key, after: after ? Number(after) : undefined, frames: frames ? Number(frames) : undefined });
  }
}

/// Lets go of every key held or queued by `press`.
export const unpress = () => call("sandbox.unpress");

/// Stops the clock, lets it run for so many frames, and waits until it has stopped again.
export async function step(frames: number) {
  await markNow();
  await call("sandbox.step", { frames });
  return waitUntil(`${frames} steps`, status, (now) => now.stepping === 0);
}

// In the play experiment.

export type Near = { what?: string; who?: string; at: [number, number]; distance: number; health?: number; doing?: string };
export type State = {
  at: [number, number];
  facing: [number, number];
  on_ground: boolean;
  indoors: boolean;
  down: boolean;
  health: number;
  stamina: number;
  fed: number;
  holding: string | null;
  carrying: Record<string, number>;
  wearing: Record<string, string>;
  near: Near[];
};

export const state = async () => (await call("play.state")) as State;

/// Moves the player, as --warp does, and waits until they are there.
export async function warp(at: [number, number], facing?: [number, number]) {
  await markNow();
  await call("play.warp", { at, facing });
  await waitFor("Warped to", { timeout: 30 });
}

/// Turns the view until its middle, where a bow shoots and a tool reaches, is on a spot so
/// many metres above the ground, and waits until it is.
export async function aim(at: [number, number], up?: number) {
  await markNow();
  await call("play.aim", { at, up });
  await waitFor("Aimed at", { timeout: 15, unless: "Could not aim" });
}

/// Takes a tool in hand by its name: the slot of the bar it is on, if the game says, or
/// else each slot in turn.
export async function hold(tool: string) {
  const now = await state();
  if (now.holding === tool) return;
  const slot = (now.bar as (string | null)[] | undefined)?.indexOf(tool) ?? -1;
  if (slot >= 0) {
    await press(String(slot + 1));
    const held = await waitUntil(`the ${tool} in hand`, state, (now) => now.holding === tool, 2).catch(() => undefined);
    if (held) return;
  }
  for (let slot = 0; slot <= 8; slot++) {
    if (slot > 0) await press(String(slot));
    const held = await waitUntil(`slot ${slot} in hand`, state, (now) => now.holding === tool, 0.5).catch(() => undefined);
    if (held) return;
  }
  throw new Error(`nothing on the bar is a ${tool}; holding ${(await state()).holding}`);
}

/// Adds to what the player carries, as --give does (`stone:10`, `iron_axe`, `health:20`),
/// and waits until each is given.
export async function give(...items: string[]) {
  await markNow();
  for (const each of items) {
    const [item, count] = each.split(":");
    await call("play.give", { item, count: count === undefined ? undefined : Number(count) });
  }
  // What did not fit, or is not an item, is said as a warning beginning --give.
  for (const _ of items) await waitFor("Given ", { timeout: 30, unless: "--give:" });
}

/// Takes raw items away (`meat`, `stone:5`; all of a kind if no count), so a test can
/// start from a known pack, and waits until each is taken.
export async function take(...items: string[]) {
  await markNow();
  for (const each of items) {
    const [item, count] = each.split(":");
    await call("play.take", { item, count: count === undefined ? undefined : Number(count) });
  }
  for (const _ of items) await waitFor("Took ", { timeout: 30 });
}

/// A tunable (D547, `terra_gen::tune`): a named number the run reads where it uses it.
export type Tunable = { name: string; value: number; default: number; min: number; max: number; doc: string; set_by: string };

export const tunables = async (prefix?: string) => (await call("tune.list", { prefix })) as Tunable[];
export const tunable = async (name: string) => (await call("tune.get", { name })) as Tunable;

/// Sets a tunable, to its default when `value` is "default", and answers what it was.
export const tune = async (name: string, value: number | "default") =>
  (await call("tune.set", { name, value: value === "default" ? null : value })) as { was: number; tunable: Tunable };

/// Resets every changed tunable whose name starts with `prefix` (all if none).
export const untune = async (prefix?: string) => ((await call("tune.reset", { prefix })) as { reset: string[] }).reset;

/// One line: `sky.cloud.body_low = 0.45 (default 0.6, 0 to 0.95, by remote): what it does`.
export const describeTunable = (t: Tunable, width = 0) => {
  const changed = t.value === t.default ? "" : `default ${t.default}, `;
  const by = t.set_by === "default" ? "" : `, by ${t.set_by}`;
  return `${t.name.padEnd(width)} = ${t.value} (${changed}${t.min} to ${t.max}${by}): ${t.doc}`;
};
