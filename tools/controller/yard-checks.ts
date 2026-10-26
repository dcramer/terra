// Checks on the controller's filmed course (yard.ts, docs/controller.md's in-game course): one
// line a check, ✗ on a failure (D388), from a run's walk record (`--fight-log`, the player as a
// walker: its body's feet, how far below them its figure is shown after a step up, the balls
// of its feet, the camera) and its log's route lines. Feet sliding is the duel's own check
// (tools/sandbox/duel/checks.ts `sliding`). yard-checks.test.ts plants each fault in a made-up
// walk. Node 22.18 or later; no dependencies.

import { type Frame, PLANT_FRAMES, type Plant, SLIDE_M, THROUGH_SOLE_M, plants, slidOf } from "../sandbox/duel/checks.ts";
import { transitions } from "./transitions.ts";

/// A check's verdict: whether it held, its line, its number (the scorecard row's value) and
/// how many faults it found.
export type Check = { name: string; ok: boolean; line: string; value: number; faults: number };
/// What a run walked: its name (its pace), its record, its log's route lines, and the
/// ground's height along the way before it was dug, a metre apart east from the start (the
/// log's `Course: ground` line).
export type Walk = { name: string; frames: Frame[]; route: string[]; ground: number[] };
/// A walker's line adds its figure's lag after a step up and whether it stands on the ground;
/// how each foot is kept; each foot's heel and ball over the world's ground (`sole`, null where
/// none is under it) and over the clip's flat ground as its clip posed them (`clip`); and how
/// far each leg stayed short of the goal it was bent to (`short`) and of the lock's goal alone
/// (`short_lock`), m; the height of the ground under each foot's heel and ball over the shown
/// root (`under`) and its heel to its ball across the up (`span`), m (since controller unit 2c-4).
type Walker = Frame["fighters"][number] & {
  below?: number;
  on_ground?: boolean;
  speed?: number;
  kept?: string[];
  sole?: (number | null)[][];
  clip?: number[][];
  short?: number[];
  short_lock?: number[];
  under?: ((number | null)[] | null)[];
  span?: (number | null)[];
};

export const FPS = 60;
/// The bar of criterion 6: the shown root rises no more than this share of a step a frame...
export const RISE_SHARE = 0.25;
/// ...and has caught its body up within so long (to within SETTLED_M).
export const SETTLE_S = 0.15;
export const SETTLED_M = 0.01;
/// A step up is the body rising faster than this a frame (a 20 deg slope at a sprint is
/// 0.04), for a total of at least STEP_M.
export const RISING_M = 0.05;
export const STEP_M = 0.08;
/// The course's lowest floor under the ground where it is dug, and the deck's height
/// (course_yard.rs): no feet are lower or higher over the ground as it was.
export const FLOOR_M = 1.5;
export const DECK_M = 1.5;
/// Each walked: the feet within so much of the floor's and the deck's height.
export const WALKED_M = 0.15;
/// Fallen through: the feet so far under the lowest floor.
export const THROUGH_M = 0.25;
/// Stuck: so long with the route not done and the body all but still.
export const STUCK_S = 1.5;
export const STILL_MS = 0.3;
/// The route's last mark is reached this near it (the walker's ROUTE_REACHED_M, 1.5 m).
export const DONE_M = 1.5;
/// The mound past the slope limit (course_yard.rs MOUND): where it stands, how far its foot
/// reaches, and how high the feet may get onto it (a step and a little).
export const MOUND = { east: 50, south: 0, foot_m: 2.83, climb_m: 0.65 };

const you = (frame: Frame) => frame.fighters.find((each) => each.who === "you") as Walker | undefined;
/// How high feet at `at` stand over the ground as it was there (between the metres logged).
export function over(ground: number[], at: number[]): number {
  const x = Math.min(Math.max(at[0], 0), ground.length - 1);
  const i = Math.min(Math.floor(x), ground.length - 2);
  return at[1] - (ground[i] + (ground[i + 1] - ground[i]) * (x - i));
}

type Step = { from: number; rise: number; shown: number; camera: number; settled: number };

/// Every step up the body took: from the frame it started rising, its rise, and the shown
/// root's and camera's greatest rise a frame over it as a share of it, and how long the shown
/// root took to catch the body up.
export function steps(frames: Frame[]): Step[] {
  const seen = frames.flatMap((frame) => {
    const w = you(frame);
    return w ? [{ frame: frame.frame, body: w.feet[1], shown: w.feet[1] - (w.below ?? 0), camera: frame.camera?.at[1] }] : [];
  });
  const out: Step[] = [];
  for (let i = 1; i < seen.length; i++) {
    if (seen[i].body - seen[i - 1].body <= RISING_M) continue;
    let j = i;
    while (j + 1 < seen.length && seen[j + 1].body - seen[j].body > RISING_M / 2) j++;
    const rise = seen[j].body - seen[i - 1].body;
    if (rise >= STEP_M) {
      let shown = 0;
      let camera = 0;
      let settled = Infinity;
      // Over this step alone: up to the next step up, or a second on.
      for (let k = i; k < seen.length && seen[k].frame - seen[i].frame <= FPS && (k <= j || seen[k].body - seen[k - 1].body <= RISING_M); k++) {
        shown = Math.max(shown, seen[k].shown - seen[k - 1].shown);
        const [a, b] = [seen[k].camera, seen[k - 1].camera];
        if (a !== undefined && b !== undefined) camera = Math.max(camera, a - b);
        if (settled === Infinity && k >= j && Math.abs(seen[k].body - seen[k].shown) <= SETTLED_M) settled = (seen[k].frame - seen[j - 1].frame) / FPS;
      }
      out.push({ from: seen[i].frame, rise, shown: shown / rise, camera: camera / rise, settled });
    }
    i = j;
  }
  return out;
}

/// Criterion 6: the shown root and the camera rise no more than RISE_SHARE of a step a frame
/// (the row's number, the worse of the two), and the root settles within SETTLE_S.
export function smooth(walks: Walk[]): Check {
  const all = walks.flatMap((walk) => steps(walk.frames).map((step) => ({ ...step, walk: walk.name })));
  if (!all.length) return { name: "step smooth", ok: false, line: "✗ no step up seen", value: NaN, faults: 1 };
  const worst = all.reduce((a, b) => (Math.max(b.shown, b.camera) > Math.max(a.shown, a.camera) ? b : a));
  const value = Math.max(worst.shown, worst.camera);
  const popped = all.filter((each) => Math.max(each.shown, each.camera) > RISE_SHARE);
  const late = all.filter((each) => each.settled > SETTLE_S);
  const ok = !popped.length && !late.length;
  const slowest = Math.max(...all.map((each) => each.settled));
  const line = `${ok ? "" : "✗ "}${all.length} steps up, worst a frame ${(worst.shown * 100).toFixed(0)}% of the step shown, ${(worst.camera * 100).toFixed(0)}% the camera (${worst.walk}, frame ${worst.from}, ${worst.rise.toFixed(2)} m); ${popped.length} over ${RISE_SHARE * 100}%, ${late.length} settled after ${SETTLE_S} s (slowest ${Number.isFinite(slowest) ? `${slowest.toFixed(2)} s` : "never"})`;
  return { name: "step smooth", ok, line, value, faults: popped.length + late.length };
}

/// A plant is a turn in place when the body goes less than TURN_STILL_M over it while it
/// turns TURNED_DEG or more: its feet are kept by turn.foot's rule (a step each ~45 deg).
export const TURN_STILL_M = 0.2;
export const TURNED_DEG = 20;

/// Whether `plant` is a turn in place, by its body's place and facing at its ends.
export function turning(frames: Frame[], plant: Plant): boolean {
  const at = (frame: number) => frames.find((each) => each.frame === frame)?.fighters.find((f) => f.who === plant.who);
  const [a, b] = [at(plant.from), at(plant.to)];
  if (!a || !b) return false;
  const went = Math.hypot(b.feet[0] - a.feet[0], b.feet[2] - a.feet[2]);
  const yaw = (f: typeof a) => Math.atan2(f.facing[0], f.facing[2]);
  const turned = Math.abs(((yaw(b) - yaw(a) + 3 * Math.PI) % (2 * Math.PI)) - Math.PI);
  return went < TURN_STILL_M && (turned * 180) / Math.PI >= TURNED_DEG;
}

/// A frame a foot's ball was in the ground: the walk's frame, the foot, how deep, m.
type Under = { frame: number; foot: number; depth: number };
/// A walk's frames as contact reads them, and the frames a ball was in the ground.
type Contact = { frames: Frame[]; under: Under[] };

/// The walk's frames with each ball's height read for contact: over the world's ground under it
/// (the record's `sole`, sampled straight down from the ball, D455's log for unit 2c-1) where
/// the record has it, else over the shown root (`balls.up`, the duel's and older records'). A
/// ball with no ground under it, or past THROUGH_SOLE_M in it, is no contact that frame, so the
/// contact frames either side of it are two plants, not one. A frame the body is in the air
/// (off the deck's end) is no contact either.
function contact(walk: Walk): Contact {
  const under: Under[] = [];
  const frames = walk.frames.map((frame) => ({
    ...frame,
    fighters: frame.fighters
      .filter((f) => (f as Walker).on_ground !== false)
      .map((f) => {
        const sole = (f as Walker).sole;
        if (!sole) return f;
        const balls = f.balls.map((ball, foot) => {
          const up = sole[foot]?.[1];
          if (!ball || typeof up !== "number") return null;
          if (up < -THROUGH_SOLE_M) {
            under.push({ frame: frame.frame, foot, depth: -up });
            return null;
          }
          return { ...ball, up };
        });
        return { ...f, balls } as typeof f;
      }),
  }));
  return { frames, under };
}

/// The plants of a walk's contact frames, a turn in place's (`turn`) or the rest's.
function walkPlants(seen: Contact, turn: boolean): Plant[] {
  return plants(seen.frames).filter((plant) => turning(seen.frames, plant) === turn);
}

/// The frames a ball was in the ground, said in a line's tail; nothing when none was.
function through(under: Under[]): string {
  if (!under.length) return "";
  const worst = under.reduce((a, b) => (b.depth > a.depth ? b : a));
  return `, ${under.length} frames a ball past ${THROUGH_SOLE_M * 100} cm in the ground (no contact; worst ${(worst.depth * 100).toFixed(1)} cm, ${worst.foot ? "right" : "left"} foot, frame ${worst.frame})`;
}

/// feet.ground's bar: a locked sole within so much of where its clip has it over its own flat
/// ground (docs/movement.md stage 4: feet on slopes within 2 cm); feet.sinking's, so far under.
export const GROUND_M = 0.02;
export const SINK_M = 0.02;
/// feet.aloft's bar: a locked sole's lower point (heel or ball) no more than this over where it
/// rests on level ground, m (the lock curves' bake reads a plant's height within 7 cm, D455).
export const ALOFT_M = 0.07;
/// pelvis.drop's bars: the shortfall placement adds to a leg, at p95 and at worst.
export const DROP_M = 0.01;
export const DROP_WORST_M = 0.03;
/// Ground under a ball this far from the shown root's level is a step or a slope: its plants
/// are counted apart, so a line says whether the check met any.
export const UNEVEN_M = 0.05;
/// How far foot placement pitches a sole to its ground, deg (`terra_motion::placement::TILT_DEG`):
/// ground under heel and ball steeper apart than this is a riser the sole straddles, resting on
/// the higher (D455).
export const TILT_DEG = 30;

/// A locked sole's heel and ball off where its clip has them over its flat ground, m: each by
/// its own height over the world's ground under it, but where the ground under heel and ball
/// differs more than the sole can pitch to (a riser straddled: the sole rests on the higher,
/// D455), the point over the lower ground is judged by the nearer of its own reading and its
/// height over the higher ground. A floating sole still floats by its higher point, a sunk one
/// still sinks by its own reading, and a heel left anywhere but on a tread's level is off.
export function soleOff(sole: number[], clip: number[], under?: (number | null)[] | null, span?: number | null): number[] {
  const own = [sole[0] - clip[0], sole[1] - clip[1]];
  const [heel, ball] = under ?? [];
  if (typeof heel !== "number" || typeof ball !== "number" || typeof span !== "number") return own;
  const rise = ball - heel;
  if (Math.abs(rise) <= Math.tan((TILT_DEG * Math.PI) / 180) * span) return own;
  const lower = rise > 0 ? 0 : 1;
  const over = own[lower] - Math.abs(rise);
  own[lower] = Math.abs(over) < Math.abs(own[lower]) ? over : own[lower];
  return own;
}

/// A frame of a locked plant: how far its sole is off where its clip has it (the worse of the
/// heel's and the ball's height over the world's ground against their height over the clip's
/// flat ground), how far it is sunk past that (0 if not), the ground under the ball over the
/// shown root, the heel's and ball's own height over the world's ground, and the leg's shortfall
/// from its goal and from the lock's goal alone.
type Held = { off: number; sunk: number; ground: number; heel: number; ball: number; short?: number; lock?: number };
type Run = { walk: string; foot: number; from: number; to: number; held: Held[] };

/// Each foot's locked plants (`kept` "planted" on the ground, PLANT_FRAMES or more on end, its
/// heel and ball over ground the record found).
function locked(walk: Walk): Run[] {
  const out: Run[] = [];
  for (const foot of [0, 1]) {
    let run: Run | undefined;
    const close = () => {
      if (run && run.held.length >= PLANT_FRAMES) out.push(run);
      run = undefined;
    };
    for (const frame of walk.frames) {
      const w = you(frame);
      const sole = w?.sole?.[foot];
      const clip = w?.clip?.[foot];
      const found = w && w.on_ground !== false && w.kept?.[foot] === "planted" && sole && clip && typeof sole[0] === "number" && typeof sole[1] === "number";
      if (!found || (run && frame.frame !== run.to + 1)) close();
      if (!found) continue;
      const [heel, ball] = [(sole[0] as number) - clip[0], (sole[1] as number) - clip[1]];
      const [heelOff, ballOff] = soleOff(sole as number[], clip, w.under?.[foot], w.span?.[foot]);
      const up = w.balls[foot]?.up ?? (sole[1] as number);
      const held = { off: Math.max(Math.abs(heelOff), Math.abs(ballOff)), sunk: Math.max(0, -heel, -ball), ground: up - (sole[1] as number), heel: sole[0] as number, ball: sole[1] as number, short: w.short?.[foot], lock: w.short_lock?.[foot] };
      run ??= { walk: walk.name, foot, from: frame.frame, to: frame.frame, held: [] };
      run.to = frame.frame;
      run.held.push(held);
    }
    close();
  }
  return out;
}

const sorted = (xs: number[]) => [...xs].sort((a, b) => a - b);
const median = (xs: number[]) => sorted(xs)[Math.floor((xs.length - 1) / 2)];
const p95 = (xs: number[]) => sorted(xs)[Math.max(0, Math.ceil(xs.length * 0.95) - 1)];
const cm = (m: number) => `${(m * 100).toFixed(1)} cm`;
const where = (run: Run) => `${run.walk} ${run.foot ? "right" : "left"} foot, frames ${run.from}-${run.to}, ground ${(median(run.held.map((h) => h.ground)) * 100).toFixed(0)} cm from the root`;
const none = (name: string): Check => ({ name, ok: false, line: "✗ no locked plant with its sole and its clip's in the records (`kept`, `sole`, `clip`)", value: NaN, faults: 1 });

/// feet.ground: each locked plant's median of its sole off where its clip has it over the
/// clip's own flat ground (heel strike and toe-off are the clip's, so they cancel; a riser
/// straddled judged as `soleOff` says); the row's number the worst plant's, m.
export function ground(walks: Walk[]): Check {
  const runs = walks.flatMap(locked);
  if (!runs.length) return none("feet ground");
  const each = runs.map((run) => ({ run, off: median(run.held.map((h) => h.off)) }));
  const worst = each.reduce((a, b) => (b.off > a.off ? b : a));
  const off = each.filter((one) => one.off > GROUND_M);
  const uneven = runs.filter((run) => run.held.some((h) => Math.abs(h.ground) >= UNEVEN_M));
  const ok = !off.length;
  const line = `${ok ? "" : "✗ "}${runs.length} locked plants (${uneven.length} on steps or slopes), ${off.length} with the sole off past ${cm(GROUND_M)}; worst ${cm(worst.off)} (${where(worst.run)})`;
  return { name: "feet ground", ok, line, value: worst.off, faults: off.length };
}

/// feet.sinking: how far a locked sole goes under where its clip has it over the ground, the
/// worst frame's, m.
export function sinking(walks: Walk[]): Check {
  const runs = walks.flatMap(locked);
  if (!runs.length) return none("feet sinking");
  const each = runs.map((run) => ({ run, sunk: Math.max(...run.held.map((h) => h.sunk)) }));
  const worst = each.reduce((a, b) => (b.sunk > a.sunk ? b : a));
  const sunk = each.filter((one) => one.sunk > SINK_M);
  const ok = !sunk.length;
  const line = `${ok ? "" : "✗ "}${runs.length} locked plants, ${sunk.length} sunk past ${cm(SINK_M)}; worst ${cm(worst.sunk)} (${where(worst.run)})`;
  return { name: "feet sinking", ok, line, value: worst.sunk, faults: sunk.length };
}

/// feet.aloft: a foot locked in the air (its clip coming in has it down while the pose shown
/// still has it coming down, a stop's), however its clip has it: each locked plant's first
/// frame (the lock's start, when its ground is frozen), its sole's lower point over the
/// world's ground, heel or ball, less where that point rests (the walk's tenth percentile of it
/// over its locked frames on level ground); the row's number the worst plant's, m. A heel
/// strike's ball is up, its heel down: only both up is aloft. A stance's end, the clip lifting
/// the foot as its lock lets go, is not a lock's start.
export function aloft(walks: Walk[]): Check {
  const each = walks.flatMap((walk) => {
    const runs = locked(walk);
    return [0, 1].flatMap((foot) => {
      const mine = runs.filter((run) => run.foot === foot);
      const level = mine.flatMap((run) => run.held.filter((h) => Math.abs(h.ground) < UNEVEN_M));
      if (!level.length) return [];
      const rest = (of: (h: Held) => number) => sorted(level.map(of))[Math.floor(level.length * 0.1)];
      const [heel, ball] = [rest((h) => h.heel), rest((h) => h.ball)];
      return mine.map((run) => ({ run, up: Math.min(run.held[0].heel - heel, run.held[0].ball - ball) }));
    });
  });
  if (!each.length) return none("feet aloft");
  const worst = each.reduce((a, b) => (b.up > a.up ? b : a));
  const up = each.filter((one) => one.up > ALOFT_M);
  const ok = !up.length;
  const line = `${ok ? "" : "✗ "}${each.length} locked plants, ${up.length} locked with the sole past ${cm(ALOFT_M)} over its rest; worst ${cm(worst.up)} (${where(worst.run)})`;
  return { name: "feet aloft", ok, line, value: worst.up, faults: up.length };
}

/// pelvis.drop: the shortfall foot placement adds to a locked leg (its shortfall from the goal
/// it is bent to, less the lock's goal's alone, so the lock's own strain is not counted), over
/// every locked frame: at p95 ("pelvis drop") and at worst ("pelvis drop worst"), m. Measured,
/// not assumed: a record without both shortfalls fails both.
export function drops(walks: Walk[]): [Check, Check] {
  const runs = walks.flatMap(locked);
  const frames = runs.flatMap((run) => run.held.map((h) => ({ run, h })));
  const missing = frames.filter(({ h }) => h.short === undefined || h.lock === undefined);
  if (!frames.length || missing.length) {
    const line = `✗ ${frames.length ? `${missing.length} of ${frames.length} locked frames` : "no locked frame"} without the leg's shortfalls in the records (\`short\`, \`short_lock\`)`;
    return [
      { name: "pelvis drop", ok: false, line, value: NaN, faults: 1 },
      { name: "pelvis drop worst", ok: false, line, value: NaN, faults: 1 },
    ];
  }
  const added = frames.map(({ run, h }) => ({ run, add: Math.max(0, (h.short as number) - (h.lock as number)) }));
  const at95 = p95(added.map((each) => each.add));
  const worst = added.reduce((a, b) => (b.add > a.add ? b : a));
  const locks = frames.map(({ h }) => h.lock as number);
  const said = `${frames.length} locked frames in ${runs.length} plants, placement adds p95 ${cm(at95)}, worst ${cm(worst.add)} (${where(worst.run)}); the lock alone leaves p95 ${cm(p95(locks))}, worst ${cm(Math.max(...locks))}`;
  const over = (bar: number) => added.filter((each) => each.add > bar).length;
  const mark = (ok: boolean) => (ok ? "" : "✗ ");
  return [
    { name: "pelvis drop", ok: at95 <= DROP_M, line: `${mark(at95 <= DROP_M)}${said}`, value: at95, faults: over(DROP_M) },
    { name: "pelvis drop worst", ok: worst.add <= DROP_WORST_M, line: `${mark(worst.add <= DROP_WORST_M)}${said}`, value: worst.add, faults: over(DROP_WORST_M) },
  ];
}

/// Feet sliding by the duel's own check on each walk's plants, a turn in place's or the
/// rest's: the row's number is the worst plant's travel, m.
function slid(walks: Walk[], turn: boolean, name: string, of: string): Check {
  const each = walks.map((walk) => {
    const seen = contact(walk);
    const found = walkPlants(seen, turn);
    const check = slidOf(found, of);
    const travel = Math.max(0, ...found.filter((plant) => plant.travel > SLIDE_M).map((plant) => plant.travel));
    return { walk: walk.name, travel, check: turn ? check : { ...check, line: check.line + through(seen.under) } };
  });
  const value = Math.max(...each.map((one) => one.travel));
  const ok = each.every((one) => one.check.ok);
  const line = `${ok ? "" : "✗ "}${each.map((one) => `${one.walk}: ${one.check.line}`).join("; ")}`;
  return { name, ok, line, value, faults: each.reduce((sum, one) => sum + one.check.faults, 0) };
}

/// Criterion 21: feet sliding, turns in place left out (`turn`).
export const feet = (walks: Walk[]): Check => slid(walks, false, "feet sliding", "plants");
/// turn.foot: feet sliding in a turn in place (the body all but still while it turns).
export const turn = (walks: Walk[]): Check => slid(walks, true, "turn foot", "turning plants");

/// The yard's own criteria on each walk, the row's number how many fail: every route mark
/// reached; the trench's floor and the deck walked on; never stuck (STUCK_S all but still before the route is done); never through the
/// ground (under the lowest floor); the mound past the slope limit never climbed; and every
/// step up settled (criterion 6's second half).
export function yard(walks: Walk[]): Check {
  const failed: string[] = [];
  const said: string[] = [];
  for (const walk of walks) {
    const marks = walk.route.filter((line) => line.startsWith("reached") || line.startsWith("gave up"));
    const given = marks.filter((line) => line.startsWith("gave up"));
    if (given.length || !marks.length) failed.push(`${walk.name} gave up ${given.length ? given.join(", ") : "(no route lines)"}`);
    const seen = walk.frames.flatMap((frame) => (you(frame) ? [{ frame: frame.frame, w: you(frame)! }] : []));
    if (!seen.length) {
      failed.push(`${walk.name} has no walker in its record`);
      continue;
    }
    if (walk.ground.length < 2) {
      failed.push(`${walk.name} has no ground line in its log`);
      continue;
    }
    // Stuck: still for STUCK_S while marks are left (the route is done once the body stands
    // where its last is reached, so a wait or a turn in place after it is not stuck); with no
    // last mark reached, done once it last moved.
    const last = /^reached (-?\d+),(-?\d+)/.exec(marks.at(-1) ?? "");
    const reached = last ? seen.findIndex((each) => Math.hypot(each.w.feet[0] - Number(last[1]), each.w.feet[2] - Number(last[2])) < DONE_M) : -1;
    const done = reached >= 0 ? reached : seen.findLastIndex((each) => (each.w.speed ?? 1) > STILL_MS);
    let still = 0;
    let stuck = 0;
    for (const [i, each] of seen.entries()) {
      still = (each.w.speed ?? 1) < STILL_MS && i < done ? still + 1 : 0;
      if (still === Math.round(STUCK_S * FPS)) stuck = each.frame;
    }
    if (stuck) failed.push(`${walk.name} stuck at frame ${stuck}`);
    const lowest = seen.reduce((a, b) => (over(walk.ground, b.w.feet) < over(walk.ground, a.w.feet) ? b : a));
    const deepest = -over(walk.ground, lowest.w.feet);
    if (deepest > FLOOR_M + THROUGH_M) failed.push(`${walk.name} fell ${deepest.toFixed(2)} m under the ground at frame ${lowest.frame}`);
    // The trench's floor and the base's deck walked, not gone round.
    const highest = Math.max(...seen.filter((each) => Math.hypot(each.w.feet[0] - MOUND.east, each.w.feet[2] - MOUND.south) >= MOUND.foot_m).map((each) => over(walk.ground, each.w.feet)));
    if (deepest < FLOOR_M - WALKED_M) failed.push(`${walk.name} never reached the trench's floor (${deepest.toFixed(2)} m down)`);
    if (highest < DECK_M - WALKED_M) failed.push(`${walk.name} never stood on the deck (${highest.toFixed(2)} m up)`);
    const onMound = seen.filter((each) => Math.hypot(each.w.feet[0] - MOUND.east, each.w.feet[2] - MOUND.south) < MOUND.foot_m);
    const climbed = Math.max(0, ...onMound.map((each) => over(walk.ground, each.w.feet)));
    if (climbed > MOUND.climb_m) failed.push(`${walk.name} climbed the steep mound ${climbed.toFixed(2)} m`);
    const late = steps(walk.frames).filter((step) => step.settled > SETTLE_S);
    if (late.length) failed.push(`${walk.name} ${late.length} steps settled late`);
    said.push(`${walk.name} ${marks.length - given.length}/${marks.length} marks, deepest ${deepest.toFixed(2)} m, highest ${highest.toFixed(2)} m, mound ${climbed.toFixed(2)} m up`);
  }
  const line = failed.length ? `✗ ${failed.length} failing: ${failed.join("; ")}` : `none failing: ${said.join("; ")}`;
  return { name: "yard", ok: !failed.length, line, value: failed.length, faults: failed.length };
}

export const check = (walks: Walk[]): Check[] => [smooth(walks), feet(walks), turn(walks), ground(walks), sinking(walks), aloft(walks), ...drops(walks), yard(walks), ...transitions(walks)];
