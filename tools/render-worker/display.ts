// The display the daemon's jobs draw through (docs/render-worker.md, D445). winit's event loop
// needs one even for an offscreen run, and a systemd user service has none: with no
// TERRA_RENDER_DISPLAY and no display of its own, the daemon starts one Xvfb on a free number
// when a job first needs it, keeps it for its lifetime, starts it again if it dies and stops it
// when the daemon exits. The decisions are jobs.ts's (`displayPlan`), tested in jobs.test.ts.

import { type ChildProcess, spawn } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { type DisplayPlan, freeDisplay, takenDisplays, xvfbArgs, xvfbBackoff } from "./jobs.ts";

const X11 = "/tmp/.X11-unix";
const list = (dir: string) => {
  try {
    return readdirSync(dir);
  } catch {
    return [];
  }
};

const alive = (c: ChildProcess) => c.exitCode === null && c.signalCode === null;

/// A free display number now, from the X sockets and lock files.
export const pickDisplay = () => freeDisplay(takenDisplays([...list(X11), ...list("/tmp")]));

export class Display {
  private child?: ChildProcess;
  private display?: string;
  private starting?: Promise<string | undefined>;
  private quickDeaths = 0;
  private stopped = false;
  /// Why the last start failed, for the job's log.
  why = "";

  readonly plan: DisplayPlan;
  private say: (line: string) => void;

  constructor(plan: DisplayPlan, say: (line: string) => void) {
    this.plan = plan;
    this.say = say;
  }

  /// What jobs see now, for the daemon's record and --doctor.
  describe(): string {
    if (this.plan.how === "inherit") return `the daemon's ${this.plan.said}`;
    if (this.plan.how === "fixed") return `${this.plan.display} (TERRA_RENDER_DISPLAY)`;
    return this.display ? `Xvfb ${this.display} (pid ${this.child?.pid})` : "Xvfb, started when a job first needs it";
  }

  /// The DISPLAY to give a job that needs one: undefined keeps the daemon's own; for an Xvfb,
  /// starts it first if it is not running, and undefined if it could not start (`why`).
  async get(): Promise<string | undefined> {
    if (this.plan.how === "inherit") return undefined;
    if (this.plan.how === "fixed") return this.plan.display;
    if (this.display && this.child && alive(this.child)) return this.display;
    this.starting ??= this.start().finally(() => (this.starting = undefined));
    return this.starting;
  }

  private async start(): Promise<string | undefined> {
    if (this.stopped) return undefined;
    const n = pickDisplay();
    const display = `:${n}`;
    const began = Date.now();
    let err = "";
    const child = spawn("Xvfb", xvfbArgs(n), { stdio: ["ignore", "ignore", "pipe"] });
    child.stderr?.on("data", (d: Buffer) => (err = `${err}${d}`.slice(-2000)));
    const exited = new Promise<string>((done) => {
      child.on("error", (e) => done(`Xvfb could not start: ${e.message}`));
      child.on("exit", (code, signal) => done(`Xvfb ${display} exited ${code ?? signal}: ${err.trim().split("\n").at(-1) ?? ""}`));
    });
    const ready = new Promise<boolean>((done) => {
      const t0 = Date.now();
      const look = () => (existsSync(`${X11}/X${n}`) ? done(true) : Date.now() - t0 > 10_000 ? done(false) : setTimeout(look, 100));
      look();
    });
    const first = await Promise.race([ready.then((ok) => (ok ? "" : `Xvfb ${display} made no socket in 10 s`)), exited]);
    if (first) {
      child.kill("SIGTERM");
      this.why = first;
      this.say(`✗ display: ${first}`);
      return undefined;
    }
    this.child = child;
    this.display = display;
    this.why = "";
    this.say(`display: Xvfb ${display} (pid ${child.pid}) for the jobs`);
    exited.then((why) => {
      if (this.child !== child) return;
      this.display = undefined;
      this.child = undefined;
      if (this.stopped) return;
      this.quickDeaths = Date.now() - began < 60_000 ? this.quickDeaths + 1 : 0;
      const wait = xvfbBackoff(this.quickDeaths);
      this.say(`✗ display: ${why}; starting it again in ${wait} s`);
      setTimeout(() => void this.get(), wait * 1000).unref();
    });
    return display;
  }

  /// Stops the Xvfb, if the daemon started one; synchronous, so it runs on exit.
  stop(): void {
    this.stopped = true;
    if (this.child && alive(this.child)) this.child.kill("SIGTERM");
  }
}
