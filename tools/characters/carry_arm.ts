// The carried arm held in the body's frame (D560): a carry pose (a look's `carry_hold`, laid on
// the arm holding it by the hold layer, terra_walker::layer) keeps its turn against the body,
// not the chest, so a torch held flame up stays up through a sprint's lean, a crouch's hunch and
// a jump's landing. It is the mesh space rotation blend shipped overlay layers lay an arm with
// (Unreal's Layered Blend per Bone "Mesh Space Rotation Blend", which ALS's overlays use for
// the arms, docs only): the shoulder blade's turn in the body is the carry pose's on the chest
// as it stands at rest, whatever the clip does to the chest; the shoulder still goes where the
// chest takes it.
//
// An upright pose's arm (the torch's) is held so and no more. A hanging one (a tool's, a
// sword's, a knife's) is held so too and gives as the legs fold (`fold`): the elbow bends, the
// upper arm goes back at the shoulder and the arm comes out from the side, so a landing's
// crouch, which brings the hand down to the knees, lifts the look clear of the ground and the
// legs instead of driving a pickaxe 34.6 cm under it, and tips it up by only the elbow's bend
// less the shoulder's turn back (held.carry.axe_down). Twin of terra_walker::carry_arm, whose numbers it reads; change both.
// Node 22.18 or later; no dependencies.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { type Glb, type Pose, type Q, len, qinv, qmul, sub, turn } from "./glb.ts";

/** The give's numbers, read from the game's source (terra_walker::carry_arm) so the two
 * cannot differ: the elbow's turn, the upper arm's back and the arm's out (degrees) with the
 * legs folded all the way,
 * and the fold a give starts at and is whole at. */
export const GIVE = (() => {
  const source = readFileSync(join(import.meta.dirname, "../../crates/terra_walker/src/carry_arm.rs"), "utf8");
  const of = (name: string) => {
    const m = source.match(new RegExp(`pub const ${name}: f32 = ([0-9.]+);`));
    if (!m) throw new Error(`carry_arm.ts: no ${name} in crates/terra_walker/src/carry_arm.rs: the give's numbers are read from it; keep the constant or change both`);
    return Number(m[1]);
  };
  return { elbow: of("GIVE_ELBOW_DEG"), back: of("GIVE_BACK_DEG"), out: of("GIVE_OUT_DEG"), from: of("FOLD_FROM"), to: of("FOLD_TO"), rate: of("GIVE_RATE") };
})();

/** The fastest a give's elbow turns, degrees a second, however the fold steps: a critically
 * damped spring's greatest speed toward a step of 1 is rate / e. */
export const ELBOW_MOST_DEG_S = (GIVE.elbow * GIVE.rate) / Math.E;

/** A give eased in time: where it is and how fast it moves, in folds (0 to 1). */
export type Eased = { at: number; speed: number };

/** `eased` after `dt` seconds toward `fold`, by a critically damped spring of `GIVE.rate`, solved
 * exactly so any step is stable (terra_walker::carry_arm's `ease`, its source named there). */
export function ease({ at, speed }: Eased, fold: number, dt: number): Eased {
  const j = speed + (at - fold) * GIVE.rate;
  const decay = Math.exp(-GIVE.rate * dt);
  return { at: fold + decay * (at - fold + j * dt), speed: decay * (speed - j * GIVE.rate * dt) };
}

/** The steps a second a clip's give is eased at here, the game's frame rate. */
const STEPS = 60;

/** The eased give through a clip `end` seconds long whose fold at a time is `fold`: as the game
 * eases it frame by frame. A loop (`loops`) is entered as it leaves itself, played once before;
 * any other clip is entered with its give settled on its first pose (a jump's start opens on a
 * crouch the game blends into; a landing opens standing, so its give has it all to catch up). */
export function easedThrough(end: number, loops: boolean, fold: (t: number) => number): (t: number) => number {
  const count = Math.round(end * STEPS);
  let state: Eased = { at: fold(0), speed: 0 };
  const through = (keep?: number[]) => {
    for (let i = 1; i <= count; i++) {
      state = ease(state, fold(i / STEPS), 1 / STEPS);
      if (keep) keep[i] = state.at;
    }
  };
  if (loops) through();
  const at = [state.at];
  through(at);
  // Between two steps, the line between them.
  return (t) => {
    const x = Math.max(0, Math.min(count, t * STEPS));
    const i = Math.min(count - 1, Math.floor(x));
    return count ? at[i] + (at[i + 1] - at[i]) * (x - i) : at[0];
  };
}

/** How far the legs are folded at `pose`, 0 standing to 1: of the straighter leg (each its
 * hip, knee and ankle), how much shorter hip to ankle is than thigh and shin laid end to end,
 * from `GIVE.from` to `GIVE.to`, eased. It has no scale, so it is one on every body. */
export function fold(pose: Pose[], legs: readonly (readonly [number, number, number])[]): number {
  const bent = Math.min(
    ...legs.map(([hip, knee, ankle]) => 1 - len(sub(pose[hip].t, pose[ankle].t)) / (len(sub(pose[hip].t, pose[knee].t)) + len(sub(pose[knee].t, pose[ankle].t)))),
  );
  const x = Math.max(0, Math.min(1, (bent - GIVE.from) / (GIVE.to - GIVE.from)));
  return x * x * (3 - 2 * x);
}

/** The turns, in the body, that hold each carried arm at `pose`: at each of `upright` and
 * `hanging` (a carried arm's clavicle, by its carry pose's kind) the chest's turn from its rest
 * (`rest`, the body's rest pose) undone, as hold.py's and posed's bends turn a bone with what
 * is below it; and a hanging arm's give by the legs' fold, at its upper arm and forearm. */
export function carryArm(body: Glb, rest: Pose[], upright: readonly string[], hanging: readonly string[] = []): (pose: Pose[], eased?: number) => Map<string, Q> {
  const parentOf = new Map<number, number>();
  body.gltf.nodes.forEach((n, i) => n.children?.forEach((c) => parentOf.set(c, i)));
  const chests = [...upright, ...hanging].map((root) => [root, parentOf.get(body.named(root))!] as const);
  const legs = hanging.length ? ["l", "r"].map((s) => [body.named(`thigh_${s}`), body.named(`calf_${s}`), body.named(`foot_${s}`)] as const) : [];
  return (pose, eased) => {
    const turns = new Map<string, Q>(chests.map(([root, chest]) => [root, qmul(rest[chest].r, qinv(pose[chest].r))]));
    if (!hanging.length) return turns;
    // The give eased through the clip where the caller plays one (`easedThrough`), else the
    // pose's own fold, a give long settled.
    const g = eased ?? fold(pose, legs);
    for (const root of hanging) {
      const s = root.slice(-1);
      // The model faces +Z, +X its left: a right arm goes out from the side turning about -Z,
      // after its upper arm goes back about +X.
      turns.set(`upperarm_${s}`, qmul(turn([0, 0, (s === "r" ? -1 : 1) * GIVE.out * g]), turn([GIVE.back * g, 0, 0])));
      turns.set(`lowerarm_${s}`, turn([-GIVE.elbow * g, 0, 0]));
    }
    return turns;
  };
}
