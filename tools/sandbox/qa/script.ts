// What a QA film's scenario writes (tools/sandbox/qa.ts): a timeline of remote calls on
// given frames of a scripted run, and the camera's moves, in the base's own space: metres
// east, up and south of its claim stone, and its cubes, half a metre each. The same
// scenario and set give the same calls every time, so the same film. Node 22.18 or later;
// no dependencies.

import { readdirSync, readFileSync } from "node:fs";
import { basename, resolve } from "node:path";

export type Vec3 = [number, number, number];
export type Call = { frame: number; method: string; params?: unknown; tag?: string; expect?: { name: string; text: string }; allow?: string[] };
/// A stretch of the film with a name, which `--part` films alone and the contact sheet labels.
export type Stage = { name: string; title: string; from: number; to: number };
/// A step of building, as a fill gave it: what went in, and on which frame of the film.
export type Step = { frame: number; what: string };

/// What a first pass of a scenario found, that its film is timed by: what each `fill` built,
/// step by step, and how many frames after its call each `expect`'s words were said.
export type Known = { fills: Record<string, string[]>; said: Record<string, number> };

/// A building kit as its file in assets/data/kits/ has it.
export type Kit = { name: string; id: number; pattern: string; roof?: string; baseline: boolean };

/// What a film is built in: one wall kit and its roof, or several meeting (`mixes`). A
/// scenario asks for a kind by what it is for, so one scenario films every set.
export class KitSet {
  readonly name: string;
  /// The wall kits, in order: one for a kit's own set, every wall kit for `mixes`.
  readonly walls: Kit[];
  readonly kits: Kit[];

  constructor(name: string, walls: Kit[], kits: Kit[]) {
    this.name = name;
    this.walls = walls;
    this.kits = kits;
  }

  /// The wall kit for the n-th thing a scenario lays, so a mix gives each a kind of its own.
  wall(n = 0): string {
    return this.walls[n % this.walls.length].name;
  }

  /// The roof a wall kit takes.
  roof(wall = this.wall()): string {
    return this.kits.find((kit) => kit.name === wall)?.roof ?? this.kits.find((kit) => kit.pattern === "roof")!.name;
  }

  /// A wall kit other than the n-th, to change a wall to: one of the same pattern if there
  /// is one, which changes in place at once as its cubes are repainted (D298), as
  /// Enshrouded's material swap does, rather than coming out and going in again.
  other(n = 0): string {
    const own = this.kits.find((kit) => kit.name === this.wall(n))!;
    const all = this.kits.filter((kit) => kit.roof && (kit.baseline || !own.baseline));
    const like = all.filter((kit) => kit.pattern === own.pattern);
    const pool = like.length > 1 ? like : all;
    const at = pool.findIndex((kit) => kit.name === own.name);
    return pool[(at + 1) % pool.length].name;
  }
}

const KITS = resolve(import.meta.dirname, "../../../assets/data/kits");

/// Every kit in the folder, read from its file, so a material added is filmed with no code.
export function readKits(folder = KITS): Kit[] {
  const files = readdirSync(folder)
    .filter((file) => file.endsWith(".ron"))
    .sort();
  const raw = files.map((file) => {
    const text = readFileSync(resolve(folder, file), "utf8");
    const number = (field: string) => Number(new RegExp(`\\b${field}:\\s*(?:Some\\()?(\\d+)`).exec(text)?.[1]);
    return { name: basename(file, ".ron"), id: number("id"), pattern: /\bpattern:\s*"(\w+)"/.exec(text)?.[1] ?? "", roofId: number("roof_kind") };
  });
  const baseline = readBaseline();
  return raw.map(({ roofId, ...kit }) => ({
    ...kit,
    roof: Number.isNaN(roofId) ? undefined : raw.find((each) => each.id === roofId)?.name,
    baseline: baseline.has(kit.name),
  }));
}

/// The building baseline's wall materials (D333): those the settlement is built of, in
/// assets/data/settlement.ron. The rest are frozen, filmed only when asked for by name.
function readBaseline(): Set<string> {
  const text = readFileSync(resolve(KITS, "../settlement.ron"), "utf8").replace(/\/\/.*$/gm, "");
  return new Set([...text.matchAll(/"(\w+)"/g)].map((match) => match[1]));
}

/// The sets a film can be made in: each wall kit (one with a roof of its own), and `mixes`,
/// the baseline's wall kits meeting one another. `all` is the baseline's sets and `mixes`.
export function readSets(kits = readKits()): KitSet[] {
  const walls = kits.filter((kit) => kit.roof);
  const base = walls.filter((kit) => kit.baseline);
  return [...walls.map((kit) => new KitSet(kit.name, [kit], kits)), new KitSet("mixes", base, kits)];
}

/// Whether a set is filmed by `--set all`: the baseline's (D333).
export const inBaseline = (set: KitSet) => set.walls.every((kit) => kit.baseline);

/// A camera move, from wherever the camera is when it starts, eased over its frames.
type Move = { start: number; frames: number } & ({ glide: [Vec3, Vec3] } | { orbit: Orbit });
type Orbit = { centre: Vec3; radius: number; height: number; from: number; to: number; look: number };

const ease = (t: number) => {
  const c = Math.min(1, Math.max(0, t));
  return c * c * (3 - 2 * c);
};
const mix = (a: Vec3, b: Vec3, t: number): Vec3 => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
const plus = (a: Vec3, b: Vec3): Vec3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
/// A point so many degrees round a centre from the south (+z), toward the east (+x).
const round = (centre: Vec3, radius: number, height: number, degrees: number): Vec3 => {
  const a = (degrees * Math.PI) / 180;
  return [centre[0] + Math.sin(a) * radius, centre[1] + height, centre[2] + Math.cos(a) * radius];
};

/// A scenario's timeline: a cursor of frames, the calls made on them, the stages they fall
/// in, and the camera's moves. Frames are counted as the sandbox's --press counts them, a
/// sixtieth of a second of play each. Written twice: first with nothing known, to find how
/// many steps each fill takes in this set, then with that known, to time the film.
export class Script {
  frame = 1;
  readonly calls: Call[] = [];
  readonly stages: Stage[] = [];
  private readonly moves: Move[] = [];
  private shift?: { until: number; by: Vec3 };
  private readonly solids: [Vec3, Vec3, number][] = [];
  /// The frames the camera is within 40 cm of a solid, found by `all`.
  readonly clipping: number[] = [];

  readonly known?: Known;

  constructor(known?: Known) {
    this.known = known;
  }

  /// A named stretch of the film: what `body` adds, from the cursor to where it leaves it.
  stage(name: string, title: string, body: () => void) {
    const from = this.frame;
    body();
    this.stages.push({ name, title, from, to: Math.max(from, this.frame - 1) });
    this.stages.sort((a, b) => a.from - b.from);
  }

  /// A call on this frame.
  call(method: string, params?: unknown) {
    this.calls.push({ frame: this.frame, method, params });
  }

  /// Moves the cursor on, the camera still moving and the game still running.
  wait(frames: number) {
    this.frame += Math.max(0, Math.round(frames));
  }

  /// The last call may be refused with words containing `text`, as the scenario means it
  /// to be; any other refusal fails the film.
  allow(text: string) {
    const last = this.calls.at(-1)!;
    (last.allow ??= []).push(text);
  }

  /// Keys pressed as the player would, named as --press names them, timed from now.
  press(...keys: string[]) {
    for (const each of keys) {
      const [, key, after, frames] = /^([^@:]+)(?:@(\d+))?(?::(\d+))?$/.exec(each)!;
      this.call("sandbox.press", { key, after: after ? Number(after) : undefined, frames: frames ? Number(frames) : undefined });
    }
  }

  /// Waits for the run to say `text` after the last call, the first pass learning how many
  /// frames after it that was; the film then knows, as `heard(name)`, the frame it is said
  /// on. Returns that frame once known.
  expect(name: string, text: string): number | undefined {
    const last = this.calls.at(-1)!;
    last.expect = { name, text };
    const after = this.known?.said[name];
    return after === undefined ? undefined : last.frame + after;
  }

  /// Builds what goes in next, a step at a time as holding the hammer does, until nothing
  /// planned is left to build, spread over about `seconds` but never slower than a step every
  /// `gap` frames nor faster than a frame a step. The first pass builds it all at once to
  /// count the steps; the film paces them. Returns each step and the frame it goes in on.
  ///
  /// `weigh` gives a step its share of the time against others, 1 if not given: footings
  /// can go in faster than walls.
  fill(name: string, { seconds = 6, gap = 12, weigh = () => 1 }: { seconds?: number; gap?: number; weigh?: (what: string) => number } = {}): Step[] {
    const steps = this.known?.fills[name];
    if (!steps) {
      this.calls.push({ frame: this.frame, method: "build.fill", params: { steps: 100000 }, tag: name });
      this.wait(2);
      return [];
    }
    if (!steps.length) return [];
    const weights = steps.map((what) => Math.max(0, weigh(what)));
    const total = weights.reduce((sum, w) => sum + w, 0) || 1;
    const frames = Math.max(1, Math.min(seconds * 60, total * gap));
    const start = this.frame;
    // Each step on the frame its share of the time so far reaches; steps falling on one
    // frame go in by one call.
    const out: Step[] = [];
    let before = 0;
    steps.forEach((what, i) => {
      out.push({ frame: start + Math.floor((before / total) * frames), what });
      before += weights[i];
    });
    for (let i = 0; i < out.length; ) {
      let j = i;
      while (j < out.length && out[j].frame === out[i].frame) j++;
      this.calls.push({ frame: out[i].frame, method: "build.fill", params: { steps: j - i } });
      i = j;
    }
    this.frame = start + Math.round(frames);
    return out;
  }

  /// A stage marked over frames already written, such as part of a fill.
  mark(name: string, title: string, from: number, to: number) {
    this.stages.push({ name, title, from, to });
    this.stages.sort((a, b) => a.from - b.from);
  }

  /// The camera glides from where it is to an eye looking at a point, eased in and out,
  /// over so many frames from now; the cursor stays, so building goes on while it moves.
  /// `after` frames from now if given, as the second leg of a move round something.
  glide(eye: Vec3, at: Vec3, frames = 90, after = 0) {
    this.moves.push({ start: this.frame + after, frames: Math.max(1, frames), glide: [eye, at] });
  }

  /// The camera cuts to an eye looking at a point.
  cut(eye: Vec3, at: Vec3) {
    this.glide(eye, at, 1);
  }

  /// The camera goes round a point, `radius` off and `height` above it, from `from` degrees
  /// round from the south toward the east to `to`, over so many frames from now, looking at
  /// the point `look` metres above it; it eases in from where it was over its first second.
  orbit(centre: Vec3, { radius, height, from, to, frames, look = 0 }: { radius: number; height: number; from: number; to: number; frames: number; look?: number }) {
    this.moves.push({ start: this.frame, frames: Math.max(1, frames), orbit: { centre, radius, height, from, to, look } });
  }

  /// Something solid the camera must keep out of, a box in metres: a building, so a move
  /// that would pass through it is found before it is filmed (`clipping`); from a frame on,
  /// if it is not there before.
  solid(min: Vec3, max: Vec3, from = 0) {
    this.solids.push([min, max, from]);
  }

  /// Until a frame the base stands elsewhere, `by` from where it will be: before its claim
  /// stone moves it there. The camera, given where the base will be, is moved by as much.
  baseMovesAt(frame: number | undefined, by: Vec3) {
    if (frame !== undefined) this.shift = { until: frame, by };
  }

  /// Every call, the camera's included, one each frame from its first move to `last`, in
  /// frame order.
  all(last = this.frame): Call[] {
    const camera: Call[] = [];
    const moves = [...this.moves].sort((a, b) => a.start - b.start);
    let pose: [Vec3, Vec3] | undefined;
    let active = -1;
    let from: [Vec3, Vec3] | undefined;
    for (let frame = moves[0]?.start ?? last + 1; frame <= last; frame++) {
      while (active + 1 < moves.length && moves[active + 1].start <= frame) {
        active++;
        from = pose;
      }
      const move = moves[active];
      const t = (frame - move.start) / move.frames;
      if ("glide" in move) {
        const target = move.glide;
        pose = from && move.frames > 1 ? [mix(from[0], target[0], ease(t)), mix(from[1], target[1], ease(t))] : target;
      } else {
        const o = move.orbit;
        const degrees = o.from + (o.to - o.from) * (0.5 - 0.5 * Math.cos(Math.PI * Math.min(1, t)));
        const target: [Vec3, Vec3] = [round(o.centre, o.radius, o.height, degrees), [o.centre[0], o.centre[1] + o.look, o.centre[2]]];
        const blend = ease((frame - move.start) / 60);
        pose = from ? [mix(from[0], target[0], blend), mix(from[1], target[1], blend)] : target;
      }
      let [eye, at] = pose;
      if (this.shift && frame < this.shift.until) [eye, at] = [plus(eye, this.shift.by), plus(at, this.shift.by)];
      const fixed = (v: Vec3) => v.map((n) => Math.round(n * 1000) / 1000);
      if (this.solids.some(([min, max, from]) => frame >= from && [0, 1, 2].every((i) => eye[i] > min[i] - 0.4 && eye[i] < max[i] + 0.4))) this.clipping.push(frame);
      camera.push({ frame, method: "build.look", params: { from: fixed(eye), at: fixed(at) } });
    }
    return [...this.calls, ...camera].sort((a, b) => a.frame - b.frame);
  }
}

/// A scenario: what it needs of the sandbox, and its timeline in a set.
export type Scenario = {
  /// What the film is of, in a line.
  about: string;
  /// The sandbox's options, beyond the film's own.
  sandbox: string[];
  /// A clip: short, of one stage, in the default run (`cargo xtask qa clips`) and the index's
  /// row for its set. What its timeline does before its first stage is not filmed.
  clip?: boolean;
  /// Writes the timeline.
  script: (s: Script, set: KitSet) => void;
};
