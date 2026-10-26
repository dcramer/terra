// Checks on the controller's filmed course (yard.ts, docs/controller.md's in-game course): one
// line a check, ✗ on a failure (D388), from a run's walk record (`--fight-log`, the player as a
// walker: its body's feet, how far below them its figure is shown after a step up, the balls
// of its feet, the camera) and its log's route lines. Feet sliding is the duel's own check
// (tools/sandbox/duel/checks.ts `sliding`). yard-checks.test.ts plants each fault in a made-up
// walk. Node 22.18 or later; no dependencies.

import { type Frame, type Plant, plants, slidOf } from "../sandbox/duel/checks.ts";

/// A check's verdict: whether it held, its line, its number (the scorecard row's value) and
/// how many faults it found.
export type Check = { name: string; ok: boolean; line: string; value: number; faults: number };
/// What a run walked: its name (its pace), its record, its log's route lines, and the
/// ground's height along the way before it was dug, a metre apart east from the start (the
/// log's `Course: ground` line).
export type Walk = { name: string; frames: Frame[]; route: string[]; ground: number[] };
/// A walker's line adds its figure's lag after a step up and whether it stands on the ground.
type Walker = Frame["fighters"][number] & { below?: number; on_ground?: boolean; speed?: number };

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

/// The plants of each walk's frames on the ground, a turn in place's (`turn`) or the rest's.
function walkPlants(walk: Walk, turn: boolean): Plant[] {
  // A contact is a foot on the ground: a frame the body is in the air (off the deck's end)
  // plants nothing.
  const frames = walk.frames.map((frame) => ({ ...frame, fighters: frame.fighters.filter((f) => (f as Walker).on_ground !== false) }));
  return plants(frames).filter((plant) => turning(frames, plant) === turn);
}

/// Feet sliding by the duel's own check on each walk's plants, a turn in place's or the
/// rest's: the row's number is the worst plant's travel, m.
function slid(walks: Walk[], turn: boolean, name: string, of: string): Check {
  const each = walks.map((walk) => ({ walk: walk.name, check: slidOf(walkPlants(walk, turn), of) }));
  const worst = (line: string) => Number(/worst ([\d.]+) cm/.exec(line)?.[1] ?? 0) / 100;
  const value = Math.max(...each.map((one) => worst(one.check.line)));
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
    // Stuck: still for STUCK_S while marks are left (the route is done once the last is reached).
    const done = seen.findLastIndex((each) => (each.w.speed ?? 1) > STILL_MS);
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

export const check = (walks: Walk[]): Check[] => [smooth(walks), feet(walks), turn(walks), yard(walks)];
