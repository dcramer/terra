// Checks on a duel's fight record (`--fight-log`, docs/movement.md's Look row): one line a
// check, ✗ on a failure (D388), read from every frame's fighters, the moves asked and the
// contacts. What each checks is said where it is defined; duel.ts runs them, and
// checks.test.ts plants each fault in a made-up record. Node 22.18 or later; no dependencies.

import { readFileSync } from "node:fs";

export type Ball = { at: number[]; up: number };
export type Fighter = {
  who: string;
  feet: number[];
  facing: number[];
  move: string | null;
  t: number;
  phase: string | null;
  cancels: boolean;
  reach: boolean;
  guard: boolean;
  stop_s: number;
  stagger_s: number;
  down: boolean;
  free: boolean;
  radius: number | null;
  balls: Ball[];
  /// Each foot's heel and ball over the world's ground under them (sampled straight down,
  /// `terra_creatures::melee::sole`), null where none was found; older records lack it.
  sole?: (number | null)[][];
};
export type Asked = { move: string; got: string };
export type Contact = { by: string; on: string; move: string; taken: string; stop_s: number };
/// The camera drawing the frame: where it is, which way it looks and its up, in the start's
/// terms; its vertical field of view in degrees and its width over its height.
export type Lens = { at: number[]; looking: number[]; up: number[]; vfov: number; aspect: number };
export type Frame = { frame: number; step: string | null; asked: Asked[]; contacts: Contact[]; fighters: Fighter[]; camera?: Lens; up?: number[] };
export type Moves = { buffer_s: number; hit_stop: Map<string, number> };
/// A check's verdict: whether it held, its line, and how many faults it found (frames, plants,
/// blows or contacts over its bar; 1 for a check that is held or not as a whole), the number
/// the combat scorecard holds (tools/combat).
export type Check = { name: string; ok: boolean; line: string; faults: number };

/// The player's pushbox, the walker's capsule (the record gives none for the player).
export const PLAYER_RADIUS_M = 0.35;
/// How far two pushboxes may overlap in a frame before it shows: bodies pressed together.
export const OVERLAP_M = 0.05;
/// How far a planted ball of the foot may travel over the ground before it reads as sliding.
export const SLIDE_M = 0.03;
/// A ball this close over its lowest is on the ground; so many frames on it, a plant.
export const PLANTED_M = 0.02;
export const PLANT_FRAMES = 4;
/// A foot's lowest is the height its ball is under for this share of the fight, not the very
/// lowest, which a frame of the ball dipping through the ground would set.
export const LOWEST_SHARE = 0.05;
/// How many frames late a move may start: the tick it is asked or its window opens, or the next.
export const LATE_FRAMES = 1;
/// The game ticks at 64 a second against 60 recorded frames, so a hold is a frame either way.
export const STOP_SLACK_FRAMES = 1.5;
export const FPS = 60;

export const read = (file: string): Frame[] =>
  readFileSync(file, "utf8")
    .split("\n")
    .filter((line) => line.trim())
    .map((line) => JSON.parse(line) as Frame);

/// The move set's buffer and each move's hit-stop, read from its RON without a parser: each
/// move is `"name": (`, its fields to the next.
export function readMoves(file: string): Moves {
  const text = readFileSync(file, "utf8").replace(/\/\/.*$/gm, "");
  const buffer = /buffer_s:\s*([\d.]+)/.exec(text);
  const body = text.slice(text.indexOf("moves:"));
  const hit_stop = new Map<string, number>();
  const names = [...body.matchAll(/"([\w-]+)":\s*\(/g)];
  names.forEach((name, i) => {
    const chunk = body.slice(name.index, names[i + 1]?.index ?? body.length);
    const stop = /hit_stop:\s*([\d.]+)/.exec(chunk);
    hit_stop.set(name[1], stop ? Number(stop[1]) : 0);
  });
  return { buffer_s: buffer ? Number(buffer[1]) : 0.35, hit_stop };
}

/// A contact names a fighter as "you" or "the partner"; the fighters say "you" or "partner".
const who = (name: string) => name.replace(/^the /, "");
/// A fighter as a line says them.
const whose = (name: string) => (name === "you" ? "the player's" : `the ${name}'s`);
const flat = (a: number[], b: number[]) => Math.hypot(a[0] - b[0], a[2] - b[2]);
const fighter = (frame: Frame, name: string) => frame.fighters.find((each) => each.who === name);

/// One run of a move by one fighter: from the frame it starts to its last.
export type Run = { who: string; move: string; from: number; to: number };

/// Every move each fighter made, a new run when the move changes or its time goes back.
export function runs(frames: Frame[]): Run[] {
  const out: Run[] = [];
  const open = new Map<string, { run: Run; t: number }>();
  for (const frame of frames) {
    const here = new Set<string>();
    for (const f of frame.fighters) {
      here.add(f.who);
      const was = open.get(f.who);
      if (was && (f.move !== was.run.move || f.t < was.t - 1e-4)) open.delete(f.who);
      if (!f.move) continue;
      const still = open.get(f.who);
      if (still) {
        still.run.to = frame.frame;
        still.t = f.t;
      } else {
        const run = { who: f.who, move: f.move, from: frame.frame, to: frame.frame };
        out.push(run);
        open.set(f.who, { run, t: f.t });
      }
    }
    for (const name of [...open.keys()]) if (!here.has(name)) open.delete(name);
  }
  return out;
}

const worstOf = <T>(items: T[], score: (item: T) => number) => items.reduce<T | undefined>((best, item) => (best === undefined || score(item) > score(best) ? item : best), undefined);
const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor((values.length - 1) / 2)] ?? 0;
const mark = (ok: boolean) => (ok ? "" : "✗ ");

/// Start latency: a move the player asks for starts the frame it is asked, or the next; one
/// that waits for its window starts the frame the window opens (the other move's `cancels`,
/// or the player free of any move), or the next. A press that never starts is a failure too.
export function latency(frames: Frame[], all = runs(frames), moves?: Moves): Check {
  const byFrame = new Map(frames.map((frame) => [frame.frame, frame]));
  const late: { at: number; move: string; frames: number; never?: boolean }[] = [];
  let asked = 0;
  for (const frame of frames) {
    for (const ask of frame.asked) {
      if (ask.got !== "started" && ask.got !== "waits") continue;
      asked++;
      const run = all.find((each) => each.who === "you" && each.move === ask.move && each.from >= frame.frame);
      // A press waits no longer than the set's buffer: a later run is another press's.
      const keep = Math.ceil((moves?.buffer_s ?? 0.35) * FPS) + LATE_FRAMES + 1;
      if (!run || (ask.got === "waits" && run.from - frame.frame > keep)) {
        late.push({ at: frame.frame, move: ask.move, frames: Infinity, never: true });
        continue;
      }
      let opened = run.from;
      if (ask.got === "started") opened = frame.frame;
      else
        for (let n = frame.frame; n < run.from; n++) {
          const you = byFrame.get(n) && fighter(byFrame.get(n)!, "you");
          if (you && (!you.move || you.cancels)) {
            opened = n;
            break;
          }
        }
      const waited = run.from - opened;
      if (waited > LATE_FRAMES) late.push({ at: frame.frame, move: ask.move, frames: waited });
    }
  }
  const worst = worstOf(late, (each) => each.frames);
  const line = worst
    ? `${late.length} of ${asked} moves asked started late, worst ${worst.never ? `${worst.move} asked at frame ${worst.at} never started` : `${worst.move} asked at frame ${worst.at}, ${worst.frames} frames after it could`}`
    : `${asked} moves asked, each started within ${LATE_FRAMES} frame of its window`;
  return { name: "start latency", ok: late.length === 0, line, faults: late.length };
}

/// An idle frame where a chain was asked: between a press that waits for its window and the
/// move it starts, the player is never out of every move.
export function chainGaps(frames: Frame[], all = runs(frames)): Check {
  const byFrame = new Map(frames.map((frame) => [frame.frame, frame]));
  const gaps: { at: number; move: string; idle: number }[] = [];
  let chains = 0;
  for (const frame of frames)
    for (const ask of frame.asked) {
      if (ask.got !== "waits") continue;
      chains++;
      const run = all.find((each) => each.who === "you" && each.move === ask.move && each.from >= frame.frame);
      const to = run?.from ?? frame.frame;
      let idle = 0;
      for (let n = frame.frame; n < to; n++) if (byFrame.get(n) && !fighter(byFrame.get(n)!, "you")?.move) idle++;
      if (idle) gaps.push({ at: frame.frame, move: ask.move, idle });
    }
  const worst = worstOf(gaps, (each) => each.idle);
  const line = worst
    ? `${gaps.length} of ${chains} chains stood idle, worst ${worst.move} asked at frame ${worst.at}: ${worst.idle} idle frames`
    : `${chains} chained moves, no idle frame between a blow and the next`;
  return { name: "chain gaps", ok: gaps.length === 0, line, faults: gaps.length };
}

/// A plant: whose foot, its first and last frame, how far its ball travelled, the move at its middle.
export type Plant = { who: string; foot: number; from: number; to: number; travel: number; move: string };

/// A ball this far under the world's ground is in it, not on it (a free foot through a tread):
/// its frame is no contact, counted apart where it is said (yard-checks' `Contact::under`).
export const THROUGH_SOLE_M = 0.02;

/// A ball's height for contact: over the world's ground under it where the record has it (the
/// fighter's `sole`, D455's log for unit 2c-2: a foot lowered onto ground under the root's level
/// read over the root came nearer its fight-wide lowest), else over the shown root (`balls.up`,
/// older records). None where its ground was not found or it is past THROUGH_SOLE_M in it.
export function contactUp(f: Fighter, foot: number): number | undefined {
  const ball = f.balls[foot];
  if (!ball) return undefined;
  if (!f.sole) return ball.up;
  const up = f.sole[foot]?.[1];
  return typeof up === "number" && up >= -THROUGH_SOLE_M ? up : undefined;
}

/// Every plant: a ball of the foot within PLANTED_M of its lowest (LOWEST_SHARE) for
/// PLANT_FRAMES frames or more, its height over the world's ground where the record has it
/// (`contactUp`). A roll and a body knocked down are left out.
export function plants(frames: Frame[]): Plant[] {
  const plants: Plant[] = [];
  const names = [...new Set(frames.flatMap((frame) => frame.fighters.map((f) => f.who)))];
  for (const name of names)
    for (const foot of [0, 1]) {
      const seen = frames.flatMap((frame) => {
        const f = fighter(frame, name);
        const up = f && contactUp(f, foot);
        return f && up !== undefined && f.move !== "roll" && !f.down ? [{ frame: frame.frame, ball: f.balls[foot], up, move: f.move ?? "standing" }] : [];
      });
      if (!seen.length) continue;
      const ups = seen.map((each) => each.up).sort((a, b) => a - b);
      const lowest = ups[Math.floor(LOWEST_SHARE * (ups.length - 1))];
      let run: typeof seen = [];
      const close = () => {
        if (run.length >= PLANT_FRAMES) {
          const travel = Math.max(...run.map((each) => flat(each.ball.at, run[0].ball.at)));
          plants.push({ who: name, foot, from: run[0].frame, to: run.at(-1)!.frame, travel, move: run[Math.floor(run.length / 2)].move });
        }
        run = [];
      };
      for (const each of seen) {
        const planted = each.up <= lowest + PLANTED_M;
        if (!planted || (run.length && each.frame !== run.at(-1)!.frame + 1)) close();
        if (planted) run.push(each);
      }
      close();
    }
  return plants;
}

/// Feet sliding while planted: every plant (`plants`) travels no more than SLIDE_M over the
/// ground. `of` names what is counted (plants, or a kind of them).
export function sliding(frames: Frame[], only: (plant: Plant) => boolean = () => true, of = "plants"): Check {
  return slidOf(plants(frames).filter(only), of);
}

/// The feet-sliding verdict on plants already found.
export function slidOf(plants: Plant[], of = "plants"): Check {
  const slid = plants.filter((plant) => plant.travel > SLIDE_M);
  const worst = worstOf(slid, (each) => each.travel);
  const line = worst
    ? `${slid.length} of ${plants.length} ${of} slid past ${SLIDE_M * 100} cm, median ${(median(slid.map((each) => each.travel)) * 100).toFixed(1)} cm, worst ${(worst.travel * 100).toFixed(1)} cm (${whose(worst.who)} ${worst.foot ? "right" : "left"} foot, frames ${worst.from}-${worst.to}, ${worst.move})`
    : `${plants.length} ${of}, none slid past ${SLIDE_M * 100} cm`;
  return { name: "feet sliding", ok: slid.length === 0, line, faults: slid.length };
}

/// Pushboxes overlapping: the two fighters' feet never closer than their radii, less OVERLAP_M.
export function overlap(frames: Frame[]): Check {
  const over: { frame: number; by: number }[] = [];
  for (const frame of frames) {
    const [a, b] = frame.fighters;
    if (!a || !b) continue;
    const by = (a.radius ?? PLAYER_RADIUS_M) + (b.radius ?? PLAYER_RADIUS_M) - flat(a.feet, b.feet);
    if (by > OVERLAP_M) over.push({ frame: frame.frame, by });
  }
  const worst = worstOf(over, (each) => each.by);
  const line = worst
    ? `${over.length} frames overlap past ${OVERLAP_M * 100} cm, worst ${(worst.by * 100).toFixed(1)} cm at frame ${worst.frame}`
    : `the fighters' pushboxes never overlap past ${OVERLAP_M * 100} cm`;
  return { name: "pushbox overlap", ok: over.length === 0, line, faults: over.length };
}

/// A blow in reach that misses: a run with its target in reach (`Move::in_reach`) during its
/// active frames lands (hit, blocked or parried), unless the target was rolling through it.
export function misses(frames: Frame[], all = runs(frames)): Check {
  const byFrame = new Map(frames.map((frame) => [frame.frame, frame]));
  const missed: Run[] = [];
  let reached = 0;
  for (const run of all) {
    const inReach: number[] = [];
    let dodged = true;
    for (let n = run.from; n <= run.to; n++) {
      const frame = byFrame.get(n);
      const me = frame && fighter(frame, run.who);
      if (!me || me.phase !== "Active" || !me.reach) continue;
      inReach.push(n);
      const other = frame.fighters.find((each) => each.who !== run.who);
      if (other?.move !== "roll") dodged = false;
    }
    if (!inReach.length) continue;
    reached++;
    // A contact is recorded on the frame after the blow's last when it ends the blow (a parry).
    const landed = frames.some((frame) => frame.frame >= run.from && frame.frame <= run.to + 1 && frame.contacts.some((c) => who(c.by) === run.who && c.move === run.move));
    if (!landed && !dodged) missed.push(run);
  }
  const first = missed[0];
  const line = first
    ? `${missed.length} of ${reached} blows in reach missed, first ${whose(first.who)} ${first.move} at frames ${first.from}-${first.to}`
    : `${reached} blows in reach in their active frames, each landed or was rolled through`;
  return { name: "blows in reach", ok: missed.length === 0, line, faults: missed.length };
}

/// Hit-stop: after each contact the striker is held for the contact's stop_s (within
/// STOP_SLACK_FRAMES), and a blow that hits holds for its move's hit_stop in the set.
export function hitStop(frames: Frame[], moves?: Moves): Check {
  const index = new Map(frames.map((frame, i) => [frame.frame, i]));
  const off: { frame: number; move: string; held: number; want: number; set?: number }[] = [];
  let contacts = 0;
  for (const frame of frames)
    for (const c of frame.contacts) {
      contacts++;
      let held = 0;
      for (let i = index.get(frame.frame)!; i < frames.length; i++) {
        const me = fighter(frames[i], who(c.by));
        if (!me || me.stop_s <= 0) break;
        held++;
      }
      const want = c.stop_s * FPS;
      const set = c.taken === "Hit" ? moves?.hit_stop.get(c.move) : undefined;
      const wrongSet = set !== undefined && Math.abs(set - c.stop_s) > 0.005;
      if (Math.abs(held - want) > STOP_SLACK_FRAMES || wrongSet) off.push({ frame: frame.frame, move: c.move, held, want, set: wrongSet ? set : undefined });
    }
  const worst = worstOf(off, (each) => Math.abs(each.held - each.want) + (each.set !== undefined ? 100 : 0));
  const line = worst
    ? `${off.length} of ${contacts} contacts held wrongly, worst ${worst.move} at frame ${worst.frame}: ${worst.held} frames for ${worst.want.toFixed(1)}${worst.set !== undefined ? `, the set's hit_stop ${worst.set} s` : ""}`
    : `${contacts} contacts, each held its hit-stop to within ${STOP_SLACK_FRAMES} frames`;
  return { name: "hit-stop", ok: off.length === 0, line, faults: off.length };
}

/// What a fight did, frame by frame: each move started and each contact. Two runs of the
/// same script from different cameras must give the same.
export function events(frames: Frame[], all = runs(frames)): string[] {
  const starts = all.map((run) => `${run.from} ${run.who} ${run.move}`);
  const contacts = frames.flatMap((frame) => frame.contacts.map((c) => `${frame.frame} ${who(c.by)} ${c.move} ${c.taken}`));
  return [...starts, ...contacts].sort((a, b) => Number.parseInt(a) - Number.parseInt(b) || a.localeCompare(b));
}

/// The same fight in every record: the same moves started and contacts on the same frames.
export function same(records: { name: string; frames: Frame[] }[]): Check {
  const [first, ...rest] = records.map((record) => ({ name: record.name, said: events(record.frames) }));
  for (const other of rest) {
    const i = first.said.findIndex((event, n) => other.said[n] !== event);
    const at = i >= 0 ? i : other.said.length !== first.said.length ? first.said.length : -1;
    if (at >= 0)
      return {
        name: "same fight",
        ok: false,
        faults: 1,
        line: `${other.name} differs from ${first.name} at event ${at + 1}: ${other.said[at] ?? "nothing"} against ${first.said[at] ?? "nothing"}`,
      };
  }
  return { name: "same fight", ok: true, faults: 0, line: `${records.length} records, the same ${first.said.length} move starts and contacts on the same frames` };
}

/// The fight where the cameras look: the fighters' middle, on the frames both stand, is on
/// average within `within` metres of `centre` (east and south of the start, as the record
/// and `--cine` give places), and never further than twice that.
export function framed(frames: Frame[], centre: number[], within: number): Check {
  const off = frames.flatMap((frame) => {
    const [a, b] = frame.fighters;
    return a && b ? [{ frame: frame.frame, by: Math.hypot((a.feet[0] + b.feet[0]) / 2 - centre[0], (a.feet[2] + b.feet[2]) / 2 - centre[2]) }] : [];
  });
  const mean = off.reduce((sum, each) => sum + each.by, 0) / Math.max(1, off.length);
  const worst = worstOf(off, (each) => each.by);
  const ok = off.length > 0 && mean <= within && (worst?.by ?? 0) <= 2 * within;
  const line = `the fighters' middle ${mean.toFixed(1)} m from where the cameras look on average, ${(worst?.by ?? 0).toFixed(1)} m at most (frame ${worst?.frame ?? "none"}; bars ${within} and ${2 * within} m)`;
  return { name: "framed", ok, line, faults: ok ? 0 : 1 };
}

/// The player's body, feet to crown, as the walker's figure stands (`assets/data/camera.ron`).
export const BODY_M = 1.8;
/// How far off the middle of the frame, as a share of its width, still reads as centred.
export const CENTRED = 0.05;
/// So many of the frames must show the partner whole.
export const SEEN_SHARE = 0.9;

/// Where a point falls in a lens's picture, -1 to 1 left to right and bottom to top, and
/// whether it is in front.
export function project(lens: Lens, p: number[]): { x: number; y: number; ahead: boolean } {
  const v = p.map((each, i) => each - lens.at[i]);
  const dot = (a: number[], b: number[]) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  const f = lens.looking;
  const u = lens.up;
  const r = [f[1] * u[2] - f[2] * u[1], f[2] * u[0] - f[0] * u[2], f[0] * u[1] - f[1] * u[0]];
  const z = dot(v, f);
  const tan = Math.tan((lens.vfov * Math.PI) / 360);
  return { x: dot(v, r) / (z * tan * lens.aspect), y: dot(v, u) / (z * tan), ahead: z > 0 };
}

/// How the player's camera frames the fight (D180, `targets` in tools/combat/refs/
/// enshrouded.json): on every drawn frame, the player's height as a share of the frame's,
/// its median within `bars`; the player's middle across the frame within CENTRED of the
/// middle (median); and the partner whole in the frame on SEEN_SHARE of them.
export function share(frames: Frame[], bars: number[]): Check {
  const up = frames.find((frame) => frame.up)?.up ?? [0, 1, 0];
  const at = (feet: number[], rise: number) => feet.map((each, i) => each + up[i] * rise);
  const seen = frames.flatMap((frame) => {
    const you = fighter(frame, "you");
    const lens = frame.camera;
    if (!you || !lens) return [];
    const [feet, crown, middle] = [0, BODY_M, BODY_M / 2].map((rise) => project(lens, at(you.feet, rise)));
    const partner = fighter(frame, "partner");
    const inside = (rise: number) => {
      const p = project(lens, at(partner!.feet, rise));
      return p.ahead && Math.abs(p.x) <= 1 && Math.abs(p.y) <= 1;
    };
    return [{ share: (crown.y - feet.y) / 2, across: Math.abs(middle.x) / 2, partner: !!partner && inside(0) && inside(BODY_M) }];
  });
  if (!seen.length) return { name: "player framed", ok: false, line: "no drawn frame records its camera", faults: 1 };
  const height = median(seen.map((each) => each.share));
  const across = median(seen.map((each) => each.across));
  const partner = seen.filter((each) => each.partner).length / seen.length;
  const inBars = seen.filter((each) => each.share >= bars[0] && each.share <= bars[1]).length / seen.length;
  const ok = height >= bars[0] && height <= bars[1] && across <= CENTRED && partner >= SEEN_SHARE;
  const pct = (x: number) => `${(100 * x).toFixed(0)}%`;
  const line = `the player ${height.toFixed(3)} of the frame's height (median; bars ${bars[0]}-${bars[1]}, ${pct(inBars)} of ${seen.length} frames inside), ${pct(across)} of its width off the middle (bar ${pct(CENTRED)}), the partner whole in ${pct(partner)} of them (bar ${pct(SEEN_SHARE)})`;
  return { name: "player framed", ok, line, faults: ok ? 0 : 1 };
}

/// Every check on one record.
export function check(frames: Frame[], moves?: Moves): Check[] {
  const all = runs(frames);
  return [latency(frames, all, moves), chainGaps(frames, all), sliding(frames), overlap(frames), misses(frames, all), hitStop(frames, moves)];
}

export const say = (each: Check) => `${mark(each.ok)}${each.name}: ${each.line}`;
